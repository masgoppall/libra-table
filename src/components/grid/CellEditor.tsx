// The in-place editor of one cell: text, a dropdown (enums, booleans) or a date picker.

import { type CellEditorKind } from "./types";
import { type CellEditing } from "./useCellEditing";

interface Props {
  edit: CellEditing;
  value: string;
  editor: CellEditorKind;
  onTab: (back: boolean) => void;
}

function useEditorKeys(p: Props) {
  return (e: React.KeyboardEvent) => {
    // App shortcuts (⌘S…) still work while editing: save the cell first.
    if (e.metaKey || e.ctrlKey) return p.edit.commit();
    e.stopPropagation();
    if (e.key === "Enter") p.edit.commit();
    else if (e.key === "Escape") p.edit.cancel();
    else if (e.key === "Tab") {
      e.preventDefault();
      p.edit.commit();
      p.onTab(e.shiftKey);
    }
  };
}

function ChoiceEditor(p: Props & { options: string[] }) {
  const onKeyDown = useEditorKeys(p);
  // A value outside the list (old data) stays selectable.
  const options = p.value && !p.options.includes(p.value) ? [p.value, ...p.options] : p.options;
  return (
    <select
      autoFocus
      value={p.value}
      onChange={(e) => {
        p.edit.change(e.target.value);
        p.edit.commit();
      }}
      onBlur={p.edit.commit}
      onKeyDown={onKeyDown}
    >
      {!p.value && <option value="">(empty)</option>}
      {options.map((o) => (
        <option key={o} value={o}>
          {o}
        </option>
      ))}
    </select>
  );
}

export function CellEditor(p: Props) {
  const onKeyDown = useEditorKeys(p);
  if (p.editor.kind === "choice") return <ChoiceEditor {...p} options={p.editor.options} />;
  return (
    <input
      autoFocus
      type={p.editor.kind === "date" ? "date" : "text"}
      value={p.value}
      onChange={(e) => p.edit.change(e.target.value)}
      onBlur={p.edit.commit}
      onKeyDown={onKeyDown}
    />
  );
}
