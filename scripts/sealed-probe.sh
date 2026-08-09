#!/usr/bin/env bash
# Probes a genuinely sealed codeatlas binary (built with
# `cargo build --no-default-features`, no dev-dependencies) — the artifact a
# security auditor would ship (ticket 15, ADR-0006). This lives in CI rather
# than in `cargo test` because every test build carries the `test-provider`
# feature via the self dev-dependency, so no locally built test binary is the
# sealed artifact, and building one inside a test would double local test
# time. docs/SECURITY.md documents this split.
#
# Usage: sealed-probe.sh <sealed-binary> [<default-binary>]
#
# With a second argument (a DEFAULT-features binary), the script first proves
# the byte-probe is not vacuous: the default binary must contain the API
# host string that the sealed binary must lack.
set -euo pipefail

sealed="${1:?usage: sealed-probe.sh <sealed-binary> [<default-binary>]}"
control="${2:-}"

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

# --- 0. Probe control: the default binary must contain the host string, or
# the byte-scan below proves nothing.
if [ -n "$control" ]; then
  grep -q -a "api.anthropic.com" "$control" \
    || fail "control binary $control lacks 'api.anthropic.com' — the byte-probe is vacuous"
  echo "ok: control binary contains the API host (probe is live)"
fi

# --- 1. Byte probe: the sealed binary must contain neither the only egress
# destination nor the HTTP client's name. (A 4-byte sequence like 'ureq'
# could in principle occur by chance in unrelated binary data; see the
# limitations section of docs/SECURITY.md. It has not in practice, and the
# API host string is 17 bytes of ASCII that only the network feature emits.)
if grep -q -a "api.anthropic.com" "$sealed"; then
  fail "sealed binary contains 'api.anthropic.com'"
fi
if grep -q -a "ureq" "$sealed"; then
  fail "sealed binary contains 'ureq'"
fi
echo "ok: sealed binary contains no API host and no HTTP client strings"

# --- 2. The sealed binary works, and --enrich fails closed with the sealed
# build's own message while still writing the structural map (story 14).
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
mkdir "$tmp/repo"
printf 'export const answer = 42;\n' > "$tmp/repo/x.ts"

set +e
out="$("$sealed" scan --enrich "$tmp/repo" 2>&1)"
code=$?
set -e
[ "$code" -ne 0 ] || fail "sealed 'scan --enrich' succeeded; it must refuse"
echo "$out" | grep -q 'compiled without' \
  || fail "sealed --enrich error does not explain the build: $out"
echo "$out" | grep -q 'network' \
  || fail "sealed --enrich error does not name the missing feature: $out"
[ -f "$tmp/repo/.codeatlas/knowledge-graph.json" ] \
  || fail "sealed --enrich left no structural map behind"
echo "ok: sealed --enrich refuses with the sealed message and the map survives"

echo "sealed probe passed"
