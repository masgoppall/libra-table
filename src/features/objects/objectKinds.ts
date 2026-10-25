// The sidebar sections of schema objects, in order, with their icons.

import {
  CalendarClock,
  Layers,
  ListOrdered,
  type LucideIcon,
  Shapes,
  SquareFunction,
  Workflow,
  Zap,
} from "lucide-react";
import { objectRef } from "@/domain/objects";
import { type NewTab, type SchemaObjectKind, type SchemaObjectRef } from "@/types";

export interface ObjectSection {
  kind: SchemaObjectKind;
  /** Section title. */
  label: string;
  icon: LucideIcon;
}

export const OBJECT_SECTIONS: ObjectSection[] = [
  { kind: "materialized_view", label: "Materialized views", icon: Layers },
  { kind: "function", label: "Functions", icon: SquareFunction },
  { kind: "procedure", label: "Procedures", icon: Workflow },
  { kind: "trigger", label: "Triggers", icon: Zap },
  { kind: "sequence", label: "Sequences", icon: ListOrdered },
  { kind: "type", label: "Types", icon: Shapes },
  { kind: "event", label: "Events", icon: CalendarClock },
];

export function objectIcon(kind: SchemaObjectKind): LucideIcon {
  return OBJECT_SECTIONS.find((s) => s.kind === kind)?.icon ?? Shapes;
}

/** The read-only definition tab of an object (reused when already open). */
export function definitionTab(o: SchemaObjectRef, where: { schema: string }): NewTab {
  return { kind: "definition", title: o.name, schema: where.schema, object: objectRef(o) };
}
