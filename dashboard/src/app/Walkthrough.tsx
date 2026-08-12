// The walkthrough of the application (story 20): one control of the live page
// lit at a time, everything else dimmed, and a sentence saying what the lit
// thing does. See `walkthrough.ts` for what the steps are and why they are
// declared rather than derived.
//
// Two components, because they have nothing to do with each other. The
// launcher lives in the header and is about *offering* the walkthrough; the
// overlay is the walkthrough. The explorer owns the boolean between them, so
// that `Escape` can close this in the same one cascade as everything else that
// opens (ticket 22) and so that starting it can put the codebase tour back to
// its starting line.
import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
} from "react";
import { useFocusReturn } from "./focus.js";
import { scrollBehaviour } from "./motion.js";
import {
  hasSeenWalkthrough,
  markWalkthroughSeen,
  WALKTHROUGH_LIT,
  walkthroughTarget,
  type WalkthroughStep,
} from "./walkthrough.js";

/** Everything focusable inside the card, in document order. The trap below
 * cycles through exactly this, which is what makes "the rest of the page is
 * unreachable" a property a test can drive rather than an attribute it can
 * read. */
const FOCUSABLE = "button:not([disabled]), [href], input, select, textarea";

/** How far the card sits from the element it is describing, and from the
 * edges of the window. */
const CARD_GAP = 14;

type Geometry = {
  top: number;
  left: number;
  width: number;
  height: number;
};

/** Keeps `value` inside `[min, max]`, preferring `min` where the range has
 * collapsed — on a window too small to hold the card at all, against the
 * near edge is a better answer than off the far one. */
function clampTo(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), Math.max(min, max));
}

function geometryOf(element: HTMLElement): Geometry {
  const rect = element.getBoundingClientRect();
  return {
    top: rect.top,
    left: rect.left,
    width: rect.width,
    height: rect.height,
  };
}

/**
 * The header's offer of the walkthrough.
 *
 * The control is always there; the callout beside it is shown once. A modal
 * that opens by itself on first load is the thing this deliberately is not —
 * but a walkthrough nobody knows about explains nothing, so the first visit
 * gets a sentence and two ways to answer it, and either answer is remembered.
 */
