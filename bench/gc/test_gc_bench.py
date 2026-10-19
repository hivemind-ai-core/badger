import unittest
import run


class MeasurementOrder(unittest.TestCase):
    def test_concurrent_gc_runners_are_refused_and_lock_is_released(self):
        from pathlib import Path
        import tempfile
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "benchmark.lock"
            with run.benchmark_lock(path):
                with self.assertRaisesRegex(RuntimeError, "another GC benchmark"):
                    with run.benchmark_lock(path):
                        self.fail("overlapping measurement admitted")
            with run.benchmark_lock(path):
                pass

    def test_pairing_keeps_every_cell_and_varies_both_orders(self):
        import random
        cells = [(case, w, v) for case in ["buffers", "control"]
                 for w in [1, 8] for v in ["baseline", "candidate"]]
        observed_groups, observed_variants = set(), set()
        for seed in range(10):
            ordered = run.measurement_order(cells, random.Random(seed), paired=True)
            self.assertCountEqual(ordered, cells)
            self.assertEqual(ordered, run.measurement_order(cells, random.Random(seed), paired=True))
            for index in range(0, len(ordered), 2):
                self.assertEqual(ordered[index][:2], ordered[index + 1][:2])
            observed_groups.add(tuple(cell[:2] for cell in ordered[::2]))
            observed_variants.add(ordered[0][2])
        self.assertGreater(len(observed_groups), 1)
        self.assertEqual(observed_variants, {"baseline", "candidate"})


