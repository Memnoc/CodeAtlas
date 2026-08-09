import { readFileSync } from "node:fs";
import path from "node:path";
import react from "@vitejs/plugin-react";
import { defineConfig, type Plugin } from "vitest/config";

/// Dev-only artifact loading: serves a local map file at /api/map and the
/// diff overlay at /api/diff so the dev server mirrors the binary's serving.
/// The files are CODEATLAS_MAP / CODEATLAS_DIFF if set, else this repo's own
/// .codeatlas/ artifacts. Everything stays on loopback; no request ever
/// leaves the machine.
function localArtifacts(): Plugin {
  const serveLocal = (
    res: import("node:http").ServerResponse,
    filePath: string,
    missing: string,
  ) => {
    try {
      const body = readFileSync(filePath, "utf8");
      res.setHeader("Content-Type", "application/json");
      res.end(body);
    } catch {
      res.statusCode = 404;
      res.setHeader("Content-Type", "application/json");
      res.end(JSON.stringify({ error: missing }));
    }
  };
  return {
    name: "codeatlas-local-artifacts",
    configureServer(server) {
      server.middlewares.use("/api/map", (_req, res) => {
        const mapPath = process.env["CODEATLAS_MAP"]
          ? path.resolve(process.env["CODEATLAS_MAP"])
          : path.resolve(import.meta.dirname, "../.codeatlas/knowledge-graph.json");
        serveLocal(
          res,
          mapPath,
          `no map at ${mapPath} — run \`codeatlas scan\` or set CODEATLAS_MAP`,
        );
      });
      server.middlewares.use("/api/diff", (_req, res) => {
        const overlayPath = process.env["CODEATLAS_DIFF"]
          ? path.resolve(process.env["CODEATLAS_DIFF"])
          : path.resolve(import.meta.dirname, "../.codeatlas/diff-overlay.json");
        serveLocal(
          res,
          overlayPath,
          `no diff overlay at ${overlayPath} — run \`codeatlas diff\` or set CODEATLAS_DIFF`,
        );
      });
    },
  };
}

export default defineConfig({
  plugins: [react(), localArtifacts()],
  test: {
    environment: "jsdom",
    setupFiles: ["./tests/setup.ts"],
  },
});
