// @vitest-environment jsdom
// Closing the window (or ⌘W with no tab left) on a connection goes back to the connections page.

import { act, screen, waitFor } from "@testing-library/react";
import { emit } from "@tauri-apps/api/event";
import { describe, expect, it } from "vitest";
import { HOME, store } from "@/store";
import { openWorkspace, press, renderApp, setupApp, tabs } from "@/test/app";

const h = setupApp();

describe("closing the window", () => {
  it("disconnects the visible connection and shows the connections page", async () => {
    renderApp();
    await openWorkspace("Local Postgres");
    await waitFor(() =>
      expect(h.backend.lastCall("set_window_view")?.args).toEqual({ inWorkspace: true }),
    );
    await act(() => emit("close-requested"));
    await waitFor(() => expect(store.get().view).toBe(HOME));
    expect(h.backend.callsOf("disconnect").length).toBe(1);
    expect(store.get().workspaces).toEqual({});
    await waitFor(() =>
      expect(h.backend.lastCall("set_window_view")?.args).toEqual({ inWorkspace: false }),
    );
  });

  it("closes the connection with ⌘W once no tab is left", async () => {
    renderApp();
    await openWorkspace("Local Postgres");
    while (tabs().length > 0) act(() => press("KeyW", "w"));
    expect(store.get().view).not.toBe(HOME);
    act(() => press("KeyW", "w"));
    await waitFor(() => expect(store.get().view).toBe(HOME));
    expect(await screen.findByText("Local Postgres")).toBeTruthy();
  });
});
