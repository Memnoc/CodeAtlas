<!-- Head slot reserved for the screen recording. -->

<p align="center">
  <img src="docs/images/brand/codeatlas-logo.svg" width="128" alt="CodeAtlas: the titan Atlas kneeling in a rose-and-iris seal, holding the knowledge graph overhead.">
</p>

<h1 align="center">CodeAtlas</h1>

<p align="center">
  <a href="https://github.com/Memnoc/CodeAtlas/releases"><img src="https://img.shields.io/github/v/release/Memnoc/CodeAtlas?color=ebbcba&label=release" alt="Latest release"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-c4a7e7" alt="License: MIT"></a>
  <a href="#install"><img src="https://img.shields.io/badge/platforms-Linux_%C2%B7_macOS-9ccfd8" alt="Platforms: Linux and macOS"></a>
  <a href="https://github.com/Memnoc/CodeAtlas/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/Memnoc/CodeAtlas/ci.yml?branch=main&label=CI" alt="CI status on main"></a>
  <a href="contract/README.md"><img src="https://img.shields.io/badge/map_contract-0.5.0-f6c177" alt="Map contract version 0.5.0"></a>
  <a href="docs/development.md"><img src="https://img.shields.io/badge/Rust-edition_2024-31748f" alt="Rust edition 2024"></a>
</p>

<p align="center">
  <a href="#install"><strong>Install</strong></a> ·
  <a href="docs/tour.md">Tour</a> ·
  <a href="docs/commands.md">Commands</a> ·
  <a href="docs/enrichment.md">Enrichment</a> ·
  <a href="#security">Security</a> ·
  <a href="https://github.com/Memnoc/CodeAtlas/releases">Releases</a>
</p>

<p align="center">
  <strong>Know what you or others have built.</strong><br>
  One command turns a repository into an interactive map: files, functions,
  classes, and the routes between them. You can search, walk, and ask questions.
</p>

<p align="center">
  Maps <strong>TypeScript · JavaScript · Rust · Python · Go · C · C++ · Markdown</strong><br>
  Offline by default: loopback only, no key, no account needed.
</p>

In the era of AI we write more code than anyone can read; that doesn't excuse us engineers from understanding that code and even more so, from having a solid
grasp on the architecture, decisions and trade-offs.
CodeAtlas is a visualization tool that helps you review the code, without having to read every single line of it.
It draws in a clear way any codebase (best with the supported ones) and works just as well on the repo you know by heart, when you
only want one function or one slice of domain logic. The map itself doesn't need a specific AI model or API key; AI-driven enrichment and questions are opt-in signaled by flags on the UX.

## Install

One downloaded file is the whole install, including the dashboard. `curl` is the default path on every platform,
and on macOS it is also the smoothest one: what the terminal fetches never
receives the quarantine mark, so Gatekeeper never gets in the way.

```sh
# macOS (Apple silicon)
curl -fLO https://github.com/Memnoc/CodeAtlas/releases/latest/download/codeatlas-aarch64-apple-darwin   # -f: fail loudly, never save an error page
chmod +x codeatlas-aarch64-apple-darwin && mv codeatlas-aarch64-apple-darwin codeatlas
./codeatlas

# Linux (x86_64)
curl -fLO https://github.com/Memnoc/CodeAtlas/releases/latest/download/codeatlas-x86_64-unknown-linux-musl
chmod +x codeatlas-x86_64-unknown-linux-musl && mv codeatlas-x86_64-unknown-linux-musl codeatlas
./codeatlas
```

<details>
<summary><strong>Mac and Linux</strong> — other targets, tag-pinned names, verification</summary>

