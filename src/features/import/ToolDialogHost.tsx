// The workspace's open tool dialog: import wizard, SQL file runner or multi-table export.

import { useStore } from "@/store";
import { type Workspace } from "@/types";
import { ExportTablesDialog } from "../export/ExportTablesDialog";
import { ImportWizard } from "./ImportWizard";
import { SqlFileDialog } from "./SqlFileDialog";

export function ToolDialogHost({ ws }: { ws: Workspace }) {
  const dialog = useStore((s) => s.toolDialog);
  const close = useStore((s) => s.closeToolDialog);
  if (!dialog || dialog.connectionId !== ws.connectionId) return null;
  switch (dialog.kind) {
    case "import":
      return <ImportWizard dialog={dialog} driver={ws.info.dialect} onClose={close} />;
    case "sql-file":
      return <SqlFileDialog dialog={dialog} onClose={close} />;
    case "export-tables":
      return <ExportTablesDialog dialog={dialog} onClose={close} />;
  }
}
