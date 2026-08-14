// What came back from asking the map questions (story 21, ticket 27; the
// thread and its usage are ticket 09, stories 10/12/13). It renders one
// [`AskState`] and nothing else — the request, the state machine and the
// Escape that closes this belong to callers, so the panel itself is as
// available to a test as it is to a reader.
//
// The panel is a conversation: every completed turn stays on screen above
// the current one, because a follow-up like "what calls it?" only reads as
// a follow-up next to the exchange it continues. A cited node is a control
// that selects it on the canvas, in every turn — an answer is a way *into*
// the map, not a wall of text beside it.
//
// Usage is the glossary's: measured or absent. A turn shows token counts
// exactly when its backend reported them, the conversation shows a total
// exactly when every turn was measured (a total missing a turn would be an
// estimate), and nothing here can render a price.
import { useEffect, useRef } from "react";
import type { Node as MapNode } from "../index.js";
import type { Answer, AskState, CompletedTurn, Usage } from "./ask.js";

/** How far above the column's bottom edge still counts as reading the newest
 * exchange, in pixels. A trackpad flick rarely parks exactly on the edge, so
 * a reader within about two lines of the 13px prose is still "at the
 * bottom"; anything further up is someone re-reading an older turn. */
const PINNED_SLACK_PX = 40;

export function AnswerPanel({
  state,
  byId,
  onSelect,
  onDismiss,
}: {
  state: AskState;
  /** The served map's nodes, for resolving citations. */
  byId: Map<string, MapNode>;
  onSelect: (id: string) => void;
  onDismiss: () => void;
}) {
  const column = useRef<HTMLElement | null>(null);
  // The autoscroll rule (ticket 17), recorded: the column follows the
  // conversation only for a reader already pinned to the bottom of it. A
  // fresh column starts pinned, every scroll gesture re-decides it, and an
  // arriving answer scrolls into view exactly when the reader was at the
  // bottom — never yanking someone who scrolled up to re-read an older
  // turn. A ref, not state: where the reader is scrolled to is not
  // something to re-render over.
  const pinned = useRef(true);

  useEffect(() => {
    if (state.phase === "idle") {
      // The next conversation is a fresh one, and it starts at its own
      // bottom — a scroll position from a dismissed thread pins nothing.
      pinned.current = true;
      return;
    }
    const el = column.current;
    if (el !== null && pinned.current) {
      el.scrollTop = el.scrollHeight;
    }
  }, [state]);

  if (state.phase === "idle") {
    return null;
  }

  // One control, and it ends the conversation (ticket 09): the carried
  // turns and the running total are cleared with the panel, so what the
  // reader cannot see can never steer their next question. It sits in the
  // first turn's row — the panel's top-right — wherever the thread starts.
  const dismiss = (
    <button
      type="button"
      className="answer-dismiss"
      onClick={onDismiss}
      aria-label="Dismiss conversation"
      title="Dismiss and start a fresh conversation (Escape)"
    >
      <span aria-hidden="true">×</span>
    </button>
  );

  return (
    // Marked, and deliberately without a walkthrough step of its own: the
    // column exists only after a question has been asked, so a step about it
    // would spotlight an absent element on most walks. The marker is what
    // accounts for the controls inside it — see `WALKTHROUGH_TRANSIENT`.
    <section
      className="answer"
      aria-label="Answer"
      data-walkthrough="answer"
      ref={column}
      onScroll={(event) => {
        const el = event.currentTarget;
        pinned.current =
          el.scrollHeight - el.scrollTop - el.clientHeight <= PINNED_SLACK_PX;
      }}
    >
      {state.turns.map((turn, i) => (
        // Index keys are the honest identity here: the thread is
        // append-only until it is cleared whole, so no turn ever reorders
        // under its key.
        <div className="answer-turn" key={`${i}-${turn.question}`}>
          <div className="answer-head">
            <p className="answer-question">{turn.question}</p>
            {i === 0 && dismiss}
          </div>
          <Exchange answer={turn.answer} byId={byId} onSelect={onSelect} />
        </div>
      ))}

      <div className="answer-turn">
        <div className="answer-head">
          {/* Shown as well as left in the field: the reader can keep typing
              the next question without losing sight of what this answers. */}
          <p className="answer-question">{state.question}</p>
          {state.turns.length === 0 && dismiss}
        </div>

        {state.phase === "asking" && (
          <p className="answer-status" role="status">
            Asking the map…
          </p>
        )}

        {state.phase === "failed" && (
          <p className="answer-error" role="alert">
            Could not answer: {state.message}
          </p>
        )}

        {state.phase === "answered" && (
          <Exchange answer={state.answer} byId={byId} onSelect={onSelect} />
        )}
      </div>

      <ConversationTotal
        turns={state.turns}
        current={state.phase === "answered" ? state.answer : null}
      />
    </section>
  );
}

