// @vitest-environment jsdom
import { PostgreSQL, sql } from "@codemirror/lang-sql";
import { EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { describe, expect, it } from "vitest";
import {
  convertIndentation,
  transformSelection,
  transformText,
  trimTrailingWhitespace,
  uppercaseKeywords,
} from "./editing";

function editor(doc: string) {
  const state = EditorState.create({
    doc,
    selection: { anchor: doc.length },
    extensions: [sql({ dialect: PostgreSQL }), uppercaseKeywords],
  });
  return new EditorView({ state, parent: document.body });
}

/** Types `text` at the end the way CodeMirror hands typed text to input handlers. */
function type(view: EditorView, text: string) {
  const at = view.state.doc.length;
  const handled = view.state
    .facet(EditorView.inputHandler)
    .some((h) => h(view, at, at, text, () => view.state.update({})));
  if (!handled)
    view.dispatch({ changes: { from: at, insert: text }, selection: { anchor: at + 1 } });
}

describe("query editor typing", () => {
  it("uppercases a keyword when the word ends", () => {
    const v = editor("select");
    type(v, " ");
    expect(v.state.doc.toString()).toBe("SELECT ");
  });

  it("leaves identifiers and quoted text alone", () => {
    const v = editor("select name from people where note = 'select");
    type(v, " ");
    expect(v.state.doc.toString()).toBe("select name from people where note = 'select ");
    const id = editor("select people");
    type(id, " ");
    expect(id.state.doc.toString()).toBe("select people ");
  });

  it("trims trailing whitespace on every line", () => {
    const v = editor("select 1   \nfrom t\t\n");
    expect(trimTrailingWhitespace(v)).toBe(true);
    expect(v.state.doc.toString()).toBe("select 1\nfrom t\n");
    expect(trimTrailingWhitespace(v)).toBe(false);
  });
});

const WIDE = String.fromCharCode(0x3000);

describe("text transforms", () => {
  it("converts widths and kana", () => {
    expect(transformText("fullwidth", "AB 1")).toBe(`ＡＢ${WIDE}１`);
    expect(transformText("halfwidth", `ＡＢ${WIDE}１`)).toBe("AB 1");
    expect(transformText("katakana", "ひらがな")).toBe("ヒラガナ");
    expect(transformText("hiragana", "カタカナ")).toBe("かたかな");
    expect(transformText("nfkc-casefold", "ＡＢＣ")).toBe("abc");
  });

  it("transforms the selection only and converts indentation", () => {
    const v = editor("select x");
    v.dispatch({ selection: { anchor: 0, head: 6 } });
    transformSelection(v, "upper");
    expect(v.state.doc.toString()).toBe("SELECT x");
    const t = editor("\tselect\n    from");
    convertIndentation(t, "spaces");
    expect(t.state.doc.toString()).toBe("  select\n    from");
    convertIndentation(t, "tabs");
    expect(t.state.doc.toString()).toBe("\tselect\n\t\tfrom");
  });
});
