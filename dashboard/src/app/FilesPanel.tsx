// The right panel's FILES tab: every file in the map, under its region, with
// the symbols each one contains. The canvas answers "how does this fit
// together"; this answers "where is the thing I already know the name of".
//
// Which is why it folds and filters. A flat list of every file stops being a
// way to find anything somewhere around a hundred entries: this repository has
// 285 files and one region holding 45 of them, so opening the tab used to push
// the other seven regions off the bottom of the panel before the reader had
// chosen anything. Groups start folded, so the tab opens on the shape of the
// repository — eight headings and their counts — and the reader expands the
// one they mean.
//
// The filter is a filter of *this list*, deliberately not a second search. The
// field in the header searches every node in the map, by name, path or
// summary, and answers with somewhere to go; this narrows what is already on
// screen and never leaves it. Matching only on path is what keeps the two
// distinguishable — a box here that also found symbols by name would be the
// header's search box in a worse position.
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
  const [filter, setFilter] = useState("");
  // Expanded rather than collapsed, so the default is folded and a region that
  // appears later — switching Domain/Structural rebuilds them all — is folded
  // too, without anything having to notice it arrived.
  const [expanded, setExpanded] = useState<ReadonlySet<string>>(new Set());

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

  const query = filter.trim().toLowerCase();
  // Only regions with something to show survive a filter. Leaving an empty
  // heading behind for each of the other seven is how a filter that works
  // still looks like one that does not.
  const shown = regions
    .map((region) => ({
      region,
      files:
        query === ""
          ? region.files
          : region.files.filter((f) => f.path.toLowerCase().includes(query)),
    }))
    .filter(({ files }) => query === "" || files.length > 0);

  return (
    <div className="files-panel">
      <input
        type="search"
        className="files-filter"
        aria-label="Filter files"
        placeholder="Filter these files by path…"
        value={filter}
        onChange={(e) => setFilter(e.target.value)}
      />

      {query !== "" && shown.length === 0 && (
        <p className="info-empty">No files match.</p>
      )}

      {shown.map(({ region, files }) => {
        // A filter opens what it matched. Making the reader expand a group
        // that is on screen *because* it matched would be asking them to
        // confirm the answer they just asked for.
        const regionOpen = query !== "" || expanded.has(region.id);
        return (
          <section
            key={region.id}
            className="files-region"
            aria-label={`Files in ${region.name}`}
          >
            <h3>
              <button
                type="button"
                className="files-region-toggle"
                aria-expanded={regionOpen}
                onClick={() =>
                  setExpanded((was) => {
                    const next = new Set(was);
                    if (!next.delete(region.id)) {
                      next.add(region.id);
                    }
                    return next;
                  })
                }
              >
                <span className="files-region-chevron" aria-hidden="true">
                  {regionOpen ? "▾" : "▸"}
                </span>
                {region.name}
                <span className="files-region-count">
                  {query === ""
                    ? region.files.length
                    : `${files.length} of ${region.files.length}`}
                </span>
              </button>
            </h3>
            {!regionOpen ? null : files.length === 0 ? (
              <p className="info-empty">No files.</p>
            ) : (
              <ul className="file-list">
                {files.map((file) => {
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
                                <span
                                  className={`symbol-kind kind-${symbol.kind}`}
                                >
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
        );
      })}
    </div>
  );
}