export function WalkthroughLauncher({
  open,
  onStart,
}: {
  open: boolean;
  onStart: () => void;
}) {
  const [offered, setOffered] = useState(hasSeenWalkthrough);
  // Closing the walkthrough must not drop focus on the floor, and where it
  // goes is the same rule the export menu obeys — see `focus.ts`.
  const button = useFocusReturn<HTMLButtonElement>(open);

  const start = () => {
    setOffered(true);
    markWalkthroughSeen();
    onStart();
  };

  return (
    <div className="walkthrough-launch" data-walkthrough="walkthrough">
      <button
        type="button"
        ref={button}
        className={`topbar-button${open ? " topbar-button-on" : ""}`}
        title="A short walkthrough of this interface — what each control is for"
        onClick={start}
      >
        Walkthrough
      </button>

      {!offered && !open && (
        <div className="walkthrough-callout" role="note" aria-label="First visit">
          <p>New here? A short walkthrough names each control on this page.</p>
          <div className="walkthrough-callout-actions">
            <button type="button" className="topbar-button" onClick={start}>
              Show me around
            </button>
            <button
              type="button"
              className="topbar-button"
              onClick={() => {
                setOffered(true);
                markWalkthroughSeen();
              }}
            >
              Not now
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

/**
 * The walkthrough itself: a modal over the whole page, with a hole cut in it.
 *
 * Rendered as a sibling of the explorer rather than inside it, because the
 * explorer is what it makes inert — a dialog inside the thing it disables is
 * a dialog nobody can reach.
 */
export function Walkthrough({
  steps,
  onClose,
}: {
  /** Already filtered to the controls this page actually has. */
  steps: readonly WalkthroughStep[];
  onClose: () => void;
}) {
  const [index, setIndex] = useState(0);
  const [geometry, setGeometry] = useState<Geometry | null>(null);
  const [cardSize, setCardSize] = useState<{ width: number; height: number } | null>(
    null,
  );
  const card = useRef<HTMLDivElement | null>(null);
  const step = steps[Math.min(index, steps.length - 1)];

  // Measure, light, and keep both true. Everything in this effect hangs off
  // the step's element, so it is torn down and rebuilt on every step: the
  // previous element loses its highlight and stops being watched at the same
  // moment the next one gains both.
  //
  // *Following a reflow* is the part worth spelling out. A spotlight cut at a
  // stale rectangle is worse than no spotlight, and the rectangle can go stale
  // for exactly three reasons while this is on screen — the page behind is
  // inert, so nothing else can move underneath it. The window resizes, which
  // both the observer on the root element and `resize` see; the element itself
  // changes size, which its own observer sees; or the page scrolls, which
  // `scroll` sees in the capture phase, wherever the scrolling happened.
  useLayoutEffect(() => {
    if (step === undefined) {
      return;
    }
    const target = walkthroughTarget(step.id);
    if (target === null) {
      return;
    }
    target.setAttribute(WALKTHROUGH_LIT, "");
    const measure = () => setGeometry(geometryOf(target));
    measure();

    // Not every step is above the fold — the panel and the canvas are a
    // scroll away on a short viewport — and a spotlight on something off
    // screen is a dimmed page with nothing in the hole.
    if (typeof target.scrollIntoView === "function") {
      target.scrollIntoView({ block: "nearest", behavior: scrollBehaviour() });
    }

    const observer =
      typeof ResizeObserver === "function" ? new ResizeObserver(measure) : null;
    observer?.observe(target);
    observer?.observe(document.documentElement);
    window.addEventListener("resize", measure);
    window.addEventListener("scroll", measure, true);
    return () => {
      target.removeAttribute(WALKTHROUGH_LIT);
      observer?.disconnect();
      window.removeEventListener("resize", measure);
      window.removeEventListener("scroll", measure, true);
    };
  }, [step]);

  // How big the card actually came out. Measured rather than estimated,
  // because the two things it decides — whether the card fits below the lit
  // element, and how far it can be pushed towards an edge before it leaves
  // the window — are both wrong by however wrong the estimate is, and one
  // step's prose is twice the length of another's.
  //
  // This cannot oscillate. The card's width is a `min()` against the viewport
  // and its height follows from that width and the step's text; neither reads
  // the position this computes, so a measurement never changes what is being
  // measured. It is a *layout* effect so the corrected position is in place
  // before the browser paints — a card that flashed at the wrong offset on
  // every step would be worse than the bug it fixes.
  useLayoutEffect(() => {
    const element = card.current;
    if (element === null) {
      return;
    }
    const measure = () => {
      const rect = element.getBoundingClientRect();
      setCardSize((previous) =>
        previous !== null &&
        previous.width === rect.width &&
        previous.height === rect.height
          ? previous
          : { width: rect.width, height: rect.height },
      );
    };
    measure();
    const observer =
      typeof ResizeObserver === "function" ? new ResizeObserver(measure) : null;
    observer?.observe(element);
    window.addEventListener("resize", measure);
    return () => {
      observer?.disconnect();
      window.removeEventListener("resize", measure);
    };
  }, [step]);

  // Focus lands on the step's text, not on the button that produced it, so a
  // screen reader reads the explanation. On every step, because a walkthrough
  // whose second step is announced by silence is a walkthrough for one reader.
  useEffect(() => {
    card.current?.focus();
  }, [index]);

  if (step === undefined) {
    return null;
  }

  const last = index === steps.length - 1;

  // Below the lit element where there is room, above it where there is not,
  // and inside the window either way. The card is `position: fixed`, so all of
  // this is stated against the viewport.
  //
  // Both axes are clamped, and both clamps are the same shape, because the bug
  // was the same on each: a preference for where the card *should* go with no
  // statement of where it may not. `.walkthrough-card` caps the card at the
  // window less two gaps in both dimensions, which is what makes these ranges
  // non-empty — the card can always be placed somewhere legal.
  const viewportWidth = typeof window === "undefined" ? 1280 : window.innerWidth;
  const viewportHeight = typeof window === "undefined" ? 800 : window.innerHeight;
  const cardWidth = cardSize?.width ?? 0;
  const cardHeight = cardSize?.height ?? 0;

  let placement: CSSProperties;
  if (geometry === null) {
    placement = { top: `${CARD_GAP}px`, left: `${CARD_GAP}px` };
  } else {
    const below =
      geometry.top + geometry.height + CARD_GAP + cardHeight <=
      viewportHeight - CARD_GAP;
    const wanted = below
      ? geometry.top + geometry.height + CARD_GAP
      : geometry.top - CARD_GAP - cardHeight;
    placement = {
      left: `${clampTo(geometry.left, CARD_GAP, viewportWidth - cardWidth - CARD_GAP)}px`,
      top: `${clampTo(wanted, CARD_GAP, viewportHeight - cardHeight - CARD_GAP)}px`,
    };
  }

  return (
    <div
      className="walkthrough"
      role="dialog"
      aria-modal="true"
      aria-label="Interface walkthrough"
      // The keyboard half of "the rest of the page is inert". `inert` on the
      // explorer is the declaration a browser acts on; this is the one that
      // holds where `inert` is unimplemented, and it is also what a test can
      // drive — tab round and see where focus went. Escape is deliberately
      // *not* here: it belongs to the explorer's single document-level
      // cascade, and a second handler is how ticket 22's dead zone reopens.
      onKeyDown={(event) => {
        if (event.key !== "Tab") {
          return;
        }
        const focusable = [
          ...(card.current?.querySelectorAll<HTMLElement>(FOCUSABLE) ?? []),
        ];
        if (focusable.length === 0) {
          return;
        }
        event.preventDefault();
        const at = focusable.indexOf(document.activeElement as HTMLElement);
        const next = event.shiftKey
          ? at <= 0
            ? focusable.length - 1
            : at - 1
          : at === -1 || at === focusable.length - 1
            ? 0
            : at + 1;
        focusable[next]?.focus();
      }}
    >
      {geometry !== null && (
        <div
          className="walkthrough-spotlight"
          aria-hidden="true"
          style={{
            top: `${geometry.top}px`,
            left: `${geometry.left}px`,
            width: `${geometry.width}px`,
            height: `${geometry.height}px`,
          }}
        />
      )}

      <div className="walkthrough-card" tabIndex={-1} ref={card} style={placement}>
        <div className="walkthrough-head">
          <p className="walkthrough-progress">
            Step {index + 1} of {steps.length}
          </p>
          <button
            type="button"
            className="walkthrough-dismiss"
            aria-label="Close walkthrough"
            title="Close walkthrough (Escape)"
            onClick={onClose}
          >
            <span aria-hidden="true">×</span>
          </button>
        </div>
        <h2>{step.title}</h2>
        <p className="walkthrough-body">{step.body}</p>
        <div className="walkthrough-controls">
          <button
            type="button"
            className="topbar-button"
            disabled={index === 0}
            onClick={() => setIndex(index - 1)}
          >
            Back
          </button>
          {/* The last step's forward control is the way out, so the walk never
              ends on a press that does nothing. */}
          <button
            type="button"
            className="topbar-button"
            onClick={() => (last ? onClose() : setIndex(index + 1))}
          >
            {last ? "Done" : "Next"}
          </button>
        </div>
      </div>
    </div>
  );
}
