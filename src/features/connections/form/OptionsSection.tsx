import { Check } from "@/components/Check";
import { type ConnectionFormState } from "./useConnectionForm";

const DEFAULT_PAGE_SIZE = 300;
const DEFAULT_ROW_LIMIT = 1000;

export function OptionsSection({ form }: { form: ConnectionFormState }) {
  const { c, setOptions } = form;
  return (
    <div className="form-grid form-section">
      <label>Rows per page</label>
      <div className="row">
        <input
          className="input num-input"
          type="number"
          min={1}
          max={5000}
          value={c.options.pageSize}
          onChange={(e) => setOptions({ pageSize: Number(e.target.value) || DEFAULT_PAGE_SIZE })}
        />
        <span className="muted">Query row limit</span>
        <input
          className="input num-input wide"
          type="number"
          min={1}
          max={100000}
          value={c.options.queryRowLimit}
          onChange={(e) =>
            setOptions({ queryRowLimit: Number(e.target.value) || DEFAULT_ROW_LIMIT })
          }
        />
        {c.driver !== "sqlite" && (
          <Check
            checked={c.options.showSystemSchemas}
            onChange={(showSystemSchemas) => setOptions({ showSystemSchemas })}
          >
            Show system schemas
          </Check>
        )}
      </div>
    </div>
  );
}
