// The sections of the settings dialog. Each edits a draft; Save sends it to Rust.

import { api } from "@/api";
import { plural } from "@/lib/format";
import { showError, toastSuccess } from "@/services/errors";
import { type AppInfo, type DefaultFilterColumn, type Settings, type Theme } from "@/types";

export interface SectionProps {
  s: Settings;
  set: (patch: Partial<Settings>) => void;
}

const THEMES: [Theme, string][] = [
  ["system", "System"],
  ["light", "Light"],
  ["dark", "Dark"],
];

function SizeSlider(props: {
  value: number;
  min: number;
  max: number;
  onChange: (n: number) => void;
}) {
  return (
    <div className="row">
      <input
        className="input grow"
        type="range"
        min={props.min}
        max={props.max}
        value={props.value}
        onChange={(e) => props.onChange(Number(e.target.value))}
      />
      <span className="mono muted size-value">{props.value} px</span>
    </div>
  );
}

export function AppearanceSection({ s, set }: SectionProps) {
  return (
    <>
      <div className="settings-section">Appearance</div>
      <div className="form-grid">
        <label>Theme</label>
        <div className="segmented wide">
          {THEMES.map(([t, label]) => (
            <button
              key={t}
              className={s.theme === t ? "active" : ""}
              onClick={() => set({ theme: t })}
            >
              {label}
            </button>
          ))}
        </div>
        <label>Interface text</label>
        <SizeSlider
          value={s.uiFontSize}
          min={11}
          max={16}
          onChange={(uiFontSize) => set({ uiFontSize })}
        />
        <label>Editor text</label>
        <SizeSlider
          value={s.editorFontSize}
          min={10}
          max={24}
          onChange={(editorFontSize) => set({ editorFontSize })}
        />
      </div>
    </>
  );
}

export function FiltersSection({ s, set }: SectionProps) {
  return (
    <>
      <div className="settings-section">Filters</div>
      <div className="form-grid">
        <label>New filters use</label>
        <select
          className="select"
          value={s.defaultFilterColumn}
          onChange={(e) => set({ defaultFilterColumn: e.target.value as DefaultFilterColumn })}
        >
          <option value="first">The first column</option>
          <option value="pk">The primary key</option>
          <option value="any">Any column</option>
        </select>
        <label>Default operator</label>
        <select
          className="select"
          value={s.defaultFilterOperator}
          onChange={(e) => set({ defaultFilterOperator: e.target.value })}
        >
          <option value="eq">=</option>
          <option value="contains">Contains</option>
          <option value="like">LIKE</option>
        </select>
      </div>
    </>
  );
}

const DEFAULT_RETENTION = 365;

async function purgeNow(days: number) {
  try {
    const n = await api.purgeHistory(days);
    toastSuccess(
      `Removed ${plural(n, "history entry", "history entries")} older than ${days} days.`,
    );
  } catch (e) {
    showError(e);
  }
}

export function HistorySection({ s, set, info }: SectionProps & { info: AppInfo | null }) {
  return (
    <>
      <div className="settings-section">History</div>
      <div className="form-grid">
        <label>Keep history</label>
        <div className="row">
          <input
            className="input num-input"
            type="number"
            min={1}
            max={3650}
            value={s.historyRetentionDays}
            onChange={(e) =>
              set({ historyRetentionDays: Number(e.target.value) || DEFAULT_RETENTION })
            }
          />
          <span className="muted">days</span>
          <button className="btn sm" onClick={() => void purgeNow(s.historyRetentionDays)}>
            Delete older now
          </button>
        </div>
        {info && (
          <>
            <label>Data folder</label>
            <span className="mono muted selectable small break-all">{info.dataDir}</span>
          </>
        )}
      </div>
    </>
  );
}
