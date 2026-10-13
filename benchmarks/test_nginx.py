"""Nginx traffic validity, configuration, CPU isolation, and report recovery."""
import contextlib
import io
import json
import os
from pathlib import Path
import struct
import tempfile
import unittest
from unittest.mock import patch

import bench
import nginx


class NginxTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)

    def test_defaults_and_allocator_scope(self):
        args = bench.parse_args(["nginx"])
        self.assertIsNone(args.allocator)
        self.assertEqual(nginx.ALLOCATORS, ("rust", "mimalloc", "glibc", "jemalloc", "tcmalloc"))
        self.assertEqual(bench.parse_args(["nginx", "--allocator", "jemalloc"]).allocator, ["jemalloc"])
        with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
            bench.parse_args(["nginx", "--allocator", "hoard"])

    def test_workloads_cover_proxy_modes_and_transfer_sizes(self):
        cases = nginx.cases(nginx.WORKLOADS)
        self.assertEqual(len(cases), 18)
        self.assertEqual(len({c["label"] for c in cases}), 18)
        self.assertEqual({c["buffering"] for c in cases if c["workload"] == "proxy"}, {"on", "off"})
        self.assertEqual({c["size"] for c in cases if c.get("upload")}, {4096, 65536, 1048576})

    def test_configuration_keeps_upload_sink_upstream_and_churn_separate(self):
        prefix = self.root / 'prefix "quoted"'
        conf = nginx.configuration(prefix, 18080, 2, [0, 2], self.root, 19000, True, "off")
        self.assertIn("worker_cpu_affinity 1 100;", conf)
        self.assertIn("keepalive_timeout 0s;", conf)
        self.assertIn("proxy_request_buffering off;", conf)
        self.assertIn("proxy_buffering off;", conf)
        self.assertNotIn("bench_sink;", conf)
        upstream = nginx.configuration(prefix, 19000, 1, [3], self.root)
        self.assertIn("bench_sink;", upstream)
        self.assertIn("client_body_buffer_size 2m;", upstream)
        self.assertNotIn("return 200", upstream)

    def test_wrk_errors_and_bad_values_are_rejected(self):
        value = dict(requests=1000, duration_us=1000000, bytes=4096000, p50_us=10, p95_us=20, p99_us=30, errors=0)
        self.assertEqual(nginx.parse_wrk("BENCH_JSON " + json.dumps(value))["requests_per_second"], 1000)
        for key, bad in (("errors", 1), ("requests", 0), ("duration_us", 0), ("p99_us", float("nan"))):
            with self.subTest(key=key), self.assertRaises(ValueError):
                nginx.parse_wrk("BENCH_JSON " + json.dumps({**value, key: bad}))
        with self.assertRaises(ValueError): nginx.parse_wrk("missing output")

    def test_trace_decodes_worker_counts_and_requires_all_workers(self):
        values = tuple(range(41))
        (self.root / "123.bin").write_bytes(struct.pack("=41Q", *values))
        result = nginx.trace_snapshot(self.root, [123])
        self.assertEqual(result["malloc_calls"], 0)
        self.assertEqual(result["requested_bytes"], 5)
        self.assertEqual(result["size_histogram"], list(range(9, 41)))
        with self.assertRaises(ValueError): nginx.trace_snapshot(self.root, [123, 124])

    def test_proxy_reuses_upstream_connections(self):
        conf = nginx.configuration(self.root, 8080, 1, [0], self.root, upstream=9090)
        self.assertIn("upstream backend { server 127.0.0.1:9090; keepalive 64; }", conf)
        self.assertEqual(conf.count("proxy_http_version 1.1;"), 5)
        self.assertNotIn("proxy_pass http://127.0.0.1", conf)

    def test_readiness_retries_transient_http_errors(self):
        process = type("Process", (), {"pid": 1, "poll": lambda self: None})()
        checks = iter([ValueError("unexpected HTTP status/body: 502, 0"), None])
        def check(port, case):
            error = next(checks)
            if error: raise error
        with patch.object(nginx, "worker_pids", return_value=[1]), patch.object(nginx, "http_check", side_effect=check):
            self.assertEqual(nginx.ready(process, 1, {}, 1), [1])

    def test_launch_retries_port_taken_after_probe(self):
        started = []
        def start(binary, prefix, conf, environment, cpus):
            started.append(conf)
            prefix.mkdir(exist_ok=True)
            (prefix / "master.log").write_text("" if len(started) > 1 else "bind() failed (98: Address already in use)")
            return object(), ["nginx"]
        def ready(process, port, case, workers):
            if len(started) == 1: raise ValueError("Nginx exited 1")
            return [7]
        with patch.object(nginx, "start_nginx", side_effect=start), patch.object(nginx, "ready", side_effect=ready), \
             patch.object(nginx, "stop"), patch.object(nginx, "free_port", side_effect=[1111, 2222]):
            process, command, port, pids = nginx.launch("nginx", self.root / "prefix", str, {}, [0], {}, 1)
        self.assertEqual((started, port, pids), (["1111", "2222"], 2222, [7]))

    def test_stop_tolerates_master_exit_after_timeout(self):
        process = unittest.mock.Mock()
        process.poll.return_value = None
        process.wait.side_effect = [nginx.subprocess.TimeoutExpired("nginx", 5), 0]
        with patch.object(nginx, "worker_pids", side_effect=FileNotFoundError):
            nginx.stop(process)
        process.terminate.assert_called_once()

    def test_overlapping_cpus_fail_before_build(self):
        args = bench.parse_args(["nginx", "--server-cpus", "0", "--load-cpus", "0", "--upstream-cpus", "1"])
        with patch.object(nginx.os, "sched_getaffinity", return_value={0, 1, 2}), \
             patch.object(bench, "build") as build, self.assertRaises(bench.BenchError):
            nginx.run(args, bench)
        build.assert_not_called()

    def test_failed_job_keeps_report_incomplete_and_profiles_separate(self):
        cases = nginx.cases(["static"])
        manifest = {"suite": "nginx", "status": "complete", "profiles": ["production", "debug-palloc"],
                    "cases": cases, "allocators": ["rust"], "controls": {"repeats": 1}, "jobs": [], "expected_jobs": 2}
        for profile in manifest["profiles"]:
            config = {"profile": profile, "allocator": "rust", "case": cases[0], "burst_cycles": 2}
            relative = profile
            directory = self.root / relative; directory.mkdir()
            result = {"status": "complete", "config": config, "case": cases[0], "idle": [], "quality_flags": [],
                      "allocator_traffic": {"allocations_per_request": 3, "requested_bytes_per_request": 4096},
                      "runs": [{"requests_per_second": 1000, "p99_us": 100, "peak_worker_rss_bytes": 4096,
                                "cpu_seconds_per_request": .00001, "errors": 0}]}
            (directory / "nginx.json").write_text(json.dumps(result))
            (directory / "config.json").write_text(json.dumps(config))
            manifest["jobs"].append({"directory": relative, "repeat": 1, "exit_status": 0})
        (self.root / "manifest.json").write_text(json.dumps(manifest))
        with contextlib.redirect_stdout(io.StringIO()): self.assertTrue(bench.report(self.root))
        result = json.loads((self.root / "results.json").read_text())
        self.assertEqual([r["profile"] for r in result["summary"]], manifest["profiles"])
        original = (self.root / "summary.md").read_bytes()
        with contextlib.redirect_stdout(io.StringIO()): self.assertTrue(bench.report(self.root))
        self.assertEqual(original, (self.root / "summary.md").read_bytes())
        manifest["jobs"][-1]["exit_status"] = 1
        (self.root / "manifest.json").write_text(json.dumps(manifest))
        with contextlib.redirect_stdout(io.StringIO()): self.assertFalse(bench.report(self.root))
        self.assertTrue(json.loads((self.root / "results.json").read_text())["errors"])


@unittest.skipUnless(os.environ.get("BENCH_NGINX_ARTIFACTS"), "set BENCH_NGINX_ARTIFACTS for the real guard check")
class NginxGuardIntegrationTests(unittest.TestCase):
    def test_wrong_allocator_rejected_before_serving(self):
        import subprocess
        artifacts = json.loads(Path(os.environ["BENCH_NGINX_ARTIFACTS"]).read_text())
        environment = {**os.environ, "LD_PRELOAD": artifacts["guard"],
                       "NGINX_BENCH_ALLOCATOR": artifacts["libraries"]["rust"]}
        result = subprocess.run([artifacts["binaries"]["production"], "-v"], env=environment,
                                capture_output=True, text=True, timeout=5)
        self.assertEqual(result.returncode, 121)
        self.assertIn("wrong malloc owner", result.stderr)


if __name__ == "__main__":
    unittest.main()
