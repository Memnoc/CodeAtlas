//! The route paths the serving binary and the dashboard have to agree on
//! (ticket 27). Rust declares them; `dashboard/src/app/wire.ts` — the one
//! module that speaks to the server, kept out of the share artifact by
//! ticket 04 — re-declares them as string literals, because a TypeScript
//! module cannot import a Rust constant — and until this test existed,
//! nothing made the second copy true. A comment saying "must match
//! `serve::ASK_ROUTE`" is a wish.
//!
//! It matters more here than a duplicated string usually would, because the
//! failure would be silent rather than loud. `readCapabilities` swallows
//! every failure by design: an old binary, the vite dev server and a `serve`
//! without `--ask` all mean "no questions here" to the reader, so a route the
//! dashboard asks for and no server serves reads exactly like a server that
//! cannot answer. A typo would make the question box permanently absent in
//! the real dashboard while every dashboard test still passed — those tests
//! stub `fetch` against the same constant they would be proving wrong.
//!
//! `/api/map` is left out deliberately rather than overlooked: a typo there
//! fails at first paint, in the load error the dashboard already has for it,
//! so it is pinned by anyone who runs the program once. The routes here are
//! the ones whose breakage is silence.
//!
//! This is the spirit of CI's `contract` job, which regenerates the schema
//! and the TypeScript types and fails on drift between the committed and the
//! generated artifacts. Two strings want a check, not a code generator.
//!
//! V2 story 17 widens the charter from the dashboard to the committed
//! documents: the serve surface is declared once, in `serve::REGISTRY`, and
//! the tests at the bottom of this file hold `docs/SECURITY.md` to naming
//! every route in it and no route beyond it, and hold both `README.md` and
//! `docs/SECURITY.md` to
//! V1 story 9's sentence with the spec as the authority. Same pattern —
//! a Rust test reading a non-Rust file, because the failure would otherwise
//! be silent: a route ships undocumented, every suite passes, and the
//! security document has quietly gone false. It did, three times, in V1.

use std::fs;
use std::path::{Path, PathBuf};

use codeatlas::enrich::ask::MAX_TURNS;
use codeatlas::serve::{ASK_ROUTE, CAPABILITIES_ROUTE, REGISTRY, SOURCE_ROUTE};

/// A dashboard source module, read whole.
fn dashboard_module(name: &str) -> String {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../dashboard/src/app")
        .join(name);
    fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read the dashboard module at {path:?}: {e}"))
}

/// A committed file at the repository root, read whole. These tests build
/// nothing: the registry is a `const` this test binary already links, and
/// everything else is a file on disk.
fn repo_file(relative: &str) -> String {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {relative} at {path:?}: {e}"))
}

#[test]
fn the_dashboard_asks_the_routes_this_binary_serves() {
    let source = dashboard_module("wire.ts");

    // `SOURCE_ROUTE` joined when the constants moved into `wire.ts`
    // (ticket 04): its drift is quieter than a hard 404 sounds, because the
    // affordance is gated on capabilities alone — the button would render,
    // and every press would fail against a route this server never served.
    for (name, route) in [
        ("ASK_ROUTE", ASK_ROUTE),
        ("CAPABILITIES_ROUTE", CAPABILITIES_ROUTE),
        ("SOURCE_ROUTE", SOURCE_ROUTE),
    ] {
        let declaration = format!("export const {name} = \"{route}\";");
        assert!(
            source.contains(&declaration),
            "dashboard/src/app/wire.ts must declare `{declaration}` to match \
             `serve::{name}` — as written, the dashboard asks a route this \
             server does not serve, and says nothing about it"
        );
    }
}

/// ADR-0012's turn bound, held in step the same way. The dashboard enforces
/// the bound itself so the server's clamp is a backstop rather than the
/// mechanism (ticket 09 builds on this constant) — two numbers drifting
/// apart would mean turns silently clamped away on every follow-up, which no
/// dashboard test would notice for the same reason as above: both halves of
/// a stubbed exchange would agree with each other and disagree with the
/// binary.
#[test]
fn the_dashboard_carries_the_turn_bound_this_binary_clamps_to() {
    let source = dashboard_module("ask.ts");

    let declaration = format!("export const MAX_TURNS = {MAX_TURNS};");
    assert!(
        source.contains(&declaration),
        "dashboard/src/app/ask.ts must declare `{declaration}` to match \
         `ask::MAX_TURNS` — as written, the dashboard and the server \
         disagree about how much history a request may carry"
    );
}

