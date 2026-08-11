// The tour of the *codebase* (spec story 6): the map's ordered walk over the
// files that carry the architecture, driven one step at a time. The CLI
// decides which files are on the walk and in what order
// (crates/codeatlas/src/semantics.rs); this panel only renders that decision
// and moves the canvas selection along with it.
//
// Named for its subject, because the product has a second walk — the
// walkthrough of the *interface*, story 20 — and "tour" on its own would not
// say which one a reader had started. The two must not run at once either,
// which is why the step index below is the explorer's rather than this
// panel's: starting the walkthrough puts this walk back to its first step,
// and a panel holding its own index could not be told to.
import { useMemo } from "react";
import type { KnowledgeGraph, Node as MapNode } from "../index.js";
import { nodesById } from "./graph.js";
import { enrichmentHint, narrativeOf } from "./labels.js";
import { ProvenanceBadge } from "./ProvenanceBadge.js";

export function TourPanel({
  map,
  onSelect,
  index,
  onIndex,
}: {
  map: KnowledgeGraph;
  /** Selects a node on the canvas — the tour's whole effect. */
  onSelect: (id: string) => void;
  /** Which step the walk is standing on, or `null` before it starts. */
  index: number | null;
  onIndex: (index: number | null) => void;
}) {
  const byId = useMemo(() => nodesById(map), [map]);
  // A step naming a node this map does not contain cannot be pointed at, so
  // it is not part of the walk. `tour` is optional in the contract.
  const steps = useMemo(() => {
    const known = nodesById(map);
    return (map.tour ?? []).filter((step) => known.has(step.node));
  }, [map]);

  if (steps.length === 0) {
    return null;
  }

  const goTo = (next: number) => {
    const step = steps[next];
    if (step === undefined) {
      return;
    }
    onIndex(next);
    onSelect(step.node);
  };

  const current = index === null ? undefined : steps[index];
  if (index === null || current === undefined) {
    return (
      <section className="tour" aria-label="Codebase tour">
        <h2>Codebase tour</h2>
        <p className="tour-progress">
          {steps.length} {steps.length === 1 ? "step" : "steps"}
        </p>
        <button type="button" className="tour-start" onClick={() => goTo(0)}>
          Start tour
        </button>
      </section>
    );
  }

  return (
    <section className="tour" aria-label="Codebase tour">
      <h2>Codebase tour</h2>
      <p className="tour-progress">
        Step {index + 1} of {steps.length}
      </p>
      <p className="tour-label">
        {current.label} <ProvenanceBadge provenance={current.provenance} />
      </p>
      {/* The label is the CLI's one-line reason for the stop. On its own it
          is a topology fact — "Entry point … fan-in 0, fan-out 8" — which is
          the right fact said in the wrong language, so the walk also gets the
          plain-words account of the file it is standing on. */}
      <Narrative map={map} node={byId.get(current.node)} />
      <div className="tour-controls">
        <button
          type="button"
          disabled={index === 0}
          onClick={() => goTo(index - 1)}
        >
          Previous
        </button>
        <button
          type="button"
          disabled={index === steps.length - 1}
          onClick={() => goTo(index + 1)}
        >
          Next
        </button>
      </div>
    </section>
  );
}

/** The plain-words account of a node, wherever a reader is looking at one. */
export function Narrative({
  map,
  node,
}: {
  map: KnowledgeGraph;
  node: MapNode | undefined;
}) {
  const byId = useMemo(() => nodesById(map), [map]);
  if (node === undefined) {
    return null;
  }
  const hint = enrichmentHint(node);
  return (
    <div className="narrative">
      {narrativeOf(map, node, byId).map((line) => (
        <p key={line}>{line}</p>
      ))}
      {hint !== null && <p className="narrative-hint">{hint}</p>}
    </div>
  );
}
