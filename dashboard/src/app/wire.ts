// The wire module (ticket 04 of V3, ADR-0013): every word the dashboard
// speaks to the serving binary — the five route strings and the function
// that fetches on each — and nothing else.
//
// Exactly one place reaches this module at run time: `App`'s served branch,
// by dynamic import. That is a trust-boundary decision, not a performance
// one. The share artifact inlines only the chunks `index.html` references,
// and a chunk reached solely through a dynamic import is not one of them —
// so the artifact does not merely decline to call these routes, it does not
// carry their names at all. A share recipient is precisely someone who does
// not hold the code (ADR-0013), and `tests/share.rs` byte-scans the artifact
// for every `serve::REGISTRY` path beside its external-host scan. Import
// this module statically from anything the main chunk reaches and that scan
// trips.
//
// The types these functions speak in stay where their consumers are:
// `ask.ts` and `source.ts` keep the envelopes, the state machines and the
// hooks (all of which the share artifact legitimately carries — a hook
// handed no function renders no affordance), and this module imports them
// as types only, which costs the chunk nothing.
import type { KnowledgeGraph } from "../index.js";
import type { Answer, Capabilities, Turn, Usage } from "./ask.js";
import type { DiffOverlay } from "./overlay.js";
import type { SourceEnvelope } from "./source.js";

/** Where the served map lives. Must match `serve::REGISTRY`'s map entry. */
export const MAP_ROUTE = "/api/map";

/** Where the diff overlay lives, when `codeatlas diff` has produced one.
 * Must match `serve::REGISTRY`'s diff entry. */
export const DIFF_ROUTE = "/api/diff";

/** Where a question goes. Must match `serve::ASK_ROUTE`. */
export const ASK_ROUTE = "/api/ask";

/** Where the dashboard asks what this server can do. Must match
 * `serve::CAPABILITIES_ROUTE`. */
export const CAPABILITIES_ROUTE = "/api/capabilities";

/** The route a mapped file's source is fetched on. Must match
 * `serve::SOURCE_ROUTE`: `GET /api/source?id=<file-node-id>` — the id, not
 * a path, because the map is the allowlist and the wire speaks file nodes
 * only. */
export const SOURCE_ROUTE = "/api/source";

/** The route answers 415 to anything else, which is what keeps another
 * origin from spending the reader's model budget: a cross-origin `fetch` can
 * only set the three "simple" content types without a preflight, and the
 * server answers no `OPTIONS`. Same-origin — this — is unaffected. */
const ASK_CONTENT_TYPE = "application/json";

/** Every capability off: what unreachable, refused and pre-capabilities
 * servers all mean, written once so the three cannot drift. */
const NO_CAPABILITIES: Capabilities = { ask: false, open_code: false };

/**
 * Fetches the served map, or throws with the server's own explanation.
 */
export async function fetchMap(): Promise<KnowledgeGraph> {
  const res = await fetch(MAP_ROUTE);
  if (!res.ok) {
    const body = (await res.json().catch(() => null)) as {
      error?: string;
    } | null;
    throw new Error(body?.error ?? `map request failed (${res.status})`);
  }
  return (await res.json()) as KnowledgeGraph;
}

/**
 * Fetches the diff overlay, if this workspace has one. The overlay is
 * optional: 404 (no `codeatlas diff` run) or any other failure simply means
 * no toggle — it never blocks the map.
 */
export async function fetchOverlay(): Promise<DiffOverlay | null> {
  try {
    const res = await fetch(DIFF_ROUTE);
    return res.ok ? ((await res.json()) as DiffOverlay) : null;
  } catch {
    return null;
  }
}

/**
 * Asks the local server what it offers. Never rejects — an old binary, the
 * dev server, or a served map with no `--ask` all mean the same thing to the
 * reader, and none of them is an error worth showing them.
 */
