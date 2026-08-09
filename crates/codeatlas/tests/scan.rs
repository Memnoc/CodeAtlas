//! Tests at the map-contract seam: run the binary against a fixture repo,
//! assert on the emitted map file. Never on internals.

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

fn read_map(repo: &Path) -> serde_json::Value {
    let raw = fs::read_to_string(repo.join(".codeatlas/knowledge-graph.json")).unwrap();
    serde_json::from_str(&raw).unwrap()
}

fn node_ids(map: &serde_json::Value) -> Vec<String> {
    map["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["id"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn scan_emits_a_map_with_typed_file_nodes() {
    let repo = materialize("simple");
    scan(repo.path());
    let map = read_map(repo.path());

    let version = map["version"].as_str().unwrap();
    let parts: Vec<_> = version.split('.').collect();
    assert!(
        parts.len() == 3 && parts.iter().all(|p| p.parse::<u64>().is_ok()),
        "version must be semver, got {version}"
    );
    assert!(
        !map["project"]["name"].as_str().unwrap().is_empty(),
        "project metadata must name the project"
    );

    let ids = node_ids(&map);
    assert!(
        ids.contains(&"file:src/main.ts".to_string()),
        "ids: {ids:?}"
    );
    assert!(ids.contains(&"file:src/util.ts".to_string()));
    assert!(ids.contains(&"file:README.md".to_string()));

    for node in map["nodes"].as_array().unwrap() {
        assert_eq!(node["kind"], "file");
        assert_eq!(node["provenance"], "structural");
    }
    assert!(map["edges"].as_array().unwrap().is_empty());
}

#[test]
fn scan_honors_gitignore_and_default_excludes() {
    let repo = materialize("simple");
    scan(repo.path());
    let ids = node_ids(&read_map(repo.path()));

    assert!(
        !ids.contains(&"file:debug.log".to_string()),
        "gitignored file leaked into the map: {ids:?}"
    );
    assert!(
        !ids.iter().any(|id| id.contains("node_modules")),
        "default-excluded directory leaked into the map: {ids:?}"
    );
    assert!(
        !ids.iter().any(|id| id.starts_with("file:target/")),
        "default-excluded directory leaked into the map: {ids:?}"
    );
    // Hidden files are source too: only the default excludes drop dot-paths.
    assert!(
        ids.contains(&"file:.github/ci.yml".to_string()),
        "hidden source file missing from the map: {ids:?}"
    );
    // The map's own output directory must never map itself.
    scan(repo.path());
    let ids = node_ids(&read_map(repo.path()));
    assert!(!ids.iter().any(|id| id.contains(".codeatlas")), "{ids:?}");
}

#[test]
fn emitted_map_validates_against_the_generated_schema() {
    let repo = materialize("simple");
    scan(repo.path());
    let map = read_map(repo.path());

    let output = assert_cmd::Command::cargo_bin("codeatlas")
        .unwrap()
        .arg("schema")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let schema: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();

    let errors: Vec<String> = validator.iter_errors(&map).map(|e| e.to_string()).collect();
    assert!(errors.is_empty(), "map violates its own schema: {errors:?}");
}

#[test]
fn scanning_the_same_input_twice_is_byte_identical() {
    let repo = materialize("simple");
    scan(repo.path());
    let first = fs::read(repo.path().join(".codeatlas/knowledge-graph.json")).unwrap();
    scan(repo.path());
    let second = fs::read(repo.path().join(".codeatlas/knowledge-graph.json")).unwrap();
    assert_eq!(first, second);
}
