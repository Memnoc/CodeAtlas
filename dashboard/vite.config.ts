import { readFileSync } from "node:fs";
import path from "node:path";
import react from "@vitejs/plugin-react";
import { defineConfig, type Plugin } from "vitest/config";

/// Dev-only map loading: serves a local map file at /api/map so the dev
/// server mirrors ticket 09's binary serving. The file is CODEATLAS_MAP if
/// set, else this repo's own self-scan output. Everything stays on loopback;
/// no request ever leaves the machine.
function localMap(): Plugin {
  return {
    name: "codeatlas-local-map",
    configureServer(server) {
      server.middlewares.use("/api/map", (_req, res) => {
        const mapPath = process.env["CODEATLAS_MAP"]
          ? path.resolve(process.env["CODEATLAS_MAP"])
          : path.resolve(import.meta.dirname, "../.codeatlas/knowledge-graph.json");
        try {
          const body = readFileSync(mapPath, "utf8");
          res.setHeader("Content-Type", "application/json");
          res.end(body);
        } catch {
          res.statusCode = 404;
          res.setHeader("Content-Type", "application/json");
          res.end(
            JSON.stringify({
              error: `no map at ${mapPath} — run \`codeatlas scan\` or set CODEATLAS_MAP`,
            }),
          );
        }
      });
    },
  };
}

export default defineConfig({
  plugins: [react(), localMap()],
  test: {
    environment: "jsdom",
    setupFiles: ["./tests/setup.ts"],
  },
});
