import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// `--mode visual` builds the fixture-backed bundle used only by the visual
// verifier (dist-visual/). The production bundle (dist/) never contains it.
export default defineConfig(({ mode }) => ({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true, host: "127.0.0.1" },
  build: {
    outDir: mode === "visual" ? "dist-visual" : "dist",
    target: "es2022",
    sourcemap: false,
    assetsInlineLimit: 0,
  },
  envPrefix: ["VITE_", "TAURI_ENV_"],
}));
