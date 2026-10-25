// Schema objects (functions, sequences, triggers…) and the sidebar's recent/pinned items.

import {
  type Db,
  type ObjectDefinition,
  type SchemaObject,
  type SchemaObjectRef,
  type SequenceInfo,
  type SidebarState,
} from "@/types";
import { call } from "./client";

export const objectsApi = {
  listSchemaObjects: (id: string, db: Db, schema: string) =>
    call<SchemaObject[]>("list_schema_objects", { id, database: db, schema }),
  objectDefinition: (id: string, db: Db, schema: string, object: SchemaObjectRef) =>
    call<ObjectDefinition>("object_definition", { id, database: db, schema, object }),
  sequenceInfo: (id: string, db: Db, schema: string, name: string) =>
    call<SequenceInfo>("sequence_info", { id, database: db, schema, name }),
  getSidebarState: (id: string) => call<SidebarState>("get_sidebar_state", { id }),
  saveSidebarState: (id: string, sidebar: SidebarState) =>
    call<void>("save_sidebar_state", { id, sidebar }),
};
