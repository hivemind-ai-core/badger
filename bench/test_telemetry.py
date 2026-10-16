"""Accounting and measurement tests; the actual runtime is covered by --smoke."""

import copy
import sys
import unittest

import telemetry as t


class OracleTests(unittest.TestCase):
    def test_repeatable_bounded_mix_and_hash(self):
        self.assertEqual(t.mix(0), 0)
        self.assertEqual(t.mix(1), 6238072747940578789)
        self.assertNotEqual(t.event_hash(0, 7, 1), t.event_hash(1, 7, 1))
        self.assertNotEqual(t.event_hash(0, 7, 1), t.event_hash(0, 7, 2))
        self.assertEqual(t.event_hash(0, 7, 1), t.event_hash(0, 7, 1))

    def fixture(self):
        config = {**t.DEFAULT, "events": 19, "shards": 3, "sample_every": 2, "flush_every": 4}
        expected = t.oracle(config)
        shards = []
        for total, samples in zip(expected["totals"], expected["samples"]):
            h = [0] * 512
            h[32] = samples
            shards.append(dict(shard=total["shard"], histogram=h, samples=samples, max_ns=32000,
                               windows=(total["count"] + 3) // 4))
        record = dict(schema=1, workload="telemetry", config=config, workers=2, elapsed_ns=1000000,
                      result=dict(totals=expected["totals"], cache=expected["cache"], shards=shards))
        return config, expected, record

    def test_partial_batch_accounting(self):
        config, expected, record = self.fixture()
        metrics = t.validate(record, config, 2, expected)
        self.assertEqual(sum(x["count"] for x in expected["totals"]), 19)
        self.assertEqual(sum(x["id_sum"] for x in expected["totals"]), 171)
        self.assertEqual(expected["cache"]["requests"], 3)
        self.assertEqual(metrics["samples"], 10)
        self.assertEqual(metrics["latency_upper_ns"]["p999"], 33999)
        self.assertEqual(metrics["semantic_allocations"]["batch_arrays"], 1)

    def test_rejects_corrupted_results(self):
        config, expected, original = self.fixture()
        for field in ["count", "id_sum", "checksum"]:
            record = copy.deepcopy(original)
            record["result"]["totals"][0][field] += 1
            with self.subTest(field=field), self.assertRaises(ValueError):
                t.validate(record, config, 2, expected)
        record = copy.deepcopy(original)
        record["result"]["shards"][0]["histogram"][0] += 1
        with self.assertRaises(ValueError):
            t.validate(record, config, 2, expected)
        with self.assertRaises(ValueError):
            t.validate(original, config, 4, expected)

    def test_rejects_lost_partial_summary_and_cache_request(self):
        config, expected, original = self.fixture()
        for section, field in [("shards", "windows"), ("cache", "requests")]:
            record = copy.deepcopy(original)
            target = record["result"][section]
            if isinstance(target, list):
                target = target[0]
            target[field] += 1
            with self.subTest(field=field), self.assertRaises(ValueError):
                t.validate(record, config, 2, expected)


class HistogramTests(unittest.TestCase):
    def test_upper_bounds_at_every_transition(self):
        last = -1
        for i in range(511):
            bound = t.bucket_upper_ns(i)
            self.assertGreater(bound, last)
            # Independent inverse of Lang's bucket definition at both boundaries.
            for ns in [last + 1, bound]:
                us, group = ns // 1000, 0
                while us >= 32:
                    us //= 2
                    group += 1
                self.assertEqual(group * 16 + us, i)
            last = bound

    def test_nearest_rank_tail_and_overflow(self):
        h = [0] * 512
        self.assertIsNone(t.percentile(h, .99))
        h[1], h[40] = 999, 1
        self.assertEqual(t.percentile(h, .999), t.bucket_upper_ns(1))
        self.assertEqual(t.percentile(h, .9999), t.bucket_upper_ns(40))
        h[511] = 1000
        self.assertIsNone(t.percentile(h, .999))


class PerfTests(unittest.TestCase):
    def test_ratios_and_unavailable_counters(self):
        raw = ("# perf output\n100;;cycles;2000;100.00;;\n"
               "250;;instructions;2000;100.00;;\n10;;LLC-loads;2000;50.00;;\n"
               "2;;LLC-load-misses;2000;50.00;;\n<not supported>;;cache-misses;0;0.00;;\n")
        parsed = t.parse_perf(raw)
        self.assertEqual(parsed["ipc"], 2.5)
        self.assertEqual(parsed["llc_load_miss_ratio"], .2)
        self.assertIsNone(parsed["counters"]["cache-misses"]["value"])
        self.assertIsNone(t.parse_perf("0;;cycles;0;0;;")["ipc"])


class ProcessTests(unittest.TestCase):
    def test_large_output_does_not_fill_a_pipe(self):
        out, err, usage = t.run_process([sys.executable, "-c", "print('x' * 200000)"], None, 5)
        self.assertEqual(len(out), 200001)
        self.assertEqual(err, "")
        self.assertGreater(usage["peak_rss_kib"], 0)

    def test_failure_and_timeout_propagate(self):
        with self.assertRaisesRegex(RuntimeError, "exit 3"):
            t.run_process([sys.executable, "-c", "raise SystemExit(3)"], None, 5)
        with self.assertRaises(TimeoutError):
            t.run_process([sys.executable, "-c", "import time; time.sleep(30)"], None, .03)


if __name__ == "__main__":
    unittest.main()
