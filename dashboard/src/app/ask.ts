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
 * shows the ones that do not resolve rather than trusting that. Usage is
 * what the exchange measurably spent, absent whenever the backend reported
 * nothing (ADR-0012) — never estimated, never a price. */
export type Answer = { answer: string; citations: string[]; usage?: Usage };

/** Token counts the serving binary relayed from the provider's response
 * envelope, under the wire's own field names. Two measured counts or the
 * whole thing absent — the glossary's rule, enforced where the wire is
 * read (see [`askServer`]). */
export type Usage = { input_tokens: number; output_tokens: number };

/** One previous turn as the wire carries it (ADR-0012): the reader's
 * question, the answer, and the node IDs the answer cited. Must match
 * `serve::AskBody`'s `turns` element — `tests/routes.rs` pins the bound
 * below, and the serving binary clamps rather than rejects whatever this
 * type lets through. */
export type Turn = { question: string; answer: string; citations: string[] };

/** The most previous turns a request may carry. Must match
 * `ask::MAX_TURNS`: the dashboard drops its own oldest turns at this bound
 * (see [`useAsk`]), so the server's clamp is a backstop rather than the
 * mechanism. */
export const MAX_TURNS = 6;

/** One finished exchange as the thread keeps it: the reader's question and
 * everything the answer arrived with — text, citations, usage. Richer than
 * the wire's [`Turn`], which carries no usage, because usage is
 * display-side: the server has no use for what an earlier answer cost. */
export type CompletedTurn = { question: string; answer: Answer };

/** What the explorer is given when questions can be answered at all. The
 * conversation so far rides along (ADR-0012); [`useAsk`] assembles it. */
export type AskFn = (question: string, turns?: Turn[]) => Promise<Answer>;

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

/** One question at a time, and what became of it — plus the conversation
 * the question continues. `turns` is every exchange completed *before* the
 * current question, oldest first, so the panel renders thread-then-current
 * without working out where the seam is. The question is kept in every
 * phase because a failure must not cost the reader their typing; `idle`
 * carries no turns because dismissal is the fresh-conversation control —
 * a hidden panel never holds an invisible history that would silently
 * steer the next question. */
export type AskState =
  | { phase: "idle" }
  | { phase: "asking"; question: string; turns: CompletedTurn[] }
  | { phase: "answered"; question: string; answer: Answer; turns: CompletedTurn[] }
  | { phase: "failed"; question: string; message: string; turns: CompletedTurn[] };

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
  // The completed exchanges, oldest first. A ref for the same reason
  // `inFlight` is: `submit` needs the conversation at the moment of the
  // press, not the one some earlier render closed over.
  const conversation = useRef<CompletedTurn[]>([]);

  const submit = useCallback(
    (raw: string) => {
      const question = raw.trim();
      if (ask === undefined || question === "" || inFlight.current) {
        return;
      }
      const mine = ++latest.current;
      inFlight.current = true;
      const prior = conversation.current;
      setState({ phase: "asking", question, turns: prior });
      // The dashboard's own enforcement of ADR-0012's turn bound: only the
      // newest MAX_TURNS ride the wire, oldest dropped first, so the
      // server's clamp is a backstop rather than the mechanism. The thread
      // on screen keeps every turn — the bound is on what a request
      // carries, not on what the reader may look back at.
      const carried: Turn[] = prior.slice(-MAX_TURNS).map((turn) => ({
        question: turn.question,
        answer: turn.answer.answer,
        citations: turn.answer.citations,
      }));
      ask(question, carried).then(
        (answer) => {
          if (latest.current === mine) {
            inFlight.current = false;
            // The exchange is complete, so it joins the conversation the
            // *next* question carries. Behind the `latest` guard: an answer
            // landing after a dismissal must not resurrect a conversation
            // the reader ended.
            conversation.current = [...prior, { question, answer }];
            setState({ phase: "answered", question, answer, turns: prior });
          }
        },
        (error: unknown) => {
          if (latest.current === mine) {
            inFlight.current = false;
            // A failed question joins nothing: retrying carries exactly the
            // turns the failed attempt carried.
            setState({
              phase: "failed",
              question,
              message: error instanceof Error ? error.message : String(error),
              turns: prior,
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
    // Dismissal is the fresh-conversation control (ticket 09): the carried
    // turns and the running total go with the panel, so the next question
    // starts from zero rather than being steered by a thread the reader
    // can no longer see.
    conversation.current = [];
    setState({ phase: "idle" });
  }, []);

  return { state, submit, dismiss };
}
