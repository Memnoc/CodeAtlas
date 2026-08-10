// The dashboard's seam component: give it a map conforming to the published
// contract and it renders the whole explorer — canvas, search, detail panel.
// It consumes only the generated contract types (ADR-0003) and never makes a
// network request.
import {
  ReactFlow,
  type Edge as FlowEdge,
  type ReactFlowInstance,
} from "@xyflow/react";
import { useCallback, useMemo, useRef, useState } from "react";
import type { KnowledgeGraph, Node as MapNode } from "../index.js";
import { FlowsPanel } from "./FlowsPanel.js";
import { type AppFlowNode, nodesById, searchNodes, toFlow } from "./graph.js";
import { nodeTypes } from "./nodes.js";
import type { DiffOverlay } from "./overlay.js";
import { ProvenanceBadge } from "./ProvenanceBadge.js";
import { TourPanel } from "./TourPanel.js";
import "@xyflow/react/dist/style.css";
import "./styles.css";

export function MapExplorer({
  map,
  overlay,
}: {
  map: KnowledgeGraph;
  /** Diff impact overlay, when `codeatlas diff` produced one. */
  overlay?: DiffOverlay | null;
}) {
  const { nodes, edges } = useMemo(() => toFlow(map), [map]);
  const [query, setQuery] = useState("");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [showOverlay, setShowOverlay] = useState(false);
  const canvas = useRef<ReactFlowInstance<AppFlowNode, FlowEdge> | null>(null);

  // Selecting from the sidebar — a search hit, an edge, a tour step, a flow
  // step — also brings the node into view: on a canvas of hundreds of nodes
  // a highlight nobody can see is not a selection. Clicking a node on the
  // canvas goes through plain `setSelectedId` instead: it is already on
  // screen, and moving the viewport under the pointer would be jarring.
  const reveal = useCallback((id: string) => {
    setSelectedId(id);
    void canvas.current?.fitView({
      nodes: [{ id }],
      duration: 0,
      maxZoom: 1.2,
      padding: 4,
    });
  }, []);

  // With the toggle on, entity nodes in the overlay's sets carry a
  // highlight; everything else renders exactly as without an overlay. The
  // selected node is marked on the canvas too, so the sidebar's selection —
  // a search hit, an edge, a tour step, a flow step — is visible where the
  // map is.
  const shownNodes = useMemo(() => {
    const changed = new Set(showOverlay && overlay ? overlay.changed : []);
    const affected = new Set(showOverlay && overlay ? overlay.affected : []);
    return nodes.map((node) => {
      if (node.type !== "entity") {
        return node;
      }
      const highlight = changed.has(node.id)
        ? ("changed" as const)
        : affected.has(node.id)
          ? ("affected" as const)
          : undefined;
      const selected = node.id === selectedId;
      if (highlight === undefined) {
        return selected ? { ...node, selected } : node;
      }
      return { ...node, selected, data: { ...node.data, highlight } };
    });
  }, [nodes, overlay, selectedId, showOverlay]);

  const results = useMemo(() => searchNodes(map, query), [map, query]);
  const selected = useMemo(
    () => map.nodes.find((n) => n.id === selectedId) ?? null,
    [map, selectedId],
  );

  return (
    <div className="explorer">
      <aside className="sidebar">
        <header className="sidebar-header">
          <h1>{map.project.name}</h1>
          <p className="contract-version">map contract {map.version}</p>
        </header>
        <input
          type="search"
          aria-label="Search nodes"
          placeholder="Search by name or path…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        {overlay && (
          <label className="overlay-toggle">
            <input
              type="checkbox"
              aria-label="Diff overlay"
              checked={showOverlay}
              onChange={(e) => setShowOverlay(e.target.checked)}
            />
            Diff overlay
            <span className="overlay-counts">
              {overlay.changed.length} changed · {overlay.affected.length}{" "}
              affected
            </span>
          </label>
        )}
        {query.trim() !== "" && (
          <ul className="search-results" aria-label="Search results">
            {results.map((n) => (
              <li key={n.id}>
                <button type="button" onClick={() => reveal(n.id)}>
                  <span className="result-name">{n.name}</span>
                  <span className="result-path">{n.path}</span>
                </button>
              </li>
            ))}
            {results.length === 0 && <li className="no-matches">No matches</li>}
          </ul>
        )}
        {/* Navigation above the thing being navigated to: the tour's
            controls must not slide down the sidebar as the detail panel
            below them grows and shrinks from step to step. */}
        <TourPanel map={map} onSelect={reveal} />
        <FlowsPanel map={map} onSelect={reveal} />
        {selected && (
          <DetailPanel map={map} node={selected} onSelect={reveal} />
        )}
      </aside>
      <main className="canvas">
        <ReactFlow
          nodes={shownNodes}
          edges={edges}
          nodeTypes={nodeTypes}
          onInit={(instance) => {
            canvas.current = instance;
          }}
          onNodeClick={(_event, node) => {
            if (node.type === "entity") {
              setSelectedId(node.id);
            }
          }}
          fitView
          minZoom={0.05}
          nodesConnectable={false}
        />
      </main>
    </div>
  );
}

function DetailPanel({
  map,
  node,
  onSelect,
}: {
  map: KnowledgeGraph;
  node: MapNode;
  onSelect: (id: string) => void;
}) {
  const byId = useMemo(() => nodesById(map), [map]);
  const touching = map.edges.filter(
    (e) => e.source === node.id || e.target === node.id,
  );

  return (
    <section className="detail" aria-label="Node detail">
      <h2>{node.name}</h2>
      <p className="detail-meta">
        <span className="detail-kind">{node.kind}</span>
        <ProvenanceBadge provenance={node.provenance} />
      </p>
      <p className="detail-path">{node.path}</p>
      {node.range && (
        <p className="detail-range">
          lines {node.range.start_line}–{node.range.end_line}
        </p>
      )}
      <p className="detail-summary">{node.summary}</p>
      <h3>Edges</h3>
      {touching.length === 0 ? (
        <p className="no-edges">No edges</p>
      ) : (
        <ul className="edge-list" aria-label="Edges">
          {touching.map((e) => {
            const outgoing = e.source === node.id;
            const otherId = outgoing ? e.target : e.source;
            const otherName = byId.get(otherId)?.name ?? otherId;
            return (
              <li key={`${e.source}--${e.kind}--${e.target}`}>
                <button type="button" onClick={() => onSelect(otherId)}>
                  {outgoing
                    ? `${e.kind} → ${otherName}`
                    : `← ${e.kind} ${otherName}`}
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}
