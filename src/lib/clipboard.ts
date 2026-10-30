// Clipboard writes go through the Tauri plugin: WebKit only lets a page write to the
// clipboard during a click or key press, and many copies here first await Rust. Reads go
// through it too: WebKit's own read shows a "Paste" button the user must click.

import { readText as pluginRead, writeText } from "@tauri-apps/plugin-clipboard-manager";

export async function copyText(text: string): Promise<void> {
  try {
    await writeText(text);
  } catch {
    // Outside Tauri (tests, browser preview).
    await navigator.clipboard.writeText(text);
  }
}

/** The clipboard's text ("" when it holds none). */
export async function readText(): Promise<string> {
  try {
    return (await pluginRead()) ?? "";
  } catch {
    try {
      return await navigator.clipboard.readText();
    } catch {
      return "";
    }
  }
}
