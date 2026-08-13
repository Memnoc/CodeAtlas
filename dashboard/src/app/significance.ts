// One ordering rule for the map's published significance, shared by every
// consumer that ranks or cuts on it.
//
// ADR-0010 publishes the number once so the tour, the default drill view and
// the info panel's rankings cannot name different files. The *order* has to
// be published once too, or the agreement lasts exactly until the first tie:
// each of those consumers takes a top-N — twelve stops, forty cards, six rows
// — and at the cut, a tie broken two ways is two different answers. So the
// tie-break here is the producer's, `a.path.cmp(b.path)` in
// crates/codeatlas/src/semantics.rs, and not `localeCompare`, which collates
// (it reads past the case and puts `docs/adr/index.md` before
// `docs/README.md`, where the producer puts `README` first) and collates
// per locale, where a map is one order everywhere.
import type { Node as MapNode } from "../index.js";

/** Two paths in the producer's order: by code unit, which is its byte order
 * for every path built from code points below U+E000 — all of ASCII, and
 * everything else a repository path is made of in practice. */
export function byPath(a: string, b: string): number {
  return a < b ? -1 : a > b ? 1 : 0;
}

/** Most significant first, ties on path. A file the map scored nothing on
 * counts as zero rather than dropping out of the order: `significance` is
 * optional in the contract, and a map written before it existed must still
 * rank — every file ties, and path order decides the whole thing. */
export function bySignificance(a: MapNode, b: MapNode): number {
  return (
    (b.significance ?? 0) - (a.significance ?? 0) || byPath(a.path, b.path)
  );
}
