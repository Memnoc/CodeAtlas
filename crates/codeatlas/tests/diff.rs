//! Tests for `codeatlas diff` at the artifact seam: materialize a fixture
//! repo, make it a real git repository, run the built binary, and assert on
//! the emitted `.codeatlas/diff-overlay.json`. Never on internals; zero LLM,
//! zero network — the diff path is pure git + graph traversal.

mod common;

use std::fs;
use std::path::Path;

use common::{git, git_init, materialize};

fn scan(repo: &Path) {
    assert_cmd::Command::cargo_bin("codeatlas")
        .unwrap()
        .arg("scan")
        .current_dir(repo)
        .assert()
        .success();
}

fn diff(repo: &Path) {
    assert_cmd::Command::cargo_bin("codeatlas")
        .unwrap()
        .arg("diff")
        .current_dir(repo)
        .assert()
        .success();
}

fn read_overlay(repo: &Path) -> serde_json::Value {
    let raw = fs::read_to_string(repo.join(".codeatlas/diff-overlay.json")).unwrap();
    serde_json::from_str(&raw).unwrap()
}

fn strings(overlay: &serde_json::Value, field: &str) -> Vec<String> {
    overlay[field]
        .as_array()
        .unwrap_or_else(|| panic!("overlay has no array field {field}: {overlay}"))
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect()
}

/// Turns the materialized fixture into a real git repository with one
/// commit. The identity and signing configuration lives in `common::git_init`,
/// local to the tempdir so the machine's global git config can never affect
/// the test.
fn git_init_and_commit(repo: &Path) {
    git_init(repo);
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", "fixture baseline"]);
}

#[test]
fn modifying_one_file_yields_exactly_its_nodes_and_their_one_hop_neighbors() {
    let repo = materialize("diffrepo");
    git_init_and_commit(repo.path());
    scan(repo.path());

    // An uncommitted working-tree edit to util.ts: its file node and its
    // contained function change; app.ts imports it and main() calls greet(),
    // so exactly those two nodes are the one-hop blast radius. README.md
    // links to app.ts, not util.ts — untouched.
    fs::write(
        repo.path().join("src/util.ts"),
        "export function greet(name: string): string {\n  return `hi ${name}`;\n}\n",
    )
    .unwrap();
    diff(repo.path());

    let overlay = read_overlay(repo.path());
    assert_eq!(
        strings(&overlay, "changed"),
        vec!["file:src/util.ts", "function:src/util.ts:greet"],
        "overlay: {overlay}"
    );
    assert_eq!(
        strings(&overlay, "affected"),
        vec!["file:src/app.ts", "function:src/app.ts:main"],
        "overlay: {overlay}"
    );
    assert_eq!(strings(&overlay, "unmapped_paths"), Vec::<String>::new());
    assert!(
        overlay["version"].is_u64(),
        "overlay must be versioned: {overlay}"
    );
}

#[test]
fn staged_and_untracked_changes_both_count_as_changed() {
    let repo = materialize("diffrepo");
    git_init_and_commit(repo.path());
    // An untracked file that the map knows about: created before the scan,
    // never committed.
    fs::write(
        repo.path().join("src/extra.ts"),
        "export function extra(): number {\n  return 7;\n}\n",
    )
    .unwrap();
    scan(repo.path());

    // A staged (but uncommitted) edit to app.ts.
    fs::write(
        repo.path().join("src/app.ts"),
        "import { greet } from \"./util\";\n\nexport function main(): void {\n  console.log(greet(\"moon\"));\n}\n",
    )
    .unwrap();
    git(repo.path(), &["add", "src/app.ts"]);
    diff(repo.path());

    let changed = strings(&read_overlay(repo.path()), "changed");
    for expected in [
        "file:src/app.ts",
        "function:src/app.ts:main",
        "file:src/extra.ts",
        "function:src/extra.ts:extra",
    ] {
        assert!(
            changed.contains(&expected.to_string()),
            "changed: {changed:?}"
        );
    }
    // The committed, untouched file is not changed.
    assert!(
        !changed.contains(&"file:src/util.ts".to_string()),
        "changed: {changed:?}"
    );
}

