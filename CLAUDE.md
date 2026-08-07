# CodeAtlas

## Northstar

This repo follows the northstar engineering skills.

### Pipeline

`/adr` (or `/adr-with-docs`) → `/to-spec` → `/to-tickets` → `/implement`,
one ticket per fresh session, built via `/tdd`, reviewed by `/crosscheck`.
All tickets done → `/harden` verifies the assembled system; after shipping,
`/next` harvests the V2 agenda. `/guided-mode` shepherds when unsure.

### Artifacts

- `CONTEXT.md` — domain glossary, root
- `docs/adr/` — decision receipts, indexed in its README.md
- `docs/specs/` — specs, `YYYY-MM-DD-<slug>.md`
- `docs/research/` — research notes, same naming
- `docs/intake/` — digested stakeholder material, same naming
- `.scratch/<spec-slug>/` — tickets; disposable once the feature ships

All created lazily on first write.
