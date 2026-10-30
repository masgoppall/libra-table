// Typing helpers of the query editor (Settings › Query editor): keywords turn UPPERCASE
// as a word ends, ⌥⌘/ toggles a block comment, and trimming trailing whitespace.

import { toggleBlockComment } from "@codemirror/commands";
import { syntaxTree } from "@codemirror/language";
import { type Extension } from "@codemirror/state";
import { EditorView, keymap } from "@codemirror/view";

/** Characters that end a word. */
const WORD_END = /^[\s(),;]$/;

/** After a keyword is typed in full (and not in a string or comment), it becomes
 * UPPERCASE; identifiers and quoted text are left alone. */
export const uppercaseKeywords: Extension = EditorView.inputHandler.of((view, from, to, text) => {
  if (!WORD_END.test(text) || from !== to) return false;
  const state = view.state;
  const line = state.doc.lineAt(from);
  const word = /[A-Za-z_]+$/.exec(line.text.slice(0, from - line.from))?.[0];
  if (!word || word === word.toUpperCase()) return false;
  const node = syntaxTree(state).resolveInner(from, -1);
  if (node.name !== "Keyword" || node.to !== from) return false;
  const start = from - word.length;
  view.dispatch({
    changes: [{ from: start, to: from, insert: word.toUpperCase() + text }],
    selection: { anchor: from + text.length },
    userEvent: "input.type",
  });
  return true;
});

export const editingKeys: Extension = keymap.of([
  { key: "Mod-Alt-/", run: toggleBlockComment, preventDefault: true },
]);

/** Removes spaces and tabs at the end of every line; true when something changed. */
export function trimTrailingWhitespace(view: EditorView): boolean {
  const changes = [];
  for (let n = 1; n <= view.state.doc.lines; n++) {
    const line = view.state.doc.line(n);
    const trailing = /[ \t]+$/.exec(line.text);
    if (trailing) changes.push({ from: line.to - trailing[0].length, to: line.to });
  }
  if (!changes.length) return false;
  view.dispatch({ changes, userEvent: "delete" });
  return true;
}

export type TextTransform =
  | "upper"
  | "lower"
  | "fullwidth"
  | "halfwidth"
  | "katakana"
  | "hiragana"
  | "nfc"
  | "nfd"
  | "nfkc-casefold";

export const TRANSFORMS: [TextTransform, string][] = [
  ["upper", "UPPERCASE"],
  ["lower", "lowercase"],
  ["fullwidth", "To Full-width"],
  ["halfwidth", "To Half-width"],
  ["katakana", "Hiragana to Katakana"],
  ["hiragana", "Katakana to Hiragana"],
  ["nfc", "Unicode NFC"],
  ["nfd", "Unicode NFD"],
  ["nfkc-casefold", "NFKC Casefold"],
];

/** U+3000, the full-width space. */
const WIDE_SPACE = String.fromCharCode(0x3000);
const WIDE_SPACE_RE = new RegExp(WIDE_SPACE, "g");

const shift = (s: string, test: (c: number) => boolean, by: number) =>
  Array.from(s, (ch) => {
    const c = ch.codePointAt(0) ?? 0;
    return test(c) ? String.fromCodePoint(c + by) : ch;
  }).join("");

export function transformText(t: TextTransform, s: string): string {
  switch (t) {
    case "upper":
      return s.toUpperCase();
    case "lower":
      return s.toLowerCase();
    case "fullwidth":
      return shift(s, (c) => c >= 0x21 && c <= 0x7e, 0xfee0).replace(/ /g, WIDE_SPACE);
    case "halfwidth":
      return shift(s, (c) => c >= 0xff01 && c <= 0xff5e, -0xfee0).replace(WIDE_SPACE_RE, " ");
    case "katakana":
      return shift(s, (c) => c >= 0x3041 && c <= 0x3096, 0x60);
    case "hiragana":
      return shift(s, (c) => c >= 0x30a1 && c <= 0x30f6, -0x60);
    case "nfc":
      return s.normalize("NFC");
    case "nfd":
      return s.normalize("NFD");
    case "nfkc-casefold":
      return s.normalize("NFKC").toLowerCase();
  }
}

/** Transforms the selected text, or everything when nothing is selected. */
export function transformSelection(view: EditorView, t: TextTransform): boolean {
  const { state } = view;
  const ranges = state.selection.ranges.filter((r) => !r.empty);
  const targets = ranges.length ? ranges : [{ from: 0, to: state.doc.length }];
  const changes = targets.map((r) => ({
    from: r.from,
    to: r.to,
    insert: transformText(t, state.sliceDoc(r.from, r.to)),
  }));
  view.dispatch({ changes, userEvent: "input" });
  return true;
}

/** Leading tabs become `width` spaces, or leading runs of `width` spaces become tabs. */
export function convertIndentation(view: EditorView, to: "spaces" | "tabs", width = 2) {
  const changes = [];
  for (let n = 1; n <= view.state.doc.lines; n++) {
    const line = view.state.doc.line(n);
    const lead = /^[ \t]*/.exec(line.text)?.[0] ?? "";
    if (!lead) continue;
    const columns = Array.from(lead).reduce((w, ch) => w + (ch === "\t" ? width : 1), 0);
    const indent =
      to === "spaces"
        ? " ".repeat(columns)
        : "\t".repeat(Math.floor(columns / width)) + " ".repeat(columns % width);
    if (indent !== lead)
      changes.push({ from: line.from, to: line.from + lead.length, insert: indent });
  }
  if (changes.length) view.dispatch({ changes, userEvent: "input" });
}
