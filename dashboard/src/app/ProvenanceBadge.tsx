// Every enrichable slot the dashboard shows — a node summary, a layer name,
// a flow name, a tour narration — says which kind of prose it is. One badge,
// used everywhere, so `structural` and `llm` never look different depending
// on where they are read.
import type { Provenance } from "../index.js";

export function ProvenanceBadge({ provenance }: { provenance: Provenance }) {
  return (
    <span className={`badge badge-${provenance}`} data-testid="provenance-badge">
      {provenance}
    </span>
  );
}
