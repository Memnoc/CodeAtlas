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
fn every_file_node_belongs_to_exactly_one_directory_derived_layer() {
    let repo = materialize("simple");
    scan(repo.path());
    let map = read_map(repo.path());

    let layers = map["layers"].as_array().expect("map must carry layers");
    let layer_ids: Vec<&str> = layers.iter().map(|l| l["id"].as_str().unwrap()).collect();
    for layer in layers {
        assert_eq!(layer["provenance"], "structural");
        assert!(
            !layer["name"].as_str().unwrap().is_empty(),
            "layer must carry a mechanical name: {layer:?}"
        );
    }
    {
        let mut sorted = layer_ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            sorted.len(),
            layer_ids.len(),
            "duplicate layers: {layer_ids:?}"
        );
    }

    // No orphan files: every file node names a layer that exists in the list.
    for node in map["nodes"].as_array().unwrap() {
        if node["kind"] != "file" {
            continue;
        }
        let layer = node["layer"]
            .as_str()
            .unwrap_or_else(|| panic!("file node without a layer: {node:?}"));
        assert!(
            layer_ids.contains(&layer),
            "file names a layer missing from the list: {node:?}"
        );
    }

    // Layers are directory-derived: files under src/ share one layer,
    // top-level files share another, and the two differ.
    let layer_of = |id: &str| {
        map["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == id)
            .unwrap()["layer"]
            .as_str()
            .unwrap()
            .to_string()
    };
    assert_eq!(
        layer_of("file:src/main.ts"),
        layer_of("file:src/lib/index.ts")
    );
    assert_eq!(layer_of("file:README.md"), layer_of("file:ts"));
    assert_ne!(layer_of("file:src/main.ts"), layer_of("file:README.md"));
}

#[test]
fn domain_flows_are_call_chains_rooted_at_functions_nothing_else_calls() {
    let repo = materialize("simple");
    scan(repo.path());
    let map = read_map(repo.path());

    let flows = map["domain_flows"]
        .as_array()
        .expect("map must carry domain flows");
    assert!(!flows.is_empty(), "fixture has call chains, so flows exist");

    // The expected flow: main() is called by nothing and calls greet().
    let main_flow = flows
        .iter()
        .find(|f| f["steps"][0] == "function:src/main.ts:main")
        .unwrap_or_else(|| panic!("no flow rooted at main: {flows:?}"));
    let steps: Vec<&str> = main_flow["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap())
        .collect();
    assert_eq!(
        steps,
        ["function:src/main.ts:main", "function:src/util.ts:greet"]
    );
    // Domains come from top-level directories.
    assert_eq!(main_flow["domain"], "src");
    assert_eq!(main_flow["provenance"], "structural");
    assert!(
        !main_flow["name"].as_str().unwrap().is_empty(),
        "flow must carry a mechanical name: {main_flow:?}"
    );

    let node_ids: std::collections::HashSet<String> = node_ids(&map).into_iter().collect();
    let called: std::collections::HashSet<&str> = map["edges"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["kind"] == "calls")
        .map(|e| e["target"].as_str().unwrap())
        .collect();
    for flow in flows {
        // Rooted at entry points only: no flow starts at a called function.
        let root = flow["steps"][0].as_str().unwrap();
        assert!(!called.contains(root), "flow rooted at a callee: {flow:?}");
        // Every step references a function node present in the map.
        for step in flow["steps"].as_array().unwrap() {
            let id = step.as_str().unwrap();
            assert!(id.starts_with("function:"), "non-function step: {flow:?}");
            assert!(node_ids.contains(id), "dangling flow step {id}: {flow:?}");
        }
    }

    // A function that calls nothing roots no flow: a chain needs a call.
    assert!(
        !flows
            .iter()
            .any(|f| f["steps"][0] == "function:src/greeter.ts:Greeter.greet"),
        "call-less function became a flow root: {flows:?}"
    );
}

#[test]
fn tour_steps_are_topology_ordered_with_mechanical_labels() {
    let repo = materialize("simple");
    scan(repo.path());
    let map = read_map(repo.path());

    let tour = map["tour"].as_array().expect("map must carry a tour");
    assert!(!tour.is_empty());

    let node_ids: std::collections::HashSet<String> = node_ids(&map).into_iter().collect();
    let mut steps: Vec<&str> = Vec::new();
    for step in tour {
        let node = step["node"].as_str().unwrap();
        assert!(node_ids.contains(node), "dangling tour step: {step:?}");
        assert!(
            !step["label"].as_str().unwrap().is_empty(),
            "tour step must carry a mechanical label: {step:?}"
        );
        assert_eq!(step["provenance"], "structural");
        steps.push(node);
    }
    {
        let mut sorted = steps.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), steps.len(), "node visited twice: {steps:?}");
    }

    // Topology ordering: entry-point files (nothing imports them, they start
    // call chains) come before the most-imported foundation module.
    let position = |id: &str| {
        steps
            .iter()
            .position(|s| *s == id)
            .unwrap_or_else(|| panic!("{id} missing from tour: {steps:?}"))
    };
    assert!(position("file:src/app.ts") < position("file:src/util.ts"));
    assert!(position("file:src/main.ts") < position("file:src/util.ts"));
}

