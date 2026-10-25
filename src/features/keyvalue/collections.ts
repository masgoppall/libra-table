// Hashes, lists, sets and sorted sets as tables of rows: which columns can be edited,
// and the edit each change, deletion or new row sends.

import { type KeyData, type KeyEdit, type RedisValue } from "./types";

export interface CollectionRow {
  id: string;
  cells: RedisValue[];
}

export interface AddAction {
  label: string;
  edit: (values: string[]) => KeyEdit | string;
}

export interface CollectionSpec {
  columns: { label: string; editable: boolean }[];
  rows: CollectionRow[];
  /** The edit for a changed row (values as typed), or an error message. */
  update: (row: number, values: string[]) => KeyEdit | string | null;
  remove: (row: number) => KeyEdit;
  add: AddAction[];
}

type Collection = Exclude<KeyData, { kind: "string" | "stream" | "other" | "missing" }>;

const id = (v: RedisValue) => (typeof v === "object" && v ? v.$binary : String(v));

function score(text: string): number | string {
  const n = Number(text.trim());
  return text.trim() === "" || Number.isNaN(n) ? "The score must be a number." : n;
}

function zsetEdit(key: RedisValue, member: RedisValue, s: string, previous?: RedisValue) {
  const n = score(s);
  if (typeof n === "string") return n;
  return { op: "zset_add" as const, key, member, score: n, previous };
}

/** Rows and edits of one collection value. */
export function collectionSpec(key: RedisValue, v: Collection): CollectionSpec {
  switch (v.kind) {
    case "hash":
      return {
        columns: [
          { label: "Field", editable: true },
          { label: "Value", editable: true },
        ],
        rows: v.entries.map((e) => ({ id: id(e.field), cells: [e.field, e.value] })),
        update: (r, [field, value]) => {
          const old = v.entries[r];
          if (!old || field === undefined || value === undefined) return null;
          const previous = field !== old.field ? old.field : undefined;
          return { op: "hash_set", key, field, value, previous };
        },
        remove: (r) => ({ op: "hash_delete", key, fields: [v.entries[r]?.field ?? ""] }),
        add: [
          {
            label: "Add field",
            edit: ([field = "", value = ""]) =>
              field ? { op: "hash_set", key, field, value } : "Enter a field name.",
          },
        ],
      };
    case "list":
      return {
        columns: [
          { label: "#", editable: false },
          { label: "Value", editable: true },
        ],
        rows: v.items.map((i) => ({ id: String(i.index), cells: [i.index, i.value] })),
        update: (r, [, value]) => {
          const old = v.items[r];
          if (!old || value === undefined) return null;
          return { op: "list_set", key, index: old.index, value, expected: old.value };
        },
        remove: (r) => {
          const old = v.items[r];
          return { op: "list_remove", key, index: old?.index ?? 0, expected: old?.value ?? "" };
        },
        add: [
          { label: "Add to end", edit: ([, value = ""]) => push(key, value, false) },
          { label: "Add to start", edit: ([, value = ""]) => push(key, value, true) },
        ],
      };
    case "set":
      return {
        columns: [{ label: "Member", editable: false }],
        rows: v.members.map((m) => ({ id: id(m), cells: [m] })),
        update: () => null,
        remove: (r) => ({ op: "set_remove", key, members: [v.members[r] ?? ""] }),
        add: [
          {
            label: "Add member",
            edit: ([member = ""]) => ({ op: "set_add", key, members: [member] }),
          },
        ],
      };
    case "zset":
      return {
        columns: [
          { label: "Member", editable: true },
          { label: "Score", editable: true },
        ],
        rows: v.members.map((m) => ({ id: id(m.member), cells: [m.member, m.score] })),
        update: (r, [member, s]) => {
          const old = v.members[r];
          if (!old || member === undefined || s === undefined) return null;
          return zsetEdit(key, member, s, member !== old.member ? old.member : undefined);
        },
        remove: (r) => ({ op: "zset_remove", key, members: [v.members[r]?.member ?? ""] }),
        add: [{ label: "Add member", edit: ([member = "", s = ""]) => zsetEdit(key, member, s) }],
      };
  }
}

function push(key: RedisValue, value: string, head: boolean): KeyEdit {
  return { op: "list_push", key, values: [value], head };
}
