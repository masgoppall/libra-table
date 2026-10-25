// Which in-place editor a column gets: enum values, booleans, dates, or plain text.

import { type CellEditorKind, type ColumnInfo, type Driver } from "@/types";

const TEXT: CellEditorKind = { kind: "text" };

function isBoolean(driver: Driver, type: string): boolean {
  if (/^(bool|boolean)$/i.test(type)) return true;
  // MySQL's BOOLEAN is TINYINT(1).
  return driver === "mysql" && /^tinyint\(1\)/i.test(type);
}

export function editorFor(
  driver: Driver,
  column: Pick<ColumnInfo, "dataType">,
  choices: string[] | undefined,
): CellEditorKind {
  if (choices?.length) return { kind: "choice", options: choices };
  const type = column.dataType.trim();
  if (isBoolean(driver, type)) {
    const options = driver === "postgres" ? ["true", "false"] : ["1", "0"];
    return { kind: "choice", options };
  }
  if (/^date$/i.test(type)) return { kind: "date" };
  return TEXT;
}
