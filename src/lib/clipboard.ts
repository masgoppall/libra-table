// Clipboard writes go through the Tauri plugin: WebKit only lets a page write to the
// clipboard during a click or key press, and many copies here first await Rust.

import { writeText } from "@tauri-apps/plugin-clipboard-manager";

export async function copyText(text: string): Promise<void> {
  try {
    await writeText(text);
  } catch {
    // Outside Tauri (tests, browser preview).
    await navigator.clipboard.writeText(text);
  }
}
