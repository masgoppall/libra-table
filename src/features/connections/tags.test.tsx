// @vitest-environment jsdom
// Custom tags: managed in Settings, picked per connection, shown and searched in the list.

import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { press, renderApp, setupApp } from "@/test/app";

const h = setupApp();

describe("custom tags", () => {
  it("shows a connection's tags and finds it by them", async () => {
    h.backend.settings = {
      ...h.backend.settings,
      customTags: [{ name: "Billing", color: "#30a46c" }],
    };
    h.backend.connections = h.backend.connections.map((c) =>
      c.name === "Prod Shop" ? { ...c, labels: ["Billing", "gone"] } : c,
    );
    renderApp();
    expect(await screen.findByText("Billing")).toBeTruthy();
    expect(screen.queryByText("gone")).toBeNull();
    fireEvent.change(screen.getByPlaceholderText("Search for connection..."), {
      target: { value: "billing" },
    });
    await waitFor(() => expect(screen.queryByText("Local Postgres")).toBeNull());
  });

  it("adds a tag in Settings", async () => {
    renderApp();
    await screen.findByText("Local Postgres");
    press("Comma", ",");
    const dialog = await screen.findByRole("dialog");
    fireEvent.click(within(dialog).getByText("Add tag"));
    fireEvent.change(within(dialog).getByLabelText("Tag name"), { target: { value: "Ops" } });
    await act(async () => fireEvent.click(within(dialog).getByText("Save")));
    await waitFor(() =>
      expect(h.backend.lastCall("save_settings")?.args.settings.customTags).toEqual([
        { name: "Ops", color: "#3e63dd" },
      ]),
    );
  });
});