export async function readCapabilities(): Promise<Capabilities> {
  try {
    const res = await fetch(CAPABILITIES_ROUTE);
    if (!res.ok) {
      return NO_CAPABILITIES;
    }
    const body = (await res.json()) as { ask?: unknown; open_code?: unknown };
    return { ask: body?.ask === true, open_code: body?.open_code === true };
  } catch {
    return NO_CAPABILITIES;
  }
}

/**
 * Puts one question to `POST /api/ask` and returns what came back, or throws
 * with the server's own explanation. Every failure the route defines carries
 * an `error` string (400 for the question, 413, 415, 500, 502 for the
 * backend), so the reader is told what the program running on their machine
 * said rather than a status number.
 *
 * `turns` is the conversation so far, oldest first (ADR-0012); the thread
 * that assembles it is ticket 09's. A call without turns sends the exact
 * body it always has, so a first question — and every caller written before
 * conversations existed — rides the wire unchanged.
 */
export async function askServer(
  question: string,
  turns: Turn[] = [],
): Promise<Answer> {
  const res = await fetch(ASK_ROUTE, {
    method: "POST",
    headers: { "Content-Type": ASK_CONTENT_TYPE },
    body: JSON.stringify(turns.length > 0 ? { question, turns } : { question }),
  });
  const body = (await res.json().catch(() => null)) as {
    answer?: unknown;
    citations?: unknown;
    usage?: unknown;
    error?: unknown;
  } | null;
  if (!res.ok) {
    throw new Error(
      typeof body?.error === "string"
        ? body.error
        : `the server answered ${res.status}`,
    );
  }
  if (typeof body?.answer !== "string") {
    throw new Error("the server's reply carried no answer");
  }
  const usage = readUsage(body.usage);
  return {
    answer: body.answer,
    citations: Array.isArray(body.citations)
      ? body.citations.filter((id): id is string => typeof id === "string")
      : [],
    // Spread rather than `usage: undefined`: an unreported usage is an
    // absent key, exactly as the wire carries it.
    ...(usage === null ? {} : { usage }),
  };
}

/** The wire's usage object, or nothing. Two numeric counts come through as
 * the measurement they are; anything less — no field, a missing count, a
 * count that is not a number — reads as no measurement at all, because a
 * number shown to the reader must be one a provider actually reported
 * (ADR-0012), never a zero standing in for silence. */
function readUsage(value: unknown): Usage | null {
  if (typeof value !== "object" || value === null) {
    return null;
  }
  const { input_tokens, output_tokens } = value as Record<string, unknown>;
  return typeof input_tokens === "number" && typeof output_tokens === "number"
    ? { input_tokens, output_tokens }
    : null;
}

/**
 * Fetches one mapped file's source, or throws with the server's own words.
 * Every failure the route defines carries an `error` string — the 404 for a
 * file the map names but the disk no longer holds is the one a reader will
 * actually meet, and its message ("re-run `codeatlas scan`…") is better
 * advice than a status number.
 *
 * The id rides as one percent-encoded query parameter: a node id contains
 * `:` and `/`, and the server decodes exactly `encodeURIComponent`'s
 * escapes.
 */
export async function fetchSource(fileId: string): Promise<SourceEnvelope> {
  const res = await fetch(`${SOURCE_ROUTE}?id=${encodeURIComponent(fileId)}`);
  const body = (await res.json().catch(() => null)) as {
    html?: unknown;
    language?: unknown;
    path?: unknown;
    truncated?: unknown;
    error?: unknown;
  } | null;
  if (!res.ok) {
    throw new Error(
      typeof body?.error === "string"
        ? body.error
        : `the server answered ${res.status}`,
    );
  }
  if (typeof body?.html !== "string" || typeof body?.path !== "string") {
    throw new Error("the server's reply carried no source");
  }
  return {
    html: body.html,
    // The one field the panel can survive missing: language is a statement
    // to the reader, not a rendering input, and the fallback's own name is
    // the honest default for an envelope that failed to make it.
    language: typeof body.language === "string" ? body.language : "plain text",
    path: body.path,
    truncated: body.truncated === true,
  };
}
