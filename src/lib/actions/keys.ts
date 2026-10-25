// Key labels ("⇧⌘K") and matching key presses to actions by physical key.

import { type ActionId, EXTRA_BINDINGS, SHORTCUTS } from "./shortcuts";

export const IS_MAC =
  typeof navigator !== "undefined" &&
  /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);

const MAC_KEYS: Record<string, string> = { Enter: "↵", Tab: "⇥", Backspace: "⌫", Escape: "esc" };
const PC_KEYS: Record<string, string> = { Escape: "Esc" };
const MAC_MODS: Record<string, string> = { Ctrl: "⌃", Alt: "⌥", Shift: "⇧", Mod: "⌘" };
const MAC_MOD_ORDER = ["Ctrl", "Alt", "Shift", "Mod"];

function splitKeys(keys: string): { mods: string[]; key: string } {
  // "Mod+=" splits into ["Mod", "="]; "Mod++" isn't used.
  const parts = keys.split("+").map((p) => (p === "" ? "+" : p));
  return { mods: parts.slice(0, -1), key: parts[parts.length - 1] ?? "" };
}

/** "Mod+Shift+K" → "⇧⌘K" on macOS, "Ctrl+Shift+K" elsewhere. */
export function keyLabel(keys: string): string {
  const { mods, key } = splitKeys(keys);
  const named = (IS_MAC ? MAC_KEYS : PC_KEYS)[key];
  const k = named ?? (key.length === 1 ? key.toUpperCase() : key);
  if (!IS_MAC) return [...mods.map((m) => (m === "Mod" ? "Ctrl" : m)), k].join("+");
  const ordered = [...mods].sort((a, b) => MAC_MOD_ORDER.indexOf(a) - MAC_MOD_ORDER.indexOf(b));
  return ordered.map((m) => MAC_MODS[m] ?? m).join("") + k;
}

/** Custom shortcuts from Settings (action id → keys); they replace the defaults. */
let custom: Partial<Record<string, string>> = {};

/** The keys an action uses now, in "Mod+K" notation. */
export function keysFor(id: ActionId): string | undefined {
  return custom[id] ?? SHORTCUTS.find((x) => x.id === id)?.keys;
}

export function shortcutFor(id: ActionId): string | undefined {
  const keys = keysFor(id);
  return keys ? keyLabel(keys) : undefined;
}

/** "Label (⌘K)" for tooltips. */
export function withShortcut(label: string, id: ActionId): string {
  const keys = shortcutFor(id);
  return keys ? `${label} (${keys})` : label;
}

interface Binding {
  id: ActionId;
  mod: boolean;
  ctrl: boolean;
  shift: boolean;
  alt: boolean;
  key: string;
}

function parseBinding(id: ActionId, keys: string): Binding {
  const parts = keys.split("+");
  const key = parts.pop() || "+";
  return {
    id,
    mod: parts.includes("Mod"),
    ctrl: parts.includes("Ctrl"),
    shift: parts.includes("Shift"),
    alt: parts.includes("Alt"),
    key: key.toLowerCase(),
  };
}

function buildBindings(): Binding[] {
  const defaults = [...SHORTCUTS, ...EXTRA_BINDINGS].filter((s) => !custom[s.id]);
  const overrides = Object.entries(custom).flatMap(([id, keys]) =>
    keys ? [{ id: id as ActionId, keys }] : [],
  );
  // Custom keys come first, so they win over a default bound to the same keys.
  return [...overrides, ...defaults].map((s) => parseBinding(s.id, s.keys));
}

let BINDINGS: Binding[] = buildBindings();

export function setCustomShortcuts(shortcuts: Record<string, string>) {
  custom = { ...shortcuts };
  BINDINGS = buildBindings();
}

const KEY_BY_CODE: Record<string, string> = {
  BracketLeft: "[",
  BracketRight: "]",
  Backslash: "\\",
  Slash: "/",
  Period: ".",
  Comma: ",",
  Equal: "=",
  Minus: "-",
};

/** The physical key, so ⇧ and ⌥ combinations match on any layout ("Shift+]" gives "}"). */
export function eventKey(e: KeyboardEvent): string {
  const byCode = KEY_BY_CODE[e.code];
  if (byCode) return byCode;
  if (/^Digit\d$/.test(e.code)) return e.code.slice(5);
  if (/^Key[A-Z]$/.test(e.code)) return e.code.slice(3).toLowerCase();
  return e.key.toLowerCase();
}

function matches(b: Binding, e: KeyboardEvent, key: string): boolean {
  if (b.key !== key) return false;
  const mod = IS_MAC ? e.metaKey : e.ctrlKey;
  // On macOS, "Ctrl+Tab" is its own binding; elsewhere Ctrl is the Mod key.
  const wantMod = b.mod || (!IS_MAC && b.ctrl);
  if (wantMod !== mod || b.shift !== e.shiftKey || b.alt !== e.altKey) return false;
  return !IS_MAC || b.ctrl === e.ctrlKey;
}

export function matchKey(e: KeyboardEvent): ActionId | null {
  const key = eventKey(e);
  return BINDINGS.find((b) => matches(b, e, key))?.id ?? null;
}

/** Text fields keep their own meaning for keys like ⌘Z. */
export function isEditingText(): boolean {
  const el = document.activeElement as HTMLElement | null;
  if (!el) return false;
  return ["INPUT", "TEXTAREA", "SELECT"].includes(el.tagName) || el.isContentEditable === true;
}

/** A key press in "Mod+Shift+K" notation; null for a bare modifier or plain typing. */
export function keysOf(e: KeyboardEvent): string | null {
  if (["Meta", "Control", "Shift", "Alt"].includes(e.key)) return null;
  const mod = IS_MAC ? e.metaKey : e.ctrlKey;
  const ctrl = IS_MAC && e.ctrlKey;
  const key = eventKey(e);
  const functionKey = /^f\d{1,2}$/.test(key);
  // A shortcut needs a modifier other than Shift, except function keys.
  if (!mod && !ctrl && !e.altKey && !functionKey) return null;
  const mods = [mod && "Mod", ctrl && "Ctrl", e.altKey && "Alt", e.shiftKey && "Shift"];
  const name = key.length === 1 ? key.toUpperCase() : functionKey ? key.toUpperCase() : e.key;
  return [...mods.filter(Boolean), name].join("+");
}

/** Other actions already using `keys` (compared the way key presses are matched). */
export function conflictsWith(keys: string, except: ActionId): ActionId[] {
  const wanted = parseBinding(except, keys);
  const same = (b: Binding) =>
    b.id !== except &&
    b.key === wanted.key &&
    b.mod === wanted.mod &&
    b.ctrl === wanted.ctrl &&
    b.shift === wanted.shift &&
    b.alt === wanted.alt;
  return [...new Set(BINDINGS.filter(same).map((b) => b.id))];
}
