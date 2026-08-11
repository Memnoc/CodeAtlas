# Ticket 23 — mechanical summaries say "17 classs"

**Status:** done
**Spec:** docs/specs/2026-08-09-codeatlas-v1.md
**Story:** 1 — a complete structural map with no LLM involved
**Blocks:** none
**Blocked by:** none

## Problem

`plural` in `crates/codeatlas/src/scan.rs:470` appends `s` unconditionally:

```rust
fn plural(word: &str, n: usize) -> String {
    if n == 1 { word.to_string() } else { format!("{word}s") }
}
```

So every file holding more than one class reads **"Rust file, 1258 lines: 38
functions, 17 classs"**. Spotted on a card in the dashboard, where the
mechanical summary is now the file card's caption and therefore on screen
constantly.

The spec's own wording for this feature is *"mechanical summaries ('Rust file,
214 lines: 3 functions')"* — prose the reader is meant to trust. A visible
misspelling in the one line that is supposed to be the dependable fallback
undercuts exactly the thing enrichment is measured against.

## Acceptance criteria

- [x] `class` pluralises to `classes`.
- [x] `function` still pluralises to `functions`, and the singular of both is
      unchanged.
- [x] The rule covers the sibilant endings that behave the same way (`s`,
      `x`, `z`, `ch`, `sh`) rather than special-casing the one word, since
      the next noun added here would hit it too.
- [x] A test asserts the summary text for a file with two classes, so the
      output is pinned rather than the helper.

## Notes

Kept deliberately small: this is a spelling rule, not an i18n system. If a
future noun needs an irregular plural, that is the moment to reconsider, not
now.
