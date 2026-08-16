// Ticket 09, the panel half (stories 10, 12, 13): the answer panel is a
// thread. Prior turns stay visible, a follow-up carries them on the wire,
// each turn shows what it measurably spent, a running total accumulates —
// and one control starts a fresh conversation.
//
// Seam 5, gesture→state only: everything here is a reader's gesture and the
// state the panel shows for it. The wire behaviour behind `askServer` is
// ticket 08's, and the usage passthrough is proven server-side in
// `crates/codeatlas/tests/serve.rs`; the stub below plays the server so the
// tests can assert what the dashboard *sends* as well as what it shows.
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { KnowledgeGraph } from "../src/index.js";
import { App } from "../src/app/App.js";
import { MAX_TURNS, type Turn } from "../src/app/ask.js";
import {
  ASK_ROUTE,
  CAPABILITIES_ROUTE,
  askServer,
} from "../src/app/wire.js";
import { selectedOnCanvas } from "./drive.js";
import smallMap from "./fixtures/small-map.json";

const map = smallMap as KnowledgeGraph;

/** Enough of a `Response` for the call sites that read one. */
function response(status: number, body: unknown): Response {
  return {
    ok: status >= 200 && status < 300,
    status,
    json: async () => body,
  } as unknown as Response;
}

type Reply = { status: number; body: unknown };

/** A stand-in for `codeatlas serve --ask`: the map, no overlay, questions
 * answered by the test's own script. Throws on anything else, so a request
 * nobody meant to make fails the test rather than leaving it. */
function servedBy(answer: (asked: AskedBody) => Reply) {
  const fetchStub = vi.fn(
    async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = String(input);
      if (url === "/api/map") {
        return response(200, map);
      }
      if (url === "/api/diff") {
        return response(404, { error: "no diff overlay" });
      }
      if (url === CAPABILITIES_ROUTE) {
        return response(200, { ask: true });
      }
      if (url === ASK_ROUTE) {
        const asked = JSON.parse(String(init?.body)) as AskedBody;
        const reply = answer(asked);
        return response(reply.status, reply.body);
      }
      throw new Error(`the dashboard requested ${url}, which nothing serves`);
    },
  );
  vi.stubGlobal("fetch", fetchStub);
  return fetchStub;
}

/** What the dashboard put on the wire — the half of the exchange the server
 * tests cannot see from here, so it is asserted here. */
type AskedBody = { question: string; turns?: Turn[] };

/** The ask-route bodies actually sent, in order. */
function askedBodies(stub: ReturnType<typeof servedBy>): AskedBody[] {
  return stub.mock.calls
    .filter(([url]) => url === ASK_ROUTE)
    .map(([, init]) => JSON.parse(String(init?.body)) as AskedBody);
}

async function servedDashboard() {
  render(<App />);
  await screen.findByLabelText("Search nodes");
}

/** Clears the field, types a question, presses Ask, and waits for the
 * answer to land — one whole turn of the conversation. */
async function turnOf(
  user: ReturnType<typeof userEvent.setup>,
  question: string,
  answerText: string,
) {
  const field = screen.getByLabelText("Search nodes");
  await user.clear(field);
  await user.type(field, question);
  await user.click(screen.getByRole("button", { name: "Ask" }));
  await screen.findByText(answerText);
}

