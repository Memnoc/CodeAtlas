// Loads the map from the local server (dev middleware now, the binary in
// ticket 09) — a same-origin request to /api/map, never anywhere else.
import { useEffect, useState } from "react";
import type { KnowledgeGraph } from "../index.js";
import { MapExplorer } from "./MapExplorer.js";

type LoadState =
  | { phase: "loading" }
  | { phase: "error"; message: string }
  | { phase: "ready"; map: KnowledgeGraph };

export function App() {
  const [state, setState] = useState<LoadState>({ phase: "loading" });

  useEffect(() => {
    let cancelled = false;
    fetch("/api/map")
      .then(async (res) => {
        if (!res.ok) {
          const body = (await res.json().catch(() => null)) as {
            error?: string;
          } | null;
          throw new Error(body?.error ?? `map request failed (${res.status})`);
        }
        return (await res.json()) as KnowledgeGraph;
      })
      .then((map) => {
        if (!cancelled) {
          setState({ phase: "ready", map });
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
      return <MapExplorer map={state.map} />;
  }
}
