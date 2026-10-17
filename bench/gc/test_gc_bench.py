import unittest
import run


class CoverageOracle(unittest.TestCase):
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


if __name__ == "__main__":
    unittest.main()
