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

fn has_edge(map: &serde_json::Value, kind: &str, source: &str, target: &str) -> bool {
    map["edges"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["kind"] == kind && e["source"] == source && e["target"] == target)
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
        assert_eq!(node["provenance"], "structural");
        if node["id"].as_str().unwrap().starts_with("file:") {
            assert_eq!(node["kind"], "file");
        }
    }
}

#[test]
fn scan_extracts_functions_and_classes_with_contains_edges() {
    let repo = materialize("simple");
    scan(repo.path());
    let map = read_map(repo.path());
    let ids = node_ids(&map);

    assert!(
        ids.contains(&"function:src/util.ts:greet".to_string()),
        "ids: {ids:?}"
    );
    assert!(ids.contains(&"function:src/main.ts:main".to_string()));
    assert!(ids.contains(&"class:src/greeter.ts:Greeter".to_string()));

    let nodes = map["nodes"].as_array().unwrap();
    let greet = nodes
        .iter()
        .find(|n| n["id"] == "function:src/util.ts:greet")
        .unwrap();
    assert_eq!(greet["kind"], "function");
    assert_eq!(greet["range"]["start_line"], 1);
    assert_eq!(greet["range"]["end_line"], 3);

    let util_file = nodes
        .iter()
        .find(|n| n["id"] == "file:src/util.ts")
        .unwrap();
    let summary = util_file["summary"].as_str().unwrap();
    assert!(
        summary.contains("TypeScript") && summary.contains("1 function"),
        "mechanical summary expected, got: {summary}"
    );

    let edges = map["edges"].as_array().unwrap();
    assert!(
        edges.iter().any(|e| e["source"] == "file:src/util.ts"
            && e["target"] == "function:src/util.ts:greet"
            && e["kind"] == "contains"),
        "missing contains edge: {edges:?}"
    );

    // Same-named methods in different classes must not collide: IDs are the
    // map's identity primitive (carry-over and referential integrity sit on
    // them), so methods are scope-qualified.
    assert!(
        ids.contains(&"function:src/pair.ts:Alpha.run".to_string()),
        "ids: {ids:?}"
    );
    assert!(ids.contains(&"function:src/pair.ts:Beta.run".to_string()));
    let mut sorted = ids.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), ids.len(), "duplicate node IDs: {ids:?}");
}

