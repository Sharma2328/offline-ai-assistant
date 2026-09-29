/// <reference types="vitest/config" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { fileURLToPath, URL } from "node:url";

// Fixed dev port so the Tauri host can point `devUrl` at a known address.
const DEV_PORT = 1420;

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      // The shared design system is consumed from source so Vite transforms its
      // TSX with the app's pipeline (no separate build step in dev).
      "@offline-ai/ui": fileURLToPath(new URL("../../packages/ui/src/index.ts", import.meta.url)),
      "@": fileURLToPath(new URL("./src", import.meta.url)),
      "@app": fileURLToPath(new URL("./src/app", import.meta.url)),
      "@features": fileURLToPath(new URL("./src/features", import.meta.url)),
      "@lib": fileURLToPath(new URL("./src/lib", import.meta.url)),
      "@styles": fileURLToPath(new URL("./src/styles", import.meta.url)),
    },
  },
  // Tauri expects a fixed port and does not tolerate it changing.
  clearScreen: false,
  server: {
    port: DEV_PORT,
    strictPort: true,
    host: "127.0.0.1",
  },
  build: {
    // Match the Rust toolchain target features; keep sourcemaps for local debugging.
    target: "es2022",
    sourcemap: true,
    outDir: "dist",
    emptyOutDir: true,
  },
  test: {
    globals: true,
    include: ["src/**/*.test.{ts,tsx}"],
    environment: "jsdom",
    setupFiles: ["./src/test/setup.ts"],
    css: false,
  },
});
