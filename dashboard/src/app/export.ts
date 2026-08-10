// Export: hands the reader the map they are looking at, as JSON.
//
// Deliberately not the share artifact. `codeatlas share` redacts enriched
// prose against an allowlist over the contract (ADR-0006) and states what it
// removed; reimplementing that here would put the same security policy in two
// places written in two languages, and the copy that drifts is the one that
// leaks. So this exports exactly the map already in the reader's hands —
// their own file when served locally, the already-redacted payload when this
// document *is* a share artifact — and the button says where to get the
// self-contained page instead.
//
// No network: the file is built from memory and handed over through an
// object URL, which never leaves the document.
import type { KnowledgeGraph } from "../index.js";

/** A download name from the project name: anything outside a conservative
 * set becomes a dash, and leading dots and dashes are dropped so the result
 * cannot read as a relative path or a hidden file. Browsers sanitise the
 * `download` attribute themselves; this is about handing over a name the
 * reader recognises, not about defending the filesystem. */
export function mapFilename(map: KnowledgeGraph): string {
  const slug = map.project.name
    .replace(/[^a-zA-Z0-9._-]+/g, "-")
    .replace(/^[.\-]+/, "")
    .replace(/[.\-]+$/, "");
  return `${slug === "" ? "map" : slug}-map.json`;
}

export function downloadMap(map: KnowledgeGraph): void {
  const blob = new Blob([JSON.stringify(map, null, 2)], {
    type: "application/json",
  });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = mapFilename(map);
  document.body.append(link);
  link.click();
  link.remove();
  // Revoking immediately is safe: the click has already handed the blob to
  // the download, and holding the URL would pin the whole map in memory.
  URL.revokeObjectURL(url);
}
