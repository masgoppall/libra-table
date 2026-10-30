// Settings for tabs, the data grid and the query editor.

import { Check } from "@/components/Check";
import { VARIABLE_FORMATS, type VariableFormat } from "@/lib/variables";
import { type EditorKeymap, type RowDensity } from "@/types";
import { type SectionProps } from "./SettingsSections";

const DENSITIES: [RowDensity, string][] = [
  ["compact", "Compact"],
  ["normal", "Normal"],
  ["comfortable", "Comfortable"],
];

export function GridSection({ s, set }: SectionProps) {
  return (
    <>
      <div className="settings-section">Tabs & data grid</div>
      <div className="form-grid">
        <label>Opening a table</label>
        <select
          className="select"
          value={s.openInNewTab ? "new" : "same"}
          onChange={(e) => set({ openInNewTab: e.target.value === "new" })}
        >
          <option value="new">Opens a new tab</option>
          <option value="same">Replaces the current table tab (⌘-click for a new one)</option>
        </select>
        <label>Row height</label>
        <div className="segmented wide">
          {DENSITIES.map(([d, label]) => (
            <button
              key={d}
              className={s.rowDensity === d ? "active" : ""}
              onClick={() => set({ rowDensity: d })}
            >
              {label}
            </button>
          ))}
        </div>
        <label>Rows</label>
        <div className="col">
          <Check
            checked={s.alternatingRows}
            onChange={(alternatingRows) => set({ alternatingRows })}
          >
            Alternating row background colors
          </Check>
          <Check checked={s.showRowNumbers} onChange={(showRowNumbers) => set({ showRowNumbers })}>
            Show row numbers
          </Check>
        </div>
      </div>
    </>
  );
}

export function EditorSection({ s, set }: SectionProps) {
  return (
    <>
      <div className="settings-section">Query editor</div>
      <div className="form-grid">
        <label>Key bindings</label>
        <select
          className="select"
          value={s.editorKeymap}
          onChange={(e) => set({ editorKeymap: e.target.value as EditorKeymap })}
        >
          <option value="default">Standard</option>
          <option value="vim">Vim</option>
        </select>
        <label>Typing</label>
        <div className="col">
          <Check
            checked={s.uppercaseKeywords}
            onChange={(uppercaseKeywords) => set({ uppercaseKeywords })}
          >
            Automatically UPPERCASE keywords
          </Check>
          <Check checked={s.closeBrackets} onChange={(closeBrackets) => set({ closeBrackets })}>
            Automatically insert closing brackets and quotes
          </Check>
        </div>
        <label>Variables</label>
        <div className="col">
          <select
            className="select"
            value={s.variableFormat}
            onChange={(e) => set({ variableFormat: e.target.value as VariableFormat })}
          >
            {VARIABLE_FORMATS.map(([f, label]) => (
              <option key={f} value={f}>
                {label}
              </option>
            ))}
          </select>
          <Check
            checked={s.variablesInStrings}
            onChange={(variablesInStrings) => set({ variablesInStrings })}
          >
            Allow searching for variables in quoted strings
          </Check>
        </div>
      </div>
    </>
  );
}
