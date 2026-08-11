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
# the byte-probe is not vacuous: the default binary must contain the strings
# that the sealed binary must lack.
set -euo pipefail

sealed="${1:?usage: sealed-probe.sh <sealed-binary> [<default-binary>]}"
control="${2:-}"

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

# --- 0. Probe control: the default binary must contain every string the
# sealed binary must lack, or the byte-scan below proves nothing.
if [ -n "$control" ]; then
  for needle in "api.anthropic.com" "claude"; do
    grep -q -a "$needle" "$control" \
      || fail "control binary $control lacks '$needle' — the byte-probe is vacuous"
  done
  echo "ok: control binary contains the API host and the CLI program name (probe is live)"
fi

# --- 1. Byte probe: the sealed binary must contain no string belonging to
# either way of reaching a model — the API destination, the HTTP client's
# name, or the program the CLI backend spawns. (A 4-byte sequence like 'ureq'
# could in principle occur by chance in unrelated binary data; see the
# limitations section of docs/SECURITY.md. It has not in practice, and the
# API host string is 17 bytes of ASCII that only the network feature emits.)
#
# `claude` is the third string and the one ADR-0008 made necessary: a
# subprocess links no crate, so `tests/sealed.rs`'s dependency-tree probe is
# blind to that backend in both directions. The program name is what is left
# to look for, and only the `agent-cli` feature emits it — `agent_cli::PROGRAM`
# and `SPEC`, plus the model help that names them. Measured on release builds
# of all three configurations: 0 occurrences sealed, 6 default, 2 with
# `agent-cli` and no `network`.
for needle in "api.anthropic.com" "ureq" "claude"; do
  if grep -q -a "$needle" "$sealed"; then
    fail "sealed binary contains '$needle'"
  fi
done
echo "ok: sealed binary contains no API host, no HTTP client and no CLI program strings"

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
# The message says the build has no enrichment backend, rather than naming
# one missing feature. Since ADR-0008 there are two features a build can lack
# and two it can have, so "compiled without the network feature" would be the
# wrong reason as often as the right one.
echo "$out" | grep -q 'no enrichment backend' \
  || fail "sealed --enrich error does not explain the build: $out"
echo "$out" | grep -q 'ADR-0006' \
  || fail "sealed --enrich error does not cite the decision behind it: $out"
[ -f "$tmp/repo/.codeatlas/knowledge-graph.json" ] \
  || fail "sealed --enrich left no structural map behind"
echo "ok: sealed --enrich refuses with the sealed message and the map survives"

# --- 2b. The CLI backend is absent behaviourally as well as in the bytes
# (ADR-0008): the sealed binary must not know `cli:claude` as a spec at all.
#
# Naming `cli:claude` here is safe only because section 1 has already run: a
# binary containing the program string never reaches this line, so a default
# binary passed by mistake as "$sealed" cannot spawn the reader's real CLI
# from inside an audit script.
#
# A fresh repository, because a map with every slot already filled reports
# "nothing to enrich" and returns before a provider is resolved at all — which
# would make both checks below pass without selecting anything.
mkdir "$tmp/repo2"
printf 'export const answer = 42;\n' > "$tmp/repo2/x.ts"

set +e
out="$("$sealed" scan --enrich --provider cli:claude "$tmp/repo2" 2>&1)"
code=$?
set -e
[ "$code" -ne 0 ] || fail "sealed 'scan --enrich --provider cli:claude' succeeded; it must refuse"
echo "$out" | grep -q 'unknown enrichment provider' \
  || fail "sealed build must not recognise cli:claude at all: $out"
echo "$out" | grep -q 'recognises none' \
  || fail "sealed cli:claude refusal does not explain the build: $out"
[ -f "$tmp/repo2/.codeatlas/knowledge-graph.json" ] \
  || fail "the cli:claude refusal left no structural map behind"
echo "ok: sealed build does not recognise cli:claude, and the map survives"

