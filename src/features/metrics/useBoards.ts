// The connection's metrics boards: loaded from Rust, saved on every change.

import { useEffect, useState } from "react";
import { api } from "@/api";
import { showError } from "@/services/errors";
import { store } from "@/store";
import { type MetricBoard } from "@/types";
import { newBoard } from "./boardModel";

export function useBoards(connectionId: string) {
  const [boards, setBoards] = useState<MetricBoard[] | null>(null);
  const [activeId, setActiveId] = useState<string | null>(null);

  useEffect(() => {
    api
      .listMetricBoards(connectionId)
      .then((bs) => {
        setBoards(bs);
        setActiveId((a) => a ?? bs[0]?.id ?? null);
      })
      .catch((e) => (showError(e), setBoards([])));
  }, [connectionId]);

  const save = (board: MetricBoard) => {
    setBoards((bs) => {
      const list = bs ?? [];
      return list.some((b) => b.id === board.id)
        ? list.map((b) => (b.id === board.id ? board : b))
        : [...list, board];
    });
    api.saveMetricBoard(connectionId, board).catch(showError);
  };

  const askName = async (title: string, initial: string) => {
    const r = await store.get().dialog({
      kind: "prompt",
      title,
      label: "Name",
      initial,
      okLabel: "Save",
    });
    return r.ok ? (r.value ?? "").trim() : "";
  };

  const create = async () => {
    const name = await askName("New metrics board", "Dashboard");
    if (!name) return;
    const b = newBoard(name);
    save(b);
    setActiveId(b.id);
  };

  const rename = async (b: MetricBoard) => {
    const name = await askName("Rename board", b.name);
    if (name) save({ ...b, name });
  };

  const remove = async (b: MetricBoard) => {
    const r = await store.get().dialog({
      kind: "confirm",
      title: "Delete board?",
      message: `“${b.name}” and its widgets will be deleted. Queries in the database stay.`,
      okLabel: "Delete board",
      danger: true,
    });
    if (!r.ok) return;
    const rest = (boards ?? []).filter((x) => x.id !== b.id);
    setBoards(rest);
    setActiveId(rest[0]?.id ?? null);
    api.deleteMetricBoard(connectionId, b.id).catch(showError);
  };

  const active = boards?.find((b) => b.id === activeId) ?? null;
  return { boards, active, setActiveId, save, create, rename, remove };
}

export type Boards = ReturnType<typeof useBoards>;
