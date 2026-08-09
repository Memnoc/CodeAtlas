//! Tests for `codeatlas diff` at the artifact seam: materialize a fixture
//! repo, make it a real git repository, run the built binary, and assert on
//! the emitted `.codeatlas/diff-overlay.json`. Never on internals; zero LLM,
//! zero network — the diff path is pure git + graph traversal.

use std::fs;
use std::path::{Path, PathBuf};

fn fixture_dir(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).unwrap();
        }
    }
}

/// Copies a committed fixture into a temp dir, activating its `_gitignore`
/// (committed under a neutral name so it cannot affect this repository).
fn materialize(name: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    copy_tree(&fixture_dir(name), dir.path());
    let inert = dir.path().join("_gitignore");
    if inert.exists() {
        fs::rename(inert, dir.path().join(".gitignore")).unwrap();
    }
    dir
}

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

fn git(repo: &Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Turns the materialized fixture into a real git repository with one
/// commit. Identity and signing are configured locally in the tempdir so
/// the machine's global git config can never affect the test.
fn git_init_and_commit(repo: &Path) {
    git(repo, &["init", "-q"]);
    git(repo, &["config", "user.email", "fixture@example.com"]);
    git(repo, &["config", "user.name", "Fixture"]);
    git(repo, &["config", "commit.gpgsign", "false"]);
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
