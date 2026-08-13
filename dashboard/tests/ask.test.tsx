// Story 21, the dashboard half (ticket 27): the search bar takes a question
// as well as a name, and the answer arrives with the nodes it cites.
//
// Driven through `<App/>` with `fetch` stubbed, because the two things worth
// asserting live on either side of that boundary — that the request is the
// same-origin JSON POST `POST /api/ask` demands (a `text/plain` one is
// refused with 415), and that what comes back becomes a way into the map.
// Nothing here reaches the network: the stub throws on any URL it does not
// recognise, so a request nobody meant to make fails the test rather than
// leaving it.
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { KnowledgeGraph } from "../src/index.js";
import { App } from "../src/app/App.js";
import {
  ASK_ROUTE,
  CAPABILITIES_ROUTE,
  askServer,
  type Turn,
} from "../src/app/ask.js";
import { SHARE_DATA_ID } from "../src/app/share.js";
import { openRegion, selectedOnCanvas } from "./drive.js";
import smallMap from "./fixtures/small-map.json";

const map = smallMap as KnowledgeGraph;

/** Enough of a `Response` for the three call sites that read one. */
function response(status: number, body: unknown): Response {
  return {
    ok: status >= 200 && status < 300,
    status,
    json: async () => body,
  } as unknown as Response;
}

type Reply = { status: number; body: unknown };

/** A stand-in for `codeatlas serve`: the map, no diff overlay, and a
 * capability answer that says whether the binary was started with `--ask`. */
function servedBy(options: {
  ask: boolean;
  answer?: (question: string) => Reply | Promise<Reply>;
}) {
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
        return response(200, { ask: options.ask });
      }
      if (url === ASK_ROUTE) {
        const asked = JSON.parse(String(init?.body)) as { question: string };
        const reply = await (options.answer ?? (() => ({
          status: 200,
          body: { answer: "no answer configured", citations: [] },
        })))(asked.question);
        return response(reply.status, reply.body);
      }
      throw new Error(`the dashboard requested ${url}, which nothing serves`);
    },
  );
  vi.stubGlobal("fetch", fetchStub);
  return fetchStub;
}

/** Renders the app and waits for the served map to arrive. */
async function servedDashboard() {
  render(<App />);
  await screen.findByLabelText("Search nodes");
}

/** Types a question and presses the control that sends it. */
async function askAbout(
  user: ReturnType<typeof userEvent.setup>,
  question: string,
) {
  await user.type(screen.getByLabelText("Search nodes"), question);
  await user.click(screen.getByRole("button", { name: "Ask" }));
}

const answer = () => within(screen.getByLabelText("Answer"));

/** The question-route calls actually made, which is what a second press
 * would cost the reader. */
function asksMade(stub: ReturnType<typeof servedBy>) {
  return stub.mock.calls.filter(([url]) => url === ASK_ROUTE);
}

/** A question route that does not answer until the test lets it — the only
 * way to stand inside the window where a second press spends money. */
function heldOpen(): {
  answer: () => Promise<Reply>;
  release: (reply: Reply) => void;
} {
  let resolve: (reply: Reply) => void = () => {};
  return {
    answer: () => new Promise<Reply>((r) => (resolve = r)),
    release: (reply) => resolve(reply),
  };
}

const realFetch = globalThis.fetch;

afterEach(() => {
  vi.unstubAllGlobals();
  // One test deletes `fetch` outright rather than stubbing it, the way a
  // file:// artifact runs; `unstubAllGlobals` cannot put that back.
  globalThis.fetch = realFetch;
  document.getElementById(SHARE_DATA_ID)?.remove();
});

