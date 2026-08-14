# README head recording — shot script

Disposable working material for the screen capture that fills the README's
head slot (the `<!-- Head slot reserved for the screen recording. -->`
comment). Delete this file once the recording is committed.

One continuous take, ~75–90 seconds, on CodeAtlas's own repository — the
committed annotation store means full prose on camera with no credential.

## Setup, before recording

- `cargo build --release` so every V2 feature is in the binary.
- Pick one theme and stick with it — dark reads best against GitHub.
- Terminal and browser side by side, other tabs closed.
- Rehearse the two Ask questions once (they spend real subscription
  tokens through `cli:claude` — rehearsal beats retakes).

## The take

1. **The one command** (~8s) — terminal: `cas`. Let the "mapped N files"
   line land and sit for a beat. This is the whole pitch: one command,
   no key.
2. **Serve** (~5s) — `caq`, click the `http://127.0.0.1:4173/` URL.
   The map appears: regions, edges, elevation.
3. **The overview** (~8s) — touch nothing for two seconds; let the whole
   map read. Then hover one region card so its description shows —
   enriched prose, no model running.
4. **The drill** (~12s) — click into `crates`, the dense one. It opens on
   the files that matter, readable. Point at the "show all" affordance,
   click it, let the wall appear — then put it back. Story 1 in four
   seconds.
5. **Magnify** (~12s) — select a well-connected file (`serve.rs` or
   `MapExplorer.tsx`), hit the breadcrumb's **Magnify** button. The
   neighbourhood draws alone. Hover a neighbour, then leave — landing
   back exactly where you were. A lens, not a place.
6. **Two readings** (~8s) — flip the grouping toggle Layer → Domain,
   pause, flip back. One toggle, same repository.
7. **The conversation** (~18s) — ask something real: *"where does a scan
   write its output?"* The answer lands in the column, map still on
   screen. Click a citation — the card lights. Then the money shot:
   follow up with *"what reads it?"* — "it" understood, the thread grows,
   token counts and the running total visible under each turn.
8. **Close** (~8s) — zoom back out to the whole map and hold two seconds.
   End on the picture, not a panel.

## After

- `cakill`.
- GitHub autoplays an `.mp4`/`.gif` dropped straight into the head slot
  where the comment marks it.
