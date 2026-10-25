// One key for the value panel: the first page of its value, "Load more" for big
// collections, and a reload after edits.

import { useCallback, useEffect, useState } from "react";
import { toAppError } from "@/api";
import { type AppError } from "@/types";
import { redisApi } from "./api";
import { type KeyTarget } from "./edits";
import { type KeyData, type KeyDetail, type RedisValue } from "./types";

/** Elements per page of a collection. */
export const VALUE_PAGE = 200;

/** `more` appended to `base` (same kind of value). */
function append(base: KeyData, more: KeyData): KeyData {
  if (base.kind === "hash" && more.kind === "hash") {
    return { ...base, entries: [...base.entries, ...more.entries] };
  }
  if (base.kind === "list" && more.kind === "list") {
    return { ...base, items: [...base.items, ...more.items] };
  }
  if (base.kind === "set" && more.kind === "set") {
    return { ...base, members: [...base.members, ...more.members] };
  }
  if (base.kind === "zset" && more.kind === "zset") {
    return { ...base, members: [...base.members, ...more.members] };
  }
  if (base.kind === "stream" && more.kind === "stream") {
    return { ...base, entries: [...base.entries, ...more.entries] };
  }
  return more;
}

export function useKeyDetail(t: KeyTarget, key: RedisValue) {
  const [detail, setDetail] = useState<KeyDetail | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  const [loading, setLoading] = useState(false);

  const reload = useCallback(async () => {
    setLoading(true);
    try {
      setDetail(await redisApi.readKey(t.connectionId, t.db, key, "", VALUE_PAGE));
      setError(null);
    } catch (e) {
      setError(toAppError(e));
    } finally {
      setLoading(false);
    }
  }, [t.connectionId, t.db, key]);

  useEffect(() => {
    void reload();
  }, [reload]);

  const loadMore = async () => {
    if (!detail?.next) return;
    setLoading(true);
    try {
      const more = await redisApi.readKey(t.connectionId, t.db, key, detail.next, VALUE_PAGE);
      setDetail({ ...more, value: append(detail.value, more.value) });
    } catch (e) {
      setError(toAppError(e));
    } finally {
      setLoading(false);
    }
  };

  return { detail, error, loading, reload, loadMore };
}
