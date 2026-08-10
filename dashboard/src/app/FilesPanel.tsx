// The right panel's FILES tab: every file in the map, under its region, with
// the symbols each one contains. The canvas answers "how does this fit
// together"; this answers "where is the thing I already know the name of".
import { useMemo, useState } from "react";
import type { KnowledgeGraph, Node as MapNode } from "../index.js";
import type { Region } from "./regions.js";

export function FilesPanel({
  map,
  regions,
  onSelectNode,
}: {
  map: KnowledgeGraph;
  regions: readonly Region[];
  onSelectNode: (id: string) => void;
}) {
  const [openId, setOpenId] = useState<string | null>(null);

  // Symbols under each file, through the containment edges that put them
  // there — the map's own answer, rather than a second guess from paths.
  const symbols = useMemo(() => {
    const byId = new Map(map.nodes.map((n) => [n.id, n]));
    const under = new Map<string, MapNode[]>();
    for (const edge of map.edges) {
      if (edge.kind !== "contains") {
        continue;
      }
      const child = byId.get(edge.target);
      if (child === undefined) {
        continue;
      }
      const bucket = under.get(edge.source);
      if (bucket === undefined) {
        under.set(edge.source, [child]);
      } else {
        bucket.push(child);
      }
    }
    return under;
  }, [map]);

  return (
    <div className="files-panel">
      {regions.map((region) => (
        <section
          key={region.id}
          className="files-region"
          aria-label={`Files in ${region.name}`}
        >
          <h3>
            {region.name}
            <span className="files-region-count">{region.files.length}</span>
          </h3>
          {region.files.length === 0 ? (
            <p className="info-empty">No files.</p>
          ) : (
            <ul className="file-list">
              {region.files.map((file) => {
                const contained = symbols.get(file.id) ?? [];
                const open = openId === file.id;
                return (
                  <li key={file.id}>
                    <div className="file-row">
                      <button
                        type="button"
                        className="file-name"
                        onClick={() => onSelectNode(file.id)}
                      >
                        {file.path}
                      </button>
                      {contained.length > 0 && (
                        <button
                          type="button"
                          className="file-expand"
                          aria-expanded={open}
                          aria-label={`${open ? "Hide" : "Show"} the ${contained.length} symbols in ${file.path}`}
                          onClick={() => setOpenId(open ? null : file.id)}
                        >
                          {contained.length}
                        </button>
                      )}
                    </div>
                    {open && (
                      <ul className="symbol-list">
                        {contained.map((symbol) => (
                          <li key={symbol.id}>
                            <button
                              type="button"
                              onClick={() => onSelectNode(symbol.id)}
                            >
                              <span className={`symbol-kind kind-${symbol.kind}`}>
                                {symbol.kind}
                              </span>
                              {symbol.name}
                            </button>
                          </li>
                        ))}
                      </ul>
                    )}
                  </li>
                );
              })}
            </ul>
          )}
        </section>
      ))}
    </div>
  );
}
