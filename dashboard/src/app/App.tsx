// Loads the map — and, when present, the diff overlay — from the local
// server (dev middleware or the serving binary): same-origin requests to
// /api/map and /api/diff, never anywhere else.
import { useEffect, useState } from "react";
import type { KnowledgeGraph } from "../index.js";
import { MapExplorer } from "./MapExplorer.js";
import type { DiffOverlay } from "./overlay.js";

type LoadState =
  | { phase: "loading" }
  | { phase: "error"; message: string }
  | { phase: "ready"; map: KnowledgeGraph; overlay: DiffOverlay | null };

export function App() {
  const [state, setState] = useState<LoadState>({ phase: "loading" });

  useEffect(() => {
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
  }, []);

  switch (state.phase) {
    case "loading":
      return <div className="loading">Loading map…</div>;
    case "error":
      return <div className="load-error">{state.message}</div>;
    case "ready":
      return <MapExplorer map={state.map} overlay={state.overlay} />;
  }
}
