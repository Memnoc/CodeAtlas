// Open code, the dashboard half (ticket 02 of V3, ADR-0013): how the one
// route that serves a mapped file's source is spoken to, and how the one
// opened file is held while the reader has it beside the map.
//
// The rules are `ask.ts`'s, because the two capabilities gate identically:
//
// - **Only a fetching function crosses into the explorer.** The explorer
//   renders share artifacts too, and a share recipient is precisely someone
//   who does not hold the code (ADR-0013 rejected source in artifacts). The
//   explorer therefore takes an [`OpenSourceFn`] as a prop; `App` supplies
//   one only for a served map whose binary said open code is on, and share
//   mode supplies none, so the affordance is absent there by construction
//   rather than by a check somebody has to remember.
// - **The capability is asked for, not assumed.** Whether this server
//   process was started with `--open-code` is a property of the process,
//   not of the map, so the capabilities route says it (`ask.ts` reads it)
//   and nothing here probes the source route to find out.
import { useCallback, useRef, useState } from "react";
import type { Range } from "../index.js";

/** The route a mapped file's source is fetched on. Must match
 * `serve::SOURCE_ROUTE`: `GET /api/source?id=<file-node-id>` — the id, not
 * a path, because the map is the allowlist and the wire speaks file nodes
 * only. */
export const SOURCE_ROUTE = "/api/source";

/** What the route answers with (ticket 01's envelope): the file's text —
 * plain this lap; highlighting and a language field are ticket 03's — its
 * repo-relative path, and whether the server cut it at the size cap. A cut
 * is disclosed, never refused (ADR-0013), so `truncated` is what the panel
 * turns into a visible notice. */
export type SourceEnvelope = {
  source: string;
  path: string;
  truncated: boolean;
};

/** What the explorer is given when source can be served at all: takes a
 * file node's id, returns the envelope or throws the server's own
 * explanation. Absent means no affordance — every share artifact, and
 * every `serve` without the flag. */
export type OpenSourceFn = (fileId: string) => Promise<SourceEnvelope>;

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
    source?: unknown;
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
  if (typeof body?.source !== "string" || typeof body?.path !== "string") {
    throw new Error("the server's reply carried no source");
  }
  return {
    source: body.source,
    path: body.path,
    truncated: body.truncated === true,
  };
}

/** One opened file, and what became of opening it. Never an empty panel:
 * every non-closed phase has something honest to show — the request under
 * way, the source, or the server's own refusal. `lit` is the line range to
 * light and land on, carried from the symbol the reader opened (the
 * contract's 1-based inclusive `range`); null means the top of the file,
 * which is where a file node opens. */
export type SourceState =
  | { phase: "closed" }
  | { phase: "opening"; fileId: string; path: string; lit: Range | null }
  | {
      phase: "open";
      fileId: string;
      path: string;
      lit: Range | null;
      envelope: SourceEnvelope;
    }
  | { phase: "failed"; fileId: string; path: string; message: string };

/**
 * Holds the one opened file. A counter, not a cancellation, exactly as
 * `useAsk` holds its question: `fetch` has already been sent by the time a
 * second file is opened, so what matters is that only the newest reply is
 * allowed to land — including after a dismissal, which would otherwise
 * reopen the panel the reader just closed. No in-flight refusal, though:
 * unlike a question, a source read costs nothing but the read, so opening
 * something else mid-flight simply wins.
 */
export function useSource(openSource: OpenSourceFn | undefined): {
  state: SourceState;
  /** Opens a file node's source, landing on `lit` when a symbol asked. */
  open: (fileId: string, path: string, lit: Range | null) => void;
  dismiss: () => void;
} {
  const [state, setState] = useState<SourceState>({ phase: "closed" });
  const latest = useRef(0);

  const open = useCallback(
    (fileId: string, path: string, lit: Range | null) => {
      if (openSource === undefined) {
        return;
      }
      const mine = ++latest.current;
      setState({ phase: "opening", fileId, path, lit });
      openSource(fileId).then(
        (envelope) => {
          if (latest.current === mine) {
            setState({ phase: "open", fileId, path, lit, envelope });
          }
        },
        (error: unknown) => {
          if (latest.current === mine) {
            setState({
              phase: "failed",
              fileId,
              path,
              message: error instanceof Error ? error.message : String(error),
            });
          }
        },
      );
    },
    [openSource],
  );

  const dismiss = useCallback(() => {
    latest.current += 1;
    setState({ phase: "closed" });
  }, []);

  return { state, open, dismiss };
}
