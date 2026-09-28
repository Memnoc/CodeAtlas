#!/usr/bin/env bash
# mac-walk.sh — prep + test walk for the v0.1.8 launcher on macOS.
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

TAG="v0.1.8"

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

THE WALK — v0.1.8 on real macOS: the 2026-09-28 findings, dry run, the new look
  0. in ~/Downloads the frame's right border must be straight on every
     row, the Übersicht.app/ one included (v0.1.7 bent it there)
  1. run:   ~/Downloads/codeatlas
  2. a centred menu appears, listing the directories where you are:
       Enter OPENS a folder (like a file manager) · h climbs
       Enter on ". (map this directory)" maps where you stand
       / types a path · o toggles open code · q quits
     walk into a repo (e.g. ~/Code/labotteghina-art), press Enter on ".",
     the frame now has REPOSITORY / OPTIONS / KEYS headings, each
     option row leads with its key in the accent colour, and a lit
     [x] is accented while an unlit [ ] is dim — say if any of that
     reads worse than the plain frame did.
     Press e then d: the enrich row and the confirm line should read
     ENRICH DRY RUN; Enter -> expect one "would enrich: N slots in M
     calls: roughly …" line, nothing bought, the structural map served.
     Then run again: o, e and a on (d off), Enter to go
  3. then hands off, and watch for, in order:
       - the terminal restored cleanly (no raw-mode debris, no lost prompt)
       - "scanning: N/N files" standing above "mapped N files"
       - "enriching: N slots in M calls" then one "batch i/M" line per
         batch and "enriched N slots" — this is the USER fix: on
         v0.1.5 every cli:claude call on a Mac died as "not logged in"
       - the browser opening itself at http://127.0.0.1:4173/
       - the search field has an Ask button; one question comes back
         citing node IDs
       - open a .css file: coloured, and the pill names CSS
       - in magnify with BOTH side panels open: the trail may take a
         second row but no crumb breaks mid-name and the file count is
         whole; the source head's path keeps whole segments and the
         language pill sits inside the panel, on its own row if it must
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
  4b. with the first server STILL RUNNING in another terminal, run it
     again and choose any repo -> expect "port 4173 ... already in use"
     before any scanning line, no browser, exit
  5. run it once more; press Enter immediately -> maps the directory
     you are standing in (the pinned ". (this directory)" row)
  6. press q on a fresh run -> clean exit, "no repository chosen"

REPORT — the friction log is the point
  - did the keys feel right? anything the footer did not explain?
  - anything that made you pause, however small
WALK
