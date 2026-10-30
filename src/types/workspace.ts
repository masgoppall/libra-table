// An open connection on screen: its sidebar position and its tabs.

import { type Capabilities } from "./schema";
import { type ConnectInfo } from "./connection";
import { type SchemaObjectRef } from "./objects";
import { type QueryVariable } from "./queryTools";

export type TabKind =
  | "data"
  | "structure"
  | "query"
  | "history"
  | "new-table"
  | "processes"
  | "users"
  | "definition"
  | "diagram"
  | "metrics";

export interface WorkspaceTab {
  id: string;
  kind: TabKind;
  title: string;
  /** Database of the server the tab works in (tabs of several databases can be open). */
  database?: string;
  schema?: string;
  table?: string;
  /** Query tabs: editor text (kept across restarts). */
  sql?: string;
  /** Query tabs opened from a saved query. */
  savedName?: string;
  /** Query tabs: run once when first shown ("Run again" from history). */
  autoRun?: boolean;
  /** Data tabs: filter to apply once when shown (opened from a foreign key). */
  pendingFilter?: { column: string; value: string };
  /** Definition tabs: the function, sequence, trigger… shown. */
  object?: SchemaObjectRef;
  /** Query tabs: the last values given to `:name` variables. */
  variables?: QueryVariable[];
  /** What the tab showed before (Back) and after (Forward) opening something in place. */
  back?: TabPlace[];
  forward?: TabPlace[];
}

/** What a tab shows, as kept in its Back / Forward history. */
export type TabPlace = Pick<
  WorkspaceTab,
  "kind" | "title" | "database" | "schema" | "table" | "object"
>;

export type NewTab = Omit<WorkspaceTab, "id">;

export interface Workspace {
  connectionId: string;
  info: ConnectInfo;
  caps: Capabilities;
  /** Database and schema the sidebar shows; new tabs start there. */
  database: string;
  /** Databases opened in this connection, in the order opened (the database rail). */
  databases?: string[];
  schema: string;
  tabs: WorkspaceTab[];
  activeTabId: string | null;
  /** Bumped to make the sidebar and open views reload metadata. */
  refreshKey: number;
  sidebarHidden?: boolean;
}
