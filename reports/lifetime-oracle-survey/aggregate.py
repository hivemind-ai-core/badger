"""Sum the `[oracle]` lines that `oracle.patch` prints over a corpus run.

Apply `oracle.patch` to `6b96579`, build `badger`, and run every program:

    for f in tests/run/*.bdg; do
        BADGER_ORACLE=1 timeout 60 badger run --release "$f" \
            >/dev/null 2>"out/$(basename "$f" .bdg).err" </dev/null
    done
    python3 -I aggregate.py out/*.err

For each inlining round: direct call sites, the candidates the inliner can
take (an inlinable callee whose representations fit), how many it accepts,
and, for each group, how many have a dead result or arguments that are all
dead after the call. `would-flip` counts the rejected candidates whose callee
fits the size budget once its returned values are dropped and its dead
assignments removed (the most a "result unused" fact can buy).

For `simplify`, `simplify2` and `reach`: the assignments left in the program,
and those whose destination is dead after them, by right-hand side.
"""

import collections
import re
import sys

ROUND = re.compile(
    r"inline round (\d+): direct=(\d+) \(result dead (\d+), args dead (\d+)\) "
    r"candidates=(\d+) accepted=(\d+) \(result dead (\d+), args dead (\d+)\) "
    r"rejected=(\d+) \(result dead (\d+), args dead (\d+)\) "
    r"would-flip-if-result-dead-discount=(\d+)"
)
KEYS = [
    "direct", "direct result dead", "direct args dead",
    "candidates", "accepted", "accepted result dead", "accepted args dead",
    "rejected", "rejected result dead", "rejected args dead", "would-flip",
]


def main(paths):
    rounds = collections.Counter()
    passes = collections.Counter()
    assigns = collections.Counter()
    kept = collections.Counter()
    named = collections.Counter()
    for p in paths:
        s = open(p, encoding="utf-8", errors="replace").read()
        for m in ROUND.finditer(s):
            for k, v in zip(KEYS, m.groups()[1:]):
                rounds[(int(m.group(1)), k)] += int(v)
        for m in re.finditer(r"\[oracle\] pass (\S+): (\d+)", s):
            passes[m.group(1)] += int(m.group(2))
        for m in re.finditer(r"\[oracle\] (\S+) assigns=(\d+)", s):
            assigns[m.group(1)] += int(m.group(2))
        for m in re.finditer(r"\[oracle\] (\S+) dead-kept (\S+): (\d+) \(named binding: (\d+)\)", s):
            kept[(m.group(1), m.group(2))] += int(m.group(3))
            named[(m.group(1), m.group(2))] += int(m.group(4))
    print(f"programs: {len(paths)}")
    for r in sorted({r for r, _ in rounds}):
        print(f"inline round {r}: " + ", ".join(f"{k} {rounds[(r, k)]}" for k in KEYS))
    print("pass changes: " + ", ".join(f"{k} {v}" for k, v in passes.items()))
    for name, n in assigns.items():
        print(f"{name}: {n} assignments")
        for (p, kind), c in sorted(kept.items()):
            if p == name:
                print(f"  dead, kept: {kind} {c} (named {named[(p, kind)]})")


if __name__ == "__main__":
    main(sys.argv[1:])
