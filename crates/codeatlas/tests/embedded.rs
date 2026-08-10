//! The dashboard that actually ships (ADR-0006, spec story 17).
//!
//! `dashboard/tests/zero-egress.test.ts` scans a production build it makes for
//! itself, into a directory of its own. That is deliberate — sharing
//! `dashboard/dist` with `build.rs` was a race (ticket 19) — but it leaves a
//! gap it cannot close from where it stands: the bytes it scans are not
//! necessarily the bytes that shipped. `build.rs` embeds `dashboard/dist`, and
//! when `node_modules` is missing it will embed a *stale* dist with only a
//! `cargo:warning`. A green dashboard suite is therefore a statement about a
//! fresh build, not about this binary.
//!
//! This file closes that gap from the only place it can be closed: inside the
//! compiled artifact, over `serve::ASSETS`, which is precisely what `serve`
//! hands to a browser and what the share artifact inlines.

mod common;

use codeatlas::serve::ASSETS;

#[test]
fn a_dashboard_is_actually_embedded() {
    // The guard the rest of this file depends on. Every assertion below is a
    // loop over ASSETS, so an empty table would make them all pass having
    // examined nothing — the exact defect ticket 19 exists for, and the
    // reason it is asserted separately rather than trusted.
    assert!(
        !ASSETS.is_empty(),
        "no dashboard is embedded in this binary, so nothing below was checked"
    );
    assert!(
        ASSETS.iter().any(|asset| asset.path == "index.html"),
        "embedded dashboard has no index.html: {:?}",
        ASSETS.iter().map(|a| a.path).collect::<Vec<_>>()
    );
}

#[test]
fn the_embedded_dashboard_references_no_external_host() {
    let mut offenders = Vec::new();
    for asset in ASSETS {
        let text = String::from_utf8_lossy(asset.bytes);
        for url in common::external_urls(&text) {
            offenders.push(format!("{}: {url}", asset.path));
        }
    }
    assert_eq!(
        offenders,
        Vec::<String>::new(),
        "the dashboard compiled into this binary would reach off-machine"
    );
}

#[test]
fn the_embedded_dashboard_opens_no_socket_of_its_own() {
    for asset in ASSETS {
        let text = String::from_utf8_lossy(asset.bytes);
        // Websockets are as much egress as http(s), and a protocol-relative
        // reference in markup resolves to an external origin when the page is
        // served over http(s).
        assert!(
            !text.contains("ws://") && !text.contains("wss://"),
            "{} references a websocket",
            asset.path
        );
        assert!(
            !text.contains("src=\"//") && !text.contains("href=\"//"),
            "{} contains a protocol-relative reference",
            asset.path
        );
    }
}
