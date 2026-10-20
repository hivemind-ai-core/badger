#!/usr/bin/env python3
"""Score solutions to the evaluation tasks.

Each task in tasks/NAME has task.md (what to do), start/ (the project an
agent is given), accept/ (acceptance tests the agent does not see), and
reference/ (a known solution: files that replace or add to start/).

    python3 eval/run.py --reference            # every task's reference solution
    python3 eval/run.py --candidates DIR       # DIR/NAME/ holds an agent's files per task
    python3 eval/run.py --candidates DIR --task add_field --json

A solution is applied over a copy of start/, the acceptance tests are
added, and then: `badger check` of the project (does it check, and how
many diagnostics), the project's own tests, and the acceptance tests. A
task is solved when the project checks and every acceptance test passes.
"""
import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
TASKS = os.path.join(HERE, "tasks")


def overlay(src, dst):
    for root, _, files in os.walk(src):
        rel = os.path.relpath(root, src)
        os.makedirs(os.path.join(dst, rel), exist_ok=True)
        for f in files:
            shutil.copy(os.path.join(root, f), os.path.join(dst, rel, f))


def changed_lines(start, solution):
    n = 0
    for root, _, files in os.walk(solution):
        for f in files:
            rel = os.path.relpath(os.path.join(root, f), solution)
            new = open(os.path.join(solution, rel), encoding="utf-8").read().splitlines()
            old_path = os.path.join(start, rel)
            old = open(old_path, encoding="utf-8").read().splitlines() if os.path.exists(old_path) else []
            n += len(set(new) ^ set(old))
    return n


def run(badger, args, cwd):
    p = subprocess.run([badger] + args, cwd=cwd, capture_output=True, text=True, timeout=300)
    return p.returncode, p.stdout, p.stderr


def tests(badger, path, cwd):
    _, out, _ = run(badger, ["test", path, "--json"], cwd)
    try:
        v = json.loads(out.strip().splitlines()[-1])
    except (ValueError, IndexError):
        return 0, None
    if not isinstance(v, dict) or v.get("diagnostics"):
        return 0, None  # it did not check, so nothing ran
    return v["passed"], v["passed"] + v["failed"]


def score(badger, task, solution):
    base = os.path.join(TASKS, task)
    with tempfile.TemporaryDirectory(prefix=f"badger-eval-{task}-") as tmp:
        project = os.path.join(tmp, "project")
        shutil.copytree(os.path.join(base, "start"), project)
        if solution and os.path.isdir(solution):
            overlay(solution, project)
        code, out, _ = run(badger, ["check", "--json", "."], project)
        try:
            diagnostics = len(json.loads(out or "[]"))
        except ValueError:
            diagnostics = None
        checks = code == 0
        own = tests(badger, ".", project)
        overlay(os.path.join(base, "accept"), project)
        accept = tests(badger, "src/accept.bdg", project)
        return {
            "task": task,
            "solved": checks and accept[1] is not None and accept[0] == accept[1],
            "checks": checks,
            "diagnostics": diagnostics,
            "tests": {"passed": own[0], "total": own[1]},
            "acceptance": {"passed": accept[0], "total": accept[1]},
            "changed_lines": changed_lines(os.path.join(base, "start"), solution) if solution and os.path.isdir(solution) else 0,
        }


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    src = ap.add_mutually_exclusive_group(required=True)
    src.add_argument("--reference", action="store_true", help="score each task's reference solution")
    src.add_argument("--candidates", help="directory with one solution directory per task")
    src.add_argument("--start", action="store_true", help="score the unchanged start projects (expected unsolved)")
    ap.add_argument("--task", action="append", help="only these tasks")
    ap.add_argument("--badger", default=os.environ.get("BADGER", "badger"))
    ap.add_argument("--json", action="store_true")
    a = ap.parse_args()
    names = sorted(a.task or [t for t in os.listdir(TASKS) if os.path.isdir(os.path.join(TASKS, t))])
    results = []
    for t in names:
        if a.reference:
            sol = os.path.join(TASKS, t, "reference")
        elif a.candidates:
            sol = os.path.join(a.candidates, t)
        else:
            sol = None
        results.append(score(a.badger, t, sol))
    solved = sum(r["solved"] for r in results)
    if a.json:
        print(json.dumps({"solved": solved, "tasks": len(results), "results": results}, indent=2))
    else:
        for r in results:
            acc = r["acceptance"]
            print(f"{r['task']:<16} {'solved' if r['solved'] else 'unsolved':<9} checks={r['checks']} "
                  f"diagnostics={r['diagnostics']} acceptance={acc['passed']}/{acc['total']} changed_lines={r['changed_lines']}")
        print(f"\n{solved}/{len(results)} solved")
    sys.exit(0 if solved == len(results) or a.start else 1)


if __name__ == "__main__":
    main()
