import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  envPrefix: ["VITE_", "TAURI_"],
  server: { port: 1420, strictPort: true },
  build: {
    target: "chrome120",
    rollupOptions: {
      input: {
        index: "index.html",
        settings: "settings.html",
      },
    },
  },
  resolve: { dedupe: ["react", "react-dom"] },
  // Vitest empties CSS by default; the token tests read the CSS files through ?raw.
  test: {
    environment: "jsdom",
    passWithNoTests: true,
    css: { include: [/\.css/] },
    setupFiles: ["./src/test-setup.ts"],
  },
});