class CoverageOracle(unittest.TestCase):
    def test_stored_sequences_include_reuse_pointer_mapping_and_escape_controls(self):
        c = dict(n=7, rounds=2, payload=2, retain=1)
        for mode in ["stored", "stored-list", "stored-escape", "stored-collected"]:
            self.assertEqual(run.expected_coverage(c | dict(mode=mode)), 36)
        self.assertEqual(run.expected_coverage(c | dict(mode="stored-reused")), 72)
        # String lengths 2..8; only 3 and 6 pass the mapped-value filter.
        self.assertEqual(run.expected_coverage(c | dict(mode="stored-pointers")), 36)

    def test_map_apis_preserve_transforms_and_overlap_semantics(self):
        c = dict(n=4, rounds=2, payload=3, retain=2)
        for mode, expected in [("map-api-projections", 418), ("map-api-entry-control", 426),
                               ("map-api-values", 414), ("map-api-filter", 208),
                               ("map-api-merge", 1010), ("map-api-composed", 1102),
                               ("map-api-parallel", 4384)]:
            self.assertEqual(run.expected_coverage(c | dict(mode=mode)), expected)

    def test_codec_oracles_include_utf8_and_exact_nested_error_paths(self):
        for mode, expected in [("codec-flat", 154), ("codec-wire", 154),
                               ("codec-nested", 2177), ("codec-invalid", 231)]:
            self.assertEqual(run.expected_coverage(dict(mode=mode, n=7)), expected)

    def test_lookup_misses_negatives_and_stored_control(self):
        c = dict(mode="lookup-map-int", n=13, retain=3, payload=11)
        for mode in ["lookup-map-misses", "lookup-list-misses"]:
            self.assertEqual(run.expected_coverage(c | dict(mode=mode)), 13)
        # Keys cycle -1,0,1,2,3,4: two full cycles and one final miss.
        self.assertEqual(run.expected_coverage(c), 2 * (7 + 38 + 69))
        for mode in ["lookup-array-int", "lookup-map-stored"]:
            self.assertEqual(run.expected_coverage(c | dict(mode=mode)), 228)
        for mode in ["lookup-map-pointers", "lookup-list-pointers"]:
            self.assertEqual(run.expected_coverage(c | dict(mode=mode)), 2 * (11 + 12 + 13))

    def test_completed_task_inputs_and_result_alias_control(self):
        c = dict(mode="tasks-input", n=3, rounds=5, payload=4, retain=64)
        payloads = [sum(i * 31 + j for j in range(4)) for i in range(3)]
        churn = 5 + sum(range(5)) * 2
        self.assertEqual(run.expected_coverage(c), sum(payloads) + churn)
        self.assertEqual(run.expected_coverage(c | dict(mode="tasks-result")), 2 * sum(payloads) + churn)
        record = dict(config=c, checksum=sum(payloads) + churn, elapsed_ns=1,
                      before=dict(collections=0), checkpoints=[dict(collections=i, pool_reserved_bytes=262144) for i in [1, 3, 3]])
        run.validate_coverage(record, c)
        with self.assertRaises(ValueError):
            run.validate_coverage(record | dict(checkpoints=[]), c)

    def test_buffer_churn_checks_growth_pointer_values_and_retained_tail(self):
        c = dict(mode="buffers-grow", n=5, retain=3, payload=4)
        values = [sum(i * 31 + j for j in range(4)) for i in range(5)]
        self.assertEqual(run.expected_coverage(c), sum(values) + sum(values[-3:]))
        values = [sum(32 + i % 3 + j for j in range(4)) for i in range(5)]
        self.assertEqual(run.expected_coverage(c | dict(mode="buffers-pointers")), sum(values) + sum(values[-3:]))
        self.assertEqual(run.expected_coverage(c | dict(mode="buffers-pointers", retain=9)), sum(values) * 2)
        self.assertEqual(run.expected_coverage(c | dict(mode="buffers-empty")), 5)
        for mode in ["buffers-capacity", "buffers-capacity-grow"]:
            self.assertEqual(run.expected_coverage(c | dict(mode=mode)), run.expected_coverage(c))
        self.assertEqual(run.expected_coverage(c | dict(mode="buffers-capacity-pointers")), sum(values) + sum(values[-3:]))

    def test_pool_burst_includes_full_burst_scattered_survivors_and_churn(self):
        c = dict(mode="pool-burst", n=7, retain=3, payload=5, rounds=70)
        latest = {i % 64: i for i in range(70)}
        expected = sum(5 + i % 3 for i in range(7))
        expected += sum(5 + i % 3 for i in [0, 2, 4])
        expected += len(latest) + sum(latest) + sum(latest.values())
        self.assertEqual(run.expected_coverage(c), expected)
        good = dict(config=c, checksum=expected, elapsed_ns=1,
                    before=dict(collections=0), checkpoints=[dict(collections=i, pool_reserved_bytes=262144) for i in [1, 3, 3]])
        run.validate_coverage(good, c)
        with self.assertRaises(ValueError):
            run.validate_coverage(good | dict(checkpoints=[]), c)
        with self.assertRaises(ValueError):
            run.validate_coverage(good | dict(checkpoints=list(reversed(good["checkpoints"]))), c)

    def test_persistent_maps_check_final_keys_and_values(self):
        self.assertEqual(run.expected_coverage(dict(mode="persistent-int", n=5, retain=3)), 3 + 3 + 9)
        self.assertEqual(run.expected_coverage(dict(mode="persistent-int", n=2, retain=9)), 2 + 1 + 1)

    def test_parallel_maps_keep_task_count_fixed_and_check_each_result(self):
        self.assertEqual(run.expected_coverage(dict(mode="parallel-persistent-int", n=5, retain=3, rounds=8)), 8 * (3 + 3 + 9))
        self.assertEqual(run.expected_coverage(dict(mode="parallel-snapshots", n=130, retain=2, payload=5, rounds=8)), 8 * (5 * 130 + 129 + 65 + 129 + 130))

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

    def test_record_windows_at_reset_boundaries(self):
        for n in [1, 3, 4, 5, 8, 9]:
            window = []
            for i in range(n):
                window = [i] if len(window) >= 4 else window + [i]
            expected = sum(window) + len(window) * 5
            self.assertEqual(run.expected_coverage(dict(mode="records-window", n=n, retain=4, payload=4)), expected)

    def test_entry_iterations_include_every_key_and_pointer_value(self):
        for mode in ["entries-immediate", "entries-stored"]:
            self.assertEqual(run.expected_coverage(dict(mode=mode, n=3, rounds=2, payload=4)), 222)

    def test_scalar_snapshots_include_old_values_and_final_partial_window(self):
        for n in [1, 63, 64, 65, 257]:
            current, saved = {}, []
            for i in range(n):
                current[i % 4] = i
                if (i + 1) % 64 == 0:
                    saved.append(current.copy())
            saved.append(current.copy())
            expected = sum(len(m) + sum(m) + sum(m.values()) for m in saved)
            self.assertEqual(run.expected_coverage(dict(mode="scalar-snapshots", n=n, retain=4)), expected)
            self.assertEqual(run.expected_coverage(dict(mode="parallel-scalar-snapshots", n=n, retain=4, rounds=8)), 8 * expected)

    def test_corruption_and_wrong_configuration_rejected(self):
        c = dict(mode="visible", n=7, rounds=2)
        good = dict(config=c, checksum=36, elapsed_ns=1)
        run.validate_coverage(good, c)
        with self.assertRaises(ValueError):
            run.validate_coverage(good | dict(checksum=35), c)
        with self.assertRaises(ValueError):
            run.validate_coverage(good | dict(config=c | dict(n=8)), c)


