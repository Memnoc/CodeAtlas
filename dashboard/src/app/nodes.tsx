import { Handle, type NodeProps } from "@xyflow/react";
import type { Anchor } from "./anchors.js";
import type { EntityFlowNode, RegionFlowNode } from "./graph.js";
import { ProvenanceBadge } from "./ProvenanceBadge.js";

/** The points edges land on, as the handles React Flow measures.
 *
 * One per edge rather than one per side: a card used to expose a single
 * point per side and every edge touching it arrived at that pixel. The
 * projection decides where they sit (`anchors.ts`); this only renders what it
 * decided, and it must render all of them — React Flow re-measures handles
 * from the DOM as soon as a card has a real bounding box, so a card that drew
 * fewer handles than the projection placed would put the knot straight back
 * in a browser while every test still passed. */
function Anchors({ anchors }: { anchors: readonly Anchor[] }) {
  return (
    <>
      {anchors.map((anchor) => (
        <Handle
          key={anchor.id}
          id={anchor.id}
          type={anchor.type}
          position={anchor.position}
          style={{ left: `${anchor.x}px` }}
        />
      ))}
    </>
  );
}

/** One region card on the overview: coloured spine, the complexity word, the
 * name, the description, and the file count.
 *
 * The description is badged by ITS provenance, not the layer's — the name
 * and the description are separate purchases (ticket 06), and borrowing the
 * name's provenance would badge a lie about half the card. Mechanical text —
 * published or synthesised — wears no badge, exactly as every card rendered
 * before the contract carried descriptions; the badge appears only when a
 * model wrote the sentence. It sits outside the clamped text, so a long
 * description can never clip its own label away. */
export function RegionNode({ data, selected }: NodeProps<RegionFlowNode>) {
  const { region, colorIndex, caption, anchors } = data;
  const fileCount = region.files.length;
  return (
    <div
      className={`region-card${selected === true ? " region-selected" : ""}`}
      data-testid={`region-${region.id}`}
      data-accent={colorIndex % 6}
    >
      <Anchors anchors={anchors ?? []} />
      <div className="region-top">
        <span className="region-eyebrow">Region</span>
        <span
          className="region-complexity"
          title="Relationships per file: under one is simple, under three moderate, otherwise complex."
        >
          {region.complexity}
        </span>
      </div>
      <span className="region-name">{region.name}</span>
      <span className="region-description-row">
        <span className="region-description" title={region.description}>
          {region.description}
        </span>
        {region.descriptionProvenance === "llm" && (
          <ProvenanceBadge provenance={region.descriptionProvenance} />
        )}
      </span>
      {caption !== undefined && (
        <span className="region-caption">{caption}</span>
      )}
      <span className="region-count">
        {fileCount} {fileCount === 1 ? "file" : "files"}
      </span>
    </div>
  );
}

/** One file inside a drilled-into region. */
export function EntityNode({ data }: NodeProps<EntityFlowNode>) {
  const { node, caption, highlight, onPath, neighbour, dim, anchors } = data;
  const classes = [
    "entity",
    `entity-${node.kind}`,
    highlight === undefined ? "" : `entity-${highlight}`,
    onPath === true ? "entity-on-path" : "",
    neighbour === true ? "entity-neighbour" : "",
    dim === true ? "entity-dim" : "",
  ]
    .filter((c) => c !== "")
    .join(" ");
  return (
    <div className={classes} title={node.path}>
      <Anchors anchors={anchors ?? []} />
      <span className="entity-kind">{node.kind}</span>
      <span className="entity-name">{node.name}</span>
      {caption !== undefined && (
        <span className="entity-caption">{caption}</span>
      )}
    </div>
  );
}

export const nodeTypes = {
  entity: EntityNode,
  region: RegionNode,
};