#[test]
fn files_that_cannot_be_parsed_still_appear_as_file_nodes() {
    let repo = materialize("simple");
    scan(repo.path()); // asserts the run completes despite broken.ts
    let map = read_map(repo.path());
    let ids = node_ids(&map);

    assert!(
        ids.contains(&"file:src/broken.ts".to_string()),
        "unparseable file must still be a file node: {ids:?}"
    );
    // Unsupported-for-symbols files stay bare file nodes.
    assert!(
        !ids.iter()
            .any(|id| id.ends_with("README.md") && !id.starts_with("file:")),
        "no symbol nodes may come from unsupported files: {ids:?}"
    );
    // A file with no extension is unsupported, even if its bare name matches
    // a supported extension (fixture has a file literally named `ts`).
    assert!(ids.contains(&"file:ts".to_string()), "ids: {ids:?}");
    assert!(
        !ids.contains(&"function:ts:sneaky".to_string()),
        "extensionless file must not be parsed: {ids:?}"
    );
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
fn import_statements_resolve_to_imports_edges_between_file_nodes() {
    let repo = materialize("simple");
    scan(repo.path());
    let map = read_map(repo.path());
    let edges = map["edges"].as_array().unwrap();

    // Extension inference: `./util` resolves to util.ts.
    assert!(
        has_edge(&map, "imports", "file:src/main.ts", "file:src/util.ts"),
        "edges: {edges:?}"
    );
    // Index-file convention: `./lib` resolves to lib/index.ts.
    assert!(
        has_edge(&map, "imports", "file:src/app.ts", "file:src/lib/index.ts"),
        "edges: {edges:?}"
    );
    // Parent-relative import completes a cross-file chain:
    // app.ts → lib/index.ts → util.ts.
    assert!(
        has_edge(&map, "imports", "file:src/lib/index.ts", "file:src/util.ts"),
        "edges: {edges:?}"
    );
    // Bare-package and missing-file imports are dropped, never dangling.
    assert!(
        !edges.iter().any(|e| {
            e["kind"] == "imports"
                && e["target"]
                    .as_str()
                    .is_some_and(|t| t.contains("missing") || t.contains("node:fs"))
        }),
        "unresolvable import leaked into the map: {edges:?}"
    );
}

#[test]
fn functions_assigned_to_consts_count_as_functions() {
    let repo = materialize("simple");
    scan(repo.path());
    let map = read_map(repo.path());
    let ids = node_ids(&map);

    // `const x = () => {}` and `const x = function () {}` are how idiomatic
    // TS declares many functions; both are function nodes.
    assert!(
        ids.contains(&"function:src/arrow.ts:shout".to_string()),
        "arrow function missing: {ids:?}"
    );
    assert!(
        ids.contains(&"function:src/arrow.ts:local".to_string()),
        "function expression missing: {ids:?}"
    );
    let shout = map["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == "function:src/arrow.ts:shout")
        .unwrap();
    assert_eq!(shout["kind"], "function");
    assert!(has_edge(
        &map,
        "contains",
        "file:src/arrow.ts",
        "function:src/arrow.ts:shout"
    ));
}

#[test]
fn exported_symbols_produce_exports_edges() {
    let repo = materialize("simple");
    scan(repo.path());
    let map = read_map(repo.path());
    let edges = map["edges"].as_array().unwrap();

    assert!(
        has_edge(
            &map,
            "exports",
            "file:src/util.ts",
            "function:src/util.ts:greet"
        ),
        "edges: {edges:?}"
    );
    assert!(
        has_edge(
            &map,
            "exports",
            "file:src/greeter.ts",
            "class:src/greeter.ts:Greeter"
        ),
        "edges: {edges:?}"
    );
    // `export const x = () => {}` is an export too.
    assert!(
        has_edge(
            &map,
            "exports",
            "file:src/arrow.ts",
            "function:src/arrow.ts:shout"
        ),
        "edges: {edges:?}"
    );
    // A module-private symbol is not exported.
    assert!(
        !edges
            .iter()
            .any(|e| e["kind"] == "exports" && e["target"] == "function:src/arrow.ts:local"),
        "non-exported symbol got an exports edge: {edges:?}"
    );
    // Methods of an exported class are not themselves exported.
    assert!(
        !edges
            .iter()
            .any(|e| e["kind"] == "exports" && e["target"] == "function:src/pair.ts:Alpha.run"),
        "method of exported class got an exports edge: {edges:?}"
    );
}

#[test]
fn function_invocations_produce_calls_edges_where_the_callee_is_resolvable() {
    let repo = materialize("simple");
    scan(repo.path());
    let map = read_map(repo.path());
    let edges = map["edges"].as_array().unwrap();

    // Cross-file: main() calls greet() through the `./util` import.
    assert!(
        has_edge(
            &map,
            "calls",
            "function:src/main.ts:main",
            "function:src/util.ts:greet"
        ),
        "edges: {edges:?}"
    );
    // Cross-file through the parent-relative import chain.
    assert!(
        has_edge(
            &map,
            "calls",
            "function:src/lib/index.ts:helper",
            "function:src/util.ts:greet"
        ),
        "edges: {edges:?}"
    );
    // Same-file call, and repeated invocations collapse to one edge.
    let shout_calls = edges
        .iter()
        .filter(|e| {
            e["kind"] == "calls"
                && e["source"] == "function:src/arrow.ts:shoutTwice"
                && e["target"] == "function:src/arrow.ts:shout"
        })
        .count();
    assert_eq!(shout_calls, 1, "edges: {edges:?}");
    // Unresolvable callees (console.log, an import that resolved nowhere)
    // produce no edge at all.
    assert!(
        !edges.iter().any(|e| {
            e["kind"] == "calls"
                && e["target"]
                    .as_str()
                    .is_some_and(|t| t.contains("console") || t.contains("ghost"))
        }),
        "unresolvable call leaked into the map: {edges:?}"
    );
}

#[test]
fn edges_carry_fixed_weights_determined_by_their_kind() {
    let repo = materialize("simple");
    scan(repo.path());
    let map = read_map(repo.path());
    let edges = map["edges"].as_array().unwrap();
    assert!(!edges.is_empty());

    let mut weight_by_kind: std::collections::HashMap<String, f64> =
        std::collections::HashMap::new();
    for edge in edges {
        let kind = edge["kind"].as_str().unwrap().to_string();
        let weight = edge["weight"].as_f64().expect("edge must carry a weight");
        assert!(weight > 0.0, "weight must be positive: {edge:?}");
        // The weight is a fixed function of the kind: every edge of one kind
        // carries the same weight.
        let seen = weight_by_kind.entry(kind).or_insert(weight);
        assert_eq!(*seen, weight, "weight varies within a kind: {edge:?}");
    }
    // Weights are typed, not decorative: structural containment and
    // cross-file imports carry different strengths.
    assert!(weight_by_kind.contains_key("contains"));
    assert!(weight_by_kind.contains_key("imports"));
    assert_ne!(weight_by_kind["contains"], weight_by_kind["imports"]);
}

#[test]
fn no_edge_references_a_node_missing_from_the_map() {
    let repo = materialize("simple");
    scan(repo.path());
    let map = read_map(repo.path());

    // The fixture contains imports of missing files and bare packages, and
    // calls to unresolvable callees; whatever the pipeline does with them,
    // every emitted edge must connect two nodes that exist.
    let ids: std::collections::HashSet<String> = node_ids(&map).into_iter().collect();
    for edge in map["edges"].as_array().unwrap() {
        for end in ["source", "target"] {
            let id = edge[end].as_str().unwrap();
            assert!(ids.contains(id), "dangling edge {end} {id}: {edge:?}");
        }
    }
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
