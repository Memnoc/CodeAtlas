//! The route paths the serving binary and the dashboard have to agree on
//! (ticket 27). Rust declares them; `dashboard/src/app/ask.ts` re-declares
//! them as string literals, because a TypeScript module cannot import a Rust
//! constant — and until this test existed, nothing made the second copy
//! true. A comment saying "must match `serve::ASK_ROUTE`" is a wish.
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

use std::fs;
use std::path::{Path, PathBuf};

use codeatlas::serve::{ASK_ROUTE, CAPABILITIES_ROUTE};

/// The dashboard module that declares both routes.
fn dashboard_routes() -> String {
    let path: PathBuf =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dashboard/src/app/ask.ts");
    fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read the dashboard's route module at {path:?}: {e}"))
}

#[test]
fn the_dashboard_asks_the_routes_this_binary_serves() {
    let source = dashboard_routes();

    for (name, route) in [
        ("ASK_ROUTE", ASK_ROUTE),
        ("CAPABILITIES_ROUTE", CAPABILITIES_ROUTE),
    ] {
        let declaration = format!("export const {name} = \"{route}\";");
        assert!(
            source.contains(&declaration),
            "dashboard/src/app/ask.ts must declare `{declaration}` to match \
             `serve::{name}` — as written, the dashboard asks a route this \
             server does not serve, and says nothing about it"
        );
    }
}
