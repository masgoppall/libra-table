import { SQL_DEFAULTS, SQL_TYPES } from "@/domain/ddl";
import { type Dialect } from "@/types";

/** Datalists the type and default inputs suggest from (`lt-types-<driver>`…). */
export function TypeSuggestions({ driver }: { driver: Dialect }) {
  return (
    <>
      <datalist id={`lt-types-${driver}`}>
        {SQL_TYPES[driver].map((t) => (
          <option key={t} value={t} />
        ))}
      </datalist>
      <datalist id={`lt-defaults-${driver}`}>
        {SQL_DEFAULTS[driver].map((t) => (
          <option key={t} value={t} />
        ))}
      </datalist>
    </>
  );
}
