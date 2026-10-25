// Beautify: sql-formatter with the driver's dialect. Only the three dialects are
// imported (formatDialect), and only the lazily loaded query chunk imports this file.

import { type DialectOptions, formatDialect, mysql, postgresql, sqlite } from "sql-formatter";
import { type Dialect } from "@/types";

const DIALECTS: Record<Dialect, DialectOptions> = { postgres: postgresql, mysql, sqlite };

/** Keeps `:name` variables and the driver's own placeholders intact. */
const PARAMS = {
  postgres: { numbered: ["$" as const], named: [":" as const] },
  mysql: { positional: true, named: [":" as const] },
  sqlite: { positional: true, numbered: ["?" as const], named: [":" as const] },
};

/** Throws when the text can't be parsed (the message says where). */
export function formatSql(text: string, driver: Dialect): string {
  return formatDialect(text, {
    dialect: DIALECTS[driver],
    keywordCase: "upper",
    tabWidth: 2,
    paramTypes: PARAMS[driver],
  });
}
