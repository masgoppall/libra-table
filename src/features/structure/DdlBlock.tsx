import { Copy } from "lucide-react";
import { copyText } from "@/lib/clipboard";
import { toastSuccess } from "@/services/errors";

export function DdlBlock({ ddl }: { ddl: string }) {
  const copy = async () => {
    await copyText(ddl);
    toastSuccess("DDL copied.");
  };
  return (
    <div className="sql-box">
      <div className="sql-box-head">
        DDL
        <span className="spacer" />
        <button className="btn ghost xs" onClick={copy}>
          <Copy size={12} /> Copy
        </button>
      </div>
      <pre className="mono selectable">{ddl}</pre>
    </div>
  );
}
