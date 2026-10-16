"""Harness regression checks; language ports are validated by run.py itself."""
import importlib.util
import sys
import unittest

import report
import run


def python_port():
    spec = importlib.util.spec_from_file_location('python_workload', run.PROGRAMS / 'workload.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class ContractTests(unittest.TestCase):
    def test_independent_oracle_at_collection_and_batch_boundaries(self):
        port = python_port()
        for name in run.ALL:
            for n in (0, 1, 63, 64, 65, 255, 256, 257):
                for seed in (0, 37, 991):
                    with self.subTest(name=name, n=n, seed=seed):
                        self.assertEqual(run.oracle(name, n, seed), port.run(name, n, seed))

    def test_protocol_rejects_wrong_answers_and_invalid_measurements(self):
        for output in ('BENCH 0 100 42\n', 'BENCH 1 100 41\n', 'BENCH 0 0 41\n',
                       'BENCH 0 100 41.5\n', 'BENCH 0 100 NaN\n', 'BENCH 0 100 41\nnoise\n'):
            with self.subTest(output=output), self.assertRaises(run.BenchError):
                run.parse_samples(output, 1, 41)

    def test_exact_integral_float_output_does_not_lose_precision(self):
        self.assertEqual(run.parse_samples('BENCH 0 100 410.0\n', 1, 410)[0]['checksum'], 410)
        self.assertEqual(run.parse_samples('BENCH 0 100 4.1e2\n', 1, 410)[0]['checksum'], 410)
        with self.assertRaises(run.BenchError):
            run.parse_samples('BENCH 0 100 9007199254740992\n', 1, 9007199254740993)

    def test_statistics_keep_range_and_sample_count(self):
        self.assertEqual(run.stats([1, 2, 300])['median_ns'], 2)
        self.assertEqual(run.stats([1, 2, 300])['max_ns'], 300)
        self.assertEqual(run.stats([1, 2, 300])['n'], 3)
        self.assertIsNone(run.stats([]))

    def test_source_jit_does_not_load_aot_objects(self):
        configured = {v.id: v for v in run.variants()}
        jit = configured['jank-source'].command
        aot = list(map(str, configured['jank-native'].builds[0]))
        self.assertNotEqual(jit[jit.index('--target-dir') + 1], aot[aot.index('--target-dir') + 1])

    def test_nonzero_exit_and_timeout_are_failures(self):
        with self.assertRaises(run.BenchError):
            run.execute([sys.executable, '-c', 'raise SystemExit(3)'])
        with self.assertRaises(run.BenchError):
            run.execute([sys.executable, '-c', 'import time; time.sleep(2)'], timeout=0.02)

    def test_report_escapes_untrusted_runtime_output(self):
        data = dict(metadata=dict(arguments={}, utc='today', cpu='<script>bad</script>', platform='test',
                                  revision='abc', logical_cpus=1), workloads={}, variants=[], results=[])
        document = report.render(data)
        self.assertNotIn('<script>bad</script>', document)
        self.assertIn('&lt;script&gt;bad&lt;/script&gt;', document)
        self.assertIn('\\u003cscript', document)


if __name__ == '__main__':
    unittest.main()
