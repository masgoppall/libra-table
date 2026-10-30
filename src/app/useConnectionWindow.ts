// A connection window opens its connection once the saved connections are known, and
// shows the tab moved into it, if any.

import { useEffect } from "react";
import { movedTab, windowConnection } from "@/lib/windowRole";
import { openConnection } from "@/services/connect";
import { showError } from "@/services/errors";
import { store, useStore } from "@/store";

export function useConnectionWindow() {
  const conn = useStore((s) => s.connections.find((c) => c.id === windowConnection));
  useEffect(() => {
    if (!conn) return;
    void openConnection(conn)
      .then((ok) => {
        const tab = movedTab();
        if (ok && tab) store.get().addTab(conn.id, tab);
      })
      .catch(showError);
  }, [conn?.id]);
}
