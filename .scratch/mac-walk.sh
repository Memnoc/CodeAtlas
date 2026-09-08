#!/usr/bin/env bash
# mac-walk.sh — prep + test walk for the v0.1.3 launcher on macOS.
#
# On the Mac:
#     cd ~/Code/CodeAtlas && git pull && bash .scratch/mac-walk.sh
#
# The script does the whole prep: detects the arch, downloads the release
# binary with curl -f (a 404 fails loudly instead of saving an error
# page), verifies the checksum as a hard gate, and leaves a verified
# executable at ~/Downloads/codeatlas. The walk itself stays in your
# hands — your hands are the test — and the checklist prints at the end.
set -euo pipefail

TAG="v0.1.3"

case "$(uname -m)" in
  arm64)  TARGET=aarch64-apple-darwin ;;
  x86_64) TARGET=x86_64-apple-darwin ;;
  *) echo "unsupported arch: $(uname -m)"; exit 1 ;;
esac

BIN="codeatlas-$TAG-$TARGET"
cd ~/Downloads
echo "downloading $BIN ..."
curl -fLO "https://github.com/Memnoc/CodeAtlas/releases/download/$TAG/$BIN"
curl -fLO "https://github.com/Memnoc/CodeAtlas/releases/download/$TAG/codeatlas-$TAG-checksums.txt"
shasum -a 256 --check --ignore-missing "codeatlas-$TAG-checksums.txt"
chmod +x "$BIN"
mv -f "$BIN" codeatlas

cat <<'WALK'

prep done — verified binary at ~/Downloads/codeatlas
(a curl download carries no quarantine mark: Gatekeeper will not block it)

THE WALK — the launcher's first human run on real macOS
  1. run:   ~/Downloads/codeatlas
  2. repository path [.]:   type a repo path (~ works), e.g. ~/Code/omarchy-site
  3. show mapped files' source? [y/N]:   answer y
  4. then hands off, and watch for, in order:
       - "scanning: N/N files" standing above "mapped N files"
       - the browser OPENING ITSELF at http://127.0.0.1:4173/
         (the `open` spawn: unit-tested, never human-watched on a Mac)
       - in the dashboard: select a symbol -> Open code -> source lit
         at its own lines
       - Ctrl-C stops the server

EDGE POKES (30 seconds)
  5. run it again; type a garbage path -> expect
     "no directory at ... — try again", and a real path still lands after
  6. run it once more; press Enter at the path -> maps the directory
     you are standing in

REPORT — the friction log is the point
  - did the browser open by itself? roughly how fast?
  - anything that made you pause, however small
WALK
