// What came back from asking the map a question (story 21, ticket 27). It
// renders one [`AskState`] and nothing else — the request, the state machine
// and the Escape that closes this belong to callers, so the panel itself is
// as available to a test as it is to a reader.
//
// A cited node is a control that selects it on the canvas, which is the whole
// point of citing node IDs rather than prose: an answer is a way *into* the
// map, not a wall of text beside it.
import type { Node as MapNode } from "../index.js";
import type { AskState } from "./ask.js";

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
  if (state.phase === "idle") {
    return null;
  }

  return (
    // Marked, and deliberately without a walkthrough step of its own: the
    // band exists only after a question has been asked, so a step about it
    // would spotlight an absent element on most walks. The marker is what
    // accounts for the controls inside it — see `WALKTHROUGH_TRANSIENT`.
    <section className="answer" aria-label="Answer" data-walkthrough="answer">
      <div className="answer-head">
        {/* Shown as well as left in the field: the reader can keep typing
            the next question without losing sight of what this answers. */}
        <p className="answer-question">{state.question}</p>
        <button
          type="button"
          className="answer-dismiss"
          onClick={onDismiss}
          aria-label="Dismiss answer"
          title="Dismiss answer (Escape)"
        >
          <span aria-hidden="true">×</span>
        </button>
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
        <>
          <p className="answer-text">{state.answer.answer}</p>
          {state.answer.citations.length === 0 ? (
            <p className="answer-uncited">
              The answer cites no nodes, so there is nothing to open from it.
            </p>
          ) : (
            <ul className="answer-citations" aria-label="Cited nodes">
              {state.answer.citations.map((id, i) => (
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
        </>
      )}
    </section>
  );
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
