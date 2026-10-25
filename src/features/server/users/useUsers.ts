// The users tab's data: roles or accounts, the selected one and what it was granted.

import { useCallback, useEffect, useState } from "react";
import { api } from "@/api";
import { type AppError, type Db, type Grantee, type UserList } from "@/types";

const EMPTY: UserList = { roles: [], accounts: [], privilegeOptions: [] };

export function useUsers(connectionId: string, db: Db, visible: boolean) {
  const [list, setList] = useState<UserList>(EMPTY);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<AppError | null>(null);
  const [selected, setSelected] = useState<Grantee | null>(null);
  const [grants, setGrants] = useState<string[] | null>(null);
  const [text, setText] = useState("");

  const load = useCallback(async () => {
    setLoading(true);
    try {
      setList(await api.listUsers(connectionId));
      setError(null);
    } catch (e) {
      setError(e as AppError);
    } finally {
      setLoading(false);
    }
  }, [connectionId]);

  const loadGrants = useCallback(async () => {
    if (!selected) return setGrants(null);
    try {
      setGrants(await api.userGrants(connectionId, db, selected));
    } catch (e) {
      setGrants([]);
      setError(e as AppError);
    }
  }, [connectionId, db, selected]);

  useEffect(() => {
    if (visible) void load();
  }, [visible, load]);
  useEffect(() => {
    void loadGrants();
  }, [loadGrants]);

  /** After a change: the list, and the selected user's grants. */
  const reload = () => {
    void load();
    void loadGrants();
  };

  return { list, loading, error, selected, setSelected, grants, reload, text, setText };
}

export type Users = ReturnType<typeof useUsers>;
