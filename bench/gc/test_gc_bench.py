import unittest
import run


class CoverageOracle(unittest.TestCase):
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


if __name__ == "__main__":
    unittest.main()
