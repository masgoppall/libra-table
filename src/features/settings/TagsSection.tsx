// Settings › Tags (TablePlus' tag manager): custom tags for connections, each a name and a
// color. The environment tags (local … production) stay as they are.

import { Plus, Trash2 } from "lucide-react";
import { TAG_COLORS, TAGS } from "@/domain/connection";
import { type CustomTag } from "@/types";
import { type SectionProps } from "./SettingsSections";

const PALETTE = ["#3e63dd", "#30a46c", "#f76b15", "#8e4ec6", "#e5484d", "#0090ff", "#ffb224"];

export function TagsSection({ s, set }: SectionProps) {
  const tags = s.customTags;
  const update = (i: number, patch: Partial<CustomTag>) =>
    set({ customTags: tags.map((t, j) => (j === i ? { ...t, ...patch } : t)) });
  const add = () =>
    set({
      customTags: [
        ...tags,
        {
          name: `Tag ${tags.length + 1}`,
          color: PALETTE[tags.length % PALETTE.length] ?? "#3e63dd",
        },
      ],
    });
  return (
    <>
      <div className="settings-section">Tags</div>
      <div className="form-grid">
        <label>Environment</label>
        <div className="row wrap tag-list">
          {TAGS.map((t) => (
            <span key={t} className={`tag-badge ${t}`} title={TAG_COLORS[t]}>
              {t}
            </span>
          ))}
        </div>
        <label>Custom tags</label>
        <div className="col tag-editor">
          {tags.map((t, i) => (
            <div className="row" key={i}>
              <input
                type="color"
                className="color-input"
                aria-label={`Color of ${t.name}`}
                value={t.color}
                onChange={(e) => update(i, { color: e.target.value })}
              />
              <input
                className="input grow"
                aria-label="Tag name"
                value={t.name}
                maxLength={40}
                onChange={(e) => update(i, { name: e.target.value })}
              />
              <button
                className="btn sm icon ghost"
                aria-label={`Delete ${t.name}`}
                onClick={() => set({ customTags: tags.filter((_, j) => j !== i) })}
              >
                <Trash2 size={13} />
              </button>
            </div>
          ))}
          <div className="row">
            <button className="btn sm" onClick={add}>
              <Plus size={13} /> Add tag
            </button>
            <span className="faint small">Pick them in a connection's settings.</span>
          </div>
        </div>
      </div>
    </>
  );
}