#[test]
fn rust_files_yield_symbols_imports_and_calls() {
    let repo = materialize("rustproj");
    scan(repo.path());
    let map = read_map(repo.path());
    let ids = node_ids(&map);
    let edges = map["edges"].as_array().unwrap();

    // Functions, structs (as classes), and impl methods scope-qualified.
    assert!(
        ids.contains(&"function:src/main.rs:main".to_string()),
        "ids: {ids:?}"
    );
    assert!(ids.contains(&"function:src/util.rs:greet".to_string()));
    assert!(ids.contains(&"class:src/shapes/mod.rs:Circle".to_string()));
    assert!(ids.contains(&"function:src/shapes/mod.rs:Circle.area".to_string()));

    // Mechanical summary names the language.
    let util = map["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == "file:src/util.rs")
        .unwrap();
    assert!(
        util["summary"].as_str().unwrap().contains("Rust"),
        "summary: {}",
        util["summary"]
    );

    // `pub` is exported; a private fn is not; methods are not.
    assert!(has_edge(
        &map,
        "exports",
        "file:src/util.rs",
        "function:src/util.rs:greet"
    ));
    assert!(has_edge(
        &map,
        "exports",
        "file:src/shapes/mod.rs",
        "class:src/shapes/mod.rs:Circle"
    ));
    assert!(
        !edges
            .iter()
            .any(|e| e["kind"] == "exports" && e["target"] == "function:src/util.rs:decorate"),
        "private fn got an exports edge: {edges:?}"
    );

    // `mod foo;` resolves to foo.rs / foo/mod.rs; `use crate::…` resolves
    // against the src/ layout.
    assert!(
        has_edge(&map, "imports", "file:src/main.rs", "file:src/util.rs"),
        "edges: {edges:?}"
    );
    assert!(
        has_edge(
            &map,
            "imports",
            "file:src/main.rs",
            "file:src/shapes/mod.rs"
        ),
        "edges: {edges:?}"
    );
    // std/external paths never resolve into the map.
    assert!(
        !edges.iter().any(|e| {
            e["kind"] == "imports" && e["target"].as_str().is_some_and(|t| t.contains("std"))
        }),
        "external import leaked: {edges:?}"
    );

    // Cross-file call through the `use` binding; same-file call to a
    // private fn.
    assert!(
        has_edge(
            &map,
            "calls",
            "function:src/main.rs:main",
            "function:src/util.rs:greet"
        ),
        "edges: {edges:?}"
    );
    assert!(
        has_edge(
            &map,
            "calls",
            "function:src/util.rs:greet",
            "function:src/util.rs:decorate"
        ),
        "edges: {edges:?}"
    );
}

#[test]
fn python_files_yield_symbols_imports_and_calls() {
    let repo = materialize("pyproj");
    scan(repo.path());
    let map = read_map(repo.path());
    let ids = node_ids(&map);
    let edges = map["edges"].as_array().unwrap();

    // Functions, classes, and methods scope-qualified.
    assert!(
        ids.contains(&"function:app.py:main".to_string()),
        "ids: {ids:?}"
    );
    assert!(ids.contains(&"function:utils.py:shout".to_string()));
    assert!(ids.contains(&"class:models.py:Greeter".to_string()));
    assert!(ids.contains(&"function:models.py:Greeter.greet".to_string()));

    let utils = map["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == "file:utils.py")
        .unwrap();
    assert!(
        utils["summary"].as_str().unwrap().contains("Python"),
        "summary: {}",
        utils["summary"]
    );

    // Convention: non-underscore top-level names are exported; underscore
    // names and methods are not.
    assert!(has_edge(
        &map,
        "exports",
        "file:utils.py",
        "function:utils.py:shout"
    ));
    assert!(has_edge(
        &map,
        "exports",
        "file:models.py",
        "class:models.py:Greeter"
    ));
    assert!(
        !edges
            .iter()
            .any(|e| e["kind"] == "exports" && e["target"] == "function:utils.py:_decorate"),
        "underscore name got an exports edge: {edges:?}"
    );
    assert!(
        !edges
            .iter()
            .any(|e| e["kind"] == "exports" && e["target"] == "function:models.py:Greeter.greet"),
        "method got an exports edge: {edges:?}"
    );

    // Same-package module import, relative import, and package import
    // through __init__.py.
    assert!(
        has_edge(&map, "imports", "file:app.py", "file:utils.py"),
        "edges: {edges:?}"
    );
    assert!(
        has_edge(&map, "imports", "file:pkg/core.py", "file:pkg/helpers.py"),
        "edges: {edges:?}"
    );
    assert!(
        has_edge(&map, "imports", "file:use_pkg.py", "file:pkg/__init__.py"),
        "edges: {edges:?}"
    );

    // Calls: cross-file through an import, same-file to a private helper,
    // and into a package's __init__.py.
    assert!(
        has_edge(
            &map,
            "calls",
            "function:app.py:main",
            "function:utils.py:shout"
        ),
        "edges: {edges:?}"
    );
    assert!(
        has_edge(
            &map,
            "calls",
            "function:utils.py:shout",
            "function:utils.py:_decorate"
        ),
        "edges: {edges:?}"
    );
    assert!(
        has_edge(
            &map,
            "calls",
            "function:pkg/core.py:run",
            "function:pkg/helpers.py:fmt"
        ),
        "edges: {edges:?}"
    );
    assert!(
        has_edge(
            &map,
            "calls",
            "function:use_pkg.py:boot",
            "function:pkg/__init__.py:api"
        ),
        "edges: {edges:?}"
    );
}

#[test]
fn go_files_yield_symbols_imports_and_calls() {
    let repo = materialize("goproj");
    scan(repo.path());
    let map = read_map(repo.path());
    let ids = node_ids(&map);
    let edges = map["edges"].as_array().unwrap();

    // Functions, structs (as classes), and methods receiver-qualified.
    assert!(
        ids.contains(&"function:main.go:main".to_string()),
        "ids: {ids:?}"
    );
    assert!(ids.contains(&"function:util/util.go:Format".to_string()));
    assert!(ids.contains(&"class:util/util.go:Formatter".to_string()));
    assert!(ids.contains(&"function:util/util.go:Formatter.Render".to_string()));

    let util = map["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == "file:util/util.go")
        .unwrap();
    assert!(
        util["summary"].as_str().unwrap().contains("Go"),
        "summary: {}",
        util["summary"]
    );

    // Capitalized names are exported; lowercase names and methods are not.
    assert!(has_edge(
        &map,
        "exports",
        "file:util/util.go",
        "function:util/util.go:Format"
    ));
    assert!(has_edge(
        &map,
        "exports",
        "file:util/util.go",
        "class:util/util.go:Formatter"
    ));
    assert!(
        !edges
            .iter()
            .any(|e| e["kind"] == "exports" && e["target"] == "function:util/util.go:indent"),
        "lowercase fn got an exports edge: {edges:?}"
    );

    // Module-path import resolves to the in-repo package; stdlib never
    // resolves.
    assert!(
        has_edge(&map, "imports", "file:main.go", "file:util/util.go"),
        "edges: {edges:?}"
    );
    assert!(
        !edges.iter().any(|e| {
            e["kind"] == "imports"
                && e["target"]
                    .as_str()
                    .is_some_and(|t| t.contains("fmt") || t.contains("strings"))
        }),
        "stdlib import leaked: {edges:?}"
    );

    // Same-package calls resolve across files without an import; same-file
    // calls resolve as everywhere else.
    assert!(
        has_edge(
            &map,
            "calls",
            "function:main.go:main",
            "function:server.go:run"
        ),
        "edges: {edges:?}"
    );
    assert!(
        has_edge(
            &map,
            "calls",
            "function:server.go:run",
            "function:server.go:banner"
        ),
        "edges: {edges:?}"
    );
    assert!(
        has_edge(
            &map,
            "calls",
            "function:util/util.go:Format",
            "function:util/util.go:indent"
        ),
        "edges: {edges:?}"
    );
}

#[test]
fn markdown_relative_links_become_edges_between_file_nodes() {
    let repo = materialize("rustproj");
    scan(repo.path());
    let map = read_map(repo.path());
    let ids = node_ids(&map);
    let edges = map["edges"].as_array().unwrap();

    // Relative links — bare, `./`-prefixed, `../`-traversing, and carrying
    // anchors — resolve to any in-map file.
    assert!(
        has_edge(&map, "imports", "file:README.md", "file:docs/guide.md"),
        "edges: {edges:?}"
    );
    assert!(
        has_edge(&map, "imports", "file:README.md", "file:src/main.rs"),
        "edges: {edges:?}"
    );
    assert!(
        has_edge(&map, "imports", "file:docs/guide.md", "file:README.md"),
        "edges: {edges:?}"
    );

    // External URLs, missing files, pure anchors, and links inside code
    // fences produce no edge.
    assert!(
        !edges.iter().any(|e| {
            e["source"] == "file:README.md"
                && e["target"]
                    .as_str()
                    .is_some_and(|t| t.contains("nope") || t.contains("example.com"))
        }),
        "unresolvable markdown link leaked: {edges:?}"
    );
    assert!(
        !edges
            .iter()
            .any(|e| e["source"] == "file:docs/guide.md" && e["target"] == "file:src/util.rs"),
        "code-fence link became an edge: {edges:?}"
    );

    // Markdown files contribute no symbol nodes.
    assert!(
        !ids.iter()
            .any(|id| id.contains(".md") && !id.starts_with("file:")),
        "markdown produced symbol nodes: {ids:?}"
    );
    let readme = map["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == "file:README.md")
        .unwrap();
    assert!(
        readme["summary"].as_str().unwrap().contains("Markdown"),
        "summary: {}",
        readme["summary"]
    );
}

#[test]
fn importing_a_non_exported_function_produces_no_call_edge() {
    let repo = materialize("simple");
    scan(repo.path());
    let map = read_map(repo.path());
    let edges = map["edges"].as_array().unwrap();

    // hidden.ts keeps `secret` module-private; sneak.ts imports and calls it
    // anyway. The map must not claim a call relationship the module system
    // does not allow.
    assert!(
        !edges.iter().any(|e| e["kind"] == "calls"
            && e["source"] == "function:src/sneak.ts:trySneak"
            && e["target"] == "function:src/hidden.ts:secret"),
        "call to a non-exported import leaked: {edges:?}"
    );
    // The legitimate in-file call to the private function still exists.
    assert!(
        has_edge(
            &map,
            "calls",
            "function:src/hidden.ts:open",
            "function:src/hidden.ts:secret"
        ),
        "edges: {edges:?}"
    );
}

#[test]
fn barrel_reexports_resolve_one_level_to_the_defining_file() {
    let repo = materialize("simple");
    scan(repo.path());
    let map = read_map(repo.path());
    let edges = map["edges"].as_array().unwrap();

    // useBarrel.ts imports greet from barrel.ts, which re-exports it from
    // util.ts: the call resolves through the barrel to the defining file.
    assert!(
        has_edge(
            &map,
            "calls",
            "function:src/useBarrel.ts:viaBarrel",
            "function:src/util.ts:greet"
        ),
        "edges: {edges:?}"
    );
    // The re-export is also a file-level dependency of the barrel.
    assert!(
        has_edge(&map, "imports", "file:src/barrel.ts", "file:src/util.ts"),
        "edges: {edges:?}"
    );
}

#[test]
fn aliased_exports_resolve_calls_to_the_defining_function() {
    let repo = materialize("simple");
    scan(repo.path());
    let map = read_map(repo.path());
    let edges = map["edges"].as_array().unwrap();

    // alias.ts publishes `internal` as `external`; a call through the alias
    // lands on the defining function node.
    assert!(
        has_edge(
            &map,
            "calls",
            "function:src/useAlias.ts:callAliased",
            "function:src/alias.ts:internal"
        ),
        "edges: {edges:?}"
    );
    // The aliased export still marks the local symbol exported.
    assert!(
        has_edge(
            &map,
            "exports",
            "file:src/alias.ts",
            "function:src/alias.ts:internal"
        ),
        "edges: {edges:?}"
    );
}

#[test]
fn dogfood_scanning_codeatlas_itself_yields_a_schema_valid_polyglot_map() {
    // The dogfood milestone: CodeAtlas maps its own repository — the Rust
    // core, the TS dashboard, and the documentation's link structure.
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    assert_cmd::Command::cargo_bin("codeatlas")
        .unwrap()
        .arg("scan")
        .arg(&repo_root)
        .assert()
        .success();
    let map = read_map(&repo_root);

    // Schema-valid against the binary's own generated schema.
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
    assert!(
        errors.is_empty(),
        "self-scan violates the schema: {errors:?}"
    );

    // Function nodes from the Rust source.
    let ids = node_ids(&map);
    assert!(
        ids.contains(&"function:crates/codeatlas/src/scan.rs:scan".to_string()),
        "missing Rust function node"
    );
    // Function nodes from the TypeScript dashboard.
    assert!(
        ids.iter()
            .any(|id| id.starts_with("function:dashboard/src/")),
        "missing TS function node"
    );
    // At least one Markdown link edge (the docs cross-reference heavily).
    assert!(
        map["edges"].as_array().unwrap().iter().any(|e| {
            e["kind"] == "imports"
                && e["source"].as_str().is_some_and(|s| s.ends_with(".md"))
                && e["target"].as_str().is_some_and(|t| t.starts_with("file:"))
        }),
        "no markdown link edge in the self-scan"
    );
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
