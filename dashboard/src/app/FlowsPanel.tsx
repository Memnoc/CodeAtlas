// Domain flows (spec story 6): the call chains the CLI projects from the
// graph, grouped by the domain they start in. A real repository has one flow
// per entry point — CodeAtlas's own map has ~140 — so the panel opens as an
// index of domains: expand a domain to see its flows, open a flow to walk its
// chain. Opening a flow lands the newcomer on its entry point, and every step
// is a node the canvas can select.
import { useMemo, useState } from "react";
import type { DomainFlow, KnowledgeGraph } from "../index.js";
import { nodesById } from "./graph.js";
import { ProvenanceBadge } from "./ProvenanceBadge.js";

/** Flows bucketed by domain, in the order the domains first appear in the
 * map — the contract emits flows deterministically, so this is too. */
function byDomain(
  flows: DomainFlow[],
): { domain: string; flows: DomainFlow[] }[] {
  const groups = new Map<string, DomainFlow[]>();
  for (const flow of flows) {
    const group = groups.get(flow.domain);
    if (group === undefined) {
      groups.set(flow.domain, [flow]);
    } else {
      group.push(flow);
    }
  }
  return [...groups].map(([domain, flows]) => ({ domain, flows }));
}

export function FlowsPanel({
  map,
  onSelect,
}: {
  map: KnowledgeGraph;
  /** Selects a node on the canvas. */
  onSelect: (id: string) => void;
}) {
  const byId = useMemo(() => nodesById(map), [map]);
  // `domain_flows` is optional in the contract; a map without call chains
  // carries none.
  const domains = useMemo(() => byDomain(map.domain_flows ?? []), [map]);
  const [openDomain, setOpenDomain] = useState<string | null>(null);
  const [openFlow, setOpenFlow] = useState<string | null>(null);

  if (domains.length === 0) {
    return null;
  }

  return (
    <section className="flows" aria-label="Domain flows">
      <h2>Domain flows</h2>
      {domains.map(({ domain, flows }) => (
        <div key={domain} className="flow-domain">
          <h3>
            <button
              type="button"
              aria-expanded={openDomain === domain}
              onClick={() =>
                setOpenDomain(openDomain === domain ? null : domain)
              }
            >
              {domain} <span className="flow-count">{flows.length}</span>
            </button>
          </h3>
          {openDomain === domain && (
            <ul className="flow-list">
              {flows.map((flow) => {
                // Only steps present in this map are offered: the point of a
                // step is that it can be pointed at on the canvas.
                const steps = flow.steps.filter((id) => byId.has(id));
                const open = openFlow === flow.id;
                return (
                  <li key={flow.id}>
                    <button
                      type="button"
                      className="flow-name"
                      aria-expanded={open}
                      onClick={() => {
                        if (open) {
                          setOpenFlow(null);
                          return;
                        }
                        setOpenFlow(flow.id);
                        const entry = steps[0];
                        if (entry !== undefined) {
                          onSelect(entry);
                        }
                      }}
                    >
                      {flow.name}{" "}
                      <ProvenanceBadge provenance={flow.provenance} />
                    </button>
                    {open && (
                      <ol
                        className="flow-steps"
                        aria-label={`Steps of ${flow.name}`}
                      >
                        {steps.map((id) => (
                          <li key={id}>
                            <button type="button" onClick={() => onSelect(id)}>
                              {byId.get(id)?.name ?? id}
                            </button>
                          </li>
                        ))}
                      </ol>
                    )}
                  </li>
                );
              })}
            </ul>
          )}
        </div>
      ))}
    </section>
  );
}
