"""Regression checks for evidence integrity; full port checks are run by --smoke."""
import importlib.util
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("realworld", Path(__file__).with_name("run.py"))
bench = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(bench)


class OracleTests(unittest.TestCase):
    def test_affine_oracle_matches_scalar_recurrence(self):
        for seed in [0, 37, 991]:
            for rounds in [2, 128]:
                total = 0
                for i in range(1025):
                    x = (i + seed) % 1000003
                    for _ in range(rounds):
                        x = (x * 17 + 23) % 1000003
                    total += x
                self.assertEqual(bench.score_sum(1025, seed, rounds), total)

    def test_state_oracle_across_snapshot_and_key_boundaries(self):
        for n in [1, 255, 256, 257, 1024, 1025, 2049]:
            current, versions = {}, []
            for i in range(n):
                current[i % 1024] = (i * 31 + 991) % 100000
                if (i + 1) % 256 == 0:
                    versions.append(current.copy())
            versions.append(current.copy())
            expected = sum((k + 1) * v for snapshot in versions for k, v in snapshot.items())
            self.assertEqual(bench.oracle("state", n, 991), expected)

    def test_fixture_exercises_utf8_and_escaping(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "input.ndjson"
            bench.fixture(path, 65, 37)
            rows = [json.loads(line) for line in path.read_text().splitlines()]
            self.assertEqual(rows[64], bench.order(64, 37))
            self.assertIn('café ☘', rows[64]["note"])
            self.assertIn('\\"', path.read_text())
            expected = sum((r["customer"] + 1) * (r["amount"] * 6 // 5) +
                           r["id"] + len(r["note"].encode()) for r in rows if r["active"])
            self.assertEqual(bench.oracle("json", 65, 37), expected)


class JavaEvidenceTests(unittest.TestCase):
    def test_java_injected_options_and_classpath_are_removed(self):
        names = ["JAVA_TOOL_OPTIONS", "JDK_JAVA_OPTIONS", "_JAVA_OPTIONS", "JDK_JAVAC_OPTIONS", "CLASSPATH"]
        with patch.dict(os.environ, {name: "injected" for name in names}):
            env = bench.clean_env()
        self.assertFalse(set(names) & set(env))

    def test_java_requires_exact_pinned_json_library(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            jar = root / "gson.jar"
            with self.assertRaisesRegex(ValueError, "pinned Gson"):
                bench.java_dependencies(root / "java", root / "javac", jar)
            jar.write_bytes(b"wrong jar at the right path")
            with self.assertRaisesRegex(ValueError, "pinned Gson"):
                bench.java_dependencies(root / "java", root / "javac", jar)

    def test_java_artifact_identity_covers_nested_classes(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "Workload.class").write_bytes(b"main")
            nested = root / "Workload$Order.class"
            nested.write_bytes(b"record")
            before = bench.artifact_identity(root)
            self.assertEqual(before["artifact_bytes"], 10)
            self.assertEqual(len(before["files"]), 2)
            nested.write_bytes(b"changed record")
            after = bench.artifact_identity(root)
            self.assertNotEqual(before["sha256"], after["sha256"])


class ProtocolTests(unittest.TestCase):
    def record(self, stdout="READY\nBENCH 0 11 7\nBENCH 1 22 7\n"):
        return dict(stdout=stdout, stderr="bench: run 0.1 ms (compilation 4.125 ms, not included)\n")

    def test_warmups_and_full_launch_compilation_are_separate(self):
        r = bench.validate(self.record(), 7, 1, 1)
        self.assertEqual(r["iterations_ns"], [11, 22])
        self.assertEqual(r["warm_ns"], [22])
        self.assertEqual(r["first_ns"], 11)
        self.assertEqual(r["compile_ns"], 4125000)

    def test_rejects_wrong_warmup_checksum_timer_index_and_extra_output(self):
        for output in ["READY\nBENCH 0 11 8\nBENCH 1 22 7\n",
                       "READY\nBENCH 0 0 7\nBENCH 1 22 7\n",
                       "READY\nBENCH 1 11 7\nBENCH 1 22 7\n",
                       "READY\nBENCH 0 11 7\n", "noise\nBENCH 0 11 7\nBENCH 1 22 7\n",
                       "READY\nBENCH 0 11 7\nBENCH 1 22 7\nextra\n"]:
            with self.subTest(output=output), self.assertRaises(ValueError):
                bench.validate(self.record(output), 7, 1, 1)

    def test_summary_weights_processes_equally(self):
        r = dict(warm_ns=[10, 10, 1000], first_ns=100, ready_ns=20, wall_ns=2000,
                 peak_rss_bytes=1000, user_ns=50, system_ns=5, compile_ns=None)
        other = r | dict(warm_ns=[30, 30, 30])
        stats = bench.summarize([r, other])
        self.assertEqual(stats["kernel_ns"]["median"], 20)
        self.assertEqual(stats["kernel_ns"]["count"], 2)
        self.assertIsNone(stats["compile_ns"])


@unittest.skipUnless(sys.platform == "linux", "Linux wait4 metrics")
class ProcessTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temporary = tempfile.TemporaryDirectory()
        cls.launcher = Path(cls.temporary.name) / "measure"
        subprocess.run(["cc", "-O2", "-Wall", "-Wextra", "-Werror", str(bench.SUITE / "measure.c"),
                        "-o", str(cls.launcher)], check=True)

    @classmethod
    def tearDownClass(cls):
        cls.temporary.cleanup()

    def process(self, *args, **kwargs):
        return bench.process(*args, **kwargs, launcher=self.launcher)

    def test_rss_excludes_resident_parent_allocations(self):
        retained = bytearray(96 * 1024 * 1024)
        r = self.process([sys.executable, "-c", "pass"], bench.clean_env(), 5)
        self.assertLess(r["peak_rss_bytes"], 32 * 1024 * 1024)
        self.assertGreater(r["launcher_tree_peak_rss_bytes"], 64 * 1024 * 1024)
        self.assertEqual(len(retained), 96 * 1024 * 1024)

    def test_ready_arrives_before_completion_and_large_output_does_not_deadlock(self):
        source = "import time; print('READY', flush=True); time.sleep(.08); print('x'*200000)"
        r = self.process([sys.executable, "-c", source], bench.clean_env(), 5, True)
        self.assertLess(r["ready_ns"] + 40_000_000, r["wall_ns"])
        self.assertGreater(len(r["stdout"]), 200000)
        self.assertGreater(r["peak_rss_bytes"], 0)

    def test_peak_rss_does_not_inherit_the_previous_child_maximum(self):
        large = self.process([sys.executable, "-c", "x=bytearray(64*1024*1024)"], bench.clean_env(), 5)
        small = self.process([sys.executable, "-c", "pass"], bench.clean_env(), 5)
        self.assertGreater(large["peak_rss_bytes"], small["peak_rss_bytes"] + 32 * 1024 * 1024)

    def test_timeout_kills_descendants(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "pid"
            source = ("import subprocess,sys,time,pathlib; "
                      "child=subprocess.Popen([sys.executable,'-c','import time; time.sleep(30)']); "
                      f"pathlib.Path({str(path)!r}).write_text(str(child.pid)); time.sleep(30)")
            with self.assertRaises(TimeoutError):
                self.process([sys.executable, "-c", source], bench.clean_env(), .2)
            child = int(path.read_text())
            for _ in range(30):
                state = Path(f"/proc/{child}/stat")
                if not state.exists() or state.read_text().split()[2] == "Z":
                    break
                time.sleep(.01)
            else:
                os.kill(child, signal.SIGKILL)
                self.fail("timeout left a running descendant")

    def test_missing_ready_and_failed_commands_are_rejected(self):
        with self.assertRaises(ValueError):
            self.process([sys.executable, "-c", "print('wrong')"], bench.clean_env(), 5, True)
        with self.assertRaises(RuntimeError):
            self.process([sys.executable, "-c", "raise RuntimeError('failure')"], bench.clean_env(), 5)


@unittest.skipUnless(os.environ.get("REALWORLD_BADGER"), "set REALWORLD_BADGER for Badger report integration")
class ReportTests(unittest.TestCase):
    def generate(self, root, data, template=None):
        runs = root / "runs"
        runs.mkdir(exist_ok=True)
        for i, report in enumerate(data):
            (runs / f"{i}.json").write_text(json.dumps(report))
        command = [os.environ["REALWORLD_BADGER"], "run", "--release", str(bench.SUITE / "report.bdg"),
                   "--", str(runs), str(root / "index.html"), str(template or bench.SUITE / "dashboard.html")]
        return subprocess.run(command, cwd=bench.ROOT, capture_output=True, text=True)

    def test_reads_all_reports_and_escapes_script_terminators(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            report = dict(schema=1, suite=bench.CONTRACT, stamp="a", status="complete", cohort="c",
                          payload="</script><script>globalThis.injected=true</script>")
            result = self.generate(root, [report, report | dict(stamp="b", status="failed")])
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("from 2 stamped reports", result.stdout)
            html = (root / "index.html").read_text()
            self.assertNotIn(report["payload"], html)
            embedded = html.split('<script id="reports" type="application/json">')[1].split('</script>')[0]
            self.assertEqual(json.loads(embedded), [report, report | dict(stamp="b", status="failed")])

    def test_rejects_incompatible_or_malformed_report(self):
        with tempfile.TemporaryDirectory() as tmp:
            result = self.generate(Path(tmp), [dict(schema=2, suite=bench.CONTRACT, stamp="a",
                                                   status="complete", cohort="c")])
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse((Path(tmp) / "index.html").exists())
        with tempfile.TemporaryDirectory() as tmp:
            result = self.generate(Path(tmp), [dict(stamp="incomplete")])
            self.assertNotEqual(result.returncode, 0)

    def test_rejects_template_without_placeholder(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            template = root / "template.html"
            template.write_text("<html></html>")
            report = dict(schema=1, suite=bench.CONTRACT, stamp="a", status="complete", cohort="c")
            result = self.generate(root, [report], template)
            self.assertNotEqual(result.returncode, 0)


if __name__ == "__main__":
    unittest.main()
