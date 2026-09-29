# Enrichment and questions

Back to the [README](../README.md) · [Tour](tour.md) ·
[Commands](commands.md) · [Security](SECURITY.md) ·
[Development](development.md)

Everything CodeAtlas writes lands in `.codeatlas/` under the scanned root,
and a scan puts a `.gitignore` there so you do not have to: the regenerated
map is ignored, the annotation store is published, and that one exception
is deliberate — this page explains it.

## Enrichment (optional)

`scan --enrich` fills the map's prose slots: node summaries, layer names,
domain-flow names, tour narration through an enrichment provider. It is
entirely opt-in, and the mechanical values are always present underneath. If the provider fails or
is never configured, you still get a complete, schema-valid structural map.

Annotations are cached in `.codeatlas/annotations.json` keyed by node
identity and a content hash, so a later scan re-attaches unchanged answers
for free and only re-purchases the parts of the map that actually changed.
**That store is meant to be committed**: one person enriches, commits, and
pushes; everyone else clones and runs a plain `codeatlas scan` — no
credential, no network, no flags, and gets the map with all its prose.
This is handy if you are working on the same repo with some colleagues.
The store records which provider, which model, and what date produced it. If you
would rather not publish it, delete the `!annotations.json` line from
`.codeatlas/.gitignore`; scans write that file only when it is missing and
never overwrite it, so your edit stands.

> One interaction to check: if your repository already ignores `.codeatlas/`
> outright, narrow that line to `**/.codeatlas/*` — CodeAtlas's own rule.
> Git never lets a nested file re-include anything under an excluded
> _directory_, so an outright exclusion keeps the store unpublished no
> matter what the nested `.gitignore` says.

There are two providers, chosen with `--provider` or
`CODEATLAS_ENRICH_PROVIDER`:

- **`claude`** — the Claude API. Credentials resolve like the official SDKs:
  `ANTHROPIC_API_KEY` first, then an `ant auth login` profile. The default
  model is `claude-opus-5` (`--model` overrides). Billed per token, to the
  key's account.
- **`cli:claude`** — the Claude CLI you are already logged into, spawned as
  a one-shot completion with no tools and no MCP servers. **CodeAtlas never
  handles a credential**, which is the whole point; `ANTHROPIC_API_KEY` is
  deliberately stripped from the child's environment.

**Name the provider.** On a default build, plain `scan --enrich` falls
through to `claude` and the API-key path, because that is the build's default
backend. If you mean your subscription, you should specify it. Notice that the absence of a flag is not
a choice:

```sh
codeatlas scan . --enrich --provider cli:claude
```

You can ask what a run would cost (i.e. tokens) before spending anything
(`--enrich --dry-run`), and every run states its price up front and reports
progress as it goes, one line per batch, the same on a terminal and in a
log. The shape (the numbers are one repository on one day, not a guarantee):

```text
mapped 287 files
enriching: 1651 slots in 67 calls: roughly 146k–194k tokens of prompt,
plus perhaps 41k–74k more coming back
  batch 1/67 — 25 slots filled
  …
enriched 1651 slots
```

**The token figure is a range** because there is no local tokenizer and a single
number would be at best a guess. The call count is exact,
computed by the same code that then makes the calls. No price is ever
printed since rates move, and on `cli:claude` there is no monetary price at all.
Batches run four at a time and **every answered batch is saved as it
lands**: interrupting a run via Ctrl-C, a rate limit or a dropped connection
keeps everything already bought, and the next `--enrich` re-purchases only
what is missing (hopefully saving a lot of tokens).
Prompts are bounded on both providers and the model receives
the slots being filled and summarized topology, never the serialized graph
and never file contents.

### Asking the map questions

`serve --ask` reaches the same providers for a different purpose: a question
about the map, answered from a bounded slice of the map alone, citing the
node IDs the answer came from. Same rule as `--enrich`: name the provider,
or the default build picks the API key:

```sh
codeatlas serve . --port 4173 --ask --provider cli:claude
```

The dashboard picks up the mod automatically: the search field spawns an **Ask** button.
Without `--ask` the feature is hidden entirely and the terminal tells you the flag exists instead.
Every question is one provider call and an enriched map answers far better than
a structural one, because the answer is drawn from the map's own prose.
