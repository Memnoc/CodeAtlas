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
import { type Chrome, readChrome, writeChrome } from "./chrome.js";
import { FilesPanel } from "./FilesPanel.js";
import { FlowsPanel } from "./FlowsPanel.js";
import { useFocusReturn } from "./focus.js";
import {
  DRILL_DEFAULT_CARDS,
  fileFlow,
  hiddenByDefault,
  magnifyFlow,
  neighbourhoodOf,
  type AppFlowNode,
  nodesById,
  regionFlow,
  regionNodeId,
  regionsHiding,
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
  // What the reader folded away last time, so a preference for a big canvas
  // survives a reload. Written on every change rather than on unmount, which
  // a closed tab never reaches.
  const [chrome, setChromeState] = useState<Chrome>(readChrome);
  const setChrome = useCallback((next: Chrome) => {
    setChromeState(next);
    writeChrome(next);
  }, []);
  const [openRegionId, setOpenRegionId] = useState<string | null>(null);
  // Which regions the reader has asked to see in full. Held here and handed
  // to the projection as an argument, never kept inside it: the same map in
  // the same state has to draw the same picture. Region-scoped and not
  // written to storage — reading a region in full is a thing someone does
  // once, deliberately, not a preference they carry between repositories.
  const [revealed, setRevealed] = useState<ReadonlySet<string>>(
    () => new Set<string>(),
  );
  const toggleRevealed = useCallback((id: string) => {
    setRevealed((current) => {
      const next = new Set(current);
      if (!next.delete(id)) {
        next.add(id);
      }
      return next;
    });
  }, []);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  // The magnify lens: the one file whose neighbourhood the canvas draws
  // instead of the open region, or null for no lens. Deliberately the only
  // state entering magnify touches — the view underneath (open region,
  // revealed set, selection, grouping) is never written, so leaving is
  // setting this back to null and there is nothing to restore. That is what
  // makes it a lens rather than a navigation step.
  const [magnifiedId, setMagnifiedId] = useState<string | null>(null);
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
  // The magnified set, computed once and read twice: the projection draws
  // it, and `reveal` asks it whether a pointed-at file is on the lens. One
  // computation, so the two can never disagree about what the lens holds.
  const magnifiedSet = useMemo(
    () => (magnifiedId === null ? null : neighbourhoodOf(map, magnifiedId)),
    [map, magnifiedId],
  );
  const magnified =
    magnifiedId === null ? null : (byId.get(magnifiedId) ?? null);
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

  // The card the canvas would draw for a node: the file itself, or the file
  // that contains it. The canvas draws files, so a symbol is pointed at
  // through the file holding it.
  const fileIdOf = useCallback(
    (id: string): string | undefined => {
      const node = byId.get(id);
      if (node === undefined) {
        return undefined;
      }
      return node.kind === "file" ? node.id : fileIdOfPath.get(node.path);
    },
    [byId, fileIdOfPath],
  );

  // Auto-reveal: anything that points at a specific file reveals that file
  // first. Since the default drill view holds all but the most significant
  // forty back, a search hit, a focused file, a tour stop or a diff mark can
  // each name a file that is not on the canvas — and each would then fail
  // silently, which is the worst way to fail, because the reader concludes
  // the feature is broken or that the file is fine.
  //
  // It writes the set the "show all" control writes, and nothing else: one
  // mechanism, so there is no second reveal path to grow its own bugs, and
  // the reader can put back what a pointer opened using the control already
  // in front of them. Additive only — a region the reader opened in full
  // stays open, and a region a pointer opened is not closed again under them
  // while they are still reading it.
  const autoReveal = useCallback(
    (fileIds: ReadonlySet<string>) => {
      const hiding = regionsHiding(regions, fileIds);
      if (hiding.size === 0) {
        return;
      }
      setRevealed((current) => {
        // Same set back when there is nothing to add, so a pointer at an
        // already-revealed region is not a state change and cannot become a
        // render loop when an effect is what is pointing.
        if ([...hiding].every((id) => current.has(id))) {
          return current;
        }
        const next = new Set(current);
        for (const id of hiding) {
          next.add(id);
        }
        return next;
      });
    },
    [regions],
  );

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
      const target = fileIdOf(id);
      if (target !== undefined) {
        // A pointer while the magnify lens is up: at a file the lens draws,
        // the lens stays — the card is right there to select, and the
        // reveal below still runs so that leaving finds it drawn rather
        // than put back behind the cut. At anything else the lens comes
        // down, because the pointer's destination is a canvas the lens is
        // covering, and a highlight nobody can see is not a selection.
        if (magnifiedSet !== null && !magnifiedSet.has(target)) {
          setMagnifiedId(null);
        }
        // Before the move, not after: the camera is being sent to this card,
        // and a card the default view is holding back is not there to be
        // moved to.
        autoReveal(new Set([target]));
        setFocus({ id: target });
      }
    },
    [byId, regionOfPath, fileIdOf, autoReveal, magnifiedSet],
  );

  const searchShown = query.trim() !== "" && !searchDismissed;

  // The same field, a second question of it: a name to match, or a question
  // to answer. Which one the reader meant is not guessed from the text —
  // pressing Ask (or Enter) is the difference, so a filename typed by someone
  // who wanted a filename never becomes a request.
  const asking = useAsk(onAsk);
  // The focus-return discipline (ticket 17): the conversation column can
  // hold focus — the dismiss control, a citation — and closing it would
  // otherwise strand the keyboard on `<body>`. The search box is the
  // control the column was opened from, and the place a fresh question
  // starts, so focus goes back there; the hook's own guard leaves a reader
  // who parked focus elsewhere exactly where they put it.
  const askReturn = useFocusReturn<HTMLInputElement>(
    asking.state.phase !== "idle",
  );
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

  // Overview → region → file → lens is a stack, and back means one step up
  // it, never two. The label names the destination because the same word at
  // these depths would mean different things, and a reader deciding whether
  // to press it is deciding where they end up.
  //
  // The lens is the innermost rung, which is also how Escape leaves it:
  // through the cascade's existing last step, never a second handler. It
  // steps back to the view it was opened over — selection intact, because
  // entering never wrote anything else to restore.
  const backStep =
    magnified !== null
      ? {
          label:
            openRegion === null
              ? "Back to regions"
              : `Back to ${openRegion.name}`,
          go: () => setMagnifiedId(null),
        }
      : openRegion === null
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
  // question, then one step back up the overview → region → file → lens
  // stack.
  //
  // The walkthrough goes first because it is the only layer that is modal
  // over the others rather than beside them — while it runs the rest of the
  // page is inert, so there is nothing else Escape could sensibly mean, and
  // anything it did reach would be a control the reader cannot see the
  // effect of.
  //
  // The answer sits below the two things that pop up *over* the page and
  // above the navigation stack. It is a column the reader deliberately put
  // beside the canvas and may be working through — following one citation
  // at a time — so anything opened on top of it goes first; but closing a
  // panel is still a smaller undo than moving the canvas, so it goes before
  // the step back. Moving it into the workspace (ticket 17) changed none of
  // this: the rung is about what dismissal costs, not where the panel docks.
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

  // The lens outranks both ordinary views: while it is up the canvas draws
  // the neighbourhood and nothing else, and the view it covers keeps its
  // state untouched underneath.
  const flow = useMemo(
    () =>
      magnifiedSet !== null
        ? magnifyFlow(map, magnifiedSet, CARD_HEIGHT, captionOf)
        : openRegion === null
          ? regionFlow(regions, links, (region) => regionCaptionOf(region, links))
          : fileFlow(map, openRegion, CARD_HEIGHT, captionOf, revealed),
    [map, magnifiedSet, openRegion, regions, links, revealed],
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

  // The diff overlay points at files too — every file it marks — and it does
  // so all over the map at once rather than one at a time, so it reveals all
  // over the map: a region the reader has not drilled into yet is already
  // open in full by the time they get there. Marks roll up to files first,
  // because `codeatlas diff` marks symbols as well and the canvas draws
  // files.
  //
  // An effect rather than the toggle's own handler, because the marks have to
  // outlive the other things that rebuild the revealed set: switching the
  // grouping clears it, and an overlay still switched on would go straight
  // back to marking cards nobody can see.
  useEffect(() => {
    if (!showOverlay || !overlay) {
      return;
    }
    const marked = new Set<string>();
    for (const id of [...overlay.changed, ...overlay.affected]) {
      const file = fileIdOf(id);
      if (file !== undefined) {
        marked.add(file);
      }
    }
    autoReveal(marked);
  }, [showOverlay, overlay, fileIdOf, autoReveal]);

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

  // The magnify control names its subject, and the subject is the file —
  // selecting a symbol magnifies the file that contains it, the same
  // roll-up the canvas marking makes.
  const selectedFile =
    selectedFileId === null ? null : (byId.get(selectedFileId) ?? null);

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
  // imports them or is imported by them. Counted off the cards actually
  // drawn rather than off the region's whole list: a file the default view
  // is holding back is not "importing nothing here", it is simply not on the
  // canvas, and counting it as the former would describe a picture nobody is
  // looking at.
  const standalone = useMemo(() => {
    if (openRegion === null) {
      return 0;
    }
    const touched = new Set(flow.edges.flatMap((e) => [e.source, e.target]));
    return flow.nodes.filter((n) => !touched.has(n.id)).length;
  }, [flow, openRegion]);

  // What the drill view is holding back, if anything — asked of the
  // projection rather than worked out here, so the control and the canvas
  // read one rule. Unchanged by revealing: it is what the *default* view
  // puts away, which is what the way back has to offer.
  const hidden = openRegion === null ? 0 : hiddenByDefault(openRegion);

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
          // The other grouping's regions are different regions, and the two
          // draw their IDs from the same well — a layer and a domain can
          // both be called `crates`. Carrying a reveal across would open a
          // region the reader never asked to see in full.
          setRevealed(new Set<string>());
          // And the selection goes with it. Auto-reveal is a one-shot call
          // made by whatever pointed at the file — a search hit, a tour
          // stop, a row in a panel — not a standing instruction, so the
          // reveal that put the selected card on the canvas does not
          // re-apply under the new grouping. Keeping the selection while
          // dropping both the open region and the reveal that served it is
          // the asymmetry: the reader drills back down and finds the detail
          // panel describing a card the canvas is not drawing, which is
          // story 3's failure arriving by the back door. Letting go is the
          // honest half — the switch already discards where the reader was
          // standing, and this makes it discard what they were pointed at
          // too, rather than re-opening a region they never asked to see in
          // full on the strength of a selection made under another grouping.
          setSelectedId(null);
          // And the lens goes with the selection it was ground over: a
          // neighbourhood magnified under one grouping is not something the
          // other grouping's reader asked to be looking through.
          setMagnifiedId(null);
        }}
        pathOpen={pathOpen}
        onTogglePath={() => setPathOpen(!pathOpen)}
        chrome={chrome}
        onChrome={setChrome}
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
          // Two refs on one field: the Escape cascade's own handle, and the
          // focus-return target the conversation column restores to.
          ref={(el) => {
            searchInput.current = el;
            askReturn.current = el;
          }}
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

      <div className="chiprow" data-walkthrough="regions">
        {/* The fold control is the row's own, and it stays behind when the
            chips go — a row that folded itself away entirely would take the
            only way back with it. */}
        <button
          type="button"
          className="chiprow-fold"
          aria-expanded={!chrome.chips}
          title={
            chrome.chips
              ? "Show the regions"
              : "Fold the regions away and give the row to the map"
          }
          onClick={() => setChrome({ ...chrome, chips: !chrome.chips })}
        >
          <span aria-hidden="true">{chrome.chips ? "▸" : "▾"}</span>
          <span className="chiprow-total">
            {regions.length} {regions.length === 1 ? "region" : "regions"}
          </span>
        </button>
        {!chrome.chips &&
          regions.map((region, i) => (
            <button
              key={region.id}
              type="button"
              className={`region-chip${openRegionId === region.id ? " region-chip-on" : ""}`}
              data-accent={i % 6}
              onClick={() => {
                // Moving between regions is leaving the lens: the chip is
                // about to change the very canvas the lens is covering.
                setMagnifiedId(null);
                setOpenRegionId(openRegionId === region.id ? null : region.id);
              }}
            >
              <span className="region-dot" aria-hidden="true" />
              {region.name}{" "}
              <span className="chip-count">{region.files.length}</span>
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

      <div className={`workspace${chrome.panel ? " workspace-folded" : ""}`}>
        {/* Folded, the panel is a rail holding one control: the way back. The
            panel itself is unmounted rather than hidden, so the walkthrough
            skips its step instead of spotlighting a hole of no size — the
            same thing it already does for a control a given page lacks. */}
        {chrome.panel ? (
          <div className="panelrail">
            <button
              type="button"
              className="panelrail-open"
              aria-expanded={false}
              title="Show the side panel"
              onClick={() => setChrome({ ...chrome, panel: false })}
            >
              <span aria-hidden="true">›</span>
              <span className="panelrail-label">Panel</span>
            </button>
          </div>
        ) : (
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
            <button
              type="button"
              className="tabs-fold"
              aria-expanded
              aria-label="Hide the side panel"
              title="Fold the side panel away and give its width to the map"
              onClick={() => setChrome({ ...chrome, panel: true })}
            >
              <span aria-hidden="true">‹</span>
            </button>
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
        )}

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
              onClick={() => {
                // Jumping to the overview is leaving the lens too — same
                // reason as the region chips.
                setMagnifiedId(null);
                setOpenRegionId(null);
              }}
            >
              Project overview
            </button>
            {openRegion !== null && (
              <>
                <span className="crumb-sep" aria-hidden="true">
                  ›
                </span>
                {magnified === null ? (
                  <>
                    <span className="crumb crumb-on">{openRegion.name}</span>
                    {/* Says what the block of cards below the layers is, so
                        it reads as a decision rather than as leftovers.
                        Counted from the edges actually drawn, so it cannot
                        disagree with the picture it is describing. */}
                    <span className="crumb-note">
                      {openRegion.files.length} files
                      {standalone > 0 &&
                        `, ${standalone} importing nothing here`}
                      {selectedFileId === null
                        ? " · click one to trace it"
                        : " · click the canvas to clear"}
                    </span>
                    {/* Disclosure, not a filter: the default view draws the
                        files the map says carry this region, and this is the
                        one gesture that puts the rest on the canvas. It
                        names both true numbers — the region's size and what
                        is being held back — so nobody has to work out what
                        they are asking for. Absent entirely on a region the
                        default view already draws whole. */}
                    {hidden > 0 && (
                      <button
                        type="button"
                        className="reveal"
                        title={
                          revealed.has(openRegion.id)
                            ? `Draw the ${DRILL_DEFAULT_CARDS} files the map says carry this region, and put the rest away`
                            : "Draw every file in this region, however dense the picture gets"
                        }
                        onClick={() => toggleRevealed(openRegion.id)}
                      >
                        {revealed.has(openRegion.id)
                          ? `Show the top ${DRILL_DEFAULT_CARDS}`
                          : `Show all ${openRegion.files.length} files (${hidden} hidden)`}
                      </button>
                    )}
                  </>
                ) : (
                  <>
                    <span className="crumb">{openRegion.name}</span>
                    <span className="crumb-sep" aria-hidden="true">
                      ›
                    </span>
                    <span className="crumb crumb-on">{magnified.name}</span>
                    {/* What the lens is drawing — or, for a file that
                        touches nothing, why one card is the whole picture
                        rather than an empty canvas. */}
                    <span className="crumb-note">
                      {flow.nodes.length === 1
                        ? "imports nothing and nothing imports it — drawn alone"
                        : `${flow.nodes.length - 1} neighbour${
                            flow.nodes.length === 2 ? "" : "s"
                          } — what it leans on below, what leans on it above`}
                    </span>
                  </>
                )}
                {/* The lens's way in, and its next hop: magnify the selected
                    file, or — already magnified — the neighbour just
                    selected. Absent while it would only redraw the lens
                    already up. */}
                {selectedFile !== null && selectedFileId !== magnifiedId && (
                  <button
                    type="button"
                    className="reveal"
                    title="Draw only this file and its direct neighbours — the files it imports and the files that import it"
                    onClick={() => setMagnifiedId(selectedFileId)}
                  >
                    Magnify {selectedFile.name}
                  </button>
                )}
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
              } else if (magnified !== null) {
                // On the lens a card may be one the view underneath holds
                // back, or belong to another region entirely — so the click
                // is a pointer at a possibly-hidden file and takes the
                // pointer path every other feature takes, not the bare
                // selection the drill view can afford.
                reveal(node.id);
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

        {/* The conversation, docked beside the map it is about (ticket 17):
            a bounded column, so the thread scrolls internally while the
            canvas keeps the remainder — and a citation click lights a card
            the reader can actually see. Renders nothing until a question
            has been asked, and the workspace's `auto` track collapses with
            it. */}
        <AnswerPanel
          state={asking.state}
          byId={byId}
          onSelect={reveal}
          onDismiss={asking.dismiss}
        />
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
