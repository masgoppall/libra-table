// Picking a color theme, importing one from a theme file (TablePlus' JSON format), and
// removing an imported one.

import { FileUp, Trash2 } from "lucide-react";
import { useRef } from "react";
import { BUILT_IN_THEMES, themeFromFile } from "@/domain/themes";
import { uid } from "@/lib/uid";
import { showError, toastSuccess } from "@/services/errors";
import { type ColorTheme } from "@/types";
import { type SectionProps } from "./SettingsSections";

function Swatch({ t }: { t: ColorTheme }) {
  const c = t.colors;
  return (
    <span className="theme-swatch" aria-hidden>
      <i style={{ background: c.sidebar ?? c.window }} />
      <i style={{ background: c.content ?? c.gridOdd }} />
      <i style={{ background: c.accent }} />
    </span>
  );
}

export function ColorThemePicker({ s, set }: SectionProps) {
  const file = useRef<HTMLInputElement>(null);
  const all = [...BUILT_IN_THEMES, ...s.customThemes];
  const current = all.find((t) => t.id === s.colorTheme);
  const imported = s.customThemes.some((t) => t.id === s.colorTheme);

  const importFile = async (f: File | undefined) => {
    if (!f) return;
    try {
      const theme = themeFromFile(await f.text(), `custom-${uid()}`);
      set({ customThemes: [...s.customThemes, theme], colorTheme: theme.id });
      toastSuccess(`Imported "${theme.name}".`, "Save the settings to keep it.");
    } catch (e) {
      showError(e);
    }
  };

  return (
    <div className="row">
      {current && <Swatch t={current} />}
      <select
        className="select grow"
        aria-label="Color theme"
        value={s.colorTheme}
        onChange={(e) => set({ colorTheme: e.target.value })}
      >
        <option value="">LibraTable (follows Theme)</option>
        <optgroup label="Built-in">
          {BUILT_IN_THEMES.map((t) => (
            <option key={t.id} value={t.id}>
              {t.name} · {t.base}
            </option>
          ))}
        </optgroup>
        {s.customThemes.length > 0 && (
          <optgroup label="Imported">
            {s.customThemes.map((t) => (
              <option key={t.id} value={t.id}>
                {t.name} · {t.base}
              </option>
            ))}
          </optgroup>
        )}
      </select>
      <button className="btn sm" onClick={() => file.current?.click()}>
        <FileUp size={13} /> Import…
      </button>
      {imported && (
        <button
          className="btn sm icon ghost"
          aria-label="Remove this theme"
          title="Remove this imported theme"
          onClick={() =>
            set({
              customThemes: s.customThemes.filter((t) => t.id !== s.colorTheme),
              colorTheme: "",
            })
          }
        >
          <Trash2 size={13} />
        </button>
      )}
      <input
        ref={file}
        type="file"
        accept=".json,application/json"
        hidden
        aria-label="Theme file"
        onChange={(e) => {
          void importFile(e.target.files?.[0]);
          e.target.value = "";
        }}
      />
    </div>
  );
}
