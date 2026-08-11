// The dashboard's seam component: give it a map conforming to the published
// contract and it renders the whole explorer — header, search, canvas, right
// panel. It consumes only the generated contract types (ADR-0003) and makes
// no network request of its own: questions (story 21) are put through an
// `onAsk` its host supplies, so the share artifact — which renders this same
// component from a `file://` page — cannot acquire a network path by being
// rendered.
//
// The canvas draws regions, not files. A repository has hundreds of files and
// a handful of regions, and the overview's job is to be readable at a glance;
// the files are one click away, inside the region that holds them.
import {
  Background,
  Controls,
  MarkerType,
  MiniMap,
  ReactFlow,
  type Edge as FlowEdge,
  type ReactFlowInstance,
} from "@xyflow/react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import type { KnowledgeGraph, Node as MapNode } from "../index.js";
import { AnswerPanel } from "./AnswerPanel.js";
import { type AskFn, useAsk } from "./ask.js";
import { FilesPanel } from "./FilesPanel.js";
import { FlowsPanel } from "./FlowsPanel.js";
import {
  fileFlow,
  type AppFlowNode,
  nodesById,
  regionFlow,
  regionNodeId,
  searchNodes,
} from "./graph.js";
import { Header, type Mode } from "./Header.js";
import {
  CARD_HEIGHT,
  captionOf,
  edgeLabelOf,
  enrichmentHint,
  narrativeOf,
  regionCaptionOf,
} from "./labels.js";
import { InfoPanel } from "./InfoPanel.js";
import { nodeTypes } from "./nodes.js";
import type { DiffOverlay } from "./overlay.js";
import { PathFinder } from "./PathFinder.js";
import { ProvenanceBadge } from "./ProvenanceBadge.js";
import {
  fileOwners,
  regionLinks,
  regionsOf,
  type RegionKind,
} from "./regions.js";
import { shortestPath } from "./paths.js";
import { Narrative, TourPanel } from "./TourPanel.js";
import { motionDuration } from "./motion.js";
import { Walkthrough } from "./Walkthrough.js";
import {
  resolveWalkthroughSteps,
  type WalkthroughStep,
} from "./walkthrough.js";
import "@xyflow/react/dist/style.css";
import "./styles.css";

type Tab = "info" | "files";

/** How long the viewport takes to travel, matching the canvas transitions in
 * `styles.css`. One number, so the cards and the camera settle together. */
const FIT_MS = 240;

