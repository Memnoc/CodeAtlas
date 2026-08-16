// Loads the map: from the embedded share payload when this document is a
// share artifact (first-class mode — checked before any network), else from
// the local server (dev middleware or the serving binary) via same-origin
// requests to the wire module's routes, never anywhere else. In share mode
// no fetch happens at all, so the artifact works from file:// where fetch is
// unusable; the diff overlay is a live-workspace feature and is absent, and
// so are questions (ADR-0009 — there is no server to ask) and source
// (ADR-0013 — a share recipient is precisely someone who does not hold the
// code).
//
// Everything that fetches lives in `wire.ts`, and this component reaches it
// only in the served branch, only by dynamic import (ticket 04). That keeps
// two promises structural rather than remembered: the explorer is handed
// fetching functions or nothing, so share mode cannot grow an affordance by
// accident — and the share artifact, which inlines exactly the chunks
// `index.html` references, never carries the wire chunk, so it does not
// even contain the routes' names. `tests/share.rs` byte-scans the artifact
// for every serve route to hold that true.
import { useEffect, useState } from "react";
import type { KnowledgeGraph } from "../index.js";
import type { AskFn } from "./ask.js";
import { MapExplorer } from "./MapExplorer.js";
import type { OpenSourceFn } from "./source.js";
import type { DiffOverlay } from "./overlay.js";
import { ShareBanner } from "./ShareBanner.js";
import { readSharePayload } from "./share.js";

type LoadState =
  | { phase: "loading" }
  | { phase: "error"; message: string }
  | {
      phase: "ready";
      map: KnowledgeGraph;
      overlay: DiffOverlay | null;
      // The fetching functions the capabilities route earned, absent when
      // it did not: under `exactOptionalPropertyTypes` an absent field is
      // what "this server cannot" has to be, and the render below hands
      // each to the explorer exactly as present as it is here.
      ask?: AskFn;
      openSource?: OpenSourceFn;
    };

export function App() {
  // Read once, synchronously: the payload is static document content.
  const [share] = useState(readSharePayload);
  const [state, setState] = useState<LoadState>({ phase: "loading" });

  useEffect(() => {
    if (share !== null) {
      return;
    }
    let cancelled = false;
    // The one place the wire module is reached: the served branch, by
    // dynamic import (see the module comment for why that placement is a
    // trust boundary, not a performance knob).
    import("./wire.js")
      .then((wire) =>
        Promise.all([
          wire.fetchMap(),
          wire.fetchOverlay(),
          // What this particular server process can do — which is not a
          // property of the map, so it is not in the map (story 16's schema
          // is a contract for external producers). Asked once, at load,
          // alongside the map rather than by probing the question route
          // later.
          wire.readCapabilities(),
        ]).then(([map, overlay, capabilities]) => {
          if (!cancelled) {
            setState({
              phase: "ready",
              map,
              overlay,
              // Spread-in rather than assigned: an explicit `undefined` is
              // not an absent optional field, and absent is what "this
              // server cannot answer questions" has to be.
              ...(capabilities.ask ? { ask: wire.askServer } : {}),
              // Same rule for source (ticket 02, ADR-0013): the function is
              // kept only when the binary said `--open-code` was given, so
              // everywhere else the affordance is absent rather than broken.
              ...(capabilities.open_code
                ? { openSource: wire.fetchSource }
                : {}),
            });
          }
        }),
      )
      .catch((err: unknown) => {
        if (!cancelled) {
          setState({ phase: "error", message: String(err) });
        }
      });
    return () => {
      cancelled = true;
    };
  }, [share]);

  if (share !== null) {
    return (
      <div className="share-shell">
        <ShareBanner redaction={share.redaction} />
        <MapExplorer map={share.map} overlay={null} shared />
      </div>
    );
  }

  switch (state.phase) {
    case "loading":
      return <div className="loading">Loading map…</div>;
    case "error":
      return <div className="load-error">{state.message}</div>;
    case "ready":
      return (
        <MapExplorer
          map={state.map}
          overlay={state.overlay}
          {...(state.ask ? { onAsk: state.ask } : {})}
          {...(state.openSource ? { onOpenSource: state.openSource } : {})}
        />
      );
  }
}
