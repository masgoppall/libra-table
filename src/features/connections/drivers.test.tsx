// @vitest-environment jsdom
// The driver picker groups every driver; engine drivers get a form from the catalog.

import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { renderApp, setupApp } from "@/test/app";

const h = setupApp();

async function openPicker() {
  renderApp();
  fireEvent.click(await screen.findByText("New connection"));
  return screen.findByRole("dialog");
}

describe("driver picker", () => {
  it("groups drivers and keeps the ones not built yet disabled", async () => {
    const dialog = await openPicker();
    expect(within(dialog).getByText("Relational")).toBeTruthy();
    expect(within(dialog).getByText("Cloud")).toBeTruthy();
    const oracle = within(dialog).getByText("Oracle").closest("button") as HTMLButtonElement;
    expect(oracle.disabled).toBe(true);
    expect(within(oracle).getByText("Coming soon")).toBeTruthy();
    fireEvent.change(within(dialog).getByPlaceholderText("Search drivers"), {
      target: { value: "snow" },
    });
    await waitFor(() => expect(within(dialog).queryByText("PostgreSQL")).toBeNull());
    expect(within(dialog).getByText("Snowflake")).toBeTruthy();
  });

  it("builds the form of an engine driver from its fields", async () => {
    const dialog = await openPicker();
    fireEvent.click(within(dialog).getByText("Snowflake"));
    fireEvent.click(within(dialog).getByText("Continue"));
    expect(await screen.findByText("Snowflake Connection")).toBeTruthy();
    fireEvent.change(screen.getByPlaceholderText("My database"), { target: { value: "WH" } });
    fireEvent.change(screen.getByPlaceholderText("xy12345.eu-central-1"), {
      target: { value: "ab123.eu-west-1" },
    });
    // No SSH or SSL sections for an HTTPS-only service.
    expect(screen.queryByText("SSL mode")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: /^Save$/ }));
    await waitFor(() => expect(h.backend.callsOf("save_connection").length).toBe(1));
    const c = h.backend.lastCall("save_connection")?.args.input.connection;
    expect(c.driver).toBe("snowflake");
    expect(c.db.extra).toEqual({ account: "ab123.eu-west-1" });
  });
});
