#!/usr/bin/env bash
# Run TLC over the models in this directory and compare every outcome with
# the expected one (README.md, "Checks"). TLC runs in a scratch directory, so
# nothing (states/, trace specs) is written into the repository.
#
#   spec/tla/run.sh            every check
#   spec/tla/run.sh NAME...    the named checks (see the list at the end)
#
# TLA2TOOLS names the jar (default ~/.local/share/tla/tla2tools.jar; TLC 2.19
# from https://github.com/tlaplus/tlaplus/releases). TLC_WORKERS sets TLC's
# -workers (default auto).
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
jar=${TLA2TOOLS:-$HOME/.local/share/tla/tla2tools.jar}
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
# the current code, prompt channels: every safety invariant holds
check channel                ChannelBadger ChannelBadger.cfg pass
# the current code, prompt channels: every blocked receive is woken
check channel-live           ChannelBadger ChannelBadgerLive.cfg pass
# the contract (OPTIMIZATION_BACKLOG.md 1.2), both kinds: everything holds
check contract               ChannelBadger ChannelBadgerContract.cfg pass
check contract-live          ChannelBadger ChannelBadgerContractLive.cfg pass
# finding F1: a value handed to a receiver that is then cancelled is lost
check f1-cancel-after-handoff ChannelBadger ChannelBadger.cfg ExactlyOnce \
    's/^    AtMostOnce$/    ExactlyOnce/'
# finding F2: a published batch wakes one receiver; a second message strands
check f2-batch-one-wake      ChannelBadger ChannelBadger.cfg NoLostWake \
    's/^    Kinds = .*/    Kinds = {"batch"}/'
# finding F3: the queue branch of send wakes the watchers and forgets them
check f3-one-step            ChannelBadger ChannelBadger.cfg OneStep '' \
    'PROPERTIES\n    OneStep'
check f3-registration-served ChannelBadger ChannelBadger.cfg RegistrationServed '' \
    'PROPERTIES\n    RegistrationServed'
# mutations: the defect the slice-4 trace shows, and the probe-register window
check m-fast-ignores-watchers ChannelBadger ChannelBadger.cfg NoLostWake \
    's/^    Mutation = .*/    Mutation = "fast_ignores_watchers"/'
check m-select-no-claim      ChannelBadger ChannelBadger.cfg NoLostWake \
    's/^    Mutation = .*/    Mutation = "select_no_claim"/'
# the liveness property alone catches the slice-4 defect too
check m-fast-ignores-watchers-live ChannelBadger ChannelBadgerLive.cfg temporal \
    's/^    Mutation = .*/    Mutation = "fast_ignores_watchers"/'
# and the batch finding F2
check f2-batch-one-wake-live ChannelBadger ChannelBadgerLive.cfg temporal \
    's/^    Kinds = .*/    Kinds = {"batch"}/'

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
