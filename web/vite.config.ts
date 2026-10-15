import { copyFileSync } from "node:fs";
import { resolve } from "node:path";
import { defineConfig } from "vitest/config";

export default defineConfig({
  // The CLI fixtures are the demo documents. Serving them as static files
  // keeps one source of truth for `md fixtures/showcase.md` and the website.
  publicDir: "../fixtures",
  plugins: [
    {
      // Serve the curl | bash installer from the site root.
      name: "install-sh",
      closeBundle() {
        copyFileSync(resolve(__dirname, "../install.sh"), resolve(__dirname, "dist/install.sh"));
      },
    },
  ],
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
