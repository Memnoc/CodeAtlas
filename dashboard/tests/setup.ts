// jsdom lacks the layout APIs React Flow measures with. These mocks are the
// ones @xyflow/react's own testing guide prescribes — just enough for the
// canvas to mount and render nodes in tests.
import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

// Vitest globals are off, so testing-library's automatic cleanup never
// registers itself; do it explicitly.
afterEach(cleanup);

// A no-op observer suffices: the app gives every flow node explicit
// width/height, so React Flow never needs a real measurement.
class ResizeObserverMock {
  observe() {}
  unobserve() {}
  disconnect() {}
}

globalThis.ResizeObserver =
  ResizeObserverMock as unknown as typeof ResizeObserver;

class DOMMatrixReadOnlyMock {
  m22: number;

  constructor(transform?: string) {
    const scale = transform?.match(/scale\(([\d.]+)\)/)?.[1];
    this.m22 = scale === undefined ? 1 : +scale;
  }
}

globalThis.DOMMatrixReadOnly =
  DOMMatrixReadOnlyMock as unknown as typeof DOMMatrixReadOnly;

Object.defineProperties(globalThis.HTMLElement.prototype, {
  offsetHeight: {
    get(this: HTMLElement) {
      return parseFloat(this.style.height) || 1;
    },
  },
  offsetWidth: {
    get(this: HTMLElement) {
      return parseFloat(this.style.width) || 1;
    },
  },
});

(
  globalThis.SVGElement.prototype as SVGElement & { getBBox: () => DOMRect }
).getBBox = () =>
  ({ x: 0, y: 0, width: 0, height: 0, top: 0, left: 0, right: 0, bottom: 0 }) as DOMRect;