const answer = () => within(screen.getByLabelText("Answer"));

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("the conversation thread", () => {
  it("keeps prior turns visible and carries them on the follow-up", async () => {
    const user = userEvent.setup();
    const stub = servedBy((asked) => ({
      status: 200,
      body:
        asked.question === "where does the program start?"
          ? {
              answer: "It starts in main.ts.",
              citations: ["file:src/main.ts"],
            }
          : { answer: "Nothing calls it; it is the entry.", citations: [] },
    }));
    await servedDashboard();

    await turnOf(user, "where does the program start?", "It starts in main.ts.");
    await turnOf(user, "what calls it?", "Nothing calls it; it is the entry.");

    // The thread: both questions and both answers on screen at once.
    expect(answer().getByText("where does the program start?")).toBeVisible();
    expect(answer().getByText("It starts in main.ts.")).toBeVisible();
    expect(answer().getByText("what calls it?")).toBeVisible();
    expect(
      answer().getByText("Nothing calls it; it is the entry."),
    ).toBeVisible();
    // The first turn's citations are still a way into the map.
    expect(answer().getByRole("button", { name: /main\.ts/ })).toBeVisible();

    // And the wire carried the conversation: no turns field on the first
    // question, the completed first turn on the second.
    const bodies = askedBodies(stub);
    expect(bodies[0]).toEqual({ question: "where does the program start?" });
    expect(bodies[1]).toEqual({
      question: "what calls it?",
      turns: [
        {
          question: "where does the program start?",
          answer: "It starts in main.ts.",
          citations: ["file:src/main.ts"],
        },
      ],
    });
  });

  it("stays visible while the follow-up is in flight", async () => {
    const user = userEvent.setup();
    let release: (reply: Reply) => void = () => {};
    let held = false;
    const stub = servedBy((asked) => {
      if (asked.question === "first?") {
        return { status: 200, body: { answer: "First answer.", citations: [] } };
      }
      held = true;
      return { status: 200, body: { answer: "unused", citations: [] } };
    });
    // Re-stub the second call to hold the reply open: the panel must show
    // the thread and the in-flight question together.
    const original = globalThis.fetch;
    vi.stubGlobal(
      "fetch",
      async (input: RequestInfo | URL, init?: RequestInit) => {
        if (String(input) === ASK_ROUTE && held) {
          return new Promise<Response>((resolve) => {
            release = (reply) => resolve(response(reply.status, reply.body));
          });
        }
        return original(input, init);
      },
    );
    await servedDashboard();

    await turnOf(user, "first?", "First answer.");
    held = true;
    const field = screen.getByLabelText("Search nodes");
    await user.clear(field);
    await user.type(field, "second?");
    await user.click(screen.getByRole("button", { name: "Ask" }));

    expect(answer().getByText("first?")).toBeVisible();
    expect(answer().getByText("First answer.")).toBeVisible();
    expect(answer().getByText("second?")).toBeVisible();
    expect(answer().getByRole("status")).toHaveTextContent(/asking/i);

    release({ status: 200, body: { answer: "Second answer.", citations: [] } });
    expect(await screen.findByText("Second answer.")).toBeVisible();
    expect(stub).toBeDefined();
  });

  it("drops its own oldest turns at the bound, before the server has to", async () => {
    // ADR-0012: the dashboard enforces the 6-turn bound itself, so the
    // server's clamp is a backstop rather than the mechanism. Eight
    // exchanges leave seven completed turns; the eighth question must carry
    // exactly the newest six.
    const user = userEvent.setup();
    const stub = servedBy((asked) => ({
      status: 200,
      body: { answer: `answer to ${asked.question}`, citations: [] },
    }));
    await servedDashboard();

    for (let i = 1; i <= 8; i += 1) {
      await turnOf(user, `question ${i}?`, `answer to question ${i}?`);
    }

    const last = askedBodies(stub).at(-1);
    expect(last?.question).toBe("question 8?");
    expect(last?.turns).toHaveLength(MAX_TURNS);
    expect(last?.turns?.map((turn) => turn.question)).toEqual([
      "question 2?",
      "question 3?",
      "question 4?",
      "question 5?",
      "question 6?",
      "question 7?",
    ]);
  });

  it("shows per-turn usage and a running conversation total", async () => {
    const user = userEvent.setup();
    servedBy((asked) => ({
      status: 200,
      body:
        asked.question === "first?"
          ? {
              answer: "First answer.",
              citations: [],
              usage: { input_tokens: 1207, output_tokens: 83 },
            }
          : {
              answer: "Second answer.",
              citations: [],
              usage: { input_tokens: 411, output_tokens: 9 },
            },
    }));
    await servedDashboard();

    await turnOf(user, "first?", "First answer.");
    // One exchange: its measured counts, and no total — a total of one
    // number would just repeat the line above it.
    expect(answer().getByText("1207 tokens in · 83 tokens out")).toBeVisible();
    expect(answer().queryByText(/conversation total/i)).not.toBeInTheDocument();

    await turnOf(user, "second?", "Second answer.");
    expect(answer().getByText("1207 tokens in · 83 tokens out")).toBeVisible();
    expect(answer().getByText("411 tokens in · 9 tokens out")).toBeVisible();
    expect(
      answer().getByText("Conversation total: 1618 tokens in · 92 tokens out"),
    ).toBeVisible();
  });

  it("shows no usage at all when the backend reports none", async () => {
    // Story 13: measured or absent. A backend that reports nothing draws no
    // usage line and no total — and no zero stands in anywhere.
    const user = userEvent.setup();
    servedBy((asked) => ({
      status: 200,
      body: { answer: `Unmeasured: ${asked.question}`, citations: [] },
    }));
    await servedDashboard();

    await turnOf(user, "first?", "Unmeasured: first?");
    await turnOf(user, "second?", "Unmeasured: second?");

    expect(answer().queryByText(/tokens in/)).not.toBeInTheDocument();
    expect(answer().queryByText(/conversation total/i)).not.toBeInTheDocument();
    expect(answer().queryByText(/\b0 tokens/)).not.toBeInTheDocument();
  });

  it("absents the total rather than undercounting when one turn went unmeasured", async () => {
    // A total missing a turn is an estimate, and the glossary's rule is
    // measured or absent — so a conversation holding any unmeasured turn
    // has no total, while the measured turns keep their own lines.
    const user = userEvent.setup();
    servedBy((asked) => ({
      status: 200,
      body:
        asked.question === "first?"
          ? {
              answer: "Measured answer.",
              citations: [],
              usage: { input_tokens: 1207, output_tokens: 83 },
            }
          : { answer: "Unmeasured answer.", citations: [] },
    }));
    await servedDashboard();

    await turnOf(user, "first?", "Measured answer.");
    await turnOf(user, "second?", "Unmeasured answer.");

    expect(answer().getByText("1207 tokens in · 83 tokens out")).toBeVisible();
    expect(answer().queryByText(/conversation total/i)).not.toBeInTheDocument();
  });

  it("starts a fresh conversation from one control", async () => {
    const user = userEvent.setup();
    const stub = servedBy((asked) => ({
      status: 200,
      body: {
        answer: `answer to ${asked.question}`,
        citations: [],
        usage: { input_tokens: 100, output_tokens: 10 },
      },
    }));
    await servedDashboard();

    await turnOf(user, "first?", "answer to first?");
    await turnOf(user, "second?", "answer to second?");
    expect(
      answer().getByText("Conversation total: 200 tokens in · 20 tokens out"),
    ).toBeVisible();

    await user.click(
      screen.getByRole("button", { name: "Dismiss conversation" }),
    );
    expect(screen.queryByLabelText("Answer")).not.toBeInTheDocument();

    // The next question starts from zero: nothing carried on the wire, and
    // the thread shows one turn with its own fresh total-less usage line.
    await turnOf(user, "third?", "answer to third?");
    expect(askedBodies(stub).at(-1)).toEqual({ question: "third?" });
    expect(answer().queryByText("first?")).not.toBeInTheDocument();
    expect(answer().queryByText(/conversation total/i)).not.toBeInTheDocument();
    expect(answer().getByText("100 tokens in · 10 tokens out")).toBeVisible();
  });

  it("still reads a single question exactly as before", async () => {
    // Story 10: a reader who asks one question and stops sees what they see
    // today — one question, one answer, its citations, one dismiss control.
    const user = userEvent.setup();
    servedBy(() => ({
      status: 200,
      body: {
        answer: "It starts in main.ts.",
        citations: ["file:src/main.ts"],
      },
    }));
    await servedDashboard();

    await turnOf(user, "where does the program start?", "It starts in main.ts.");

    expect(answer().getByText("where does the program start?")).toBeVisible();
    expect(
      within(answer().getByLabelText("Cited nodes")).getByRole("button", {
        name: /main\.ts/,
      }),
    ).toBeVisible();
    await user.keyboard("{Escape}");
    await waitFor(() => {
      expect(screen.queryByLabelText("Answer")).not.toBeInTheDocument();
    });
  });
});

