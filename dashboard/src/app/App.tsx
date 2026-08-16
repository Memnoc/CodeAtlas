// Loads the map: from the embedded share payload when this document is a
// share artifact (first-class mode — checked before any network), else from
// the local server (dev middleware or the serving binary) via same-origin
// requests to /api/map, /api/diff and /api/capabilities, never anywhere else.
// In share mode no fetch happens at all, so the artifact works from file://
// where fetch is unusable; the diff overlay is a live-workspace feature and
// is absent, and so are questions (ADR-0009 — there is no server to ask).
//
// This is also the only component that fetches, which is what keeps that
// promise structural rather than remembered: the explorer is handed an asking
// function, and in share mode it is handed none.
import { useEffect, useState } from "react";
import type { KnowledgeGraph } from "../index.js";
import { askServer, type Capabilities, readCapabilities } from "./ask.js";
import { MapExplorer } from "./MapExplorer.js";
import { fetchSource } from "./source.js";
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
      capabilities: Capabilities;
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
    const map = fetch("/api/map").then(async (res) => {
      if (!res.ok) {
        const body = (await res.json().catch(() => null)) as {
          error?: string;
        } | null;
        throw new Error(body?.error ?? `map request failed (${res.status})`);
      }
      return (await res.json()) as KnowledgeGraph;
    });
    // The overlay is optional: 404 (no `codeatlas diff` run) or any other
    // failure simply means no toggle — it never blocks the map.
    const overlay = fetch("/api/diff")
      .then(async (res) =>
        res.ok ? ((await res.json()) as DiffOverlay) : null,
      )
      .catch(() => null);
    // What this particular server process can do — which is not a property
    // of the map, so it is not in the map (story 16's schema is a contract
    // for external producers). Asked once, at load, alongside the map rather
    // than by probing the question route later.
    const capabilities = readCapabilities();
    Promise.all([map, overlay, capabilities])
      .then(([map, overlay, capabilities]) => {
        if (!cancelled) {
          setState({ phase: "ready", map, overlay, capabilities });
        }
      })
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
          // Spread-in rather than assigned: under
          // `exactOptionalPropertyTypes` an explicit `undefined` is not the
          // same as an absent optional field, and absent is what "this
          // server cannot answer questions" has to be.
          {...(state.capabilities.ask ? { onAsk: askServer } : {})}
          // Same rule for source (ticket 02, ADR-0013): the affordance is
          // handed in only when the binary said `--open-code` was given, so
          // everywhere else it is absent rather than broken.
          {...(state.capabilities.open_code
            ? { onOpenSource: fetchSource }
            : {})}
        />
      );
  }
}
