# 05 — C and C++ extraction

**Spec:** docs/specs/2026-08-09-codeatlas-v1.md

**What to build:** Scanning a C or C++ codebase yields function/class/struct
nodes and the relationships that matter there: `#include` edges resolved
within the repository, header/source pairing, and call edges where
resolvable. C/C++ gets its own ticket because include resolution and the
header/implementation split are genuinely different work from module-import
languages.

**Blocked by:** 02 — Parser interface; 03 — Import and call edges.

**Status:** ready

- [ ] C and C++ implement the parser interface with grammars compiled in
- [ ] `#include "..."` directives resolve to edges between files inside the
      repo; system includes (`<...>`) are ignored or marked external, never
      dangling
- [ ] A header and its implementation file are related by an edge (pairing by
      include and/or naming convention)
- [ ] Function nodes with line ranges; call edges where the callee is
      resolvable within the repo
- [ ] Fixture test covers a header/source pair, a repo-internal include
      chain, and a system include
