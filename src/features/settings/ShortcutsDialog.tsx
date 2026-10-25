// Keyboard shortcuts: the list, and changing any action's keys (click the keys).

import { Keyboard, RotateCcw } from "lucide-react";
import { Modal } from "@/components/Modal";
import {
  type ActionId,
  keyLabel,
  keysFor,
  NON_ACTION_KEYS,
  SHORTCUT_GROUPS,
  SHORTCUTS,
  type ShortcutGroup,
} from "@/lib/actions";
import { cx } from "@/lib/cx";
import { useShortcutEditor } from "./useShortcutEditor";

/** ⌘1…⌘9 is a family of keys, not one shortcut to change. */
const FIXED: ActionId[] = ["tab-1"];

/** One row per action of the group (an action listed twice shows once). */
function actionsOf(group: ShortcutGroup) {
  const seen = new Set<string>();
  return SHORTCUTS.filter((s) => s.group === group && !seen.has(s.id) && seen.add(s.id));
}

type Editor = ReturnType<typeof useShortcutEditor>;

function ActionRow({ id, label, editor }: { id: ActionId; label: string; editor: Editor }) {
  const keys = keysFor(id) ?? "";
  const isCustom = id in editor.custom;
  if (FIXED.includes(id)) {
    return (
      <div className="shortcut-row">
        <span>{label}</span>
        <kbd>{keyLabel(keys)}</kbd>
      </div>
    );
  }
  const recording = editor.recording === id;
  return (
    <div className={cx("shortcut-row", isCustom && "custom")}>
      <span>{label}</span>
      {isCustom && (
        <button
          className="btn ghost icon xs"
          title="Reset to default"
          onClick={() => editor.reset(id)}
        >
          <RotateCcw size={11} />
        </button>
      )}
      <button
        className={cx("shortcut-keys", recording && "recording")}
        title="Click, then press the new shortcut"
        onClick={() => editor.record(recording ? null : id)}
        onKeyDown={editor.onRecordKey}
        onBlur={() => recording && editor.record(null)}
      >
        {recording ? "Press keys…" : keyLabel(keys)}
      </button>
    </div>
  );
}

export function ShortcutsDialog({ onClose }: { onClose: () => void }) {
  const editor = useShortcutEditor();
  const anyCustom = Object.keys(editor.custom).length > 0;
  return (
    <Modal
      title="Keyboard shortcuts"
      subtitle="Click a shortcut to change it"
      icon={<Keyboard size={18} />}
      size="lg"
      onClose={onClose}
      footer={
        <>
          <button className="btn" disabled={!anyCustom} onClick={editor.resetAll}>
            <RotateCcw size={13} /> Reset all
          </button>
          <span className="spacer" />
          <button className="btn primary" onClick={onClose}>
            Done
          </button>
        </>
      }
    >
      <div className="shortcut-columns">
        {SHORTCUT_GROUPS.map((g) => (
          <div key={g} className="shortcut-group">
            <div className="shortcut-title">{g}</div>
            {actionsOf(g).map((s) => (
              <ActionRow key={s.id} id={s.id} label={s.label} editor={editor} />
            ))}
            {(NON_ACTION_KEYS[g] ?? []).map(([keys, label]) => (
              <div key={label} className="shortcut-row">
                <span>{label}</span>
                <kbd>{keys}</kbd>
              </div>
            ))}
          </div>
        ))}
      </div>
    </Modal>
  );
}
