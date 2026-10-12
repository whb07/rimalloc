"""Regression checks for benchmark orchestration and saved-result reporting."""

import contextlib
import io
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import bench


def fixture(directory, scenario, allocator, value):
    parent = directory / "criterion" / "case" / "new"
    parent.mkdir(parents=True)
    (parent / "benchmark.json").write_text(json.dumps({
        "full_id": bench.full_id(scenario, allocator), "value_str": allocator,
    }))
    (parent / "sample.json").write_text(json.dumps({"iters": [1] * 10, "times": [value] * 10}))
    (parent / "estimates.json").write_text(json.dumps({
        "mean": {"point_estimate": value, "confidence_interval": {
            "lower_bound": value * 0.99, "upper_bound": value * 1.01,
        }}, "median": {"point_estimate": value},
    }))


class EntryPointTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="benchmark tests ")
        self.root = Path(self.temporary.name)
        self.addCleanup(self.temporary.cleanup)
        self.silence = contextlib.ExitStack()
        self.silence.enter_context(contextlib.redirect_stdout(io.StringIO()))
        self.silence.enter_context(contextlib.redirect_stderr(io.StringIO()))
        self.addCleanup(self.silence.close)

    def saved_run(self, values=(10,)):
        scenario = "glibc-thread"
        manifest = {"status": "complete", "scenarios": [scenario],
                    "controls": {"samples": 10, "repeats": len(values)}, "workers": []}
        for repeat, value in enumerate(values, 1):
            for allocator in bench.ALLOCATORS:
                relative = f"round-{repeat:03}/{scenario}/{allocator}"
                fixture(self.root / relative, scenario, allocator,
                        value if allocator == bench.ALLOCATORS[0] else 20)
                manifest["workers"].append({"directory": relative, "exit_status": 0})
        bench.write_json(self.root / "manifest.json", manifest)
        return manifest

    def test_default_and_invalid_cli(self):
        args = bench.parse_args([])
        self.assertEqual((args.command, args.samples, args.measurement_secs, args.warmup_secs, args.repeats),
                         ("run", 10, 3, 1, 1))
        for arguments in (["--samples", "9"], ["--scenario", "typo"], ["--jobs", "0"],
                          ["--full", "--scenario", "malloc-large"], ["--repeats", "0"]):
            with self.subTest(arguments=arguments), self.assertRaises(SystemExit):
                bench.parse_args(arguments)

    def test_cargo_selection_uses_messages_not_timestamps(self):
        library = self.root / "expected.so"
        library.touch()
        (self.root / "newer.so").touch()
        output = json.dumps({"reason": "compiler-artifact", "target": {"name": "mechanical_mimalloc"},
                             "filenames": [str(library), "irrelevant.rlib"], "fresh": True})
        self.assertEqual(bench.cargo_artifact(output, "mechanical_mimalloc", shared=True), library)
        with self.assertRaises(bench.BenchError):
            bench.cargo_artifact(output, "mimalloc_bench")

    def test_incremental_c_build_is_called_on_cache_hit(self):
        calls = []

        def execute(command, log, environment=None):
            calls.append(command)
            log.parent.mkdir(parents=True, exist_ok=True)
            log.write_text("build log")
            if command[0] == "cargo":
                shared = command[1] == "rustc"
                artifact = Path(environment["CARGO_TARGET_DIR"]) / ("allocator.so" if shared else "mimalloc_bench")
                artifact.parent.mkdir(parents=True, exist_ok=True)
                artifact.write_bytes(b"artifact")
                return json.dumps({"reason": "compiler-artifact", "target": {
                    "name": "mechanical_mimalloc" if shared else "mimalloc_bench",
                }, "filenames": [str(artifact)], "executable": None if shared else str(artifact)})
            if command[:2] == ["cmake", "--build"]:
                directory = Path(command[2])
                directory.mkdir(parents=True, exist_ok=True)
                (directory / "libmimalloc.so").write_bytes(b"C artifact")
            return ""

        with patch.object(bench, "ROOT", self.root), patch.object(bench, "execute", execute), \
             patch.object(bench, "capture", return_value="host: x86_64-unknown-linux-gnu"), \
             patch.object(bench.shutil, "which", side_effect=lambda name: "/tools/" + name), \
             patch.dict(os.environ, {}, clear=True):
            first = bench.build(2)
            second = bench.build(2)
            self.assertEqual(first["cache"], second["cache"])
            self.assertEqual(first["sha256"], second["sha256"])
            self.assertEqual(sum(c[:2] == ["cmake", "--build"] for c in calls), 2)
            with patch.dict(os.environ, {"CFLAGS": "-march=native"}):
                self.assertNotEqual(bench.build(2)["cache"], first["cache"])

    def test_current_and_historical_allocator_labels(self):
        for label in (bench.ALLOCATORS[0], "rust"):
            pairs = bench.SUMMARY.native_pairs([
                {"full_id": f"mimalloc-bench-native/case/{label}", "median": 10},
                {"full_id": "mimalloc-bench-native/case/c-mimalloc-3.3.2", "median": 20},
            ])
            self.assertEqual(len(pairs), 1)
            self.assertEqual(pairs[0]["ratio"], 0.5)

    def test_half_native_pair_is_rejected(self):
        with self.assertRaisesRegex(SystemExit, "case: has c-mimalloc-3.3.2, renamed"):
            bench.SUMMARY.native_pairs([
                {"full_id": "mimalloc-bench-native/case/renamed", "median": 10},
                {"full_id": "mimalloc-bench-native/case/c-mimalloc-3.3.2", "median": 20},
            ])

    def test_non_json_child_line_is_plain_output(self):
        output = bench.execute([sys.executable, "-c", "print('{\"truncated'); print('done')"], self.root / "child.log")
        self.assertEqual(output, '{"truncated\ndone\n')

    def test_reporting_aggregates_repetition_medians_and_keeps_units(self):
        self.saved_run((10, 20, 1000))
        with patch.object(bench, "build") as build, patch.object(bench, "execute") as execute:
            self.assertTrue(bench.report(self.root))
            build.assert_not_called()
            execute.assert_not_called()
        results = json.loads((self.root / "results.json").read_text())
        self.assertEqual(results["geomean"], 1)
        self.assertEqual(len(results["measurements"]), 6)
        self.assertIn("per-1M-malloc-free-pairs", (self.root / "summary.md").read_text())
        before = (self.root / "summary.md").read_bytes()
        self.assertTrue(bench.report(self.root))
        self.assertEqual(before, (self.root / "summary.md").read_bytes())

    def test_missing_allocator_makes_report_incomplete(self):
        manifest = self.saved_run()
        manifest["workers"].pop()
        bench.write_json(self.root / "manifest.json", manifest)
        self.assertFalse(bench.report(self.root))
        results = json.loads((self.root / "results.json").read_text())
        self.assertIsNone(results["geomean"])
        self.assertIn("INCOMPLETE", (self.root / "summary.md").read_text())

    def test_invalid_samples_and_estimates_are_rejected(self):
        self.saved_run()
        directory = self.root / "round-001/glibc-thread" / bench.ALLOCATORS[0] / "criterion"
        parent = directory / "case/new"
        original = (parent / "sample.json").read_text()
        for times in ([10] * 9, [0] * 10, [float("nan")] * 10, [True] * 10):
            (parent / "sample.json").write_text(json.dumps({"iters": [1] * 10, "times": times}))
            self.assertFalse(bench.report(self.root))
        (parent / "sample.json").write_text(original)
        (parent / "sample.json").write_text(json.dumps({"iters": None, "times": [10] * 10}))
        self.assertFalse(bench.report(self.root))
        (parent / "sample.json").write_text(original)
        estimates = json.loads((parent / "estimates.json").read_text())
        estimates["median"]["point_estimate"] = -1
        (parent / "estimates.json").write_text(json.dumps(estimates))
        self.assertFalse(bench.report(self.root))

    def test_existing_output_is_preserved_before_building(self):
        sentinel = self.root / "keep.txt"
        sentinel.write_text("previous results")
        with patch.object(bench, "build") as build:
            self.assertEqual(bench.main(["run", "--output", str(self.root)]), 1)
            build.assert_not_called()
        self.assertEqual(sentinel.read_text(), "previous results")

    def test_gate_delegation_preserves_exit_code(self):
        for gate, script in bench.GATES.items():
            with patch.object(bench, "execute", side_effect=bench.BenchError("gate failed", 7)) as execute:
                self.assertEqual(bench.main([gate]), 7)
                self.assertEqual(execute.call_args.args[0], [str(bench.ROOT / script)])

    def test_failed_build_is_saved_as_incomplete(self):
        output = self.root / "failed"
        with patch.object(bench, "build", side_effect=bench.BenchError("compiler failed")):
            self.assertEqual(bench.main(["run", "--scenario", "malloc-large", "--output", str(output)]), 1)
        manifest = json.loads((output / "manifest.json").read_text())
        self.assertEqual(manifest["status"], "failed")
        self.assertEqual(manifest["error"], "compiler failed")
        self.assertIsNone(json.loads((output / "results.json").read_text())["geomean"])

    def test_isolated_workers_alternate_order_and_have_distinct_directories(self):
        output = self.root / "run"
        artifacts = {"executable": "fixture", "rust_library": "rust.so", "c_library": "c.so", "cache": str(self.root)}
        for name in ("harness.log", "rust.log", "c-configure.log", "c-build.log", "artifacts.json"):
            (self.root / name).write_text("fixture")
        calls = []

        def execute(command, log, environment):
            if "--list" in command:
                return "\n".join(bench.full_id("malloc-large", a) + ": benchmark" for a in bench.ALLOCATORS)
            calls.append(environment)
            fixture(Path(environment["CRITERION_HOME"]).parent, "malloc-large",
                    environment["MIMALLOC_BENCH_ALLOCATORS"], 10)
            return ""

        with patch.object(bench, "build", return_value=artifacts), patch.object(bench, "execute", execute):
            self.assertEqual(bench.main(["--scenario", "malloc-large", "--repeats", "2", "--output", str(output)]), 0)
        self.assertEqual([c["MIMALLOC_BENCH_ALLOCATORS"] for c in calls], list(bench.ALLOCATORS + bench.ALLOCATORS[::-1]))
        self.assertEqual(len({c["CRITERION_HOME"] for c in calls}), 4)
        self.assertTrue(all(c["MIMALLOC_BENCH_SKIP_BUILD"] == "1" for c in calls))

    def test_interrupted_run_preserves_completed_measurements(self):
        output = self.root / "interrupted"
        artifacts = {"executable": "fixture", "rust_library": "rust.so", "c_library": "c.so", "cache": str(self.root)}
        for name in ("harness.log", "rust.log", "c-configure.log", "c-build.log", "artifacts.json"):
            (self.root / name).write_text("fixture")

        def execute(command, log, environment):
            if "--list" in command:
                return "\n".join(bench.full_id("malloc-large", a) + ": benchmark" for a in bench.ALLOCATORS)
            if environment["MIMALLOC_BENCH_ALLOCATORS"] == bench.ALLOCATORS[1]:
                raise KeyboardInterrupt
            fixture(Path(environment["CRITERION_HOME"]).parent, "malloc-large", bench.ALLOCATORS[0], 10)
            return ""

        with patch.object(bench, "build", return_value=artifacts), patch.object(bench, "execute", execute):
            self.assertEqual(bench.main(["--scenario", "malloc-large", "--output", str(output)]), 130)
        manifest = json.loads((output / "manifest.json").read_text())
        self.assertEqual(manifest["status"], "interrupted")
        self.assertEqual([w["exit_status"] for w in manifest["workers"]], [0, 130])
        results = json.loads((output / "results.json").read_text())
        self.assertEqual(len(results["measurements"]), 1)
        self.assertIsNone(results["geomean"])

    def test_full_records_host_unavailable_cases(self):
        output = self.root / "full"
        artifacts = {"executable": "fixture", "rust_library": "rust.so", "c_library": "c.so", "cache": str(self.root)}
        for name in ("harness.log", "rust.log", "c-configure.log", "c-build.log", "artifacts.json"):
            (self.root / name).write_text("fixture")
        unavailable = {"cache-scratchN", "cache-thrashN"}

        def execute(command, log, environment):
            if "--list" in command:
                return "\n".join(bench.full_id(s, a) + ": benchmark"
                                 for s in bench.NATIVE_SCENARIOS if s not in unavailable for a in bench.ALLOCATORS)
            if command[1].endswith("applications.py"):
                from test_applications import application_fixture
                config = json.loads(Path(command[2]).read_text())
                application_fixture(Path(command[3]) / "application.json", config["scenario"], 10)
                return ""
            fixture(Path(environment["CRITERION_HOME"]).parent,
                    environment["MIMALLOC_BENCH_SCENARIOS"], environment["MIMALLOC_BENCH_ALLOCATORS"], 10)
            return ""

        with patch.object(bench, "build", return_value=artifacts), patch.object(bench, "execute", execute), \
             patch.object(bench.applications, "build", return_value={}):
            self.assertEqual(bench.main(["--full", "--output", str(output)]), 0)
        manifest = json.loads((output / "manifest.json").read_text())
        self.assertEqual(set(manifest["unavailable"]), unavailable)
        self.assertEqual(len(manifest["scenarios"]), 21)
        self.assertEqual(len(manifest["workers"]), 42)

    @unittest.skipUnless(sys.platform == "linux", "Linux process-group cleanup")
    def test_interrupt_terminates_active_worker(self):
        pid_file = self.root / "worker.pid"
        child = "import os,sys,time; open(sys.argv[1],'w').write(str(os.getpid())); print('ready',flush=True); time.sleep(60)"
        driver = (
            "import sys; sys.path.insert(0,sys.argv[1]); import bench; from pathlib import Path; "
            "bench.execute([sys.executable,'-c',sys.argv[2],sys.argv[3]],Path(sys.argv[4]))"
        )
        process = subprocess.Popen([sys.executable, "-c", driver, str(bench.ROOT / "benchmarks"), child,
                                    str(pid_file), str(self.root / "worker.log")],
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        try:
            while process.stdout.readline().strip() != "ready":
                if process.poll() is not None:
                    self.fail("worker exited before becoming ready")
            process.send_signal(signal.SIGINT)
            process.communicate(timeout=10)
            self.assertNotEqual(process.returncode, 0)
            with self.assertRaises(ProcessLookupError):
                os.kill(int(pid_file.read_text()), 0)
        finally:
            if process.poll() is None:
                process.kill()
                process.communicate()


if __name__ == "__main__":
    unittest.main()
