// ER diagram: the schema's tables in one call, saved positions, image files.

import { type Db, type DiagramLayout, type SchemaDiagram } from "@/types";
import { call } from "./client";

export const diagramApi = {
  schemaDiagram: (id: string, db: Db, schema: string) =>
    call<SchemaDiagram>("schema_diagram", { id, database: db, schema }),
  getDiagramLayout: (id: string, key: string) =>
    call<DiagramLayout>("get_diagram_layout", { id, key }),
  saveDiagramLayout: (id: string, key: string, layout: DiagramLayout) =>
    call<void>("save_diagram_layout", { id, key, layout }),
  saveDiagramFile: (path: string, file: { svg?: string; pngBase64?: string }) =>
    call<void>("save_diagram_file", {
      path,
      svg: file.svg ?? null,
      pngBase64: file.pngBase64 ?? null,
    }),
};