export function MapExplorer({
  map,
  overlay,
  shared = false,
  onAsk,
}: {
  map: KnowledgeGraph;
  /** Diff impact overlay, when `codeatlas diff` produced one. */
  overlay?: DiffOverlay | null;
  /** True when this document is a share artifact rather than the served
   * dashboard: the map in hand is already redacted, and its reader has
   * nothing installed to run a CLI with. */
  shared?: boolean;
  /** How to put a question to the serving binary, when the binary this
   * dashboard came from was started with `--ask` (ADR-0009). Absent means the
   * search bar takes names only — which is every share artifact, and every
   * `serve` without the flag. */
  onAsk?: AskFn;
}) {
  const [mode, setMode] = useState<Mode>("overview");
  const [grouping, setGrouping] = useState<RegionKind>("structural");
  const [openRegionId, setOpenRegionId] = useState<string | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  // Dismissal is tracked apart from the query, because putting the results
  // away and giving up on the search are different intentions: a reader who
  // clicks the canvas wants to see it, not to retype what they were looking
  // for.
  const [searchDismissed, setSearchDismissed] = useState(false);
  const searchRow = useRef<HTMLDivElement | null>(null);
  const searchInput = useRef<HTMLInputElement | null>(null);
  const [tab, setTab] = useState<Tab>("info");
  const [pathOpen, setPathOpen] = useState(false);
  const [exportOpen, setExportOpen] = useState(false);
  const [pathFrom, setPathFrom] = useState<MapNode | null>(null);
  const [pathTo, setPathTo] = useState<MapNode | null>(null);
  const [showOverlay, setShowOverlay] = useState(false);
  // The codebase tour's position, held here rather than inside its own panel
  // so that starting the walkthrough of the interface can put it back to its
  // starting line. Two walks running at once is the collision story 20 was
  // written to avoid.
  const [tourIndex, setTourIndex] = useState<number | null>(null);
  // The steps are resolved when the reader presses the control, not declared
  // ahead of time: what is on the page is what gets walked, and a page
  // without a question box or a diff overlay is never told about one.
  const [walkthrough, setWalkthrough] = useState<WalkthroughStep[] | null>(
    null,
  );
  const canvas = useRef<ReactFlowInstance<AppFlowNode, FlowEdge> | null>(null);

  const byId = useMemo(() => nodesById(map), [map]);
  const regions = useMemo(() => regionsOf(map, grouping), [map, grouping]);
  const links = useMemo(() => regionLinks(map, regions), [map, regions]);
  const openRegion = regions.find((r) => r.id === openRegionId) ?? null;

  // Which region holds a given node. Symbol nodes have no region of their
  // own; they are reached through the file that contains them, so the file's
  // region is theirs. Same lookup the link counter uses, so the canvas and
  // the counts can never disagree about where a file lives.
  const regionOfPath = useMemo(() => fileOwners(regions), [regions]);

  const fileIdOfPath = useMemo(() => {
    const byPath = new Map<string, string>();
    for (const node of map.nodes) {
      if (node.kind === "file") {
        byPath.set(node.path, node.id);
      }
    }
    return byPath;
  }, [map]);

  // Selecting from anywhere but the canvas — a search hit, an edge, a tour
  // step, a flow step, a ranking row — also brings the node into view. On a
  // canvas of regions that means opening the region holding it first: a
  // highlight nobody can see is not a selection.
  //
  // The move itself is left to the effect below rather than done here. The
  // node being revealed may live in a region that is not open yet, and until
  // React has rendered that region's files the canvas has never heard of it —
  // asking it to move to that node from inside the click handler asks it to
  // find something that does not exist. A fresh object every call, so
  // requesting the same node twice still moves.
  const [focus, setFocus] = useState<{ id: string } | null>(null);
  const reveal = useCallback(
    (id: string) => {
      setSelectedId(id);
      setSearchDismissed(true);
      const node = byId.get(id);
      if (node === undefined) {
        return;
      }
      const region = regionOfPath.get(node.path);
      if (region !== undefined) {
        setOpenRegionId(region);
      }
      // The canvas draws files, so a symbol is shown by its file.
      const target = node.kind === "file" ? node.id : fileIdOfPath.get(node.path);
      if (target !== undefined) {
        setFocus({ id: target });
      }
    },
    [byId, regionOfPath, fileIdOfPath],
  );

  const searchShown = query.trim() !== "" && !searchDismissed;

  // The same field, a second question of it: a name to match, or a question
  // to answer. Which one the reader meant is not guessed from the text —
  // pressing Ask (or Enter) is the difference, so a filename typed by someone
  // who wanted a filename never becomes a request.
  const asking = useAsk(onAsk);
  // Whether pressing Ask right now would send anything: there is a server to
  // ask and something typed to ask it. Not "this server can answer
  // questions" — that is `onAsk` on its own, and conflating the two is how
  // both call sites ended up bolting a second clause onto one name.
  const canSubmit = onAsk !== undefined && query.trim() !== "";
  const askQuestion = () => {
    if (!canSubmit) {
      return;
    }
    // The results list is what the reader was just told about the *name*;
    // leaving it open would cover the answer they asked for.
    setSearchDismissed(true);
    asking.submit(query);
  };

  // Starting the walkthrough is also a tidying-up. Two things follow from it
  // being modal: nothing else may be left open underneath — the cascade has
  // one order, and two layers both claiming to be innermost is how ticket
  // 22's dead zone was built — and the *other* walk in this product, the
  // codebase tour behind the Learn switch, must not be left parked mid-step
  // behind the thing that interrupted it.
  const startWalkthrough = () => {
    setSearchDismissed(true);
    setExportOpen(false);
    setTourIndex(null);
    setWalkthrough(resolveWalkthroughSteps());
  };

  // Overview → region → file is a stack, and back means one step up it, never
  // two. The label names the destination because the same word at three
  // depths would mean three different things, and a reader deciding whether
  // to press it is deciding where they end up.
  const backStep =
    openRegion === null
      ? null
      : selectedId !== null
        ? {
            label: `Back to ${openRegion.name}`,
            go: () => setSelectedId(null),
          }
        : { label: "Back to regions", go: () => setOpenRegionId(null) };

  // Escape is the same gesture without the pointer, and the whole cascade
  // lives here — one listener, on the document, so it fires wherever focus
  // happens to be. Splitting it (the overlay's Escape on the input, the rest
  // on the document) left a hole: tab once from the input and focus lands on
  // a result button, where neither handler was listening and Escape did
  // nothing at all.
  //
  // Order is innermost-first, and it has to be written down because every
  // layer wants the same key: the walkthrough, then the search overlay, then
  // the share/export menu, then the path panel, then the answer to a
  // question, then one step back up the overview → region → file stack.
  //
  // The walkthrough goes first because it is the only layer that is modal
  // over the others rather than beside them — while it runs the rest of the
  // page is inert, so there is nothing else Escape could sensibly mean, and
  // anything it did reach would be a control the reader cannot see the
  // effect of.
  //
  // The answer sits below the two things that pop up *over* the page and
  // above the navigation stack. It is a band the reader deliberately put
  // there and may be working through — following one citation at a time —
  // so anything opened on top of it goes first; but closing a panel is still
  // a smaller undo than moving the canvas, so it goes before the step back.
  //
  // No dependency array on purpose. The handler closes over `searchShown`,
  // `pathOpen`, `backStep` and the asking state, all of which change on
  // almost every render;
  // re-subscribing each time is cheaper than the stale closure that any
  // hand-maintained list here would eventually produce.
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") {
        return;
      }
      if (walkthrough !== null) {
        setWalkthrough(null);
        return;
      }
      if (searchShown) {
        // A `type="search"` input clears itself on Escape in some browsers,
        // and the query is the thing the reader keeps.
        event.preventDefault();
        setSearchDismissed(true);
        searchInput.current?.focus();
        return;
      }
      if (exportOpen) {
        setExportOpen(false);
        return;
      }
      if (pathOpen) {
        setPathOpen(false);
        return;
      }
      if (asking.state.phase !== "idle") {
        asking.dismiss();
        return;
      }
      backStep?.go();
    };
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  });

  // Put the results away when the reader looks somewhere else. Listening on
  // `pointerdown` rather than `click` means the overlay is gone before the
  // press completes, so it never covers what is being aimed at — and because
  // nothing here calls `preventDefault` or `stopPropagation`, the click still
  // reaches whatever was underneath. Closing on `click` instead would work
  // too; closing on `mousedown` *and* consuming the event is the version that
  // makes the first click on a region chip do nothing.
  useEffect(() => {
    if (searchDismissed) {
      return;
    }
    const onPointerDown = (event: PointerEvent) => {
      const target = event.target;
      if (target instanceof Node && searchRow.current?.contains(target)) {
        return;
      }
      setSearchDismissed(true);
    };
    document.addEventListener("pointerdown", onPointerDown);
    return () => document.removeEventListener("pointerdown", onPointerDown);
  }, [searchDismissed]);

  const pathIds = useMemo(() => {
    if (pathFrom === null || pathTo === null) {
      return new Set<string>();
    }
    const found = shortestPath(map, pathFrom.id, pathTo.id);
    return new Set(found?.map((n) => n.id) ?? []);
  }, [map, pathFrom, pathTo]);

  const flow = useMemo(
    () =>
      openRegion === null
        ? regionFlow(regions, links, (region) => regionCaptionOf(region, links))
        : fileFlow(map, openRegion, CARD_HEIGHT, captionOf),
    [map, openRegion, regions, links],
  );

  // Drilling in and back out replaces every node on the canvas. React Flow's
  // `fitView` prop only fires once, at init, so without this the viewport
  // stayed wherever the overview left it and a wide region — this repository's
  // own `crates` is eight thousand pixels across — opened showing its top-left
  // corner. Easing rather than cutting, because the two drawings share no
  // node and a cut between them gives nothing to follow.
  //
  // Declared before the focus effect so that a reveal into a not-yet-open
  // region settles on its node: React runs effects in order, and the second
  // move overrides the first.
  useEffect(() => {
    void canvas.current?.fitView({
      duration: motionDuration(FIT_MS),
      maxZoom: 1.2,
      padding: 0.12,
    });
  }, [flow]);

  useEffect(() => {
    if (focus === null) {
      return;
    }
    void canvas.current?.fitView({
      nodes: [{ id: focus.id }],
      duration: motionDuration(FIT_MS),
      maxZoom: 1.2,
      // Deliberately loose: a single node filling the viewport tells you
      // where it is and nothing about what is around it.
      padding: 4,
    });
  }, [focus]);

  const results = useMemo(() => searchNodes(map, query), [map, query]);
  const selected = useMemo(
    () => (selectedId === null ? null : (byId.get(selectedId) ?? null)),
    [byId, selectedId],
  );

  // The canvas draws files, so it marks the file that holds the selection —
  // choosing a function in the detail panel lights up the file it lives in
  // rather than nothing at all.
  const selectedFileId =
    selected === null
      ? null
      : selected.kind === "file"
        ? selected.id
        : (fileIdOfPath.get(selected.path) ?? null);

  // What the selected file touches on this canvas. A drawing of forty files
  // and eighty-five imports is dense however well it is laid out, so the
  // canvas answers one question at a time: pick a file and its own
  // relationships come forward while the rest of the drawing steps back.
  // Nothing selected means nothing is hidden — the whole region is the
  // answer to "what is in here".
  const neighbours = useMemo(() => {
    const found = new Set<string>();
    if (selectedFileId === null) {
      return found;
    }
    for (const edge of flow.edges) {
      if (edge.source === selectedFileId) {
        found.add(edge.target);
      }
      if (edge.target === selectedFileId) {
        found.add(edge.source);
      }
    }
    return found;
  }, [flow.edges, selectedFileId]);

  // Files the layout parked below the layers because nothing in the region
  // imports them or is imported by them.
  const standalone = useMemo(() => {
    if (openRegion === null) {
      return 0;
    }
    const touched = new Set(flow.edges.flatMap((e) => [e.source, e.target]));
    return openRegion.files.filter((f) => !touched.has(f.id)).length;
  }, [flow.edges, openRegion]);

  const shownEdges = useMemo<FlowEdge[]>(() => {
    if (selectedFileId === null) {
      return flow.edges;
    }
    const [lit, rest]: [FlowEdge[], FlowEdge[]] = [[], []];
    for (const edge of flow.edges) {
      const touches =
        edge.source === selectedFileId || edge.target === selectedFileId;
      const phrase = touches
        ? edgeLabelOf(edge.source === selectedFileId)
        : null;
      (touches ? lit : rest).push({
        ...edge,
        className: touches ? "edge-lit" : "edge-dim",
        ...(phrase === null ? {} : { label: phrase }),
        // Direction is worth ink only on the edges being read: an arrowhead
        // on all eighty-five is more of exactly what makes this unreadable.
        ...(touches
          ? { markerEnd: { type: MarkerType.ArrowClosed, width: 14, height: 14 } }
          : {}),
      });
    }
    // Lit last, so the edges under the question are drawn over the rest.
    return [...rest, ...lit];
  }, [flow.edges, selectedFileId]);

  const shownNodes = useMemo<AppFlowNode[]>(() => {
    // Overlay membership rolled up to files: `codeatlas diff` marks symbols
    // as well as files, and a symbol the canvas does not draw would
    // otherwise take its highlight with it. A file is marked when it or
    // anything it contains is.
    const pathsOf = (ids: string[]) =>
      new Set(
        ids.flatMap((id) => {
          const node = byId.get(id);
          return node === undefined ? [] : [node.path];
        }),
      );
    const changed = pathsOf(showOverlay && overlay ? overlay.changed : []);
    const affected = pathsOf(showOverlay && overlay ? overlay.affected : []);
    return flow.nodes.map((node) => {
      if (node.type === "region") {
        return node.id === regionNodeId(openRegionId ?? "")
          ? { ...node, selected: true }
          : node;
      }
      const path = node.data.node.path;
      // Spread-in rather than assigned: under `exactOptionalPropertyTypes` an
      // explicit `undefined` is not the same as an absent optional field.
      const highlight = changed.has(path)
        ? ("changed" as const)
        : affected.has(path)
          ? ("affected" as const)
          : undefined;
      const neighbour = neighbours.has(node.id);
      return {
        ...node,
        selected: node.id === selectedFileId,
        data: {
          ...node.data,
          node: node.data.node,
          ...(highlight === undefined ? {} : { highlight }),
          ...(pathIds.has(node.id) ? { onPath: true } : {}),
          ...(neighbour ? { neighbour: true } : {}),
          ...(selectedFileId !== null && !neighbour && node.id !== selectedFileId
            ? { dim: true }
            : {}),
        },
      };
    });
  }, [
    byId,
    flow,
    overlay,
    selectedFileId,
    showOverlay,
    openRegionId,
    pathIds,
    neighbours,
  ]);

  return (
    /* `inert` while the walkthrough runs, which is the browser-level
       statement of "this is behind a modal": not focusable, not clickable,
       out of the accessibility tree. The walkthrough itself is portalled to
       the document body for exactly that reason — a dialog inside the thing
       it disables is a dialog nobody can reach. */
    <div className="explorer" inert={walkthrough !== null}>
      <Header
        map={map}
        mode={mode}
        onMode={setMode}
        grouping={grouping}
        onGrouping={(next) => {
          setGrouping(next);
          setOpenRegionId(null);
        }}
        pathOpen={pathOpen}
        onTogglePath={() => setPathOpen(!pathOpen)}
        shared={shared}
        exportOpen={exportOpen}
        onExportOpen={setExportOpen}
        walkthroughOpen={walkthrough !== null}
        onStartWalkthrough={startWalkthrough}
      />

      <div className="searchrow" ref={searchRow} data-walkthrough="search">
        <span className="search-glyph" aria-hidden="true">
          ⌕
        </span>
        <input
          ref={searchInput}
          type="search"
          aria-label="Search nodes"
          placeholder={
            onAsk === undefined
              ? "Search nodes by name, path, or summary…"
              : "Search by name, path, or summary — or ask a question and press Enter"
          }
          value={query}
          onChange={(e) => {
            setQuery(e.target.value);
            setSearchDismissed(false);
          }}
          // Enter is the keyboard half of the Ask button. Without a question
          // backend it does nothing at all, which is the same as before.
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              askQuestion();
            }
          }}
          // A click, not focus. Focus is too ambiguous a signal to reopen on:
          // it fires when Escape restores focus here (which would reopen the
          // list the reader just dismissed, immediately) and when tabbing
          // through on the way somewhere else. Pressing the field, or typing
          // in it, is an intention.
          onClick={() => setSearchDismissed(false)}
        />
        {onAsk !== undefined && (
          <button
            type="button"
            className="ask-button"
            data-walkthrough="ask"
            // Disabled while one question is in flight, so pressing again
            // reads as refused rather than as ignored. The refusal itself is
            // `useAsk.submit`'s — every way of asking meets it there, which
            // a `disabled` attribute on one of two controls cannot do.
            disabled={!canSubmit || asking.state.phase === "asking"}
            title="Answer this question from the map (the local server asks the model)"
            onClick={askQuestion}
          >
            Ask
          </button>
        )}
        {query.trim() !== "" && !searchDismissed && (
          <ul className="search-results" aria-label="Search results">
            {results.slice(0, 40).map((n) => (
              <li key={n.id}>
                <button type="button" onClick={() => reveal(n.id)}>
                  <span className="result-name">{n.name}</span>
                  <span className="result-path">{n.path}</span>
                </button>
              </li>
            ))}
            {results.length === 0 && <li className="no-matches">No matches</li>}
          </ul>
        )}
      </div>

      {/* Below the field it was asked from, above the map it is about, and
          left standing while the reader follows its citations into the
          canvas. */}
      <AnswerPanel
        state={asking.state}
        byId={byId}
        onSelect={reveal}
        onDismiss={asking.dismiss}
      />

      <div className="chiprow" data-walkthrough="regions">
        <span className="chiprow-total">
          {regions.length} {regions.length === 1 ? "region" : "regions"}
        </span>
        {regions.map((region, i) => (
          <button
            key={region.id}
            type="button"
            className={`region-chip${openRegionId === region.id ? " region-chip-on" : ""}`}
            data-accent={i % 6}
            onClick={() =>
              setOpenRegionId(openRegionId === region.id ? null : region.id)
            }
          >
            <span className="region-dot" aria-hidden="true" />
            {region.name} <span className="chip-count">{region.files.length}</span>
          </button>
        ))}
        {overlay && (
          <label className="overlay-toggle" data-walkthrough="diff">
            <input
              type="checkbox"
              aria-label="Diff overlay"
              checked={showOverlay}
              onChange={(e) => setShowOverlay(e.target.checked)}
            />
            Diff overlay
            <span
              className="overlay-counts"
              title="Counts are of nodes, symbols included. The canvas draws files, so it marks the file holding each."
            >
              {overlay.changed.length} changed · {overlay.affected.length}{" "}
              affected nodes
            </span>
          </label>
        )}
      </div>

      <div className="workspace">

        <aside className="rightpanel" data-walkthrough="panel">
          <div className="tabs" role="tablist" aria-label="Detail">
            {(["info", "files"] as const).map((t) => (
              <button
                key={t}
                type="button"
                role="tab"
                aria-selected={tab === t}
                className={`tab${tab === t ? " tab-on" : ""}`}
                onClick={() => setTab(t)}
              >
                {t === "info" ? "Info" : "Files"}
              </button>
            ))}
          </div>

          {pathOpen && (
            <PathFinder
              map={map}
              from={pathFrom}
              to={pathTo}
              onPick={(end, node) =>
                end === "from" ? setPathFrom(node) : setPathTo(node)
              }
              onSelectNode={reveal}
            />
          )}

          {selected && (
            <DetailPanel map={map} node={selected} onSelect={reveal} />
          )}

          {/* The two header switches each do one job, and this is where that
              shows: Overview | Learn chooses what the panel is *for* —
              the facts, or the guided read through them — while
              Domain | Structural only changes how the canvas groups files,
              which the Info panel then describes either way. */}
          {tab === "files" ? (
            <FilesPanel map={map} regions={regions} onSelectNode={reveal} />
          ) : mode === "learn" ? (
            <>
              <TourPanel
                map={map}
                onSelect={reveal}
                index={tourIndex}
                onIndex={setTourIndex}
              />
              <FlowsPanel map={map} onSelect={reveal} />
            </>
          ) : (
            <InfoPanel
              map={map}
              regions={regions}
              links={links}
              onSelectNode={reveal}
              onOpenRegion={setOpenRegionId}
            />
          )}
        </aside>

        <main className="canvas" data-walkthrough="canvas">
          <nav className="breadcrumb" aria-label="Canvas scope">
            {/* Beside the trail rather than instead of it: the breadcrumb
                says where you are, and this says how to leave. Both still
                work, and so does clicking the canvas. */}
            {backStep !== null && (
              <button
                type="button"
                className="back"
                data-testid="back"
                onClick={backStep.go}
              >
                <span aria-hidden="true">←</span> {backStep.label}
              </button>
            )}
            <button
              type="button"
              className={openRegion === null ? "crumb crumb-on" : "crumb"}
              onClick={() => setOpenRegionId(null)}
            >
              Project overview
            </button>
            {openRegion !== null && (
              <>
                <span className="crumb-sep" aria-hidden="true">
                  ›
                </span>
                <span className="crumb crumb-on">{openRegion.name}</span>
                {/* Says what the block of cards below the layers is, so it
                    reads as a decision rather than as leftovers. Counted
                    from the edges actually drawn, so it cannot disagree
                    with the picture it is describing. */}
                <span className="crumb-note">
                  {openRegion.files.length} files
                  {standalone > 0 &&
                    `, ${standalone} importing nothing here`}
                  {selectedFileId === null
                    ? " · click one to trace it"
                    : " · click the canvas to clear"}
                </span>
              </>
            )}
          </nav>
          <ReactFlow
            nodes={shownNodes}
            edges={shownEdges}
            nodeTypes={nodeTypes}
            onInit={(instance) => {
              canvas.current = instance;
            }}
            onNodeClick={(_event, node) => {
              if (node.type === "region") {
                setOpenRegionId(node.data.region.id);
              } else {
                setSelectedId(node.id);
              }
            }}
            /* Clicking empty canvas puts the whole region back. Focus that
               cannot be let go of is a trap, not a feature. */
            onPaneClick={() => setSelectedId(null)}
            fitView
            minZoom={0.05}
            nodesConnectable={false}
          >
            <Background gap={22} size={1} />
            <Controls showInteractive={false} />
            <MiniMap pannable zoomable ariaLabel="Canvas minimap" />
          </ReactFlow>
        </main>
      </div>

      {walkthrough !== null &&
        createPortal(
          <Walkthrough
            steps={walkthrough}
            onClose={() => setWalkthrough(null)}
          />,
          document.body,
        )}
    </div>
  );
}

