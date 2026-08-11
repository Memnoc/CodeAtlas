// Loads the map: from the embedded share payload when this document is a
// share artifact (first-class mode — checked before any network), else from
// the local server (dev middleware or the serving binary) via same-origin
// requests to /api/map and /api/diff, never anywhere else. In share mode no
// fetch happens at all, so the artifact works from file:// where fetch is
// unusable; the diff overlay is a live-workspace feature and is absent.
import { useEffect, useState } from "react";
import type { KnowledgeGraph } from "../index.js";
import { MapExplorer } from "./MapExplorer.js";
import type { DiffOverlay } from "./overlay.js";
import { ShareBanner } from "./ShareBanner.js";
import { readSharePayload } from "./share.js";

type LoadState =
  | { phase: "loading" }
  | { phase: "error"; message: string }
  | { phase: "ready"; map: KnowledgeGraph; overlay: DiffOverlay | null };

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
    Promise.all([map, overlay])
      .then(([map, overlay]) => {
        if (!cancelled) {
          setState({ phase: "ready", map, overlay });
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
      return <MapExplorer map={state.map} overlay={state.overlay} />;
  }
}
