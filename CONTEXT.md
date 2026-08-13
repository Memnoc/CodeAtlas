# CodeAtlas

One command turns a repository into a knowledge graph served as an
interactive dashboard — optionally enriched with model-written prose, never
dependent on it.

## Language

**Map**:
The knowledge graph one scan produces — nodes, edges, layers, domain flows,
and a tour — published as JSON conforming to the map contract.
_Avoid_: graph (alone), atlas

**Map contract**:
The JSON Schema generated from the Rust types (ADR-0003); the only shape a
map is allowed to have. Consumers read the contract, never the Rust.
_Avoid_: schema (alone — there are other schemas)

**Node**:
One thing in the map: a file or a symbol. Every node carries a path, a
summary, and provenance.

**Symbol**:
A named thing a file contains — a function or a class. A symbol inherits its
file's layer through containment.

**Edge**:
A directed relationship between two nodes, of exactly four kinds:
`contains` and `exports` run from a file to its own symbols; `imports` and
`calls` are the **relating** kinds — the only ones that say how two things
connect, and the only ones any count on the dashboard includes.

**Layer**:
The structural grouping: every file belongs to exactly one layer, derived
from its top-level directory (root-level files share `root`). Published in
the map.
_Avoid_: folder, package

**Domain**:
The behavioural grouping: the bucket a call flow's root file gives it.
Derived from flows; not published as its own entity in the contract.

**Region**:
The dashboard's word for one group on the canvas — a layer under the
structural grouping, or a domain under the domain grouping. A view-side
concept; the contract does not know it.

**Domain flow**:
One entry point's call chain, walked depth-first and deterministically. The
map publishes one flow per entry point.
_Avoid_: trace

**Entry point**:
A function nothing calls that itself calls something — the root a domain
flow grows from, and the "where to start" the info panel lists.

**Significance**:
The published per-file number answering "which files matter": import fan-in
+ import fan-out + 1 if the file hosts an entry point, computed at scan
(ADR-0010). Every consumer — tour selection, default disclosure, rankings —
reads it; none re-derives it.
_Avoid_: importance, weight, score (alone)

**Tour**:
The map's bounded, curated walk (at most 12 stops) over the files that carry
the architecture. Produced at scan, carried in the map.
_Avoid_: walkthrough (that is the dashboard's), onboarding

**Walkthrough**:
The dashboard's spotlight tour of its own controls. Lives in the dashboard,
not the map.
_Avoid_: tour (reserved for the map's)

**Provenance**:
Who wrote a label: `structural` (mechanical) or `llm` (enrichment). Every
prose-bearing thing in the map carries it, and the dashboard badges it.

**Enrichment**:
The optional, paid pass that buys prose — names, summaries, tour labels —
from a provider. It may relabel what the mechanical pass created; it never
creates structure (ADR-0004).

**Provider**:
A backend that answers enrichment requests and questions: the Claude API
(key-billed) or the Claude CLI (subscription), behind one trait
(ADR-0004, ADR-0008).
_Avoid_: backend (alone), model (the provider wraps one)

**Annotation store**:
The committed file (`.codeatlas/annotations.json`) holding purchased prose
keyed by content hash, so a clone explains itself and nothing is re-bought
(ADR-0005, ADR-0007).

**Slice**:
The bounded selection of map nodes one question is answered from — never
the whole map, and the bound is stated (ADR-0009).

**Citation**:
A node ID an answer claims it drew on. Only IDs actually shown to the
provider survive; an invented citation is dropped.

**Turn**:
One question, its answer, and that answer's citations. The unit a
conversation is made of and bounded in.

**Conversation**:
The client-carried sequence of turns the dashboard may send alongside a new
question — at most 6, oldest dropped first, clamped by the server, never
stored by it (ADR-0012).
_Avoid_: session, chat, thread

**Usage**:
Token counts a provider's response envelope reports, surfaced per turn.
Measured or absent — never estimated, and never a price.
_Avoid_: cost, spend

**Share artifact**:
The single redacted HTML file `share` exports: no server, no token, opens by
double-click, and inherits the dashboard's renderer.

**Sealed build**:
The build configuration whose compile-time feature gate proves zero egress
(ADR-0006). The egress suite and the sealed probe test this posture; the
documents only describe it.
