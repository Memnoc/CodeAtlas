// Share mode (ticket 14): the share artifact embeds the redacted map and
// its redaction disclosure as an inline JSON script. The Rust side
// (crates/codeatlas/src/share.rs) writes this element; the app reads it
// before ever considering the network, so a double-clicked file:// artifact
// works with zero requests.
import type { KnowledgeGraph } from "../index.js";

/** Element ID of the embedded payload — must match the Rust emitter. */
export const SHARE_DATA_ID = "codeatlas-share-data";

/** What the artifact discloses about its own redaction (spec story 10). */
export type RedactionDisclosure = {
  /** The string redacted values were replaced with. */
  marker: string;
  /** Every field path the allowlist classifies as redactable. */
  policy: string[];
  /** Field paths actually redacted in this map, with counts. */
  redacted: { field: string; count: number }[];
};

export type SharePayload = {
  map: KnowledgeGraph;
  redaction: RedactionDisclosure;
};

/**
 * Reads the embedded share payload, if this document is a share artifact.
 * Returns null in the served dashboard (no such element).
 */
export function readSharePayload(): SharePayload | null {
  const element = document.getElementById(SHARE_DATA_ID);
  if (element === null || element.textContent === null) {
    return null;
  }
  try {
    return JSON.parse(element.textContent) as SharePayload;
  } catch {
    return null;
  }
}