describe("the conversation column, beside the canvas (story 26)", () => {
  // Ticket 17: the thread moves from a band above the canvas to a column
  // docked beside it. Gesture→state only, per seam 5 — where the column
  // *is* (a workspace sibling of the canvas) is state jsdom can see; how
  // wide it paints is the stylesheet contract's half.

  /** The answer column and the canvas, as the workspace holds them — or
   * throws, because every assertion here is about that arrangement. */
  function columnAndCanvas() {
    const column = screen.getByLabelText("Answer");
    const workspace = document.querySelector(".workspace");
    if (workspace === null) {
      throw new Error("no workspace rendered");
    }
    const canvas = workspace.querySelector("main.canvas");
    if (canvas === null) {
      throw new Error("no canvas in the workspace");
    }
    return { column, workspace, canvas };
  }

  it("opens as a column in the workspace, beside a canvas still drawn", async () => {
    const user = userEvent.setup();
    servedBy(() => ({
      status: 200,
      body: { answer: "It greets people.", citations: [] },
    }));
    await servedDashboard();

    await turnOf(user, "what does this do?", "It greets people.");

    const { column, workspace, canvas } = columnAndCanvas();
    // Docked in the workspace as the canvas's sibling — not a band between
    // the search bar and the chips, which is where it used to grow until
    // the map fell off the screen.
    expect(column.parentElement).toBe(workspace);
    expect(column.previousElementSibling).toBe(canvas);
    // The canvas is still the reader's to use: its nodes are still there
    // to click while the column is open.
    expect(
      canvas.querySelector('.react-flow__node[data-id="region:src"]'),
    ).not.toBeNull();
  });

  it("gives the single-question reader the same column, not a special case", async () => {
    const user = userEvent.setup();
    servedBy(() => ({
      status: 200,
      body: { answer: "One answer.", citations: [] },
    }));
    await servedDashboard();

    await turnOf(user, "one question?", "One answer.");

    // One turn, and the conversation already lives where six would: the
    // workspace, beside the canvas, dismiss control and all.
    const { column, workspace } = columnAndCanvas();
    expect(column.parentElement).toBe(workspace);
    expect(
      within(column).getByRole("button", { name: "Dismiss conversation" }),
    ).toBeVisible();
  });

  it("draws the cited card on the canvas beside the open column", async () => {
    // The choice the citations exist to remove: a citation click must light
    // a card on a canvas the reader can see, with the thread still beside
    // it — column and canvas working together, in one test.
    const user = userEvent.setup();
    servedBy(() => ({
      status: 200,
      body: { answer: "It starts in main.ts.", citations: ["file:src/main.ts"] },
    }));
    await servedDashboard();
    await turnOf(user, "where does it start?", "It starts in main.ts.");

    await user.click(
      within(screen.getByLabelText("Cited nodes")).getByRole("button"),
    );

    await waitFor(() => {
      expect(selectedOnCanvas()).toBe("file:src/main.ts");
    });
    const { column, canvas } = columnAndCanvas();
    // The card is drawn, not merely selected somewhere off screen: the
    // reveal put it on the canvas the reader is looking at.
    expect(
      canvas.querySelector('.react-flow__node[data-id="file:src/main.ts"]'),
    ).not.toBeNull();
    // And the thread is still open beside it to keep reading from.
    expect(within(column).getByText("It starts in main.ts.")).toBeVisible();
  });

  it("leaves focus in the search box when the column opens", async () => {
    // Opening must not seize focus: the reader who asked from the field is
    // mid-typing-flow, and the next follow-up starts there too.
    const user = userEvent.setup();
    servedBy(() => ({
      status: 200,
      body: { answer: "It greets people.", citations: [] },
    }));
    await servedDashboard();
    const field = screen.getByLabelText("Search nodes");

    await user.type(field, "what does this do?{Enter}");
    await screen.findByText("It greets people.");

    expect(field).toHaveFocus();
  });

  it("returns focus to the search box when the column is dismissed from inside", async () => {
    // The `useFocusReturn` discipline: closing a layer that took focus with
    // it must not strand the keyboard on <body>. The dismiss control is
    // inside the column, so pressing it is exactly that case.
    const user = userEvent.setup();
    servedBy(() => ({
      status: 200,
      body: { answer: "It greets people.", citations: [] },
    }));
    await servedDashboard();
    await turnOf(user, "what does this do?", "It greets people.");

    await user.click(
      screen.getByRole("button", { name: "Dismiss conversation" }),
    );

    expect(screen.queryByLabelText("Answer")).not.toBeInTheDocument();
    expect(screen.getByLabelText("Search nodes")).toHaveFocus();
  });

  it("scrolls the newest exchange into view for a reader at the bottom", async () => {
    // The recorded rule: autoscroll only when the reader is already pinned
    // to the bottom of the thread — the reader following along is carried
    // to what arrives.
    const user = userEvent.setup();
    servedBy((asked) => ({
      status: 200,
      body: { answer: `answer to ${asked.question}`, citations: [] },
    }));
    await servedDashboard();
    await turnOf(user, "first?", "answer to first?");

    const column = screen.getByLabelText("Answer");
    // jsdom lays nothing out, so the column's metrics are stated: a 300px
    // viewport onto 1000px of thread, scrolled to the bottom.
    Object.defineProperties(column, {
      scrollHeight: { configurable: true, value: 1000 },
      clientHeight: { configurable: true, value: 300 },
    });
    fireEvent.scroll(column, { target: { scrollTop: 700 } });

    await turnOf(user, "second?", "answer to second?");

    expect(column.scrollTop).toBe(1000);
  });

  it("never steals the scroll from a reader partway up an older turn", async () => {
    // The rule's other half: a reader who scrolled up to re-read is not
    // pinned, and an arriving answer must not yank them to the bottom.
    const user = userEvent.setup();
    servedBy((asked) => ({
      status: 200,
      body: { answer: `answer to ${asked.question}`, citations: [] },
    }));
    await servedDashboard();
    await turnOf(user, "first?", "answer to first?");

    const column = screen.getByLabelText("Answer");
    Object.defineProperties(column, {
      scrollHeight: { configurable: true, value: 1000 },
      clientHeight: { configurable: true, value: 300 },
    });
    fireEvent.scroll(column, { target: { scrollTop: 100 } });

    await turnOf(user, "second?", "answer to second?");

    expect(column.scrollTop).toBe(100);
  });
});

describe("askServer reads usage off the wire", () => {
  it("keeps a measured usage and drops anything less", async () => {
    // Measured or absent, enforced where the wire is read: two numeric
    // counts come through, and a partial or malformed object reads as no
    // measurement — never as zero.
    const bodies: unknown[] = [
      {
        answer: "ok",
        citations: [],
        usage: { input_tokens: 1207, output_tokens: 83 },
      },
      { answer: "ok", citations: [] },
      { answer: "ok", citations: [], usage: { input_tokens: 1207 } },
      {
        answer: "ok",
        citations: [],
        usage: { input_tokens: "1207", output_tokens: 83 },
      },
      { answer: "ok", citations: [], usage: "1207 in, 83 out" },
    ];
    let call = 0;
    vi.stubGlobal("fetch", async () =>
      response(200, bodies[(call += 1) - 1]),
    );

    const measured = await askServer("q");
    expect(measured.usage).toEqual({ input_tokens: 1207, output_tokens: 83 });

    for (let i = 1; i < bodies.length; i += 1) {
      const unmeasured = await askServer("q");
      expect(
        unmeasured.usage,
        `reply ${i} reports nothing usable and must read as absent`,
      ).toBeUndefined();
    }
  });
});
