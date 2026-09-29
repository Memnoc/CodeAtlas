# Commands

Back to the [README](../README.md) · [Tour](tour.md) ·
[Enrichment](enrichment.md) · [Security](SECURITY.md) ·
[Development](development.md)

## Subcommands

| Command        | What it does                                                                                                                                                                                                                                                                                                                        |
| -------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `scan [PATH]`  | Walk the repo, parse it, write the map to `.codeatlas/knowledge-graph.json`. `--enrich` additionally fills prose slots through an LLM (see [Enrichment](enrichment.md)); `--provider` chooses which one.                                                                                                                                                  |
| `serve [PATH]` | Serve the dashboard and the local map from memory on `127.0.0.1`. `--port` chooses the port; there is deliberately no `--host`. `--open-code` additionally serves a mapped file's own source to the dashboard. `--ask` additionally answers questions about the map at `POST /api/ask`, through the same providers `--enrich` uses. |
| `diff [PATH]`  | Project a git diff onto the map: changed nodes plus their one-hop blast radius, written to `.codeatlas/diff-overlay.json`. Pure git and graph traversal — no LLM, no network.                                                                                                                                                       |
| `share [PATH]` | Export one self-contained, redacted HTML file that opens by double-click, with no server and no external requests.                                                                                                                                                                                                                  |
| `schema`       | Print the JSON Schema of the map contract.                                                                                                                                                                                                                                                                                          |

The dashboard picks up the diff overlay automatically when one exists, offering
a toggle that distinguishes changed nodes from the ones they affect.

## The terminal menu

`./codeatlas` will spawn a small terminal menu that guides you through
options: `Enter` opens a folder exactly like a file manager, `h` climbs,
`/` types a path,
`Enter` on the pinned `.` row maps the directory you are in — one confirm
screen states plainly whether open code is on (`o` toggles it), and, on a
build with the CLI backend, whether it will enrich first (`e`, or `d` for
a dry run that states the price and buys nothing) and serve with Ask
(`a`): both go through your own `claude` login and nothing else, so there
is no key to configure and no flag to get wrong. Then it scans,
serves, and opens the map at `http://127.0.0.1:4173/` itself. If the port
is already taken it says so before scanning anything. The
menu remembers nothing: no history, no file written anywhere but the
repository you choose, and only ever appears when you run the binary by
hand at a terminal; in scripts and pipes a bare invocation prints usage,
exactly as a CLI should.

## Shell aliases

Point the first path at wherever your binary lives, whether that is the downloaded file or
your clone's build. The model-touching pair carry `--provider cli:claude` on
purpose: baking the flag in makes the bare-flag trap described in
[Enrichment](enrichment.md) impossible to hit from muscle memory.

```sh
alias codeatlas='$HOME/Code/CodeAtlas/target/release/codeatlas'
alias cas='codeatlas scan .'                                        # map this repo
alias cav='codeatlas serve .'                                       # view it (127.0.0.1:4173)
alias caq='codeatlas serve . --ask --provider cli:claude'           # view it + questions
alias cae='codeatlas scan . --enrich --provider cli:claude'         # buy prose for what changed
alias caed='codeatlas scan . --enrich --dry-run --provider cli:claude'  # say the price, spend nothing
alias cakill='pkill -x codeatlas'                                   # stop a running server
```