/// V2 story 17, deferred ticket 36's registry option: the route set is
/// derived from `serve::REGISTRY` — the dispatch table `handle` itself
/// walks, so this set is the served surface by construction — never from a
/// scan of the source, which could not fail for a route spelled
/// unexpectedly. The one conditional entry (`POST /api/ask`, registered
/// only while `--ask` puts a backend behind it) is checked unconditionally,
/// because the document describes both shapes of the server; the registry
/// `const` is the same slice in every build configuration, so this test is
/// too.
///
/// The naming check is a plain substring match — a code fence, a longer
/// path such as `/api/mapx`, or a sentence recording a route's removal
/// would each satisfy it falsely — accepted here because the document is
/// reviewed prose rather than adversarial input, and the sibling test below
/// holds every `/api/` token the document does mention to the registry.
#[test]
fn every_registered_route_is_named_in_the_security_document() {
    let document = repo_file("docs/SECURITY.md");

    for route in REGISTRY {
        assert!(
            document.contains(route.path),
            "docs/SECURITY.md does not name `{path}` — the server answers \
             `{method} {path}` (serve::REGISTRY, crates/codeatlas/src/serve.rs), \
             and every route the registry holds must be named in the security \
             document; a route shipping undocumented is the drift this test \
             exists to catch",
            method = route.method,
            path = route.path,
        );
    }
}

/// Every distinct `/api/...` path the document mentions, in order of first
/// mention. The rule is mechanical: each occurrence of `/api/` anywhere in
/// the document — prose, bullet and code fence alike — extended rightward
/// through ASCII alphanumerics, `-` and `_`, and stopped by any other
/// character (`/` included: every registered path is one segment). A bare
/// `/api/` naming no segment is not a route mention and is skipped.
fn api_route_mentions(document: &str) -> Vec<String> {
    const PREFIX: &str = "/api/";
    let mut mentions: Vec<String> = Vec::new();
    let mut rest = document;
    while let Some(start) = rest.find(PREFIX) {
        let after = &rest[start + PREFIX.len()..];
        let end = after
            .find(|c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '_')
            .unwrap_or(after.len());
        let path = format!("{PREFIX}{}", &after[..end]);
        if end > 0 && !mentions.contains(&path) {
            mentions.push(path);
        }
        rest = &after[end..];
    }
    mentions
}

/// The other direction of the same drift (ticket 11's crosscheck): a route
/// removed from `serve::REGISTRY` but still described in `docs/SECURITY.md`
/// fails nothing without this test, and a stale route description in a
/// security document is the same species of false claim as a missing one.
/// Every `/api/` token the document mentions ([`api_route_mentions`] is the
/// rule) must be a path the registry holds. Checked against the `const`
/// slice rather than any runtime table for the reason the test above gives:
/// the document describes both shapes of the server, and the `const` is the
/// same in every build configuration, so this test is too.
#[test]
fn every_route_the_security_document_names_is_still_registered() {
    let document = repo_file("docs/SECURITY.md");
    let mentions = api_route_mentions(&document);
    assert!(
        !mentions.is_empty(),
        "docs/SECURITY.md mentions no `/api/...` route at all — the served \
         surface has left the document entirely, and this test is scanning \
         nothing"
    );

    for path in mentions {
        assert!(
            REGISTRY.iter().any(|route| route.path == path),
            "docs/SECURITY.md names `{path}`, but serve::REGISTRY \
             (crates/codeatlas/src/serve.rs) no longer holds it — a route \
             still described in the security document after leaving the \
             registry is stale, the same false claim as an undocumented \
             route in the other direction"
        );
    }
}

/// Markdown wraps lines where it likes and quotes with `> `, so the copies
/// of one sentence are compared as words, not bytes: quoting stripped,
/// whitespace collapsed, every other character — backticks and the em-dash
/// included — still load-bearing.
fn normalized(text: &str) -> String {
    text.split_whitespace()
        .filter(|word| *word != ">")
        .collect::<Vec<_>>()
        .join(" ")
}

/// V1 story 9's amended sentence is the one every security document holds
/// to, and the V1 spec is the authority: this test lifts the sentence from
/// `docs/specs/2026-08-09-codeatlas-v1.md` — between the emphasis markers
/// the amendment set it in — and requires `README.md` and `docs/SECURITY.md`
/// to carry it verbatim, so no copy can drift from the spec alone.
#[test]
fn story_9s_sentence_is_pinned_verbatim_in_readme_and_security_document() {
    let spec = "docs/specs/2026-08-09-codeatlas-v1.md";
    let text = repo_file(spec);
    let opening = "*CodeAtlas has exactly two ways to reach a model";
    let start = text
        .find(opening)
        .unwrap_or_else(|| panic!("{spec} no longer opens story 9's amended sentence with `{opening}` — this test needs the authority before it can hold the copies to it"));
    let rest = &text[start + 1..];
    let end = rest.find('*').unwrap_or_else(|| {
        panic!("{spec} never closes story 9's sentence's emphasis — cannot tell where the authority's sentence ends")
    });
    let sentence = normalized(&rest[..end]);

    for document in ["README.md", "docs/SECURITY.md"] {
        assert!(
            normalized(&repo_file(document)).contains(&sentence),
            "{document} does not carry V1 story 9's sentence verbatim — the \
             spec ({spec}) is the authority, and both README.md and \
             docs/SECURITY.md pin it word for word:\n{sentence}"
        );
    }
}
