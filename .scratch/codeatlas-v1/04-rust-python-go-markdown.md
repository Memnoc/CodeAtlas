# 04 — Remaining scripted languages: Rust, Python, Go + Markdown links

**Spec:** docs/specs/2026-08-09-codeatlas-v1.md

**What to build:** Scanning a Rust, Python, or Go repo yields the same
structural richness TS/JS already has — function/class/struct nodes, imports,
calls — through the same parser interface. Markdown files contribute link
edges to the files they reference. The dogfood milestone: CodeAtlas maps its
own repository, both the Rust core and the TS dashboard.

**Blocked by:** 02 — Parser interface; 03 — Import and call edges.

**Status:** done

- [x] Rust, Python, and Go each implement the parser interface with grammars
      compiled in and language-appropriate import resolution (`use` / `mod`,
      Python imports, Go packages)
- [x] Call edges emitted where resolvable, same rules as TS/JS
- [x] Markdown files appear as file nodes with link edges to files they
      reference by relative path
- [x] Per-language fixture tests assert expected nodes and edges
- [x] Smoke test: running `codeatlas scan` on the CodeAtlas repo itself
      produces a schema-valid map containing nodes from both languages
