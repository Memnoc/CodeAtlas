#!/usr/bin/env bash
# Renders docs/RELEASE_NOTES_TEMPLATE.md into one release's notes body. Every
# {{slot}} is filled from something measured here — git history, the built
# artifacts' names and sizes, the checksums file's actual name — never typed
# by hand; that is the template's own contract. Template-facing HTML comments
# do not ship in the rendered body.
#
# Usage: release-notes.sh <tag> <artifacts-dir> <checksums-file-name>
# <checksums-file-name> is expected inside <artifacts-dir>. Output on stdout.
# Run from the repository root (it reads docs/ and the git history).
set -euo pipefail

tag="${1:?usage: release-notes.sh <tag> <artifacts-dir> <checksums-file-name>}"
dist="${2:?usage: release-notes.sh <tag> <artifacts-dir> <checksums-file-name>}"
checksums="${3:?usage: release-notes.sh <tag> <artifacts-dir> <checksums-file-name>}"
template="docs/RELEASE_NOTES_TEMPLATE.md"

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

[ -f "$template" ] || fail "no template at $template (run from the repo root)"
[ -f "$dist/$checksums" ] || fail "no checksums file at $dist/$checksums"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# {{highlights}}: commit subjects since the previous v* tag — measured
# history, not a hand-written changelog. A first release (no previous tag)
# carries its whole history: everything the project is stands behind it.
prev="$(git describe --tags --abbrev=0 --match 'v*' HEAD^ 2>/dev/null || true)"
if [ -n "$prev" ]; then
  git log --format='- %s' "$prev..HEAD" > "$work/highlights"
else
  git log --format='- %s' > "$work/highlights"
fi
[ -s "$work/highlights" ] || fail "no commits found for the highlights slot"

# {{artifact-list}}: one row per file actually in the artifacts directory,
# each with its measured byte size — the checksums file rides beside the
# binaries, exactly as it does on the release page.
(
  cd "$dist"
  for f in codeatlas-*; do
    [ -f "$f" ] || fail "artifact glob matched nothing in $dist"
    printf -- '- `%s` — %s bytes\n' "$f" "$(wc -c < "$f" | tr -d ' [:space:]')"
  done
) > "$work/artifacts"

# {{checksums-verify-command}}: both stanzas a downloader might have.
{
  printf 'sha256sum --check --ignore-missing %s       # Linux\n' "$checksums"
  printf 'shasum -a 256 --check --ignore-missing %s   # macOS\n' "$checksums"
} > "$work/cksum-cmd"

# {{attestation-verify-command}}: <target> is the same placeholder the
# README's download command uses — any artifact name from the list above.
printf 'gh attestation verify codeatlas-%s-<target> --repo Memnoc/CodeAtlas\n' \
  "$tag" > "$work/attest-cmd"

rendered="$(awk \
  -v HL="$work/highlights" \
  -v AL="$work/artifacts" \
  -v CV="$work/cksum-cmd" \
  -v AV="$work/attest-cmd" \
  -v CK="$checksums" '
  /^<!--/ { skip = 1 }
  skip { if (/-->[[:space:]]*$/) skip = 0; next }
  $0 == "{{highlights}}" { while ((getline l < HL) > 0) print l; next }
  $0 == "{{artifact-list}}" { while ((getline l < AL) > 0) print l; next }
  $0 == "{{checksums-verify-command}}" { while ((getline l < CV) > 0) print l; next }
  $0 == "{{attestation-verify-command}}" { while ((getline l < AV) > 0) print l; next }
  { gsub(/\{\{checksums-file\}\}/, CK); print }
' "$template")"

# A slot this script does not know about must fail the render, not ship as
# literal braces in a release body.
if printf '%s\n' "$rendered" | grep -q '{{'; then
  fail "unfilled slot survived the render: $(printf '%s\n' "$rendered" | grep '{{' | head -1)"
fi

# cat -s: a removed comment block must not leave a double blank line behind.
printf '%s\n' "$rendered" | cat -s
