// Editor marks of a run: the statement that runs flashes briefly, and statements that
// failed in the last run get a gutter marker (with the error as tooltip) until the
// next run. Both follow edits made after the run.

import { RangeSet, StateEffect, StateField } from "@codemirror/state";
import { Decoration, type DecorationSet, EditorView, gutter, GutterMarker } from "@codemirror/view";

export interface ErrorMark {
  from: number;
  message: string;
}

const FLASH_MS = 700;

const flashEffect = StateEffect.define<{ from: number; to: number } | null>();
const errorsEffect = StateEffect.define<ErrorMark[]>();

const flashMark = Decoration.mark({ class: "cm-run-flash" });

const flashField = StateField.define<DecorationSet>({
  create: () => Decoration.none,
  update(deco, tr) {
    let next = deco.map(tr.changes);
    for (const e of tr.effects) {
      if (!e.is(flashEffect)) continue;
      next = e.value
        ? Decoration.set([flashMark.range(e.value.from, e.value.to)])
        : Decoration.none;
    }
    return next;
  },
  provide: (f) => EditorView.decorations.from(f),
});

class ErrorMarker extends GutterMarker {
  constructor(readonly message: string) {
    super();
  }
  override eq(other: ErrorMarker) {
    return other.message === this.message;
  }
  override toDOM() {
    const el = document.createElement("span");
    // The spacer (no message) only sizes the gutter.
    el.className = this.message ? "cm-run-error" : "cm-run-error-spacer";
    el.title = this.message;
    el.textContent = "●";
    return el;
  }
}

const errorField = StateField.define<RangeSet<GutterMarker>>({
  create: () => RangeSet.empty,
  update(set, tr) {
    let next = set.map(tr.changes);
    for (const e of tr.effects) {
      if (!e.is(errorsEffect)) continue;
      const doc = tr.state.doc;
      const marks = e.value
        .filter((m) => m.from <= doc.length)
        .map((m) => new ErrorMarker(m.message).range(doc.lineAt(m.from).from));
      next = RangeSet.of(marks, true);
    }
    return next;
  },
});

const errorGutter = gutter({
  class: "cm-run-gutter",
  markers: (view) => view.state.field(errorField),
  initialSpacer: () => new ErrorMarker(""),
});

export const runMarks = [flashField, errorField, errorGutter];

function clamp(view: EditorView, n: number) {
  return Math.max(0, Math.min(n, view.state.doc.length));
}

/** Highlights from..to for a moment. */
export function flashRange(view: EditorView, from: number, to: number) {
  const range = { from: clamp(view, from), to: clamp(view, to) };
  if (range.from >= range.to) return;
  view.dispatch({ effects: flashEffect.of(range) });
  setTimeout(() => {
    if (view.dom.isConnected) view.dispatch({ effects: flashEffect.of(null) });
  }, FLASH_MS);
}

/** Replaces the error markers with those of the last run. */
export function markErrors(view: EditorView, marks: ErrorMark[]) {
  view.dispatch({ effects: errorsEffect.of(marks) });
}

/** Lines that have an error marker (for tests). */
export function errorLines(view: EditorView): number[] {
  const lines: number[] = [];
  view.state.field(errorField).between(0, view.state.doc.length, (from) => {
    lines.push(view.state.doc.lineAt(from).number);
  });
  return lines;
}
