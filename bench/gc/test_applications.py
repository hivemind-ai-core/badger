import unittest

import applications as app
import profile_applications as cpu


class Contracts(unittest.TestCase):
    def test_cpu_inclusive_counts_do_not_double_count_recursive_frames(self):
        result = cpu.counts("entry;decode;decode;allocate 7\nentry;decode 3\nworker;allocate 2\n")
        self.assertEqual(result["samples"], 12)
        self.assertEqual(result["self"], {"allocate": 9, "decode": 3})
        self.assertEqual(result["inclusive"], {"entry": 10, "decode": 10, "allocate": 9, "worker": 2})

    def test_both_protocols_reject_wrong_results_and_sample_counts(self):
        for suite, name in [("realworld", "state"), ("compare", "snapshots")]:
            expected = (app.rw if suite == "realworld" else app.compare).oracle(name, 257, 37)
            prefix = "READY\n" if suite == "realworld" else ""
            record = dict(stdout=prefix + f"BENCH 0 10 {expected}\nBENCH 1 20 {expected}\n", stderr="")
            self.assertEqual(app.validate(record, suite, name, 257, 37, 1, 1)["warm_ns"], [20])
            for bad in [record | dict(stdout=prefix + f"BENCH 0 10 {expected + 1}\nBENCH 1 20 {expected}\n"),
                        record | dict(stdout=prefix + f"BENCH 0 10 {expected}\n")]:
                with self.assertRaises((ValueError, app.compare.BenchError)):
                    app.validate(bad, suite, name, 257, 37, 1, 1)

    def test_processes_contribute_one_median_and_only_complete_pairs(self):
        def run(variant, repetition, values):
            return dict(suite="compare", case="records", workers=1, variant=variant,
                        repetition=repetition, process=dict(warm_ns=values, peak_rss_bytes=1024))
        runs = [run("candidate", 0, [1, 2, 90])]
        self.assertNotIn("paired_work_change", app.summarize(runs)[0])
        runs += [run("baseline", 0, [1, 4, 80]), run("baseline", 1, [10, 20, 30])]
        row = app.summarize(runs)[0]
        self.assertEqual(row["variants"]["baseline"]["kernel_ns"]["median"], 12)
        self.assertEqual(row["paired_work_change"]["median"], -0.5)
        self.assertEqual(row["paired_work_change"]["count"], 1)
        self.assertEqual(row["faster_pairs"], 1)
        # A fast median can hide a costly tail. Each process still contributes
        # one independent observation: the candidate's block mean is slower.
        self.assertAlmostEqual(row["variants"]["baseline"]["kernel_block_mean_ns"]["median"], 145 / 6)
        self.assertAlmostEqual(row["paired_block_mean_change"]["median"], 8 / 85)
        self.assertEqual(row["paired_block_mean_change"]["count"], 1)
        self.assertEqual(row["block_mean_faster_pairs"], 0)


if __name__ == "__main__":
    unittest.main()