describe("asking the map a question", () => {
  it("answers a question typed into the search bar", async () => {
    const user = userEvent.setup();
    servedBy({
      ask: true,
      answer: () => ({
        status: 200,
        body: {
          answer: "The program starts in main.ts and greets through Greeter.",
          citations: ["function:src/main.ts:main", "class:src/greeter.ts:Greeter"],
        },
      }),
    });
    await servedDashboard();

    await askAbout(user, "where does the program start?");

    expect(
      await screen.findByText(/the program starts in main\.ts/i),
    ).toBeVisible();
    const cited = within(answer().getByLabelText("Cited nodes"));
    expect(cited.getByRole("button", { name: /main/ })).toBeVisible();
    expect(cited.getByRole("button", { name: /Greeter/ })).toBeVisible();
  });

  it("sends the same-origin JSON POST the question route accepts", async () => {
    // `POST /api/ask` answers 415 to anything but application/json — that
    // demand is what stops another origin spending the reader's model
    // budget, so the dashboard's request has to satisfy it.
    const user = userEvent.setup();
    const fetchStub = servedBy({
      ask: true,
      answer: () => ({ status: 200, body: { answer: "ok", citations: [] } }),
    });
    await servedDashboard();

    await askAbout(user, "what is this?");
    await screen.findByText("ok");

    const call = fetchStub.mock.calls.find(([url]) => url === ASK_ROUTE);
    expect(call, "the dashboard must post to the question route").toBeDefined();
    const init = call?.[1];
    expect(init?.method).toBe("POST");
    expect(init?.headers).toEqual({ "Content-Type": "application/json" });
    expect(JSON.parse(String(init?.body))).toEqual({
      question: "what is this?",
    });
  });

  it("carries previous turns on the wire, and no turns field without them", async () => {
    // Ticket 08's typed wire shape (ADR-0012): ticket 09's thread hands
    // `askServer` the conversation, and it must ride the body unchanged —
    // while a call without turns keeps sending the exact body it always
    // has, which the app-driven test above pins.
    const fetchStub = servedBy({
      ask: true,
      answer: () => ({ status: 200, body: { answer: "ok", citations: [] } }),
    });
    const turns: Turn[] = [
      {
        question: "where does it start?",
        answer: "In main.ts.",
        citations: ["file:src/main.ts"],
      },
    ];

    await askServer("what calls it?", turns);

    const call = fetchStub.mock.calls.find(([url]) => url === ASK_ROUTE);
    expect(call, "the follow-up must reach the question route").toBeDefined();
    expect(JSON.parse(String(call?.[1]?.body))).toEqual({
      question: "what calls it?",
      turns,
    });
  });

  it("still finds a node by name while it can also answer questions", async () => {
    // The search this replaces nothing of: a reader typing a filename must
    // still get the filename, with no request made at all.
    const user = userEvent.setup();
    const fetchStub = servedBy({ ask: true });
    await servedDashboard();

    await user.type(screen.getByLabelText("Search nodes"), "guide.md");

    const results = within(screen.getByLabelText("Search results"));
    expect(results.getByText("guide.md")).toBeInTheDocument();
    expect(results.getAllByRole("button")).toHaveLength(1);
    expect(fetchStub.mock.calls.map(([url]) => url)).not.toContain(ASK_ROUTE);
  });

  it("makes a citation a way into the map", async () => {
    const user = userEvent.setup();
    servedBy({
      ask: true,
      answer: () => ({
        status: 200,
        body: {
          answer: "Start at the entry point.",
          citations: ["function:src/main.ts:main"],
        },
      }),
    });
    await servedDashboard();

    await askAbout(user, "where does the program start?");
    await user.click(
      within(await screen.findByLabelText("Cited nodes")).getByRole("button"),
    );

    // The canvas draws files, so the function is shown by the file holding
    // it — the same reveal a search hit performs.
    await waitFor(() => {
      expect(selectedOnCanvas()).toBe("file:src/main.ts");
    });
    expect(
      within(screen.getByLabelText("Node detail")).getByRole("heading", {
        name: "main",
      }),
    ).toBeInTheDocument();
    // And the answer is still there to work through: one citation followed
    // is not the end of reading it.
    expect(screen.getByLabelText("Answer")).toBeInTheDocument();
  });

  it("degrades visibly on a citation naming a node the map does not have", async () => {
    // The server filters citations against the map it answered from, so this
    // should not happen — but a control that silently does nothing is the
    // worst way to be wrong, and the map can be re-scanned under a running
    // server.
    const user = userEvent.setup();
    servedBy({
      ask: true,
      answer: () => ({
        status: 200,
        body: {
          answer: "It happens in the ledger.",
          citations: ["file:src/ledger.ts", "file:src/main.ts"],
        },
      }),
    });
    await servedDashboard();

    await askAbout(user, "where is the ledger?");

    const cited = within(await screen.findByLabelText("Cited nodes"));
    expect(cited.getByText(/file:src\/ledger\.ts/)).toBeVisible();
    expect(cited.getByText(/not in this map/i)).toBeVisible();
    // Not a control: nothing to press that would do nothing.
    expect(
      cited.queryByRole("button", { name: /ledger/ }),
    ).not.toBeInTheDocument();
    // The citations that do resolve are unaffected by the one that does not.
    expect(cited.getByRole("button", { name: /main\.ts/ })).toBeVisible();
  });

  it("says an answer is in flight, and replaces it with the answer", async () => {
    const user = userEvent.setup();
    let release: (reply: Reply) => void = () => {};
    servedBy({
      ask: true,
      answer: () =>
        new Promise<Reply>((resolve) => {
          release = resolve;
        }),
    });
    await servedDashboard();

    await askAbout(user, "what does this project do?");

    expect(answer().getByRole("status")).toHaveTextContent(/asking/i);
    release({
      status: 200,
      body: { answer: "It greets people.", citations: [] },
    });

    expect(await screen.findByText("It greets people.")).toBeVisible();
    expect(answer().queryByRole("status")).not.toBeInTheDocument();
  });

  it("says what failed without discarding the question", async () => {
    const user = userEvent.setup();
    servedBy({
      ask: true,
      answer: () => ({
        status: 502,
        body: { error: "the backend refused: no credentials" },
      }),
    });
    await servedDashboard();

    await askAbout(user, "why does this fail?");

    expect(await screen.findByRole("alert")).toHaveTextContent(
      /the backend refused: no credentials/,
    );
    // The reader's typing survives the failure, so retrying is one press.
    expect(screen.getByLabelText("Search nodes")).toHaveValue(
      "why does this fail?",
    );
    expect(screen.getByRole("button", { name: "Ask" })).toBeEnabled();
  });

  it("is absent when the binary was started without --ask", async () => {
    // Runtime discovery, not a build-time constant: the same dashboard bytes
    // serve both, and the capability route is what tells them apart.
    const user = userEvent.setup();
    const fetchStub = servedBy({ ask: false });
    await servedDashboard();

    expect(
      screen.queryByRole("button", { name: "Ask" }),
    ).not.toBeInTheDocument();

    // Typing and pressing Enter is the other way in; it must not post either.
    await user.type(screen.getByLabelText("Search nodes"), "anything?{Enter}");
    expect(screen.queryByLabelText("Answer")).not.toBeInTheDocument();
    expect(fetchStub.mock.calls.map(([url]) => url)).not.toContain(ASK_ROUTE);
    // And the search the reader does have still works.
    expect(screen.getByLabelText("Search results")).toBeInTheDocument();
  });

  it("is absent in a share artifact, which has no server", async () => {
    // Ticket 28's shape: an embedded payload and no usable fetch at all, the
    // way a double-clicked file:// page runs. ADR-0009 rejected giving that
    // page a network path, so the question box cannot be in it.
    const user = userEvent.setup();
    const script = document.createElement("script");
    script.id = SHARE_DATA_ID;
    script.type = "application/json";
    script.textContent = JSON.stringify({
      map,
      redaction: { marker: "[redacted]", policy: ["Node.summary"], redacted: [] },
    });
    document.head.append(script);
    // @ts-expect-error simulating a runtime without usable fetch
    delete globalThis.fetch;

    render(<App />);

    expect(
      screen.queryByRole("button", { name: "Ask" }),
    ).not.toBeInTheDocument();
    await user.type(screen.getByLabelText("Search nodes"), "how does this work?{Enter}");
    expect(screen.queryByLabelText("Answer")).not.toBeInTheDocument();
  });

  it("presses Enter to ask, rather than only the button", async () => {
    const user = userEvent.setup();
    servedBy({
      ask: true,
      answer: (question) => ({
        status: 200,
        body: { answer: `asked: ${question}`, citations: [] },
      }),
    });
    await servedDashboard();

    await user.type(
      screen.getByLabelText("Search nodes"),
      "what is a region?{Enter}",
    );

    expect(await screen.findByText("asked: what is a region?")).toBeVisible();
  });

  it("costs one model call however many times Enter is pressed", async () => {
    // The keyboard has no `disabled` attribute to carry the in-flight rule,
    // so the rule cannot live on the button: it is `useAsk.submit`'s, where
    // both ways of asking meet it. Two presses used to be two POSTs, and the
    // reader pays for both.
    const user = userEvent.setup();
    const held = heldOpen();
    const fetchStub = servedBy({ ask: true, answer: held.answer });
    await servedDashboard();
    const field = screen.getByLabelText("Search nodes");

    await user.type(field, "what does this do?{Enter}");
    await screen.findByLabelText("Answer");
    await user.type(field, "{Enter}");
    await user.type(field, "{Enter}");

    expect(asksMade(fetchStub)).toHaveLength(1);
    // Refused, not broken: the one answer still lands.
    held.release({
      status: 200,
      body: { answer: "It greets people.", citations: [] },
    });
    expect(await screen.findByText("It greets people.")).toBeVisible();
  });

  it("costs one model call however many times Ask is pressed", async () => {
    const user = userEvent.setup();
    const held = heldOpen();
    const fetchStub = servedBy({ ask: true, answer: held.answer });
    await servedDashboard();
    await user.type(screen.getByLabelText("Search nodes"), "what does this do?");
    const askButton = screen.getByRole("button", { name: "Ask" });

    await user.click(askButton);
    await screen.findByLabelText("Answer");
    await user.click(askButton);
    await user.click(askButton);

    expect(askButton).toBeDisabled();
    expect(asksMade(fetchStub)).toHaveLength(1);
    held.release({
      status: 200,
      body: { answer: "It greets people.", citations: [] },
    });
    expect(await screen.findByText("It greets people.")).toBeVisible();
  });
});

