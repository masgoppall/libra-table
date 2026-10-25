// @vitest-environment jsdom
// Safe mode levels and read-only connections in the data tab.

import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { openWorkspace, query, renderApp, setupApp, sidebar } from "@/test/app";
import { type ConnectionOptions } from "@/types";

const h = setupApp();

function setOptions(patch: Partial<ConnectionOptions>) {
  const c = h.backend.connections[0];
  if (c) c.options = { ...c.options, ...patch };
}

async function openPeople() {
  renderApp();
  await openWorkspace("Local Postgres");
  fireEvent.click(within(sidebar()).getByText("people"));
  return screen.findByText("Bob");
}

describe("safe mode", () => {
  it("asks to type the connection name for 'Safe: writes'", async () => {
    setOptions({ safeMode: "safe_writes" });
    fireEvent.doubleClick(await openPeople());
    const editor = query<HTMLInputElement>(".grid-cell input");
    fireEvent.change(editor, { target: { value: "Robert" } });
    fireEvent.keyDown(editor, { key: "Enter" });
    fireEvent.click(await screen.findByRole("button", { name: /^Commit$/ }));
    const dialog = await screen.findByRole("dialog");
    const ok = within(dialog).getByRole("button", { name: "Commit" }) as HTMLButtonElement;
    expect(ok.disabled).toBe(true);
    fireEvent.change(within(dialog).getByRole("textbox"), { target: { value: "Local Postgres" } });
    expect(ok.disabled).toBe(false);
    await act(async () => {
      fireEvent.click(ok);
    });
    await waitFor(() => expect(h.backend.callsOf("commit_changes").length).toBe(1));
  });

  it("doesn't edit rows on a read-only connection", async () => {
    setOptions({ readOnly: true });
    const bob = await openPeople();
    expect(screen.getByText("Read-only connection")).toBeTruthy();
    expect(document.querySelector(".toptab .read-only")).toBeTruthy();
    fireEvent.doubleClick(bob);
    expect(document.querySelector(".grid-cell input")).toBeNull();
  });
});
