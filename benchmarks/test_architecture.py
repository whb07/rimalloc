"""Metric validation, report recovery, and optional real C++ ownership checks."""
import contextlib
import io
import json
import os
from pathlib import Path
import shlex
import subprocess
import tempfile
import unittest

import architecture
import bench
import retention


def rows_for(scenario, cycles=2):
    return [dict(phase=phase, cycle=cycle, elapsed_ns=0 if phase == "baseline" else 100,
                 operations=0 if phase == "baseline" else 1, live_bytes=0, rss_bytes=4096,
                 rss_delta_bytes=0, minor_faults=0, major_faults=0, occupied_pages=-1)
            for phase, cycle in architecture.expected_phases(scenario, cycles)]


class ArchitecturalReportTests(unittest.TestCase):
    def test_rejects_missing_duplicate_and_reordered_phases(self):
        rows = rows_for("cross-thread")
        for damaged in (rows[:-1], rows + rows[-1:], [rows[1], rows[0], *rows[2:]]):
            with self.assertRaises(ValueError):
                architecture.validate(damaged, "cross-thread", 2)

    def test_rejects_invalid_measurements(self):
        for key, value in (("elapsed_ns", float("nan")), ("elapsed_ns", -1),
                           ("elapsed_ns", True), ("minor_faults", -1),
                           ("live_bytes", "4096"), ("occupied_pages", -2)):
            rows = rows_for("thread-churn")
            rows[1][key] = value
            with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                architecture.validate(rows, "thread-churn", 2)

    def test_zero_free_operations_allowed_for_one_block_pages(self):
        rows = rows_for("segment-occupancy")
        rows[2]["operations"] = 0
        architecture.validate(rows, "segment-occupancy", 2)

    def test_report_preserves_cycles_repeats_and_missing_worker(self):
        with tempfile.TemporaryDirectory() as name, contextlib.redirect_stdout(io.StringIO()):
            root = Path(name)
            manifest = {"suite": "architecture", "status": "complete", "scenarios": ["thread-churn"],
                        "controls": {"cycles": 2, "repeats": 3}, "allocators": list(bench.ALLOCATORS), "workers": []}
            for repeat, ns in enumerate((100, 200, 900), 1):
                for allocator in bench.ALLOCATORS:
                    relative = f"round-{repeat:03}/thread-churn/{allocator}"
                    worker = root / relative
                    worker.mkdir(parents=True)
                    rows = rows_for("thread-churn")
                    for row in rows[1:]:
                        row["elapsed_ns"] = ns + row["cycle"]
                    (worker / "measurements.json").write_text(json.dumps(rows))
                    manifest["workers"].append({"directory": relative, "exit_status": 0})
            (root / "manifest.json").write_text(json.dumps(manifest))
            self.assertTrue(bench.report(root))
            result = json.loads((root / "results.json").read_text())
            self.assertEqual(len(result["measurements"]), 42)
            self.assertEqual(result["summary"][1]["rust_elapsed_ns"], 200)
            self.assertEqual(result["summary"][4]["rust_elapsed_ns"], 201)
            saved = {n: (root / n).read_bytes() for n in ("summary.md", "results.csv", "results.json")}
            self.assertTrue(bench.report(root))
            self.assertEqual(saved, {n: (root / n).read_bytes() for n in saved})
            manifest["workers"][-1]["exit_status"] = 1
            (root / "manifest.json").write_text(json.dumps(manifest))
            self.assertFalse(bench.report(root))
            result = json.loads((root / "results.json").read_text())
            self.assertTrue(result["errors"])
            self.assertEqual(result["summary"], [])

    def test_architecture_cli_defaults(self):
        args = bench.parse_args(["architecture"])
        self.assertEqual((args.bytes, args.block_size, args.threads, args.cycles, args.repeats),
                         (64 * 1024 * 1024, 4096, 4, 8, 3))


@unittest.skipUnless(os.environ.get("ARCHITECTURE_TEST_RUST_LIB") and os.environ.get("ARCHITECTURE_TEST_C_LIB"),
                     "set ARCHITECTURE_TEST_RUST_LIB and ARCHITECTURE_TEST_C_LIB for real-library integration")
class ArchitecturalIntegrationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temporary = tempfile.TemporaryDirectory()
        cls.executable = Path(cls.temporary.name) / "architectural-bench"
        subprocess.run(shlex.split(os.environ.get("CXX", "c++")) + [
            "-std=c++20", "-O2", "-Wall", "-Wextra", "-Werror", "-pthread",
            "-I", str(bench.ROOT / "mimalloc-3.3.2/include"),
            str(bench.ROOT / "benches/architectural_bench.cpp"), "-ldl", "-o", str(cls.executable)], check=True)

    @classmethod
    def tearDownClass(cls):
        cls.temporary.cleanup()

    def measure(self, library, scenario, block=4096):
        output = subprocess.check_output([str(self.executable), "--library", library,
            "--scenario", scenario, "--bytes", str(4 * 1024 * 1024), "--block-size", str(block),
            "--threads", "2", "--cycles", "2"], text=True, timeout=30)
        return architecture.validate([json.loads(line) for line in output.splitlines()], scenario, 2)

    def test_all_four_probes_against_both_libraries(self):
        for variable in ("ARCHITECTURE_TEST_RUST_LIB", "ARCHITECTURE_TEST_C_LIB"):
            for scenario in architecture.SCENARIOS:
                with self.subTest(library=variable, scenario=scenario):
                    rows = self.measure(os.environ[variable], scenario)
                    if scenario == "segment-occupancy":
                        sparse = next(r for r in rows if r["phase"] == "one-per-page")
                        packed = rows[-1]
                        self.assertEqual(sparse["live_bytes"], packed["live_bytes"])
                        self.assertGreater(sparse["occupied_pages"], packed["occupied_pages"])
                        self.assertEqual(sparse["occupied_pages"] * 4096, sparse["live_bytes"])
                        self.assertEqual(rows[-2]["live_bytes"], 0)
                    elif scenario == "decommit-oscillation":
                        self.assertEqual(rows[0]["purge_delay_ms"], 0)
                        self.assertGreater(rows[4]["minor_faults"], 0)
                    else:
                        for row in rows[1:]:
                            if "free" in row["phase"]:
                                self.assertEqual(row["live_bytes"], 0)
                                self.assertEqual(row["operations"], 1024)

    def test_occupancy_across_size_classes(self):
        for variable in ("ARCHITECTURE_TEST_RUST_LIB", "ARCHITECTURE_TEST_C_LIB"):
            for block in (64, 65536):
                with self.subTest(library=variable, block=block):
                    rows = self.measure(os.environ[variable], "segment-occupancy", block)
                    self.assertEqual(rows[1]["live_bytes"], 4 * 1024 * 1024)
                    self.assertEqual(rows[-2]["occupied_pages"], 0)

    def test_retention_ownership_and_phase_handoffs(self):
        for variable in ("ARCHITECTURE_TEST_RUST_LIB", "ARCHITECTURE_TEST_C_LIB"):
            for action in retention.ACTIONS:
                with self.subTest(library=variable, action=action):
                    directory = Path(self.temporary.name) / variable / action
                    command = [str(self.executable), "--library", os.environ[variable],
                               "--scenario", "burst-idle", "--action", action,
                               "--bytes", "4194304", "--block-size", "64", "--threads", "1", "--idle-ms", "100"]
                    observation = retention.observe(command, directory, os.environ.copy(), 100, 10)
                    result = retention.summarize(observation, 0 if action == "empty" else 4194304, 100)
                    self.assertEqual([e["phase"] for e in observation["events"]], list(retention.PHASES))
                    self.assertGreater(result["refill_ms"], 0)
                    self.assertEqual(observation["events"][-1]["live_bytes"], 0)

    def test_invalid_cli_and_missing_library_fail(self):
        for args in (("--cycles", "0"), ("--library", "/nonexistent", "--scenario", "cross-thread")):
            result = subprocess.run([str(self.executable), *args], capture_output=True, text=True, timeout=5)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("architectural benchmark:", result.stderr)


if __name__ == "__main__":
    unittest.main()