An Intel Mac or an Arm Linux box swaps the target: `x86_64-apple-darwin`,
`aarch64-unknown-linux-musl` (the Linux binaries are static musl). The
`latest` URLs always fetch the newest release; every binary also exists
under a tag-pinned name, with a `-sealed` variant beside each, and the
[release notes](https://github.com/Memnoc/CodeAtlas/releases) carry the
SHA-256 checksums file and GitHub build-provenance attestation to verify
what you downloaded.

</details>

<details>
<summary>Run it <strong>bare</strong> — the terminal menu</summary>

`./codeatlas` with nothing after it opens a small terminal menu: pick the
repository like a file manager, flip open code, enrich, dry run and ask
with one key each, press Enter, and it scans, serves, and opens the map at
`http://127.0.0.1:4173/` itself. It remembers nothing and only appears at
a real terminal; in scripts and pipes a bare invocation prints usage. The
keys, one by one: [Commands → The terminal menu](docs/commands.md#the-terminal-menu).

</details>

The commands to remember are:

```sh
./codeatlas scan .     # writes .codeatlas/knowledge-graph.json
./codeatlas serve .    # serves the map on http://127.0.0.1:4173/
```

Everything CodeAtlas writes lands in `.codeatlas/` under the scanned root,
and a scan puts a `.gitignore` there so you do not have to: the regenerated
map is ignored, the annotation store is published, and that one exception
is deliberate — [Enrichment](docs/enrichment.md) explains it.

> **macOS, browser downloads only:** these binaries are not Apple-signed or
> notarized, so macOS quarantines what a _browser_ saves and Gatekeeper
> refuses to run it. In other words, safety first: verify the
> download — the checksums file and the provenance attestation,
> exactly as the release notes show — and only then clear the mark with
> `xattr -d com.apple.quarantine <the file>`. (System
> Settings → Privacy & Security → "Allow Anyway" reaches the same end
> through more clicks.) The `curl` path above skips all of this, so keep that in mind.

<details>
<summary><strong>Build from source</strong> — Rust (edition 2024) and Node 24</summary>

```sh
# one-time: the dashboard is compiled into the binary, so its deps must exist
cd dashboard && npm ci && cd ..

cargo build --release

./target/release/codeatlas scan .     # writes .codeatlas/knowledge-graph.json
./target/release/codeatlas serve .    # serves the map on http://127.0.0.1:4173/
```

</details>

## Documentation

- **[Tour](docs/tour.md)** — how it works, what the map looks like, the
  languages it parses and highlights.
- **[Commands](docs/commands.md)** — every subcommand and flag, the terminal
  menu key by key, shell aliases.
- **[Enrichment](docs/enrichment.md)** — buying prose for the map, the two
  providers, what a run costs, asking the map questions.
- **[Security](docs/SECURITY.md)** — the audit entry point: every guarantee
  mapped to the code and the test that holds it.
- **[Development](docs/development.md)** — building and testing, the
  design record, the transparency position, release status.
- **[Map contract](contract/README.md)** — the versioned schema other tools
  can build on. **[ADRs](docs/adr/)** and **[specs](docs/specs/)** — the
  decisions and each version's scope.

## Security

> CodeAtlas has exactly two ways to reach a model — an HTTPS POST to
> `api.anthropic.com`, and spawning the already-authenticated `claude` CLI.
> Each sits behind its own Cargo feature; each is reachable only from
> `scan --enrich`, `serve --ask`, and the launcher's enrich and ask
> toggles, which select the CLI alone. The sealed build has neither.

For the HTTPS route the destination is a hardcoded constant, and redirects
and environment proxies are disabled at the transport level, so the
transport cannot be steered elsewhere. Building with `--no-default-features`
produces the sealed binary, in which every command still works and both
`--enrich` and `--ask` refuse with a clear message.

These claims were tested and holds true to the best of my knowledge.
**[docs/SECURITY.md](docs/SECURITY.md)** is the audit entry point: it maps
each guarantee to the code and the committed test that enforces it, states
what a model receives on each path, names the CI jobs that run them, and
records the honest limitations.

## Status

v0.1 is out and stable: prebuilt, checksummed, provenance-attested binaries
for Linux and macOS with a sealed variant beside each, the point releases
driven by field feedback. The story-by-story record lives in
[Development → Status](docs/development.md#status) and the
[releases](https://github.com/Memnoc/CodeAtlas/releases).

## License

MIT — see [`LICENSE`](LICENSE). Copyright (c) 2026 Matteo Stara (Memnoc).

The kneeling titan in [`docs/images/brand/`](docs/images/brand/) is AI-generated imagery: Claude drew it in a design session on
2026-09-09, and the SVG states so inside the file. No copyright is claimed
over it.

## Thanks

CodeAtlas is openly and strongly inspired by
[Understand Anything](https://github.com/Egonex-AI/Understand-Anything) by
Yuxiang Lin, and its execution was shaped throughout by studying that
project's. If you want the original, larger take on making a codebase
explain itself, start there.
[Rose Pine](https://rosepinetheme.com/) - the most beautiful color scheme, ever present in all of my set ups and creations.