function DetailPanel({
  map,
  node,
  onSelect,
}: {
  map: KnowledgeGraph;
  node: MapNode;
  onSelect: (id: string) => void;
}) {
  const byId = useMemo(() => nodesById(map), [map]);
  const touching = map.edges.filter(
    (e) => e.source === node.id || e.target === node.id,
  );

  return (
    <section className="detail" aria-label="Node detail">
      <h2>{node.name}</h2>
      <p className="detail-meta">
        <span className="detail-kind">{node.kind}</span>
        <ProvenanceBadge provenance={node.provenance} />
      </p>
      <p className="detail-path">{node.path}</p>
      {node.range && (
        <p className="detail-range">
          lines {node.range.start_line}–{node.range.end_line}
        </p>
      )}
      <p className="detail-summary">{node.summary}</p>
      {/* What the summary cannot say without enrichment: where this sits and
          what it touches, in sentences rather than a fan-in/fan-out pair. */}
      <Narrative map={map} node={node} />
      <h3>Edges</h3>
      {touching.length === 0 ? (
        <p className="no-edges">No edges</p>
      ) : (
        <ul className="edge-list" aria-label="Edges">
          {touching.map((e) => {
            const outgoing = e.source === node.id;
            const otherId = outgoing ? e.target : e.source;
            const otherName = byId.get(otherId)?.name ?? otherId;
            return (
              <li key={`${e.source}--${e.kind}--${e.target}`}>
                <button type="button" onClick={() => onSelect(otherId)}>
                  {outgoing
                    ? `${e.kind} → ${otherName}`
                    : `← ${e.kind} ${otherName}`}
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}
