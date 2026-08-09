# 09 — CLI serves the embedded dashboard

**Spec:** docs/specs/2026-08-09-codeatlas-v1.md

**What to build:** `codeatlas serve` opens the map in a browser with nothing
installed but the binary: the production dashboard build is embedded in the
Rust binary and served on a loopback address from `.codeatlas/`. No Node
runtime, no dev server, no downloads — the single-binary story of ADR-0002
made real.

**Blocked by:** 08 — Dashboard renders a map.

**Status:** done

- [x] The dashboard's production build is embedded in the binary at compile
      time
- [x] `codeatlas serve` binds only to a loopback address and serves the
      dashboard plus the local map artifacts
- [x] Works on a machine with no Node runtime; nothing is fetched from
      anywhere at runtime
- [x] End-to-end test: scan a fixture repo, serve, fetch the page and the
      graph over loopback, assert both respond
