import { defineConfig } from "vitest/config";

// The review surface is a static site (ADR-0009). `base: "./"` keeps asset URLs
// relative so the built `dist/` works served from any path. Vitest runs the
// pure lib/*.test.ts under Node — no DOM environment needed (those modules are
// deliberately DOM-free).
export default defineConfig({
  base: "./",
  // Vite's proxy forwards the browser's Host unless changeOrigin is set, and `sona serve` refuses any Host but its own.
  // It refuses a foreign Origin too, so this dev server's own pages are passed on as the server's.
  server: {
    port: 5173,
    strictPort: true,
    proxy: {
      "/api": {
        target: "http://127.0.0.1:8977",
        changeOrigin: true,
        configure: (proxy) =>
          proxy.on("proxyReq", (req) => {
            if (/^http:\/\/(localhost|127\.0\.0\.1):5173$/.test(String(req.getHeader("origin") ?? ""))) req.setHeader("origin", "http://127.0.0.1:8977");
          }),
      },
    },
  },
  test: {
    environment: "node",
    include: ["lib/**/*.test.ts"],
  },
});
