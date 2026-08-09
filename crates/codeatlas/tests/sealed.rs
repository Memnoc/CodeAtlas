//! Mechanical verification of the sealed build's dependency tree (ticket 15,
//! ADR-0006): `--no-default-features` must link no networking crates, so that
//! in the sealed binary sending data anywhere is a compile error.
//!
//! The test shells out to `cargo tree` — a metadata resolution, not a build,
//! so it stays cheap — with `--locked --offline` so it can neither touch the
//! network nor rewrite `Cargo.lock`. A control test asserts the default
//! tree DOES contain the HTTP client, proving the probe reads real data and
//! the sealed assertion cannot pass vacuously.
//!
//! The complementary probe on the sealed binary's *bytes* (no
//! `api.anthropic.com`, no `ureq` strings; `--enrich` fails with the sealed
//! build's message) lives in CI (`scripts/sealed-probe.sh`), because it
//! needs a binary compiled without dev-dependencies — `cargo test` builds
//! always carry the `test-provider` feature via the self dev-dependency, so
//! no locally built test binary is the genuinely sealed artifact.

use std::process::Command;

/// Crates whose presence in a dependency tree means networking code is
/// linked: the HTTP client we actually use, its TLS stack, and every other
/// mainstream Rust HTTP/TLS stack in case a future dependency smuggles one
/// in transitively.
const NETWORKING_CRATES: &[&str] = &[
    "ureq",
    "rustls",
    "webpki",
    "webpki-roots",
    "tokio",
    "hyper",
    "reqwest",
    "native-tls",
    "openssl",
];

/// Runs `cargo tree` for this package with the given extra feature flags and
/// returns the crate names in the tree (first token of every line).
fn dependency_tree(feature_flags: &[&str]) -> Vec<String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let output = Command::new(cargo)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args([
            "tree",
            "-p",
            "codeatlas",
            "-e",
            "normal",
            "--prefix",
            "none",
            "--locked",
            "--offline",
        ])
        .args(feature_flags)
        .output()
        .expect("cargo tree runs");
    assert!(
        output.status.success(),
        "cargo tree failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("cargo tree output is UTF-8")
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .map(str::to_string)
        .collect()
}

/// True when `crate_name` is `probe` itself or a member of its family
/// (`rustls` matches `rustls` and `rustls-pki-types`, never `trust-dns`).
fn matches_family(crate_name: &str, probe: &str) -> bool {
    crate_name == probe
        || crate_name
            .strip_prefix(probe)
            .is_some_and(|r| r.starts_with('-'))
}

#[test]
fn sealed_dependency_tree_links_no_networking_crates() {
    let crates = dependency_tree(&["--no-default-features"]);
    assert!(
        !crates.is_empty(),
        "cargo tree returned an empty dependency tree"
    );
    let offenders: Vec<&String> = crates
        .iter()
        .filter(|c| {
            NETWORKING_CRATES
                .iter()
                .any(|probe| matches_family(c, probe))
        })
        .collect();
    assert!(
        offenders.is_empty(),
        "the sealed (--no-default-features) build links networking crates \
         (ADR-0006 forbids any): {offenders:?}"
    );
}

#[test]
fn default_dependency_tree_contains_the_http_client_control() {
    // Control for the probe itself: the default build's tree must contain
    // ureq (the ADR-0004 HTTP client). If this fails, the sealed assertion
    // above is not to be trusted — the tree walk went blind, or the crate
    // graph changed shape and the probe list needs revisiting.
    let crates = dependency_tree(&[]);
    assert!(
        crates.iter().any(|c| c == "ureq"),
        "default-features cargo tree does not show ureq: the sealed probe \
         would be vacuous; got {} crates",
        crates.len()
    );
}
