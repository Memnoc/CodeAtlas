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

TAG="v0.1.4"

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

THE WALK — the modal's first human run on real macOS
  1. run:   ~/Downloads/codeatlas
  2. a bordered menu appears, listing the directories where you are:
       j/k move · Enter choose · l/h descend and climb · / type a path
       o toggles "open code in dashboard" · q quits
     navigate to a repo (e.g. ~/Code/omarchy-site), press o, then Enter
  3. then hands off, and watch for, in order:
       - the terminal restored cleanly (no raw-mode debris, no lost prompt)
       - "scanning: N/N files" standing above "mapped N files"
       - the browser opening itself at http://127.0.0.1:4173/
       - in the dashboard: select a symbol -> Open code -> source lit
         at its own lines, and the language pill on a non-highlighted
         file now says "plain text — no grammar shipped for this file"
       - in magnify with both side panels open: the "Back to assets"
         button stays on ONE line and the caption truncates instead
         (the fix for your last report's layout finding)
       - Ctrl-C stops the server

EDGE POKES (30 seconds)
  4. run it again; press / and type a garbage path -> expect
     "no directory at ..." inside the frame, and a real path still
     lands after
  5. run it once more; press Enter immediately -> maps the directory
     you are standing in (the pinned ". (this directory)" row)
  6. press q on a fresh run -> clean exit, "no repository chosen"

REPORT — the friction log is the point
  - did the keys feel right? anything the footer did not explain?
  - anything that made you pause, however small
WALK