# The control for the check above, and it has to be an indirect one. Asking
# the control binary for `cli:claude` would spawn the reader's real Claude CLI
# and spend their subscription, so it asks for a `cli:` spec that no build
# accepts: a binary with the CLI backend refuses it by naming `cli:claude`
# (the "not a general run-that-program hatch" message, which is gated on the
# feature), while the sealed binary above did not know the prefix existed.
# Different answers to the same question is what makes the sealed one
# evidence of an absent backend rather than of a binary that refuses
# everything.
if [ -n "$control" ]; then
  mkdir "$tmp/repo3"
  printf 'export const answer = 42;\n' > "$tmp/repo3/x.ts"
  set +e
  out="$("$control" scan --enrich --provider cli:nonsense "$tmp/repo3" 2>&1)"
  set -e
  echo "$out" | grep -q 'the only CLI backend is' \
    || fail "control binary does not have the CLI backend — the check above is vacuous: $out"
  echo "ok: control binary has the CLI backend and refuses any other cli: spec (probe is live)"
fi

# --- 3. Provider selection tells the truth about a binary with no backend
# (ticket 29). This is the only place the "recognises none" branch can be
# exercised at all: every `cargo test` build carries `test-provider`, so
# `fake:` and `fail` are always selectable there and the list is never empty.
#
# The claude check is scoped to --provider's own paragraph on purpose. A
# page-wide search would trip over --model, which legitimately names the
# provider in order to say the build does not have it — explaining an absence
# requires naming the thing that is absent.
provider_paragraph() {
  "$1" scan --help 2>&1 | sed -n '/--provider/,/^$/p'
}

help="$("$sealed" scan --help 2>&1)"
sealed_provider="$(provider_paragraph "$sealed")"
if ! echo "$help" | grep -q -- '--provider'; then
  fail "sealed --help omits --provider; the flag must not vanish: $help"
fi
if ! echo "$sealed_provider" | grep -q 'recognises none'; then
  fail "sealed --provider does not say it has no backend: $sealed_provider"
fi
if echo "$sealed_provider" | grep -qi 'claude'; then
  fail "sealed --provider offers a backend the binary cannot select: $sealed_provider"
fi
echo "ok: sealed --provider offers no backend and says why"

if [ -n "$control" ]; then
  if ! provider_paragraph "$control" | grep -qi 'claude'; then
    fail "control --provider lacks 'claude' — the help probe above is vacuous"
  fi
  echo "ok: control binary offers the Claude backend (help probe is live)"
fi

# --- 4. `serve --ask` explains itself rather than failing obscurely (ticket
# 34, ADR-0009). Like section 3 this can only be checked here: every `cargo
# test` build carries `test-provider`, so a backend is always selectable
# there and the no-backend branch is unreachable.
#
# The sealed binary must refuse at startup — before binding, so there is no
# server left listening on a route it cannot serve.
ask_paragraph() {
  "$1" serve --help 2>&1 | sed -n '/--ask/,/^$/p'
}

sealed_ask="$(ask_paragraph "$sealed")"
if ! echo "$sealed_ask" | grep -q -- '--ask'; then
  fail "sealed 'serve --help' omits --ask; the flag must not vanish: $sealed_ask"
fi
if ! echo "$sealed_ask" | grep -q 'recognises none'; then
  fail "sealed --ask does not say it has no backend: $sealed_ask"
fi

set +e
out="$("$sealed" serve --port 0 --ask "$tmp/repo" 2>&1)"
code=$?
set -e
[ "$code" -ne 0 ] || fail "sealed 'serve --ask' started; it must refuse"
echo "$out" | grep -q 'no enrichment backend' \
  || fail "sealed 'serve --ask' does not explain the build: $out"
echo "$out" | grep -q -- '--ask' \
  || fail "sealed 'serve --ask' does not name the flag that failed: $out"
echo "ok: sealed serve --ask refuses at startup and says why"

if [ -n "$control" ]; then
  if ! ask_paragraph "$control" | grep -qi 'claude'; then
    fail "control --ask lacks 'claude' — the --ask help probe is vacuous"
  fi
  echo "ok: control binary offers a question backend (--ask probe is live)"
fi

echo "sealed probe passed"
