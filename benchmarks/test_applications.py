"""Application parsing, preload validation, and comparison routing regressions."""
import contextlib
import io
import json
import os
import subprocess
import tarfile
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import applications
import bench


def application_fixture(path, scenario="redis", samples=10, ns=1000):
    commands = ["SET", "GET"] if scenario == "redis" else ["fillrandom"]
    components = [{"command": c, "rps": 1e9 / ns, "avg_latency_ms": 0.1,
                   "p50_latency_ms": 0.1, "p95_latency_ms": 0.2, "p99_latency_ms": 0.3} for c in commands]
    result = {"scenario": scenario, "samples": [{"normalized_ns": ns, "components": components,
              "allocator_pid": 1234, "allocator_verified": True} for _ in range(samples)]}
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(result))
    return result


class ApplicationTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.silence = contextlib.ExitStack()
        self.silence.enter_context(contextlib.redirect_stdout(io.StringIO()))
        self.silence.enter_context(contextlib.redirect_stderr(io.StringIO()))
        self.addCleanup(self.silence.close)

    def test_redis_parser_rejects_missing_commands_and_invalid_rates(self):
        header = '"test","rps","avg_latency_ms","p50_latency_ms","p95_latency_ms","p99_latency_ms"\n'
        valid = header + '"SET","20000","0.1","0.1","0.2","0.3"\n"GET","40000","0.1","0.1","0.2","0.3"\n'
        self.assertEqual(applications.parse_redis(valid)[1]["rps"], 40000)
        for output in (valid.split('"GET"')[0], valid.replace('"20000"', '"nan"'), valid.replace('"20000"', '"0"')):
            with self.assertRaises(ValueError):
                applications.parse_redis(output)

    def test_rocksdb_parser_rejects_missing_duplicate_and_zero_rates(self):
        valid = "fillrandom   :       2.345 micros/op 426439 ops/sec; 100 MB/s\n"
        self.assertEqual(applications.parse_rocksdb(valid)["rps"], 426439)
        for output in ("not a benchmark", valid * 2, valid.replace("426439", "0")):
            with self.assertRaises(ValueError):
                applications.parse_rocksdb(output)

    def test_report_rejects_wrong_owner_partial_samples_and_inconsistent_timing(self):
        path = self.root / "application.json"
        application_fixture(path)
        self.assertEqual(applications.measurement(path, "redis", 10)["median"], 1000)
        for field, bad in (("allocator_verified", False), ("allocator_pid", 0), ("normalized_ns", 1)):
            result = application_fixture(path)
            result["samples"][0][field] = bad
            path.write_text(json.dumps(result))
            with self.assertRaises(ValueError):
                applications.measurement(path, "redis", 10)
        application_fixture(path, samples=9)
        with self.assertRaises(ValueError):
            applications.measurement(path, "redis", 10)

    def test_source_checksum_mismatch_rejected_before_extraction(self):
        (self.root / "redis-6.2.7.tar.gz").write_bytes(b"corrupted archive")
        with self.assertRaisesRegex(ValueError, "checksum mismatch"):
            applications.source("redis", self.root)
        self.assertFalse((self.root / "redis-6.2.7").exists())

    def test_interrupted_extraction_and_copy_are_not_reused(self):
        package = self.root / "package"
        (package / "pkg-1.0").mkdir(parents=True)
        (package / "pkg-1.0/file").write_text("data")
        archive = self.root / "sources/pkg-1.0.tar.gz"
        archive.parent.mkdir()
        with tarfile.open(archive, "w:gz") as output:
            output.add(package / "pkg-1.0", "pkg-1.0")
        spec = ("1.0", "https://example.invalid/pkg.tar.gz", applications.file_sha256(archive))
        with patch.object(tarfile.TarFile, "extractall", side_effect=KeyboardInterrupt):
            with self.assertRaises(KeyboardInterrupt):
                applications.source("pkg", archive.parent, spec)
        self.assertEqual([p.name for p in archive.parent.iterdir()], [archive.name])
        directory = applications.source("pkg", archive.parent, spec)
        self.assertEqual((directory / "file").read_text(), "data")

        copy = self.root / "copy"
        with patch.object(applications.shutil, "copytree", side_effect=KeyboardInterrupt):
            with self.assertRaises(KeyboardInterrupt):
                applications.copy_tree(directory, copy)
        self.assertFalse(copy.exists())
        applications.copy_tree(directory, copy)
        self.assertEqual((copy / "file").read_text(), "data")

    def test_application_workers_route_and_alternate_without_native_filter(self):
        output = self.root / "run"
        artifacts = {"executable": "native", "rust_library": "rust.so", "c_library": "c.so", "cache": str(self.root)}
        for name in ("harness.log", "rust.log", "c-configure.log", "c-build.log", "artifacts.json"):
            (self.root / name).write_text("fixture")
        calls = []
        def execute(command, log, environment):
            if "--list" in command:
                return ""
            config = json.loads(Path(command[2]).read_text())
            calls.append(config)
            self.assertNotIn("LD_PRELOAD", environment)
            application_fixture(Path(command[3]) / "application.json", config["scenario"], config["samples"],
                                1000 if config["library"] == "rust.so" else 2000)
            return ""
        with patch.object(bench, "build", return_value=artifacts), patch.object(bench, "execute", execute), \
             patch.object(applications, "build", return_value={}):
            self.assertEqual(bench.main(["--scenario", "redis", "--scenario", "rocksdb", "--repeats", "2",
                                         "--output", str(output)]), 0)
        self.assertEqual([c["library"] for c in calls], ["rust.so", "c.so"] * 2 + ["c.so", "rust.so"] * 2)
        result = json.loads((output / "results.json").read_text())
        self.assertTrue(result["complete"])
        self.assertIsNone(result["geomean"])
        self.assertEqual(len(result["summary"]), 2)
        self.assertTrue(all(r["rust_c_ratio"] == 0.5 for r in result["summary"]))
        original = (output / "summary.md").read_bytes()
        self.assertTrue(bench.report(output))
        self.assertEqual((output / "summary.md").read_bytes(), original)

