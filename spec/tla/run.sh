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
# finding F4: a batch receiver woken to retry and then cancelled took its
# wake with it (four tasks: three receivers); fixed in ac3f85c, its claim
# passes on. The contract carries the same rule
check f4-cancel-after-retry-wake ChannelBadger ChannelBadger.cfg pass "$four"
check f4-cancel-after-retry-wake-live ChannelBadger ChannelBadgerLive.cfg pass "$four"
check f4-no-cancel           ChannelBadger ChannelBadger.cfg pass \
    "$four;s/^    MaxCancels = .*/    MaxCancels = 0/"
# the same four tasks at one worker and one worker per task, with a spurious
# wake (safety only: the liveness form without symmetry is too large)
wide='s/^    WorkerCounts = .*/    WorkerCounts = {1, 3}/;s/^    MaxSpurious = .*/    MaxSpurious = 1/'
check f4-workers             ChannelBadger ChannelBadger.cfg pass "$four;$wide"
check f4-contract            ChannelBadger ChannelBadgerContract.cfg pass "$four"
check f4-contract-live       ChannelBadger ChannelBadgerContractLive.cfg pass "$four"
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
# one wake per publication with ac3f85c's claims: a receiver waits with a
# message published (NoLostWake), but each taker passes the next wake on, so
# every blocked receive is still woken
check m-one-wake-claimed     ChannelBadger ChannelBadger.cfg NoLostWake \
    's/^    Mutation = .*/    Mutation = "one_wake_claimed"/;s/^    Kinds = .*/    Kinds = {"batch"}/'
check m-one-wake-claimed-live ChannelBadger ChannelBadgerLive.cfg pass \
    's/^    Mutation = .*/    Mutation = "one_wake_claimed"/;s/^    Kinds = .*/    Kinds = {"batch"}/'
check m-queue-forgets-watchers ChannelBadger ChannelBadger.cfg OneStep \
    's/^    Mutation = .*/    Mutation = "queue_forgets_watchers"/' 'PROPERTIES\n    OneStep'
check m-queue-forgets-watchers-reg ChannelBadger ChannelBadger.cfg RegistrationServed \
    's/^    Mutation = .*/    Mutation = "queue_forgets_watchers"/' 'PROPERTIES\n    RegistrationServed'
# the code before ac3f85c: a woken receiver that dies keeps its claim; F4
# comes back (NoLostWake, liveness), and ClaimsHeld catches the dead claim
# at three tasks already
dead='s/^    Mutation = .*/    Mutation = "dead_keeps_claim"/'
check m-dead-keeps-claim     ChannelBadger ChannelBadger.cfg NoLostWake "$four;$dead;/^    ClaimsHeld$/d"
check m-dead-keeps-claim-live ChannelBadger ChannelBadgerLive.cfg temporal "$four;$dead"
check m-dead-keeps-claim-held ChannelBadger ChannelBadger.cfg ClaimsHeld \
    "$dead;s/^    Kinds = .*/    Kinds = {\"batch\"}/"

# --- GC handshake (GcHandshake.tla) -----------------------------------------
# gcm MUTATION: the sed script that selects a mutation
gcm() { echo "s/^    Mutation = .*/    Mutation = \"$1\"/"; }
# gconly NAME: the sed script that keeps TypeOK and NAME (an invariant or a
# property) and drops every other invariant and property
gconly() { echo "/^INVARIANTS/,\${/^    /{/^    \\(TypeOK\\|$1\\)\$/!d}}"; }
# the QSBR handshake of 6cd8e06: every property holds
check gc                     GcHandshake GcHandshake.cfg pass
# finding G1: a slot the grace period read as acknowledged falls back to the
# old epoch in online's window (published, not yet re-read), contrary to
# stop_the_world's comment; harmless (the worker backs out)
check gc-acks-stay           GcHandshake GcHandshake.cfg AcksStay \
    "$(gconly AcksStay);s/^INVARIANTS\$/INVARIANTS\n    AcksStay/"
# online's re-read may test the parity alone (a variant, not a defect)
check gc-recheck-parity      GcHandshake GcHandshake.cfg pass "$(gcm recheck_parity)"
# mutations: each breaks the property its check names
check gc-online-no-publish   GcHandshake GcHandshake.cfg EveryWorkerObservesEpoch \
    "$(gcm online_no_publish);$(gconly EveryWorkerObservesEpoch)"
check gc-online-no-publish-slot GcHandshake GcHandshake.cfg SlotMatches \
    "$(gcm online_no_publish);$(gconly SlotMatches)"
check gc-online-no-recheck   GcHandshake GcHandshake.cfg EveryWorkerObservesEpoch \
    "$(gcm online_no_recheck);$(gconly EveryWorkerObservesEpoch)"
check gc-online-no-backout   GcHandshake GcHandshake.cfg temporal \
    "$(gcm online_no_backout);$(gconly CollectionCompletes)"
check gc-online-no-backout-spin GcHandshake GcHandshake.cfg temporal \
    "$(gcm online_no_backout);$(gconly SpinnersProceed)"
check gc-unlock-before-online GcHandshake GcHandshake.cfg EveryWorkerObservesEpoch \
    "$(gcm unlock_before_online);$(gconly EveryWorkerObservesEpoch)"
check gc-offline-before-publish GcHandshake GcHandshake.cfg RootsPublished \
    "$(gcm offline_before_publish);$(gconly RootsPublished)"
check gc-read-before-advance GcHandshake GcHandshake.cfg EveryWorkerObservesEpoch \
    "$(gcm read_before_advance);$(gconly EveryWorkerObservesEpoch)"
check gc-scan-before-grace   GcHandshake GcHandshake.cfg EveryWorkerObservesEpoch \
    "$(gcm scan_before_grace);$(gconly EveryWorkerObservesEpoch)"
check gc-nested-grace        GcHandshake GcHandshake.cfg ScanUnderStop \
    "$(gcm nested_grace);$(gconly ScanUnderStop)"
check gc-nested-grace-resume GcHandshake GcHandshake.cfg ResumeAfterSweep \
    "$(gcm nested_grace);$(gconly ResumeAfterSweep)"
check gc-nested-resume-ends  GcHandshake GcHandshake.cfg ScanUnderStop \
    "$(gcm nested_resume_ends);$(gconly ScanUnderStop)"
check gc-nested-resume-ends-resume GcHandshake GcHandshake.cfg ResumeAfterSweep \
    "$(gcm nested_resume_ends);$(gconly ResumeAfterSweep)"
check gc-poll-acks           GcHandshake GcHandshake.cfg SlotMatches \
    "$(gcm poll_acks);$(gconly SlotMatches)"
check gc-poll-acks-observes  GcHandshake GcHandshake.cfg EveryWorkerObservesEpoch \
    "$(gcm poll_acks);$(gconly EveryWorkerObservesEpoch)"

exit $failed
