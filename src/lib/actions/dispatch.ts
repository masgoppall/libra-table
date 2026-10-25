// Views register action handlers in layers: dialogs, the active tab, its workspace,
// then the app. An action runs the first handler that accepts it.

import { useEffect, useRef } from "react";
import { type ActionId } from "./shortcuts";

/** Returns false to let the next layer (or the browser) handle the action. */
export type Handler = () => boolean | void;
export type Handlers = Partial<Record<ActionId, Handler>>;

export const LAYER = { app: 0, workspace: 1, tab: 2, dialog: 3 } as const;
export type LayerPriority = (typeof LAYER)[keyof typeof LAYER];

export type ActionSource = "key" | "menu" | "ui";

interface Layer {
  seq: number;
  priority: number;
  handlers: { current: Handlers };
}

/** A key the page handled can still reach the native menu within this window. */
const DUPLICATE_MS = 150;

let layers: Layer[] = [];
let seq = 0;
let last: { id: ActionId; at: number; source: ActionSource } | null = null;

function isDuplicate(id: ActionId, source: ActionSource, now: number): boolean {
  return !!last && last.id === id && last.source !== source && now - last.at < DUPLICATE_MS;
}

/** Runs the action; true when a handler took it. */
export function dispatch(id: ActionId, source: ActionSource = "ui"): boolean {
  const now = Date.now();
  if (isDuplicate(id, source, now)) return true;
  const ordered = [...layers].sort((a, b) => b.priority - a.priority || b.seq - a.seq);
  for (const layer of ordered) {
    const handler = layer.handlers.current[id];
    if (handler && handler() !== false) {
      last = { id, at: now, source };
      return true;
    }
  }
  return false;
}

/** Registers handlers while `active`. The latest handlers are always used. */
export function useActions(priority: LayerPriority, active: boolean, handlers: Handlers) {
  const ref = useRef(handlers);
  ref.current = handlers;
  useEffect(() => {
    if (!active) return;
    const layer: Layer = { seq: ++seq, priority, handlers: ref };
    layers.push(layer);
    return () => {
      layers = layers.filter((l) => l !== layer);
    };
  }, [active, priority]);
}

/** Handlers for ⌘1…⌘9 from a function of the tab number (1-based). */
export function tabNumberHandlers(pick: (n: number) => boolean | void): Handlers {
  const entries = Array.from({ length: 9 }, (_, i) => [`tab-${i + 1}`, () => pick(i + 1)]);
  return Object.fromEntries(entries) as Handlers;
}

/** Test helper. */
export function resetActions() {
  layers = [];
  last = null;
}