class SweepAccounting(unittest.TestCase):
    def test_mark_and_remembered_trace_conservation(self):
        heap = dict(mark_candidates=110, marked_objects=50, marked_leaf_objects=30,
                    traced_objects=27, remembered_traced_objects=7)
        run.validate_sweep(dict(collections=[dict(heap=heap)]))
        for patch in [dict(mark_candidates=49), dict(marked_leaf_objects=51),
                      dict(traced_objects=26), dict(remembered_traced_objects=-1),
                      dict(marked_objects=True)]:
            with self.assertRaisesRegex(ValueError, 'marking/trace'):
                run.validate_sweep(dict(collections=[dict(heap=heap | patch)]))
        del heap['marked_leaf_objects']
        with self.assertRaisesRegex(ValueError, 'marking/trace'):
            run.validate_sweep(dict(collections=[dict(heap=heap)]))

    def test_work_and_helper_conservation(self):
        heap = dict(swept=100, sweep_work_per_worker=[95, 5, 0],
                    sweep_helpers=1, sweep_work_per_helper=[5])
        run.validate_sweep(dict(collections=[dict(heap=heap)]))
        for patch in [dict(swept=99), dict(sweep_work_per_worker=[95, -5, 10]),
                      dict(sweep_helpers=2), dict(sweep_work_per_helper=[0]),
                      dict(sweep_work_per_helper=[101]),
                      dict(sweep_helpers=2, sweep_work_per_helper=[5, 95]),
                      dict(sweep_helpers_created=2), dict(sweep_helpers_created=-1)]:
            with self.assertRaises(ValueError):
                run.validate_sweep(dict(collections=[dict(heap=heap | patch)]))
        run.validate_sweep(dict(collections=[dict(heap=heap | dict(sweep_helpers=0,
                                                                  sweep_work_per_helper=[]))]))


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
        profile["requested_size_classes"] = dict(through_1024=[dict(rounded_total_bytes=32, allocated=copy.deepcopy(counts["allocated"]))],
                                                over_1024=dict(objects=0, requested_bytes=0, cell_bytes=0))
        run.validate_allocations(profile)
        bad = copy.deepcopy(profile)
        bad["requested_size_classes"]["through_1024"][0]["allocated"]["objects"] -= 1
        with self.assertRaises(ValueError):
            run.validate_allocations(bad)
        for category, metric in [("reclaimed_before_survival", "requested_bytes"),
                                 ("first_survival", "objects"),
                                 ("full_survivor_observations", "cell_bytes")]:
            bad = copy.deepcopy(profile)
            bad["totals"][category][metric] += 1
            with self.assertRaises(ValueError):
                run.validate_allocations(bad)
        profile["buffer_storage"] = dict(
            events={name: dict(calls=calls, bytes=volume) for name, calls, volume in [
                ("native_alloc", 2, 64), ("native_realloc_growth", 1, 16),
                ("native_adopt", 1, 32), ("native_free", 2, 72),
                ("inline", 2, 96), ("inline_to_native", 1, 0)]},
            native_owned_bytes_remaining=40, native_owned_blocks_remaining=1)
        run.validate_allocations(profile)
        for kind, key, value in [("native_free", "calls", 4), ("native_free", "bytes", 73),
                                 ("inline_to_native", "calls", 3), ("inline_to_native", "bytes", 1),
                                 ("native_adopt", "bytes", -1)]:
            bad = copy.deepcopy(profile)
            bad["buffer_storage"]["events"][kind][key] = value
            with self.assertRaises(ValueError):
                run.validate_allocations(bad)
        bad = copy.deepcopy(profile)
        bad["remaining_metadata_objects"] = 3
        with self.assertRaises(ValueError):
            run.validate_allocations(bad)
        profile["cell_sources"] = dict(pooled_reuse=dict(objects=3, requested_bytes=72, cell_bytes=96),
                                       pooled_bump=dict(objects=7, requested_bytes=168, cell_bytes=224),
                                       system=dict(objects=0, requested_bytes=0, cell_bytes=0))
        profile["pool_events"] = dict(refills=dict(chunks=1, bytes=262144), trimmed=dict(chunks=0, bytes=0),
                                      reserved_bytes_remaining=262144)
        run.validate_allocations(profile)
        for source, metric, value in [("pooled_reuse", "objects", 4),
                                      ("pooled_bump", "cell_bytes", -1),
                                      ("system", "requested_bytes", 1)]:
            bad = copy.deepcopy(profile)
            bad["cell_sources"][source][metric] = value
            with self.assertRaises(ValueError):
                run.validate_allocations(bad)
        bad = copy.deepcopy(profile)
        bad["cell_sources"]["system"], bad["cell_sources"]["pooled_reuse"] = bad["cell_sources"]["pooled_reuse"], bad["cell_sources"]["system"]
        with self.assertRaises(ValueError):
            run.validate_allocations(bad)
        bad = copy.deepcopy(profile)
        bad["pool_events"]["reserved_bytes_remaining"] += 1
        with self.assertRaises(ValueError):
            run.validate_allocations(bad)


if __name__ == "__main__":
    unittest.main()
