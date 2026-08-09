// The diff impact overlay `codeatlas diff` writes to
// .codeatlas/diff-overlay.json and the server exposes at /api/diff. This is
// an internal artifact, deliberately NOT part of the published map contract
// (ADR-0003), so its type is declared here beside the app rather than
// generated from the schema.
export type DiffOverlay = {
  version: number;
  /** Node IDs whose file content changed, sorted. */
  changed: string[];
  /** One-hop blast radius: neighbors of changed nodes, sorted. */
  affected: string[];
  /** Changed repo-relative paths the map has no node for, sorted. */
  unmapped_paths: string[];
};
