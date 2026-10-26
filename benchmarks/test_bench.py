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
                          ["--full", "--scenario", "malloc-large"], ["--repeats", "0"], ["--allocator", "c"],
                          ["--allocator", "rust", "--rival", "glibc"]):
            with self.subTest(arguments=arguments), self.assertRaises(SystemExit):
                bench.parse_args(arguments)

    def test_cargo_selection_uses_messages_not_timestamps(self):
        library = self.root / "expected.so"
        library.touch()
        (self.root / "newer.so").touch()
        output = json.dumps({"reason": "compiler-artifact", "target": {"name": "rimalloc"},
                             "filenames": [str(library), "irrelevant.rlib"], "fresh": True})
        self.assertEqual(bench.cargo_artifact(output, "rimalloc", shared=True), library)
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
                    "name": "rimalloc" if shared else "mimalloc_bench",
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
            libraries = bench.build(2, harness=False)
            self.assertNotIn("executable", libraries)
            self.assertNotIn(["cargo", "bench"], [c[:2] for c in calls[-4:]])
            record = json.loads((Path(first["cache"]) / "artifacts.json").read_text())
            self.assertEqual(record["executable"], first["executable"])
            with patch.dict(os.environ, {"CFLAGS": "-march=native"}):
                self.assertNotEqual(bench.build(2)["cache"], first["cache"])

    def test_bolt_build_links_relocations_and_uses_the_optimized_library(self):
        rust_flags = []

        def execute(command, log, environment=None):
            log.parent.mkdir(parents=True, exist_ok=True)
            log.write_text("build log")
            if command[0] == "cargo":
                shared = command[1] == "rustc"
                if shared:
                    rust_flags.append(environment.get("RUSTFLAGS"))
                artifact = Path(environment["CARGO_TARGET_DIR"]) / ("allocator.so" if shared else "mimalloc_bench")
                artifact.parent.mkdir(parents=True, exist_ok=True)
                artifact.write_bytes(b"artifact")
                return json.dumps({"reason": "compiler-artifact", "target": {
                    "name": "rimalloc" if shared else "mimalloc_bench",
                }, "filenames": [str(artifact)], "executable": None if shared else str(artifact)})
            if command[:2] == ["cmake", "--build"]:
                directory = Path(command[2])
                directory.mkdir(parents=True, exist_ok=True)
                (directory / "libmimalloc.so").write_bytes(b"C artifact")
            return ""

        def optimize(library, output, log):
            output.write_bytes(b"bolted " + library.read_bytes())
            return {"profiled_functions": 1, "stale_functions": 0, "log": str(log)}

        profile = self.root / "profile.fdata"
        profile.write_text("profile")
        with patch.object(bench, "ROOT", self.root), patch.object(bench, "execute", execute), \
             patch.object(bench, "capture", return_value="host: x86_64-unknown-linux-gnu"), \
             patch.object(bench.shutil, "which", side_effect=lambda name: "/tools/" + name), \
             patch.object(bench.bolt, "PROFILE", profile), patch.object(bench.bolt, "version", return_value="18"), \
             patch.object(bench.bolt, "optimize", side_effect=optimize), patch.dict(os.environ, {}, clear=True):
            plain = bench.build(2, harness=False)
            with patch.dict(os.environ, {"MIMALLOC_BENCH_BOLT": "1"}):
                bolted = bench.build(2, harness=False)
        self.assertEqual(rust_flags, [None, bench.bolt.LINK_FLAGS])
        self.assertNotEqual(plain["cache"], bolted["cache"])
        self.assertEqual(Path(bolted["rust_library"]).read_bytes(), b"bolted artifact")
        self.assertEqual(bolted["bolt"]["stale_functions"], 0)

    def test_current_and_historical_allocator_labels(self):
        for label in (bench.ALLOCATORS[0], "rust"):
            pairs = bench.SUMMARY.native_pairs([
                {"full_id": f"mimalloc-bench-native/case/{label}", "median": 10},
                {"full_id": "mimalloc-bench-native/case/c-mimalloc-3.3.2", "median": 20},
            ])
            self.assertEqual(len(pairs), 1)
            self.assertEqual(pairs[0]["ratio"], 0.5)

    def test_screen_verdict_needs_agreement_and_size(self):
        self.assertEqual(bench.screen_verdict([0.97, 0.975, 0.98, 0.97, 1.01]), "faster")
        self.assertEqual(bench.screen_verdict([0.99, 0.99, 0.992, 0.988, 1.01]), "likely faster")
        self.assertEqual(bench.screen_verdict([1.03, 1.04, 1.02, 1.05, 0.99]), "SLOWER")
        self.assertEqual(bench.screen_verdict([0.97, 1.03, 0.97, 1.03, 0.97]), "~")
        self.assertEqual(bench.screen_verdict([1.002, 0.998, 1.001]), "~")

    def test_remote_collect_offset_matches_mangled_symbol(self):
        listing = ("0000000000012c80 T mi_malloc\n"
                   "000000000001e3c0 t _ZN19rimalloc9allocator4free9abandoned"
                   "25collect_after_remote_free17h5495e9882d1c6d28E\n")
        with patch.object(bench, "capture", return_value=listing):
            self.assertEqual(bench.remote_collect_offset(Path("lib.so")), 0)
        with patch.object(bench, "capture", return_value=listing.replace("1e3c0", "1e260")):
            self.assertEqual(bench.remote_collect_offset(Path("lib.so")), 32)
        with patch.object(bench, "capture", return_value="0000000000012c80 T mi_malloc\n"):
            self.assertIsNone(bench.remote_collect_offset(Path("lib.so")))

    def test_screen_rejects_too_few_rounds(self):
        with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
            bench.parse_args(["screen", "--rounds", "2"])

    def test_half_native_pair_is_reported(self):
        warning = io.StringIO()
        with contextlib.redirect_stderr(warning):
            pairs = bench.SUMMARY.native_pairs([
                {"full_id": "mimalloc-bench-native/case/renamed", "median": 10},
                {"full_id": "mimalloc-bench-native/case/c-mimalloc-3.3.2", "median": 20},
            ])
        self.assertEqual(pairs, [])
        self.assertIn("case: has c-mimalloc-3.3.2, renamed", warning.getvalue())

    def test_unexpected_error_is_recorded_with_numeric_status(self):
        manifest = {"workers": [{"directory": "worker"}]}
        with bench.run_lifecycle(self.root / "manifest.json", manifest, manifest["workers"]):
            raise RuntimeError("probe crashed")
        saved = json.loads((self.root / "manifest.json").read_text())
        self.assertEqual((saved["status"], saved["error"], saved["exit_status"]), ("failed", "RuntimeError: probe crashed", 1))
        self.assertEqual(saved["workers"][0]["exit_status"], 1)
        with self.assertRaises(SystemExit), bench.run_lifecycle(self.root / "manifest.json", {}, []):
            raise SystemExit(3)

    def test_missing_criterion_result_is_value_error(self):
        with self.assertRaisesRegex(ValueError, "missing Criterion results"):
            bench.SUMMARY.collect_results(self.root, [], {"absent"}, ["absent"])

    def test_undecodable_child_output_is_kept(self):
        output = bench.execute([sys.executable, "-c", "import sys; sys.stdout.buffer.write(b'bad \\xff byte\\n')"],
                               self.root / "child.log")
        self.assertEqual(output, "bad \ufffd byte\n")

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

    def test_rival_workers_get_static_tls_room(self):
        artifacts = {"rust_library": "rust.so", "c_library": "c.so",
                     "rivals": {"jemalloc": "/lib/libjemalloc.so.2", "glibc": "glibc"}}
        args = bench.parse_args(["run"])
        listing = bench.worker_environment(artifacts, args, self.root, "glibc-thread", "rimalloc_raw,jemalloc")
        self.assertEqual(listing["GLIBC_TUNABLES"], bench.rivals.STATIC_TLS_TUNABLE)
        self.assertEqual(listing["MIMALLOC_BENCH_LIB_JEMALLOC"], "/lib/libjemalloc.so.2")
        self.assertNotIn("MIMALLOC_BENCH_LIB_GLIBC", listing)
        with patch.dict(os.environ, {}, clear=True):
            rust = bench.worker_environment(artifacts, args, self.root, "glibc-thread", "rimalloc_raw")
        self.assertNotIn("GLIBC_TUNABLES", rust)

    def test_report_adds_rival_columns_and_geomeans(self):
        manifest = self.saved_run((10,))
        relative = "round-001/glibc-thread/glibc"
        fixture(self.root / relative, "glibc-thread", "glibc", 40)
        manifest["workers"].append({"directory": relative, "exit_status": 0})
        manifest["allocators"] = [*bench.ALLOCATORS, "glibc"]
        bench.write_json(self.root / "manifest.json", manifest)
        self.assertTrue(bench.report(self.root))
        results = json.loads((self.root / "results.json").read_text())
        self.assertEqual(results["rival_geomeans"], {"glibc": 0.25})
        row = results["summary"][0]
        self.assertEqual((row["glibc_median_ns"], row["rust_glibc_ratio"]), (40, 0.25))
        self.assertIn("Rust vs glibc", (self.root / "summary.md").read_text())

    def test_allocator_selection_runs_only_the_named_allocators(self):
        args = bench.parse_args(["run", "--allocator", "rust", "--allocator", "tcmalloc", "--allocator", "rust"])
        self.assertEqual([bench.ALLOCATOR_NAMES[n] for n in dict.fromkeys(args.allocator)],
                         [bench.ALLOCATORS[0], "tcmalloc"])

    def test_report_handles_allocator_subsets(self):
        scenario = "glibc-thread"
        for allocators, values in (([bench.ALLOCATORS[0]], {bench.ALLOCATORS[0]: 10}),
                                   ([bench.ALLOCATORS[0], "tcmalloc"], {bench.ALLOCATORS[0]: 10, "tcmalloc": 40}),
                                   ([bench.ALLOCATORS[1]], {bench.ALLOCATORS[1]: 20})):
            with self.subTest(allocators=allocators):
                root = self.root / "-".join(allocators)
                manifest = {"status": "complete", "scenarios": [scenario], "allocators": allocators,
                            "controls": {"samples": 10, "repeats": 1}, "workers": []}
                for allocator in allocators:
                    relative = f"round-001/{scenario}/{allocator}"
                    fixture(root / relative, scenario, allocator, values[allocator])
                    manifest["workers"].append({"directory": relative, "exit_status": 0})
                bench.write_json(root / "manifest.json", manifest)
                self.assertTrue(bench.report(root))
                results = json.loads((root / "results.json").read_text())
                row = results["summary"][0]
                self.assertIsNone(results["geomean"])
                self.assertEqual("rust_median_ns" in row, bench.ALLOCATORS[0] in allocators)
                self.assertEqual("c_median_ns" in row, bench.ALLOCATORS[1] in allocators)
                self.assertNotIn("rust_c_ratio", row)
                if "tcmalloc" in allocators:
                    self.assertEqual(row["rust_tcmalloc_ratio"], 0.25)
                    self.assertEqual(results["rival_geomeans"], {"tcmalloc": 0.25})
                header = (root / "results.csv").read_text().splitlines()[0].split(",")
                self.assertEqual(header[:2], ["scenario", "metric"])
                self.assertNotIn("c_median_ns" if bench.ALLOCATORS[1] not in allocators else "rust_median_ns", header)

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
        artifacts = {"executable": "fixture", "rust_library": "rust.so", "c_library": "c.so", "cache": str(self.root),
                     "logs": ["harness.log", "rust.log", "c-configure.log", "c-build.log"]}
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
        artifacts = {"executable": "fixture", "rust_library": "rust.so", "c_library": "c.so", "cache": str(self.root),
                     "logs": ["harness.log", "rust.log", "c-configure.log", "c-build.log"]}
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
        artifacts = {"executable": "fixture", "rust_library": "rust.so", "c_library": "c.so", "cache": str(self.root),
                     "logs": ["harness.log", "rust.log", "c-configure.log", "c-build.log"]}
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
        for signum in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
            with self.subTest(signal=signum.name):
                self.assert_signal_terminates_worker(signum)

    def assert_signal_terminates_worker(self, signum):
        pid_file = self.root / f"worker-{signum.name}.pid"
        child = "import os,sys,time; open(sys.argv[1],'w').write(str(os.getpid())); print('ready',flush=True); time.sleep(60)"
        driver = (
            "import signal,sys; sys.path.insert(0,sys.argv[1]); import bench; from pathlib import Path; "
            "[signal.signal(s, bench.interrupt) for s in (signal.SIGTERM, signal.SIGHUP)]; "
            "bench.execute([sys.executable,'-c',sys.argv[2],sys.argv[3]],Path(sys.argv[4]))"
        )
        process = subprocess.Popen([sys.executable, "-c", driver, str(bench.ROOT / "benchmarks"), child,
                                    str(pid_file), str(self.root / "worker.log")],
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        try:
            while process.stdout.readline().strip() != "ready":
                if process.poll() is not None:
                    self.fail("worker exited before becoming ready")
            process.send_signal(signum)
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
