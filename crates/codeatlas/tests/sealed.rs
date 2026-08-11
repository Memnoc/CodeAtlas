//! Mechanical verification of the build's dependency tree (ticket 15,
//! ADR-0006): `--no-default-features` must link no networking crates, so that
//! in the sealed binary sending data anywhere is a compile error — and
//! neither must `--no-default-features --features agent-cli`, which is
//! ADR-0008's third configuration (ticket 32).
//!
//! The test shells out to `cargo tree` — a metadata resolution, not a build,
//! so it stays cheap — with `--locked --offline` so it can neither touch the
//! network nor rewrite `Cargo.lock`. A control test asserts the default
//! tree DOES contain the HTTP client, proving the probe reads real data and
//! the sealed assertions cannot pass vacuously.
//!
//! # What a tree cannot see
//!
//! A subprocess adds no dependency, so nothing here can tell whether the
//! `agent-cli` backend was compiled in. This file would read exactly the same
//! if it were — a guard that cannot fail, which is why ADR-0008 called for a
//! differently-shaped proof. That proof is
//! `the_cli_backend_is_selectable_exactly_where_it_is_compiled_in`
//! (`src/enrich.rs`), the sealed refusal in `tests/enrich.rs`, and the byte
//! probe for the program string in `scripts/sealed-probe.sh`.
//!
//! The complementary probe on the sealed binary's *bytes* (no
//! `api.anthropic.com`, no `ureq`, no `claude` strings; `--enrich` fails with
//! the sealed build's message) lives in CI (`scripts/sealed-probe.sh`),
//! because it needs a binary compiled without dev-dependencies — `cargo test`
//! builds always carry the `test-provider` feature via the self
//! dev-dependency, so no locally built test binary is the genuinely sealed
//! artifact.

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

/// ADR-0008's third configuration, in the half a dependency tree can see:
/// with `agent-cli` on and `network` off the binary must still link no HTTP
/// client. That is the whole posture — *HTTP client absent, approved CLI
/// permitted* — and the feature adding no dependency is the reason it holds,
/// which is precisely why it needs asserting rather than assuming: a future
/// dependency added under `agent-cli` would break it silently, and the
/// sealed test above would not notice because it never turns that feature on.
///
/// The other half — that the CLI backend really is compiled in here — cannot
/// be seen from a tree at all, for the same reason. It is
/// `the_cli_backend_is_selectable_exactly_where_it_is_compiled_in`
/// (`src/enrich.rs`) and the byte probe in `scripts/sealed-probe.sh`.
#[test]
fn the_agent_cli_configuration_links_no_networking_crates_either() {
    let crates = dependency_tree(&["--no-default-features", "--features", "agent-cli"]);
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
        "the agent-cli configuration links networking crates, so ADR-0008's \
         `HTTP client absent, approved CLI permitted` posture is not what it \
         claims: {offenders:?}"
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
