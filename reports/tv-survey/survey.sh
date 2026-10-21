#!/bin/sh
# Translation-validation survey (§4.3): compile every tests/run program with
# the validator on for every pass (`BADGER_MIR_TV=all`), and collect one line
# per function a pass changed.
#
#   reports/tv-survey/survey.sh BADGER LEVEL OUT.tsv
#
# BADGER is the CLI binary, LEVEL the MIR optimisation level (release,
# aggressive, superopt). Each line of OUT.tsv is: program, pass, function,
# verdict, microseconds, statements, reason. The per-program summaries go to
# OUT.tsv.log.
set -eu
badger=$1
level=$2
out=$3
tmp=$(mktemp -d)
: > "$out"
: > "$out.log"
ls tests/run/*.bdg | xargs -P "${JOBS:-8}" -I{} sh -c '
    name=$(basename {} .bdg)
    log='"$tmp"'/$name.tsv
    BADGER_MIR_TV=all BADGER_MIR_TV_LOG=$log "'"$badger"'" compile --opt "'"$level"'" -o '"$tmp"'/$name.bdgmir {} \
        > '"$tmp"'/$name.err 2>&1 || true
    touch "$log"
'
for f in "$tmp"/*.tsv; do
    name=$(basename "$f" .tsv)
    sed "s|^|$name\t|" "$f" >> "$out"
    { echo "== $name"; cat "$tmp/$name.err"; } >> "$out.log"
done
rm -rf "$tmp"
