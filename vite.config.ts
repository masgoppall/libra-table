import { fileURLToPath } from "node:url";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";
import { brandArt, pruneBrand } from "./vite.brand";

const host = process.env.TAURI_DEV_HOST;

// Tauri expects a fixed port and must not have Vite clear the Rust output.
export default defineConfig({
  plugins: [react(), brandArt(fileURLToPath(new URL("./public", import.meta.url))), pruneBrand()],
  resolve: {
    alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) },
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: {
    target: "safari15",
    sourcemap: false,
  },
  test: {
    environment: "node",
    include: ["src/**/*.test.ts", "src/**/*.test.tsx"],
    // UI flows load lazy tabs (editor, diagram, metrics); give a busy machine room.
    testTimeout: 15000,
  },
});
