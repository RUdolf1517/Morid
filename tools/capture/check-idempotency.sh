#!/bin/sh
# Prove the idempotency claim instead of asserting it.
#
# Runs the scenario capture twice, hashes every file under fixtures/ before,
# between and after the runs, and compares the sorted manifests and the
# porcelain status.  Prints IDEMPOTENCY_OK, or the first path that differs.
#
# Requirements match tools/capture/capture.py: QT_PREFIX, CAPTURE_DRIVER and a
# built legacy executable (see fixtures/README.md and build-reference.sh).
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
cd "$ROOT"

: "${QT_PREFIX:?set QT_PREFIX to the Qt 5.15.16 prefix used by build-reference.sh}"
: "${CAPTURE_DRIVER:?set CAPTURE_DRIVER to the driver built by build-reference.sh}"
LEGACY=${CAPTURE_LEGACY:-/home/user/.cache/morid-capture/legacy-build/media-downloader}
if [ ! -x "$LEGACY" ]; then
    echo "idempotency: legacy executable not found: $LEGACY" >&2
    exit 2
fi

WORK=$(mktemp -d "${TMPDIR:-/tmp}/morid-idempotency.XXXXXX")
trap 'rm -rf "$WORK"' EXIT INT TERM

manifest() {
    # Sorted "sha256  path" for every file under fixtures/, excluding nothing.
    find fixtures -type f -print0 |
        LC_ALL=C sort -z |
        xargs -0 sha256sum
}

first_difference() {
    # $1 and $2 are sorted manifests; print the first path that differs.
    diff -U0 "$1" "$2" > "$WORK/diff.txt" || true
    awk '/^[-+]/ && !/^(---|\+\+\+)/ { print; exit }' "$WORK/diff.txt" |
        sed -e 's/^-//' -e 's/^+//' -e 's/^[0-9a-f]*  //'
}

git status --porcelain > "$WORK/status-before.txt"
manifest > "$WORK/manifest-before.txt"

echo "capture run 1"
python3 tools/capture/capture.py --legacy "$LEGACY" > "$WORK/capture-1.log" 2>&1 || {
    echo "idempotency: capture run 1 failed:" >&2
    cat "$WORK/capture-1.log" >&2
    exit 1
}
manifest > "$WORK/manifest-run-1.txt"

echo "capture run 2"
python3 tools/capture/capture.py --legacy "$LEGACY" > "$WORK/capture-2.log" 2>&1 || {
    echo "idempotency: capture run 2 failed:" >&2
    cat "$WORK/capture-2.log" >&2
    exit 1
}
manifest > "$WORK/manifest-run-2.txt"
git status --porcelain > "$WORK/status-after.txt"

HASHED=$(wc -l < "$WORK/manifest-run-2.txt" | tr -d ' ')
echo "hashed_files=$HASHED"

STATUS=0
DIFFERENT=$(first_difference "$WORK/manifest-run-1.txt" "$WORK/manifest-run-2.txt") || true
if [ -n "$DIFFERENT" ]; then
    echo "DIFFERENT fixture after a second capture: $DIFFERENT"
    STATUS=1
else
    echo "sha256_manifest_diff=empty"
fi

STATUS_DIFF=$(first_difference "$WORK/status-before.txt" "$WORK/status-after.txt") || true
if [ -n "$STATUS_DIFF" ]; then
    echo "DIFFERENT git status line: $STATUS_DIFF"
    STATUS=1
else
    echo "git_status_diff=empty"
fi

[ "$HASHED" -gt 0 ] || { echo "idempotency: hashed no files" >&2; exit 1; }
[ "$STATUS" -eq 0 ] || exit 1
echo "IDEMPOTENCY_OK files=$HASHED"
