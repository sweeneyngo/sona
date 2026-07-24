import { defineConfig } from "vitest/config";

// The review surface is a static site (ADR-0009). `base: "./"` keeps asset URLs
// relative so the built `dist/` works served from any path. Vitest runs the
// pure lib/*.test.ts under Node — no DOM environment needed (those modules are
// deliberately DOM-free).
export default defineConfig({
  base: "./",
  server: { port: 5173, strictPort: true },
  test: {
    environment: "node",
    include: ["lib/**/*.test.ts"],
  },
});
