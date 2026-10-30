// "Export Results from All Tabs": every result tab that has rows goes to a file of its own
// in a folder, in one of the copy formats (the rows the tab holds, as shown).

import { open } from "@tauri-apps/plugin-dialog";
import { api } from "@/api";
import { copyRows, type CopyFormat } from "@/lib/copyRows";
import { plural } from "@/lib/format";
import { showError, toastSuccess } from "@/services/errors";
import { type Dialect } from "@/types";
import { type ResultItem } from "./results";

export const RESULT_FILE_FORMATS: [CopyFormat, string, string][] = [
  ["csv", "CSV", "csv"],
  ["tsv", "TSV", "tsv"],
  ["json", "JSON", "json"],
  ["sql", "SQL INSERT", "sql"],
  ["markdown", "Markdown", "md"],
  ["html", "HTML", "html"],
];

export const exportable = (items: ResultItem[]) =>
  items.filter((i) => !i.result.error && !i.explain && i.result.columns.length > 0);

/** File name and contents for each result with rows. */
export function resultFiles(items: ResultItem[], format: CopyFormat, driver: Dialect) {
  const ext = RESULT_FILE_FORMATS.find(([f]) => f === format)?.[2] ?? "txt";
  return exportable(items).map((item, i) => {
    const r = item.result;
    const names = r.columns.map((c) => c.name);
    const opts = { driver, header: format === "csv" || format === "tsv" };
    return { name: `result-${i + 1}.${ext}`, contents: copyRows(format, names, r.rows, opts) };
  });
}

export async function exportAllResults(items: ResultItem[], format: CopyFormat, driver: Dialect) {
  const files = resultFiles(items, format, driver);
  if (!files.length) return;
  const dir = await open({ directory: true, title: "Choose a folder for the result files" });
  if (typeof dir !== "string") return;
  try {
    for (const f of files) await api.saveTextFile(`${dir}/${f.name}`, f.contents);
    toastSuccess(`Exported ${plural(files.length, "result")}.`, dir);
  } catch (e) {
    showError(e);
  }
}
