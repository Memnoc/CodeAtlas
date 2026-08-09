// The redaction disclosure banner (spec story 10): a share artifact tells
// its reader what was redacted from it — field paths and counts — and that
// live-workspace features (the diff overlay) are not part of a snapshot.
import type { RedactionDisclosure } from "./share.js";

export function ShareBanner({
  redaction,
}: {
  redaction: RedactionDisclosure;
}) {
  const details =
    redaction.redacted.length === 0
      ? `Nothing was redacted — this map contains no LLM-enriched prose ` +
        `(redactable fields: ${redaction.policy.join(", ")}).`
      : `LLM-enriched prose was replaced with ${redaction.marker}: ` +
        redaction.redacted
          .map(({ field, count }) => `${field} (${count})`)
          .join(", ") +
        ".";
  return (
    <aside className="share-banner" role="note" aria-label="Redaction disclosure">
      <strong>Shared snapshot</strong> — {details} The diff overlay is not
      included in shared maps.
    </aside>
  );
}
