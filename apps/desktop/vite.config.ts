import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

// https://v2.tauri.app/start/frontend/vite/
export default defineConfig({
  plugins: [react()],
  // Garde les erreurs du compilateur Rust visibles dans le terminal.
  clearScreen: false,
  server: {
    // Doit correspondre à `build.devUrl` dans `src-tauri/tauri.conf.json`.
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
  test: {
    include: ["src/**/*.test.{ts,tsx}"],
  },
});
