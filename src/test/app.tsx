// Renders the whole app against the fake backend, with helpers to drive it.

import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach } from "vitest";
import App from "@/app/App";
import { IS_MAC, resetActions } from "@/lib/actions";
import { HOME, useStore } from "@/store";
import { FakeBackend } from "./fakeBackend";
import { installJsdomShims } from "./jsdom";

const INITIAL = useStore.getState();

/** Sets up shims, a fresh backend and a fresh store for every test of the file. */
export function setupApp() {
  installJsdomShims();
  const harness = { backend: new FakeBackend() };
  beforeEach(() => {
    resetActions();
    harness.backend = new FakeBackend();
    harness.backend.install();
    useStore.setState({ ...INITIAL, view: HOME }, true);
  });
  afterEach(() => cleanup());
  return harness;
}

export function renderApp() {
  return render(<App />);
}

const MOD = IS_MAC ? { metaKey: true } : { ctrlKey: true };

/** Presses a shortcut like the user would (physical key code + modifiers). */
export function press(code: string, key: string, mods: Record<string, boolean> = {}) {
  const target = (document.activeElement as HTMLElement | null) ?? document.body;
  fireEvent.keyDown(target, { code, key, ...MOD, ...mods });
}

export async function openWorkspace(name: string) {
  fireEvent.doubleClick(await screen.findByText(name));
  await screen.findByText("people");
}

export const sidebar = () => document.querySelector(".sidebar") as HTMLElement;
export const tabs = () => Array.from(document.querySelectorAll<HTMLElement>(".tabbar .tab"));
export const tabAt = (i: number) => tabs()[i] as HTMLElement;

export function query<T extends Element = HTMLElement>(
  selector: string,
  root: ParentNode = document,
) {
  const el = root.querySelector<T>(selector);
  if (!el) throw new Error(`No element matches ${selector}`);
  return el;
}

export function queryAll<T extends Element = HTMLElement>(
  selector: string,
  root: ParentNode = document,
) {
  return Array.from(root.querySelectorAll<T>(selector));
}
