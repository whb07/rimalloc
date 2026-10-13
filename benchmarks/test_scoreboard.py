"""Scoreboard geomeans, suite split, and tolerance breaches."""
import contextlib
import io
import json
from pathlib import Path
import tempfile
import unittest

import scoreboard


def native_dir(root, rows):
    root.mkdir()
    (root / "manifest.json").write_text(json.dumps({"schema": 1}))
    lines = ["scenario,metric,rust_median_ns,c_median_ns,jemalloc_median_ns"]
    lines += [f"{name},per-workload,{rust},{c},{je}" for name, rust, c, je in rows]
    (root / "results.csv").write_text("\n".join(lines) + "\n")
    return root


class ScoreboardTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)

    def run_board(self, *directories):
        with contextlib.redirect_stdout(io.StringIO()) as out:
            code = scoreboard.main([str(d) for d in directories])
        return code, out.getvalue()

    def test_native_and_application_suites_win_separately(self):
        directory = native_dir(self.root / "native", [("a", 98, 100, 120), ("b", 101, 100, 110), ("redis", 99, 100, 130)])
        suites = scoreboard.ratios(scoreboard.native_values(directory))
        self.assertEqual(set(suites), {"native", "applications"})
        self.assertEqual(set(suites["native"]), {"c-mimalloc", "jemalloc"})
        code, out = self.run_board(directory)
        self.assertEqual(code, 0, out)
        self.assertIn("WIN", out)

    def test_tolerance_breach_fails_even_with_winning_geomean(self):
        directory = native_dir(self.root / "native", [("a", 90, 100, 100), ("b", 90, 100, 100), ("c", 104, 100, 100)])
        rows = {(r[0], r[1]): r for r in scoreboard.score(scoreboard.ratios(scoreboard.native_values(directory)))}
        suite, rival, geomean, worst, ratio, breaches, won = rows[("native", "c-mimalloc")]
        self.assertLess(geomean, 1.0)
        self.assertEqual((worst, breaches, won), ("c", ["c"], False))
        # +4% is inside the 10% tolerance for non-mimalloc rivals.
        self.assertTrue(rows[("native", "jemalloc")][-1])
        self.assertEqual(self.run_board(directory)[0], 1)

    def test_nginx_uses_inverse_throughput(self):
        directory = self.root / "nginx"
        directory.mkdir()
        (directory / "manifest.json").write_text(json.dumps({"suite": "nginx"}))
        (directory / "results.csv").write_text(
            "profile,case,allocator,requests_per_second\n"
            "production,static,rust,110\nproduction,static,mimalloc,100\nproduction,static,glibc,50\n")
        ratios = scoreboard.ratios(scoreboard.nginx_values(directory))["nginx"]
        self.assertAlmostEqual(ratios["c-mimalloc"]["production/static"], 100 / 110)
        self.assertAlmostEqual(ratios["glibc"]["production/static"], 50 / 110)


    def test_later_directory_overrides_and_mixes_with_older_runs(self):
        old = native_dir(self.root / "old", [("a", 200, 100, 100)])
        new = native_dir(self.root / "new", [("a", 90, 100, 100)])
        mixed = self.root / "mixed"
        mixed.mkdir()
        (mixed / "manifest.json").write_text(json.dumps({"schema": 1}))
        (mixed / "results.csv").write_text("scenario,metric,rust_median_ns,c_median_ns,glibc_median_ns\n"
                                           "a,per-workload,95,,190\n")
        values = {}
        for directory in (old, new, mixed):
            for key, by_allocator in scoreboard.native_values(directory).items():
                values.setdefault(key, {}).update(by_allocator)
        result = scoreboard.ratios(values)["native"]
        # Rust 95 comes from the last run; C 100 survives from earlier runs.
        self.assertAlmostEqual(result["c-mimalloc"]["a"], 0.95)
        self.assertAlmostEqual(result["glibc"]["a"], 0.5)


if __name__ == "__main__":
    unittest.main()
