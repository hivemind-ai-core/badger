#!/usr/bin/env bash
# Run TLC over the models in this directory and compare every outcome with
# the expected one (README.md, "Checks"). TLC runs in a scratch directory, so
# nothing (states/, trace specs) is written into the repository.
#
#   spec/tla/run.sh            every check
#   spec/tla/run.sh NAME...    the named checks (see the list at the end)
#   spec/tla/run.sh JAR [NAME...]  the same, with the TLC jar's path first
#
# TLA2TOOLS (or a first argument ending in .jar) names the jar (default ~/.local/share/tla/tla2tools.jar; TLC 2.19
# from https://github.com/tlaplus/tlaplus/releases). TLC_WORKERS sets TLC's
# -workers (default auto).
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
jar=${TLA2TOOLS:-$HOME/.local/share/tla/tla2tools.jar}
if [ $# -gt 0 ] && [[ $1 == *.jar ]]; then jar=$1; shift; fi
workers=${TLC_WORKERS:-auto}
[ -f "$jar" ] || { echo "run.sh: no TLC jar at $jar (set TLA2TOOLS)" >&2; exit 2; }

work=$(mktemp -d "${TMPDIR:-/tmp}/badger-tla.XXXXXX")
trap 'rm -rf "$work"' EXIT
cp "$here"/*.tla "$here"/*.cfg "$work"/

failed=0

# check NAME MODULE BASE_CFG EXPECT [SED_SCRIPT [APPENDED_TEXT]]
#   EXPECT: "pass", or the name of the property TLC must report violated
#   ("temporal" for a liveness violation, which TLC does not name)
check() {
    local name=$1 module=$2 base=$3 expect=$4 script=${5:-} extra=${6:-}
    if [ ${#selected[@]} -gt 0 ] && [[ " ${selected[*]} " != *" $name "* ]]; then
        return
    fi
    local cfg="$work/$name.cfg" out="$work/$name.out"
    if [ -n "$script" ]; then sed -e "$script" "$work/$base" > "$cfg"; else cp "$work/$base" "$cfg"; fi
    [ -z "$extra" ] || printf '%b\n' "$extra" >> "$cfg"
    local t0 t1
    t0=$(date +%s)
    (cd "$work" && java -XX:+UseParallelGC -cp "$jar" tlc2.TLC -workers "$workers" \
        -metadir "$work/states-$name" -config "$cfg" "$module.tla") > "$out" 2>&1 || true
    t1=$(date +%s)
    local got
    if grep -q "Model checking completed. No error has been found." "$out"; then
        got=pass
    elif grep -qE "Error: (Invariant|Action property) [A-Za-z]+ is violated" "$out"; then
        got=$(grep -oE "Error: (Invariant|Action property) [A-Za-z]+ is violated" "$out" \
              | head -1 | awk '{print $(NF-2)}')
    elif grep -q "Temporal properties were violated" "$out"; then
        got=temporal
    else
        got=error
    fi
    local states
    states=$(grep -oE "[0-9,]+ distinct states found" "$out" | tail -1 | cut -d' ' -f1)
    local verdict=ok
    [ "$got" = "$expect" ] || { verdict=UNEXPECTED; failed=1; }
    printf '%-28s %-10s expected %-24s got %-24s %10s states %5ss\n' \
        "$name" "$verdict" "$expect" "$got" "${states:-?}" "$((t1 - t0))"
    if [ "$verdict" != ok ]; then
        sed -n '1,200p' "$out" >&2
    fi
}

selected=("$@")

# --- channel state machine (ChannelBadger.tla) -------------------------------
cur='s/^    Impl = .*/    Impl = "current"/'
four='s/^    Tasks = .*/    Tasks = {t1, t2, t3, t4}/;s/^    Kinds = .*/    Kinds = {"batch"}/;s/^    Ops = .*/    Ops = 1/;s/^    WorkerCounts = .*/    WorkerCounts = {1}/;s/^    MaxSpurious = .*/    MaxSpurious = 0/'
# the current code, prompt channels: every safety invariant holds
check channel                ChannelBadger ChannelBadger.cfg pass
# the current code, prompt channels: every blocked receive is woken
check channel-live           ChannelBadger ChannelBadgerLive.cfg pass
# the current code under the contract's configuration (both kinds, every
# property), with the list and word invariants and WatchKept
check channel-full           ChannelBadger ChannelBadgerContract.cfg pass "$cur" \
    'INVARIANTS\n    CountsMatch\n    OneEntryPerKind\n    PhasePublished\nPROPERTIES\n    WatchKept'
check channel-full-live      ChannelBadger ChannelBadgerContractLive.cfg pass "$cur"
# the contract (OPTIMIZATION_BACKLOG.md 1.2), both kinds: everything holds
check contract               ChannelBadger ChannelBadgerContract.cfg pass
check contract-live          ChannelBadger ChannelBadgerContractLive.cfg pass
# the contract lets a publication forget the watchers; the code does not
check contract-forgets-watchers ChannelBadger ChannelBadgerContract.cfg WatchKept '' \
    'PROPERTIES\n    WatchKept'
# findings F1-F3 of the model of the code before the refactor: fixed
check f1-cancel-after-handoff ChannelBadger ChannelBadger.cfg pass \
    's/^    AtMostOnce$/    ExactlyOnce/'
check f2-batch-one-wake      ChannelBadger ChannelBadger.cfg pass \
    's/^    Kinds = .*/    Kinds = {"batch"}/'
check f2-batch-one-wake-live ChannelBadger ChannelBadgerLive.cfg pass \
    's/^    Kinds = .*/    Kinds = {"batch"}/'
check f3-one-step            ChannelBadger ChannelBadger.cfg pass '' \
    'PROPERTIES\n    OneStep'
check f3-registration-served ChannelBadger ChannelBadger.cfg pass '' \
    'PROPERTIES\n    RegistrationServed'
# finding F4: a batch receiver woken to retry and then cancelled takes its
# wake with it (four tasks: three receivers); without the cancellation the
# same configuration passes
check f4-cancel-after-retry-wake ChannelBadger ChannelBadger.cfg NoLostWake "$four"
check f4-cancel-after-retry-wake-live ChannelBadger ChannelBadgerLive.cfg temporal "$four"
check f4-no-cancel           ChannelBadger ChannelBadger.cfg pass \
    "$four;s/^    MaxCancels = .*/    MaxCancels = 0/"
# mutations: the defect the slice-4 trace shows, and the probe-register window
check m-fast-ignores-watchers ChannelBadger ChannelBadger.cfg NoLostWake \
    's/^    Mutation = .*/    Mutation = "fast_ignores_watchers"/'
check m-fast-ignores-watchers-live ChannelBadger ChannelBadgerLive.cfg temporal \
    's/^    Mutation = .*/    Mutation = "fast_ignores_watchers"/'
check m-select-no-claim      ChannelBadger ChannelBadger.cfg NoLostWake \
    's/^    Mutation = .*/    Mutation = "select_no_claim"/'
# one worker, locked code that does not mark the word STALE: the published
# phase is wrong, and nothing else is (a decline on it is always safe)
check m-no-stale             ChannelBadger ChannelBadger.cfg PhasePublished \
    's/^    Mutation = .*/    Mutation = "no_stale"/'
check m-no-stale-safe        ChannelBadger ChannelBadger.cfg pass \
    's/^    Mutation = .*/    Mutation = "no_stale"/;/^    PhasePublished$/d'
# the code before the refactor, one defect at a time: F1, F2, F3 come back
check m-cancel-retracts      ChannelBadger ChannelBadger.cfg ExactlyOnce \
    's/^    Mutation = .*/    Mutation = "cancel_retracts"/;s/^    AtMostOnce$/    ExactlyOnce/'
check m-one-wake-per-batch   ChannelBadger ChannelBadger.cfg NoLostWake \
    's/^    Mutation = .*/    Mutation = "one_wake_per_batch"/;s/^    Kinds = .*/    Kinds = {"batch"}/'
check m-one-wake-per-batch-live ChannelBadger ChannelBadgerLive.cfg temporal \
    's/^    Mutation = .*/    Mutation = "one_wake_per_batch"/;s/^    Kinds = .*/    Kinds = {"batch"}/'
check m-queue-forgets-watchers ChannelBadger ChannelBadger.cfg OneStep \
    's/^    Mutation = .*/    Mutation = "queue_forgets_watchers"/' 'PROPERTIES\n    OneStep'
check m-queue-forgets-watchers-reg ChannelBadger ChannelBadger.cfg RegistrationServed \
    's/^    Mutation = .*/    Mutation = "queue_forgets_watchers"/' 'PROPERTIES\n    RegistrationServed'

# --- GC handshake (GcHandshake.tla) -----------------------------------------
check gc                     GcHandshake GcHandshake.cfg pass
check gc-check-before-inc    GcHandshake GcHandshake.cfg EveryWorkerObservesEpoch \
    's/^    Mutation = .*/    Mutation = "check_before_inc"/'
check gc-unlock-before-inc   GcHandshake GcHandshake.cfg EveryWorkerObservesEpoch \
    's/^    Mutation = .*/    Mutation = "unlock_before_inc"/'
check gc-dec-before-publish  GcHandshake GcHandshake.cfg RootsPublished \
    's/^    Mutation = .*/    Mutation = "dec_before_publish"/'
check gc-nested-collect      GcHandshake GcHandshake.cfg ScanUnderStop \
    's/^    Mutation = .*/    Mutation = "nested_collect"/'

exit $failed
