// Story 21, the dashboard half (ticket 27): asking the served map a question
// in prose. The model call itself is the serving binary's (ADR-0009) — this
// file only knows how to speak to the two routes that expose it, and how to
// hold the one question in flight.
//
// Two rules shape what is here rather than in `MapExplorer`:
//
// - **Only this file fetches.** The explorer renders share artifacts too, and
//   ADR-0009 rejected giving a double-clicked `file://` page a network path.
//   The explorer therefore takes an [`AskFn`] as a prop; `App` supplies one
//   only for a served map whose binary said it can answer, and share mode
//   supplies none, so the feature is absent there by construction rather than
//   by a check somebody has to remember.
// - **The capability is asked for, not assumed.** Whether a particular server
//   process was started with `--ask` is a property of that process, not of the
//   map (the map schema is a published contract for external producers —
//   story 16, ADR-0003), so it is not in the payload; and probing the question
//   route to find out would be a request made to answer a question nobody
//   asked. It has a route of its own.
import { useCallback, useRef, useState } from "react";

/** Where a question goes. Must match `serve::ASK_ROUTE`. */
export const ASK_ROUTE = "/api/ask";

/** Where the dashboard asks what this server can do. Must match
 * `serve::CAPABILITIES_ROUTE`. */
export const CAPABILITIES_ROUTE = "/api/capabilities";

/** The route answers 415 to anything else, which is what keeps another
 * origin from spending the reader's model budget: a cross-origin `fetch` can
 * only set the three "simple" content types without a preflight, and the
 * server answers no `OPTIONS`. Same-origin — this — is unaffected. */
const ASK_CONTENT_TYPE = "application/json";

/** An answer and the nodes it was drawn from. Citations are node IDs; the
 * server drops any that its map does not contain, and the explorer still
 * shows the ones that do not resolve rather than trusting that. */
export type Answer = { answer: string; citations: string[] };

/** One previous turn as the wire carries it (ADR-0012): the reader's
 * question, the answer, and the node IDs the answer cited. Must match
 * `serve::AskBody`'s `turns` element — `tests/routes.rs` pins the bound
 * below, and the serving binary clamps rather than rejects whatever this
 * type lets through. */
export type Turn = { question: string; answer: string; citations: string[] };

/** The most previous turns a request may carry. Must match
 * `ask::MAX_TURNS`: the dashboard drops its own oldest turns at this bound
 * (ticket 09), so the server's clamp is a backstop rather than the
 * mechanism. */
export const MAX_TURNS = 6;

/** What the explorer is given when questions can be answered at all. */
export type AskFn = (question: string) => Promise<Answer>;

/** What the serving binary said it can do. Absent, unreachable, or older
 * than this route all read the same way: no questions. */
export type Capabilities = { ask: boolean };

/**
 * Asks the local server what it offers. Never rejects — an old binary, the
 * dev server, or a served map with no `--ask` all mean the same thing to the
 * reader, and none of them is an error worth showing them.
 */
export async function readCapabilities(): Promise<Capabilities> {
  try {
    const res = await fetch(CAPABILITIES_ROUTE);
    if (!res.ok) {
      return { ask: false };
    }
    const body = (await res.json()) as { ask?: unknown };
    return { ask: body?.ask === true };
  } catch {
    return { ask: false };
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
  return {
    answer: body.answer,
    citations: Array.isArray(body.citations)
      ? body.citations.filter((id): id is string => typeof id === "string")
      : [],
  };
}

/** One question at a time, and what became of it. The question is kept in
 * every phase because a failure must not cost the reader their typing. */
export type AskState =
  | { phase: "idle" }
  | { phase: "asking"; question: string }
  | { phase: "answered"; question: string; answer: Answer }
  | { phase: "failed"; question: string; message: string };

/**
 * Holds the question in flight. A counter, not a cancellation: `fetch` has
 * already been sent by the time a second question is asked, so what matters
 * is that only the newest reply is allowed to land — including after a
 * dismissal, which would otherwise reopen the panel the reader just closed.
 *
 * It is also where asking twice for one answer is refused. That rule lives
 * here rather than at the call sites because there is more than one way to
 * ask — the Ask button and the Enter key — and a copy of the rule per entry
 * point is exactly how the keyboard one came to be missing it: two presses
 * bought two model calls for one answer. `submit` is the one door they both
 * go through, so the guard on it cannot be bypassed by adding a third.
 */
export function useAsk(ask: AskFn | undefined): {
  state: AskState;
  submit: (question: string) => void;
  dismiss: () => void;
} {
  const [state, setState] = useState<AskState>({ phase: "idle" });
  const latest = useRef(0);
  // A ref rather than the rendered `state`, because two presses can happen
  // before React has re-rendered either of them: a phase read out of the
  // closure would still say "idle" for the second one.
  const inFlight = useRef(false);

  const submit = useCallback(
    (raw: string) => {
      const question = raw.trim();
      if (ask === undefined || question === "" || inFlight.current) {
        return;
      }
      const mine = ++latest.current;
      inFlight.current = true;
      setState({ phase: "asking", question });
      ask(question).then(
        (answer) => {
          if (latest.current === mine) {
            inFlight.current = false;
            setState({ phase: "answered", question, answer });
          }
        },
        (error: unknown) => {
          if (latest.current === mine) {
            inFlight.current = false;
            setState({
              phase: "failed",
              question,
              message: error instanceof Error ? error.message : String(error),
            });
          }
        },
      );
    },
    [ask],
  );

  const dismiss = useCallback(() => {
    latest.current += 1;
    // Putting the answer away frees the field for the next question, which
    // is what the Ask button has always done on dismissal. The reply already
    // paid for is not cancelled — it is discarded by the counter above.
    inFlight.current = false;
    setState({ phase: "idle" });
  }, []);

  return { state, submit, dismiss };
}