/** One completed exchange: the answer, its citations, and — when the
 * backend measured it — what it spent. Shared between the thread's turns
 * and the current answer so the two can never drift apart. */
function Exchange({
  answer,
  byId,
  onSelect,
}: {
  answer: Answer;
  byId: Map<string, MapNode>;
  onSelect: (id: string) => void;
}) {
  return (
    <>
      <p className="answer-text">{answer.answer}</p>
      {answer.citations.length === 0 ? (
        <p className="answer-uncited">
          The answer cites no nodes, so there is nothing to open from it.
        </p>
      ) : (
        <ul className="answer-citations" aria-label="Cited nodes">
          {answer.citations.map((id, i) => (
            <li key={`${id}-${i}`}>
              <Citation
                id={id}
                node={byId.get(id) ?? null}
                onSelect={onSelect}
              />
            </li>
          ))}
        </ul>
      )}
      {answer.usage !== undefined && (
        <p className="answer-usage">{counts(answer.usage)}</p>
      )}
    </>
  );
}

/** The running total (story 12), under two absences (story 13). No line at
 * all until the thread holds more than one exchange — a total of one number
 * would only repeat the line above it — and no line unless *every* exchange
 * was measured, because a total missing a turn is an undercount presented
 * as a measurement. Absent, both times, rather than wrong. */
function ConversationTotal({
  turns,
  current,
}: {
  turns: CompletedTurn[];
  current: Answer | null;
}) {
  const answers = current === null
    ? turns.map((turn) => turn.answer)
    : [...turns.map((turn) => turn.answer), current];
  if (answers.length < 2) {
    return null;
  }
  const measured = answers
    .map((answer) => answer.usage)
    .filter((usage): usage is Usage => usage !== undefined);
  if (measured.length !== answers.length) {
    return null;
  }
  return (
    <p className="answer-usage answer-total">
      Conversation total:{" "}
      {counts({
        input_tokens: measured.reduce((sum, u) => sum + u.input_tokens, 0),
        output_tokens: measured.reduce((sum, u) => sum + u.output_tokens, 0),
      })}
    </p>
  );
}

/** The one wording for token counts, so the per-turn line and the total
 * cannot phrase the same measurement two ways. Counts only — the deliberate
 * absence of anything to render a price with is ADR-0012's decision. */
function counts(usage: Usage): string {
  return `${usage.input_tokens} tokens in · ${usage.output_tokens} tokens out`;
}

/** One cited node, or one that cannot be opened.
 *
 * The server filters citations against the map it answered from, so a
 * dangling one should not arrive — but the map is re-read per request and can
 * be re-scanned under a running server, and a control that silently does
 * nothing is the worst available way to be wrong. An ID that resolves to no
 * node is shown as what it is: text, saying why it is not a button. */
function Citation({
  id,
  node,
  onSelect,
}: {
  id: string;
  node: MapNode | null;
  onSelect: (id: string) => void;
}) {
  if (node === null) {
    return (
      <span className="citation-missing">
        <span className="result-name">{id}</span>
        <span className="result-path">
          cited, but not in this map — it may have been re-scanned away
        </span>
      </span>
    );
  }
  return (
    <button type="button" onClick={() => onSelect(node.id)}>
      <span className="result-name">{node.name}</span>
      <span className="result-path">{node.path}</span>
    </button>
  );
}
