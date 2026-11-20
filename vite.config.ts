import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

/** Dev server host for mobile Tauri targets; unset on desktop. */
const HOST = globalThis.process?.env?.TAURI_DEV_HOST;

/**
 * Vite config for the Tauri window: fixed port 1420, no screen clearing, src-tauri not watched.
 *
 * @example
 * // pnpm dev   -> http://localhost:1420
 * // pnpm build -> dist/
 */
export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: HOST || false,
    hmr: HOST ? { protocol: "ws", host: HOST, port: 1421 } : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
  },
});
