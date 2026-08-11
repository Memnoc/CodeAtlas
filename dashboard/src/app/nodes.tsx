import { Handle, Position, type NodeProps } from "@xyflow/react";
import type { EntityFlowNode, RegionFlowNode } from "./graph.js";

/** One region card on the overview: coloured spine, the complexity word, the
 * name, the mechanical description, and the file count. */
export function RegionNode({ data, selected }: NodeProps<RegionFlowNode>) {
  const { region, colorIndex, caption } = data;
  const fileCount = region.files.length;
  return (
    <div
      className={`region-card${selected === true ? " region-selected" : ""}`}
      data-testid={`region-${region.id}`}
      data-accent={colorIndex % 6}
    >
      <Handle type="target" position={Position.Top} />
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
      <span className="region-description">{region.description}</span>
      {caption !== undefined && (
        <span className="region-caption">{caption}</span>
      )}
      <span className="region-count">
        {fileCount} {fileCount === 1 ? "file" : "files"}
      </span>
      <Handle type="source" position={Position.Bottom} />
    </div>
  );
}

/** One file inside a drilled-into region. */
export function EntityNode({ data }: NodeProps<EntityFlowNode>) {
  const { node, caption, highlight, onPath, neighbour, dim } = data;
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
      <Handle type="target" position={Position.Top} />
      <span className="entity-kind">{node.kind}</span>
      <span className="entity-name">{node.name}</span>
      {caption !== undefined && (
        <span className="entity-caption">{caption}</span>
      )}
      <Handle type="source" position={Position.Bottom} />
    </div>
  );
}

export const nodeTypes = {
  entity: EntityNode,
  region: RegionNode,
};
