#!/usr/bin/env bash
# Smoke-runs one built release artifact on the runner that built it (spec
# seam 6, ADR-0014): scan a three-file fixture tree and require the exact
# mapped-files line the CLI prints (`mapped {files} files`, src/lib.rs). A
# binary that starts but cannot walk a tree, or a build that silently
# changed the one number the CLI states to a reader, fails here rather than
# in a reader's terminal after download.
#
# Usage: release-smoke.sh <binary>
#
# RELEASE_SMOKE_EXPECT overrides the expected line — the tamper lever the
# release workflow's `tamper_smoke` dispatch input pulls to prove this
# assertion can fail (the repository's proven-able-to-fail rule). Unset or
# empty means the real expectation.
set -euo pipefail

bin="${1:?usage: release-smoke.sh <binary>}"
expect="${RELEASE_SMOKE_EXPECT:-mapped 3 files}"

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# Three files, three languages: every regular file becomes a File node
# (scan.rs walks everything and labels unparsed languages Plain), so the
# expected count is exactly the fixture's file count.
mkdir "$tmp/repo"
printf 'export const answer = 42;\n' > "$tmp/repo/a.ts"
printf 'def main():\n    pass\n' > "$tmp/repo/b.py"
printf 'fn main() {}\n' > "$tmp/repo/c.rs"

set +e
out="$("$bin" scan "$tmp/repo" 2>&1)"
code=$?
set -e
[ "$code" -eq 0 ] || fail "scan exited $code: $out"

# -Fx: the whole line, verbatim — a count drift or a reworded line fails.
echo "$out" | grep -Fxq "$expect" \
  || fail "expected the line '$expect' from a three-file fixture, got: $out"
[ -f "$tmp/repo/.codeatlas/knowledge-graph.json" ] \
  || fail "scan reported success but left no map behind"

echo "ok: $(basename "$bin") scanned the fixture and said '$expect'"
