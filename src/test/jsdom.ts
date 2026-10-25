// Fills the gaps of jsdom that the app relies on. Call once per test file.

import { configure } from "@testing-library/react";
import { beforeAll } from "vitest";

/** Lazily loaded tabs (editor, diagram, metrics) can take over a second on a busy machine. */
const ASYNC_TIMEOUT_MS = 5000;

const SIZE = { width: 1000, height: 600 };

function fixedSize(prop: string, value: number) {
  Object.defineProperty(HTMLElement.prototype, prop, { configurable: true, get: () => value });
}

export function installJsdomShims() {
  configure({ asyncUtilTimeout: ASYNC_TIMEOUT_MS });
  beforeAll(() => {
    // The virtualized grid needs a viewport size; jsdom reports 0.
    fixedSize("offsetHeight", SIZE.height);
    fixedSize("offsetWidth", SIZE.width);
    fixedSize("clientHeight", SIZE.height);
    fixedSize("clientWidth", SIZE.width);
    const rect = { x: 0, y: 0, top: 0, left: 0, right: SIZE.width, bottom: SIZE.height, ...SIZE };
    HTMLElement.prototype.getBoundingClientRect = () =>
      ({ ...rect, toJSON: () => ({}) }) as DOMRect;
    HTMLElement.prototype.scrollIntoView = () => undefined;
    globalThis.ResizeObserver = class {
      observe() {}
      unobserve() {}
      disconnect() {}
    };
    window.matchMedia = ((media: string) => ({
      matches: false,
      media,
      addEventListener() {},
      removeEventListener() {},
    })) as unknown as typeof window.matchMedia;
    const clipboard = { writeText: async () => undefined };
    Object.defineProperty(navigator, "clipboard", { configurable: true, value: clipboard });
  });
}
