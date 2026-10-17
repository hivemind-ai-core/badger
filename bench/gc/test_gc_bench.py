import unittest
import run


class CoverageOracle(unittest.TestCase):
    def test_persistent_maps_check_final_keys_and_values(self):
        self.assertEqual(run.expected_coverage(dict(mode="persistent-int", n=5, retain=3)), 3 + 3 + 9)
        self.assertEqual(run.expected_coverage(dict(mode="persistent-int", n=2, retain=9)), 2 + 1 + 1)

    def test_builders_check_last_replacements_and_keys(self):
        c = dict(mode="map-replace", n=5, retain=3, rounds=2, payload=7)
        self.assertEqual(run.expected_coverage(c), (3 + 0 + 1 + 2 + 3 + 4 + 2) * 2)
        self.assertEqual(run.expected_coverage(c | dict(mode="set-builder")), 12)
        self.assertEqual(run.expected_coverage(c | dict(mode="map-strings")), (3 + 3 + 7 + 8 + 9) * 2)

    def test_messages_include_all_words_retention_and_fanout(self):
        c = dict(mode="messages", n=5, retain=3, payload=4)
        messages = [sum(i * 31 + j for j in range(4)) for i in range(5)]
        self.assertEqual(run.expected_coverage(c), sum(messages))
        self.assertEqual(run.expected_coverage(c | dict(mode="messages-retained")), sum(messages) + sum(messages[-3:]))
        self.assertEqual(run.expected_coverage(c | dict(mode="messages-retained", retain=9)), sum(messages) * 2)
        self.assertEqual(run.expected_coverage(c | dict(mode="messages-fanout")), sum(messages) * 2)
        for suffix in ["", "-slow", "-retained", "-fanout"]:
            self.assertEqual(run.expected_coverage(c | dict(mode="messages-int" + suffix)),
                             run.expected_coverage(c | dict(mode="messages" + suffix)))

    def test_buffers_include_retained_tail_and_wrap(self):
        c = dict(mode="bytes", n=253, retain=4, payload=17)
        self.assertEqual(run.expected_coverage(c), 31375 + 1 + 68)
        c.update(n=2, retain=4)
        self.assertEqual(run.expected_coverage(c), 1 + 34 + 2)

    def test_snapshots_include_last_values_of_each_history_slot(self):
        c = dict(mode="snapshots", n=130, retain=2, payload=5)
        self.assertEqual(run.expected_coverage(c), 5 * 130 + 129 + 65 + 129 + 130)

    def test_pipeline_boundaries(self):
        for n, expected in [(1, 0), (3, 0), (4, 6), (7, 18)]:
            self.assertEqual(run.expected_coverage(dict(mode="stored", n=n, rounds=2)), expected * 2)

    def test_corruption_and_wrong_configuration_rejected(self):
        c = dict(mode="visible", n=7, rounds=2)
        good = dict(config=c, checksum=36, elapsed_ns=1)
        run.validate_coverage(good, c)
        with self.assertRaises(ValueError):
            run.validate_coverage(good | dict(checksum=35), c)
        with self.assertRaises(ValueError):
            run.validate_coverage(good | dict(config=c | dict(n=8)), c)


class ChildRss(unittest.TestCase):
    def test_native_launcher_excludes_runner_memory(self):
        import os
        from pathlib import Path
        import re
        import subprocess
        import tempfile
        import telemetry
        with tempfile.TemporaryDirectory() as directory:
            helper = Path(directory) / "rss-spawn"
            subprocess.run(["cc", "-O2", str(Path(__file__).with_name("rss_spawn.c")), "-o", str(helper)], check=True)
            ballast = bytearray(64 << 20)
            _, _, direct = telemetry.run_process(["/bin/true"], os.environ.copy(), 10)
            _, err, _ = telemetry.run_process([str(helper), "/bin/true"], os.environ.copy(), 10)
            child_rss = int(re.search(r"\[gc-bench child rss\] (\d+)", err)[1])
            self.assertLess(child_rss, direct["peak_rss_kib"] // 2)
            self.assertEqual(len(ballast), 64 << 20)


class AllocationCensus(unittest.TestCase):
    def test_conservation_rejects_corrupted_bytes_and_survival(self):
        import copy
        values = dict(allocated=10, reclaimed_before_survival=4,
                      reclaimed_after_survival=2, remaining=4,
                      first_survival=5, remaining_after_survival=3,
                      minor_survivor_observations=5, full_survivor_observations=2)
        counts = {key: dict(objects=n, requested_bytes=n * 24, cell_bytes=n * 32)
                  for key, n in values.items()}
        profile = dict(schema=1, scope="process-managed-allocations", totals=counts,
                       sites=[dict(counts=copy.deepcopy(counts))], remaining_metadata_objects=4)
        profile["allocation_sizes"] = dict(small_pool_classes=[dict(cell_size=32, allocated=counts["allocated"])],
                                           large_system_cells=dict(objects=0, requested_bytes=0, cell_bytes=0))
        run.validate_allocations(profile)
        for category, metric in [("reclaimed_before_survival", "requested_bytes"),
                                 ("first_survival", "objects"),
                                 ("full_survivor_observations", "cell_bytes")]:
            bad = copy.deepcopy(profile)
            bad["totals"][category][metric] += 1
            with self.assertRaises(ValueError):
                run.validate_allocations(bad)
        bad = copy.deepcopy(profile)
        bad["remaining_metadata_objects"] = 3
        with self.assertRaises(ValueError):
            run.validate_allocations(bad)


if __name__ == "__main__":
    unittest.main()
