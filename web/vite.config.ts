import { defineConfig } from "vitest/config";

export default defineConfig({
  // The CLI fixtures are the demo documents. Serving them as static files
  // keeps one source of truth for `md fixtures/showcase.md` and the website.
  publicDir: "../fixtures",
  build: {
    outDir: "dist",
    emptyOutDir: true,
    sourcemap: true,
  },
  server: {
    port: 5173,
  },
  test: {
    environment: "happy-dom",
    include: ["src/**/*.test.ts"],
  },
});