describe("Escape closes the answer, in the explorer's one cascade", () => {
  it("closes the answer before it steps back out of a region", async () => {
    const user = userEvent.setup();
    servedBy({
      ask: true,
      answer: () => ({
        status: 200,
        body: { answer: "It greets people.", citations: [] },
      }),
    });
    await servedDashboard();
    await openRegion(user, "Source code");
    await askAbout(user, "what does this do?");
    await screen.findByLabelText("Answer");

    await user.keyboard("{Escape}");

    expect(screen.queryByLabelText("Answer")).not.toBeInTheDocument();
    // One layer per press: the region the reader was reading stays open.
    expect(screen.queryByTestId("region-docs")).not.toBeInTheDocument();
  });

  it("reaches the answer from focus outside the panel entirely", async () => {
    // What makes this the explorer's cascade rather than a third handler of
    // its own: focus is parked on a canvas node, which no listener scoped to
    // the answer panel can see. Focus *inside* the panel would not prove it
    // — a panel-scoped handler would pass that test too.
    const user = userEvent.setup();
    servedBy({
      ask: true,
      answer: () => ({
        status: 200,
        body: { answer: "Start at the entry point.", citations: [] },
      }),
    });
    await servedDashboard();
    await askAbout(user, "where does the program start?");
    await screen.findByLabelText("Answer");

    const canvasNode = document.querySelector<HTMLElement>(
      '.react-flow__node[data-id="region:src"]',
    );
    if (canvasNode === null) {
      throw new Error("no canvas node to park focus on");
    }
    canvasNode.focus();
    // The parking has to have taken, or this asserts nothing about focus.
    expect(document.activeElement).toBe(canvasNode);
    await user.keyboard("{Escape}");

    expect(screen.queryByLabelText("Answer")).not.toBeInTheDocument();
  });

  it("reaches it from focus inside the panel too, on a citation", async () => {
    // Ticket 22's dead zone was a keyboard reader unable to close what they
    // had opened, and citations are exactly the kind of new focus target
    // that reopens it.
    const user = userEvent.setup();
    servedBy({
      ask: true,
      answer: () => ({
        status: 200,
        body: {
          answer: "Start at the entry point.",
          citations: ["file:src/main.ts"],
        },
      }),
    });
    await servedDashboard();
    await askAbout(user, "where does the program start?");

    const citation = within(await screen.findByLabelText("Cited nodes")).getByRole(
      "button",
    );
    citation.focus();
    expect(document.activeElement).toBe(citation);
    await user.keyboard("{Escape}");

    expect(screen.queryByLabelText("Answer")).not.toBeInTheDocument();
  });

  it("dismisses the answer with a control too, keeping the question", async () => {
    const user = userEvent.setup();
    servedBy({
      ask: true,
      answer: () => ({
        status: 200,
        body: { answer: "It greets people.", citations: [] },
      }),
    });
    await servedDashboard();
    await askAbout(user, "what does this do?");
    await screen.findByLabelText("Answer");

    // "Dismiss conversation" since ticket 09: the same control, and it now
    // also ends the thread it closes.
    await user.click(
      screen.getByRole("button", { name: "Dismiss conversation" }),
    );

    expect(screen.queryByLabelText("Answer")).not.toBeInTheDocument();
    expect(screen.getByLabelText("Search nodes")).toHaveValue(
      "what does this do?",
    );
  });
});
