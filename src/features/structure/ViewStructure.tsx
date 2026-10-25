import { type TableStructure } from "@/types";
import { DdlBlock } from "./DdlBlock";

/** Views are read-only: their columns and definition. */
export function ViewStructure({ structure }: { structure: TableStructure }) {
  return (
    <div className="designer">
      <h3 className="designer-h">Columns</h3>
      <table className="plain">
        <thead>
          <tr>
            <th>Name</th>
            <th>Type</th>
            <th>Nullable</th>
          </tr>
        </thead>
        <tbody>
          {structure.columns.map((c) => (
            <tr key={c.name}>
              <td className="selectable">{c.name}</td>
              <td className="mono selectable">{c.dataType}</td>
              <td>{c.nullable ? "YES" : "NO"}</td>
            </tr>
          ))}
        </tbody>
      </table>
      <DdlBlock ddl={structure.ddl} />
    </div>
  );
}
