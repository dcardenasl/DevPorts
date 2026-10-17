import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

// The panel is loaded from the application bundle, so assets use relative URLs.
export default defineConfig({
  base: "./",
  plugins: [react()],
  build: { outDir: "dist", target: "safari15", sourcemap: false },
  server: { port: 5173, strictPort: true },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./vitest.setup.ts"],
  },
});
