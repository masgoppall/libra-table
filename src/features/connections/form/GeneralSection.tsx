import { connectionColor, TAGS } from "@/domain/connection";
import { useStore } from "@/store";
import { type Tag } from "@/types";
import { fieldAttrs, FieldLabel } from "./fields";
import { type ConnectionFormState } from "./useConnectionForm";

export function GeneralSection({ form }: { form: ConnectionFormState }) {
  const groups = useStore((s) => s.groups);
  const { c, set } = form;
  return (
    <div className="form-grid">
      <FieldLabel form={form} field="name">
        Name
      </FieldLabel>
      <input
        className="input"
        {...fieldAttrs(form, "name")}
        value={c.name}
        placeholder="My database"
        autoFocus
        onChange={(e) => set({ name: e.target.value })}
      />
      <label>Tag</label>
      <div className="row">
        <select
          className="select"
          value={c.tag}
          onChange={(e) => set({ tag: e.target.value as Tag })}
        >
          {TAGS.map((t) => (
            <option key={t} value={t}>
              {t}
            </option>
          ))}
        </select>
        <span className="muted">Color</span>
        <input
          type="color"
          className="color-input"
          value={connectionColor(c)}
          onChange={(e) => set({ color: e.target.value })}
        />
        {c.color && (
          <button className="btn sm ghost" onClick={() => set({ color: "" })}>
            Use tag color
          </button>
        )}
        <span className="muted">Group</span>
        <select
          className="select"
          value={c.groupId ?? ""}
          onChange={(e) => set({ groupId: e.target.value || null })}
        >
          <option value="">No group</option>
          {groups.map((g) => (
            <option key={g.id} value={g.id}>
              {g.name}
            </option>
          ))}
        </select>
      </div>
      {c.tag === "production" && (
        <>
          <span />
          <div className="callout warn">
            Production: statements that change data ask for confirmation first.
          </div>
        </>
      )}
    </div>
  );
}