#[test]
fn changed_paths_absent_from_the_map_are_noted_and_never_dangle() {
    let repo = materialize("diffrepo");
    git_init_and_commit(repo.path());
    scan(repo.path());

    // Created after the scan: git sees it, the map does not.
    fs::write(repo.path().join("src/new.ts"), "export const n = 1;\n").unwrap();
    diff(repo.path());

    let overlay = read_overlay(repo.path());
    assert_eq!(strings(&overlay, "unmapped_paths"), vec!["src/new.ts"]);

    // Nothing dangles: every node ID in the overlay exists in the map.
    let map: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(repo.path().join(".codeatlas/knowledge-graph.json")).unwrap(),
    )
    .unwrap();
    let ids: std::collections::HashSet<String> = map["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["id"].as_str().unwrap().to_string())
        .collect();
    for field in ["changed", "affected"] {
        for id in strings(&overlay, field) {
            assert!(ids.contains(&id), "dangling {field} node {id}");
        }
    }
}

#[test]
fn renaming_a_file_reports_the_old_paths_nodes_and_their_blast_radius() {
    let repo = materialize("diffrepo");
    git_init_and_commit(repo.path());
    scan(repo.path());

    // A staged rename. Git's rename detection (diff.renames defaults on)
    // would collapse this into one pair and report only the destination —
    // which the map has never seen — leaving the old path's nodes unmarked
    // and the blast radius empty: a rename must never render as zero risk.
    git(repo.path(), &["mv", "src/util.ts", "src/renamed.ts"]);
    diff(repo.path());

    let overlay = read_overlay(repo.path());
    // The old path is what the map knows: its file and symbol nodes are the
    // changed set, and its importers/callers are the blast radius.
    assert_eq!(
        strings(&overlay, "changed"),
        vec!["file:src/util.ts", "function:src/util.ts:greet"],
        "overlay: {overlay}"
    );
    assert_eq!(
        strings(&overlay, "affected"),
        vec!["file:src/app.ts", "function:src/app.ts:main"],
        "overlay: {overlay}"
    );
    // The destination is unknown to the map until the next scan: noted.
    assert_eq!(
        strings(&overlay, "unmapped_paths"),
        vec!["src/renamed.ts"],
        "overlay: {overlay}"
    );
}

#[test]
fn running_diff_twice_on_identical_state_is_byte_identical() {
    let repo = materialize("diffrepo");
    git_init_and_commit(repo.path());
    scan(repo.path());
    fs::write(
        repo.path().join("src/util.ts"),
        "export function greet(): string {\n  return \"x\";\n}\n",
    )
    .unwrap();

    diff(repo.path());
    let first = fs::read(repo.path().join(".codeatlas/diff-overlay.json")).unwrap();
    diff(repo.path());
    let second = fs::read(repo.path().join(".codeatlas/diff-overlay.json")).unwrap();
    assert_eq!(first, second);
}

#[test]
fn a_repo_with_no_commits_counts_every_known_file_as_changed() {
    let repo = materialize("diffrepo");
    git(repo.path(), &["init", "-q"]);
    scan(repo.path());
    diff(repo.path());

    // No HEAD to diff against: every file git knows about (here all
    // untracked) is changed, so every file node in the map turns up.
    let changed = strings(&read_overlay(repo.path()), "changed");
    for expected in ["file:README.md", "file:src/app.ts", "file:src/util.ts"] {
        assert!(
            changed.contains(&expected.to_string()),
            "changed: {changed:?}"
        );
    }
}

#[test]
fn diff_without_a_map_asks_for_a_scan_first() {
    let repo = materialize("diffrepo");
    git_init_and_commit(repo.path());

    assert_cmd::Command::cargo_bin("codeatlas")
        .unwrap()
        .arg("diff")
        .current_dir(repo.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("codeatlas scan"));
}

#[test]
fn diff_outside_a_git_work_tree_fails_with_a_clear_error() {
    let repo = materialize("diffrepo");
    scan(repo.path());

    assert_cmd::Command::cargo_bin("codeatlas")
        .unwrap()
        .arg("diff")
        .current_dir(repo.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("git work tree"));
}
