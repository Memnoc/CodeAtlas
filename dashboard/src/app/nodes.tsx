import { Handle, Position, type NodeProps } from "@xyflow/react";
import type { EntityFlowNode, LayerFlowNode } from "./graph.js";

export function EntityNode({ data }: NodeProps<EntityFlowNode>) {
  const { node } = data;
  return (
    <div className={`entity entity-${node.kind}`} title={node.path}>
      <Handle type="target" position={Position.Top} />
      <span className="entity-kind">{node.kind}</span>
      <span className="entity-name">{node.name}</span>
      <Handle type="source" position={Position.Bottom} />
    </div>
  );
}

export function LayerGroupNode({ data }: NodeProps<LayerFlowNode>) {
  return (
    <div
      className="layer-group"
      data-testid={`layer-group-${data.layerId}`}
    >
      <div className="layer-label">{data.label}</div>
    </div>
  );
}

export const nodeTypes = {
  entity: EntityNode,
  layerGroup: LayerGroupNode,
};