# Explicit paths keep regular unit checks independent of downloaded applications.

@unittest.skipUnless(os.environ.get("BENCH_APPLICATION_ARTIFACTS") and
                     os.environ.get("ARCHITECTURE_TEST_RUST_LIB") and os.environ.get("ARCHITECTURE_TEST_C_LIB"),
                     "set application artifact manifest and both allocator paths for integration")
class ApplicationIntegrationTests(unittest.TestCase):
    def test_real_redis_and_rocksdb_use_both_allocators(self):
        apps = json.loads(Path(os.environ["BENCH_APPLICATION_ARTIFACTS"]).read_text())
        with tempfile.TemporaryDirectory() as name:
            for scenario in applications.SCENARIOS:
                for variable in ("ARCHITECTURE_TEST_RUST_LIB", "ARCHITECTURE_TEST_C_LIB"):
                    with self.subTest(scenario=scenario, allocator=variable):
                        directory = Path(name) / scenario / variable
                        config = {"scenario": scenario, "samples": 2, "operations": 10000, "threads": 2,
                                  "value_size": 256, "timeout": 30, "library": os.environ[variable], "apps": apps}
                        applications.worker(config, directory)
                        result = applications.measurement(directory / "application.json", scenario, 2)
                        self.assertGreater(result["median"], 0)
                        for sample in result["application"]["samples"]:
                            self.assertTrue(sample["allocator_verified"])
                            with self.assertRaises(ProcessLookupError):
                                os.kill(sample["allocator_pid"], 0)
                        self.assertFalse(list(directory.glob("**/db")))

    def test_redis_client_failure_cleans_up_server(self):
        apps = json.loads(Path(os.environ["BENCH_APPLICATION_ARTIFACTS"]).read_text())
        apps["redis_benchmark"] = "/bin/false"
        with tempfile.TemporaryDirectory() as name:
            directory = Path(name)
            config = {"scenario": "redis", "samples": 1, "operations": 10000, "threads": 1,
                      "value_size": 256, "timeout": 30,
                      "library": os.environ["ARCHITECTURE_TEST_RUST_LIB"], "apps": apps}
            with self.assertRaises(subprocess.CalledProcessError):
                applications.worker(config, directory)
            pid = int((directory / "sample-000/allocator-owned.txt").read_text())
            with self.assertRaises(ProcessLookupError):
                os.kill(pid, 0)
            saved = json.loads((directory / "application.json").read_text())
            self.assertEqual(saved["samples"], [])

    def test_preload_guard_rejects_system_malloc(self):
        apps = json.loads(Path(os.environ["BENCH_APPLICATION_ARTIFACTS"]).read_text())
        with tempfile.TemporaryDirectory() as name:
            environment = {**os.environ, "LD_PRELOAD": apps["allocator_check"],
                           "BENCH_ALLOCATOR_EXPECTED": os.environ["ARCHITECTURE_TEST_RUST_LIB"],
                           "BENCH_ALLOCATOR_CHECK": str(Path(name) / "owned.txt")}
            result = subprocess.run([apps["redis_server"], "--version"], env=environment,
                                    capture_output=True, text=True, timeout=10)
            self.assertEqual(result.returncode, 121)
            self.assertIn("wrong malloc library", result.stderr)


if __name__ == "__main__":
    unittest.main()
