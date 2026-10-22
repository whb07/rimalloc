"""Reject misleading decay measurements before interpreting X33 observations."""
import os
import unittest

import bench
import retention


class RetentionMeasurements(unittest.TestCase):
    def test_transient_drop_is_not_sustained_decay(self):
        mib = 2**20
        samples = [{"kind": "periodic", "sample_ns": i * 10**9, "rss_bytes": (1 + excess) * mib}
                   for i, excess in enumerate((10, 4, 10, 4, 3, 2))]
        result = retention.window_metrics(samples, "rss_bytes", mib, 11 * mib, 0, 5 * 10**9)
        self.assertEqual(result["t50_seconds"], 3)
        self.assertEqual(result["t50_status"], "observed")
        self.assertIsNone(result["t90_seconds"])
        self.assertEqual(result["t90_status"], "right-censored")
        self.assertEqual(result["excess_byte_seconds"], 27 * mib)
        self.assertEqual(result["at_5s_bytes"], 3 * mib)
        self.assertEqual(result["at_5s_sample_seconds"], 5)

    def test_phase_snapshots_cannot_supply_three_periodic_confirmations(self):
        samples = [{"kind": "intervened", "sample_ns": i * 10**9, "rss_bytes": 0}
                   for i in range(4)]
        result = retention.window_metrics(samples, "rss_bytes", 0, 2**20, 0, 3 * 10**9)
        self.assertEqual(result["t50_status"], "right-censored")

    def test_negligible_initial_excess_has_no_decay_estimate(self):
        samples = [{"kind": "periodic", "sample_ns": i * 10**9, "rss_bytes": 0} for i in range(4)]
        result = retention.window_metrics(samples, "rss_bytes", 0, os.sysconf("SC_PAGE_SIZE"), 0, 3 * 10**9)
        self.assertEqual(result["t90_status"], "not-informative")

    def test_missing_phases_are_rejected(self):
        with self.assertRaisesRegex(ValueError, "incomplete"):
            retention.summarize({"events": [], "samples": []}, 64 * 1024 * 1024, 10000)

    def test_cli_keeps_observation_windows_and_repetitions_explicit(self):
        args = bench.parse_args(["retention", "--worker-cpu", "2", "--sampler-cpu", "4"])
        self.assertEqual((args.bytes, args.block_size, args.idle_ms, args.sample_ms, args.repeats),
                         (64 * 1024 * 1024, 64, 10000, 100, 5))


if __name__ == "__main__":
    unittest.main()
