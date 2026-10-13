"""TechEmpower webserver runner: database, response checks, wrk parsing, allocator mapping."""
import json
import os
from pathlib import Path
import tempfile
import unittest

import webserver

FORTUNES = ("<!DOCTYPE html><html><head><title>Fortunes</title></head><body><table><tr><th>id</th><th>message</th></tr>"
            + "<tr><td>11</td><td>&lt;script&gt;alert(&quot;This should not be displayed in a browser alert box.&quot;);"
            "&lt;&#x2f;script&gt;</td></tr>" + "<tr><td>1</td><td>x</td></tr>" * 11
            + "<tr><td>0</td><td>Additional fortune added at request time.</td></tr></table></body></html>")


class WebserverTests(unittest.TestCase):
    def test_database_has_the_techempower_fortunes(self):
        with tempfile.TemporaryDirectory() as directory:
            webserver.create_database(Path(directory) / "fortune.db")

    def test_response_checks_accept_correct_and_reject_wrong_bodies(self):
        webserver.check_response("json", 200, "application/json", b'{"message":"Hello, World!"}')
        webserver.check_response("plaintext", 200, "text/plain", b"Hello, World!")
        webserver.check_response("fortunes", 200, "text/html; charset=utf-8", FORTUNES.encode())
        for scenario, status, content_type, body in (
                ("json", 200, "application/json", b'{"message":"hello"}'),
                ("plaintext", 500, "text/plain", b"Hello, World!"),
                ("fortunes", 200, "text/html", FORTUNES.replace("&lt;script&gt;", "<script>").encode()),
                ("fortunes", 200, "text/html", FORTUNES.replace("<tr><td>1</td><td>x</td></tr>", "", 1).encode())):
            with self.assertRaises(ValueError, msg=(scenario, body[:40])):
                webserver.check_response(scenario, status, content_type, body)

    def test_plaintext_pipelines_and_keeps_throughput_without_latency(self):
        self.assertIn("for i = 1, 16 do", webserver.lua_script(16))
        self.assertNotIn("init = function", webserver.lua_script(1))
        summary = {"requests": 1000, "duration_us": 500000, "bytes": 1, "p50_us": 700.0, "p95_us": 0.0, "p99_us": 0.0, "errors": 0}
        result = webserver.parse_wrk("BENCH_JSON " + json.dumps(summary), 16)
        self.assertEqual((result["requests_per_second"], result["p99_us"]), (2000.0, None))
        with self.assertRaises(ValueError):
            webserver.parse_wrk("BENCH_JSON " + json.dumps({**summary, "errors": 3}), 16)
        with self.assertRaises(ValueError):  # unpipelined runs keep the latency checks
            webserver.parse_wrk("BENCH_JSON " + json.dumps(summary), 1)

    def test_allocator_check_requires_the_library_and_no_other(self):
        mapped = next(line.split(None, 5)[5] for line in Path("/proc/self/maps").read_text().splitlines()
                      if len(line.split(None, 5)) == 6 and ".so" in line)
        webserver.check_allocator(os.getpid(), mapped, [mapped, "/absent/libother.so"])
        webserver.check_allocator(os.getpid(), None, ["/absent/libother.so"])
        with self.assertRaises(ValueError):
            webserver.check_allocator(os.getpid(), "/absent/liballoc.so", [])
        with self.assertRaises(ValueError):
            webserver.check_allocator(os.getpid(), None, [mapped])


if __name__ == "__main__":
    unittest.main()
