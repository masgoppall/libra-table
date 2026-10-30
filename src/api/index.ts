// Typed wrappers around the Rust commands (src-tauri/src/commands), one module per domain.

import { appApi } from "./app";
import { connectionsApi } from "./connections";
import { dataApi } from "./data";
import { diagramApi } from "./diagram";
import { aiApi } from "./ai";
import { exportApi } from "./export";
import { metadataApi } from "./metadata";
import { metricsApi } from "./metrics";
import { objectsApi } from "./objects";
import { historyApi, queryApi, savedQueriesApi } from "./query";
import { schemaChangesApi } from "./schemaChanges";
import { serverApi, toolsApi } from "./server";
import { transferApi } from "./transfer";

export const api = {
  ...appApi,
  ...connectionsApi,
  ...metadataApi,
  ...dataApi,
  ...schemaChangesApi,
  ...queryApi,
  ...historyApi,
  ...savedQueriesApi,
  ...exportApi,
  ...aiApi,
  ...serverApi,
  ...toolsApi,
  ...objectsApi,
  ...transferApi,
  ...diagramApi,
  ...metricsApi,
};

export { isAppError, toAppError } from "./client";
