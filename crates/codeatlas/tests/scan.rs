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

/// The number a mechanical tour label cites after `key` — e.g.
/// `cited("src/util.ts — fan-in 4, fan-out 0", "fan-in ")` is 4.
fn cited(label: &str, key: &str) -> u64 {
    let at = label
        .find(key)
        .unwrap_or_else(|| panic!("label does not cite `{key}`: {label}"));
    let digits: String = label[at + key.len()..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits
        .parse()
        .unwrap_or_else(|_| panic!("no number after `{key}`: {label}"))
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

    // The mechanical summary is the fallback a reader is meant to trust when
    // a map is not enriched, and it is a file card's caption on screen — so
    // it has to be spelled correctly. `pair.ts` holds Alpha and Beta.
    let pair = nodes
        .iter()
        .find(|n| n["id"] == "file:src/pair.ts")
        .unwrap();
    let pair_summary = pair["summary"].as_str().unwrap();
    assert!(
        pair_summary.contains("2 classes"),
        "expected `2 classes`, got: {pair_summary}"
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
fn rust_crate_name_paths_resolve_across_a_workspace() {
    let repo = materialize("rustws");
    scan(repo.path());
    let map = read_map(repo.path());
    let edges = map["edges"].as_array().unwrap();

    // A crate naming itself. Integration tests have no other way to reach
    // their own library — `crate::` does not span the tests/ boundary — so
    // dropping this form orphans every tests/ file in every Rust project.
    // The package is `atlas-engine` and the path is `atlas_engine`: Cargo's
    // own normalisation, which the resolver has to share.
    assert!(
        has_edge(
            &map,
            "imports",
            "file:crates/atlas-engine/tests/it.rs",
            "file:crates/atlas-engine/src/engine.rs"
        ),
        "a crate naming itself did not resolve: {edges:?}"
    );

    // Workspace siblings. Multi-crate is the ordinary shape of a Rust
    // project, and without this every crate in one is an island.
    assert!(
        has_edge(
            &map,
            "imports",
            "file:crates/cli/src/main.rs",
            "file:crates/atlas-engine/src/engine.rs"
        ),
        "a workspace sibling did not resolve: {edges:?}"
    );

    // Package `atlas-tools` lives in `toolbox/`. Nothing in the path says
    // so, so this edge exists only if the manifest is what decides a crate's
    // name — a directory-name guess cannot find it.
    assert!(
        has_edge(
            &map,
            "imports",
            "file:crates/cli/src/main.rs",
            "file:crates/toolbox/src/lib.rs"
        ),
        "a package named differently from its directory did not resolve: {edges:?}"
    );

    // `log` is a crate in this tree and also one of the best-known names on
    // crates.io. The one in the tree wins, deliberately: the alternative is a
    // denylist of every published name, and an edge to a file the reader can
    // open beats no edge.
    //
    // It is also duplicated at vendor/log, so this pins which of two equally
    // valid candidates is chosen — the nearer one. Without a rule the answer
    // would depend on hash iteration order and differ between runs; with the
    // wrong rule a workspace reaches into its own vendor directory.
    assert!(
        has_edge(
            &map,
            "imports",
            "file:crates/cli/src/main.rs",
            "file:crates/log/src/lib.rs"
        ),
        "a scanned crate lost to its crates.io namesake: {edges:?}"
    );
    assert!(
        !has_edge(
            &map,
            "imports",
            "file:crates/cli/src/main.rs",
            "file:vendor/log/src/lib.rs"
        ),
        "the workspace reached past its own crate into a vendored namesake: {edges:?}"
    );

    // The same rule read from the other side. Sorted by path, `crates/log`
    // comes first, so an importer inside `vendor/` is what proves the choice
    // is nearness rather than merely a stable order.
    assert!(
        has_edge(
            &map,
            "imports",
            "file:vendor/app/src/main.rs",
            "file:vendor/log/src/lib.rs"
        ),
        "a vendored crate resolved against the workspace instead of its own tree: {edges:?}"
    );

    // `use serde::Serialize` names a crate that is *not* in the scanned tree,
    // and inventing an edge for it would be worse than dropping one. Asserted
    // as the exact edge set rather than "no target called serde": a wrongly
    // resolved external path does not point at something named after the
    // crate, it points at some real file, so only the whole set catches it.
    let mut from_main: Vec<&str> = edges
        .iter()
        .filter(|e| e["kind"] == "imports" && e["source"] == "file:crates/cli/src/main.rs")
        .filter_map(|e| e["target"].as_str())
        .collect();
    from_main.sort_unstable();
    assert_eq!(
        from_main,
        [
            "file:crates/atlas-engine/src/engine.rs",
            "file:crates/log/src/lib.rs",
            "file:crates/toolbox/src/lib.rs"
        ],
        "main.rs names four crates and exactly three of them are in the tree"
    );
}

#[test]
fn a_rust_crate_at_the_repository_root_still_knows_its_own_name() {
    // The commonest Rust layout there is: one crate, `src/` at the top. Its
    // directory name appears in no scanned path, so the manifest is the only
    // place its name exists.
    let repo = materialize("rustroot");
    scan(repo.path());
    let map = read_map(repo.path());
    let edges = map["edges"].as_array().unwrap();

    assert!(
        has_edge(&map, "imports", "file:tests/it.rs", "file:src/util.rs"),
        "`use root_lib::util` did not resolve at the repository root: {edges:?}"
    );

    // The last segment of a `use` path may be a module rather than an item —
    // the same shape that `from pkg import util` has in Python, where it
    // needed a rule of its own (ticket 20). Rust resolves it already; this
    // is what keeps that answer from going stale.
    assert!(
        has_edge(&map, "imports", "file:src/lib.rs", "file:src/deep/leaf.rs"),
        "`use crate::deep::leaf` stopped at the module above the leaf: {edges:?}"
    );
}

#[test]
fn typescript_nodenext_specifiers_resolve_to_their_source_files() {
    let repo = materialize("simple");
    scan(repo.path());
    let map = read_map(repo.path());
    let edges = map["edges"].as_array().unwrap();

    // TypeScript under NodeNext requires source to name the emitted file:
    // `./util.js` is `util.ts` on disk. Dropping these leaves whole
    // TypeScript codebases looking unconnected.
    assert!(
        has_edge(&map, "imports", "file:src/nodenext.ts", "file:src/util.ts"),
        "`./util.js` did not resolve to util.ts: {edges:?}"
    );
    assert!(
        has_edge(
            &map,
            "imports",
            "file:src/nodenext.ts",
            "file:src/widget.tsx"
        ),
        "`./widget.jsx` did not resolve to widget.tsx: {edges:?}"
    );

    // The literal path still wins. `twin.js` genuinely exists beside
    // `twin.ts`, so rewriting the extension must not shadow it — otherwise
    // the fix breaks every JavaScript project it touches.
    assert!(
        has_edge(&map, "imports", "file:src/nodenext.ts", "file:src/twin.js"),
        "a real .js file lost to its .ts sibling: {edges:?}"
    );
    assert!(
        !has_edge(&map, "imports", "file:src/nodenext.ts", "file:src/twin.ts"),
        "specifier resolved past a file that exists: {edges:?}"
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

    // Import degree read straight off the emitted edges. The mechanical
    // label claims these numbers, so the test checks the claim rather than
    // hard-coding counts that every new fixture file would invalidate.
    let mut fan_in: std::collections::HashMap<&str, u64> = std::collections::HashMap::new();
    let mut fan_out: std::collections::HashMap<&str, u64> = std::collections::HashMap::new();
    for edge in map["edges"].as_array().unwrap() {
        if edge["kind"] != "imports" {
            continue;
        }
        *fan_out.entry(edge["source"].as_str().unwrap()).or_default() += 1;
        *fan_in.entry(edge["target"].as_str().unwrap()).or_default() += 1;
    }

    let node_ids: std::collections::HashSet<String> = node_ids(&map).into_iter().collect();
    let mut steps: Vec<&str> = Vec::new();
    for step in tour {
        let node = step["node"].as_str().unwrap();
        assert!(node_ids.contains(node), "dangling tour step: {step:?}");
        let label = step["label"].as_str().unwrap();
        assert!(
            !label.is_empty(),
            "tour step must carry a mechanical label: {step:?}"
        );
        // A label that cites topology must cite the topology actually
        // emitted, or it quietly lies as the graph grows around it.
        assert_eq!(
            cited(label, "fan-in "),
            fan_in.get(node).copied().unwrap_or(0),
            "label cites a fan-in the graph does not have: {step:?}"
        );
        assert_eq!(
            cited(label, "fan-out "),
            fan_out.get(node).copied().unwrap_or(0),
            "label cites a fan-out the graph does not have: {step:?}"
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

    // Curated, not an inventory: a file nothing imports, that imports
    // nothing, and that starts no call chain teaches a newcomer nothing
    // about how the pieces connect, so it is off the walk entirely.
    for id in [
        "file:README.md",
        "file:src/pair.ts",
        "file:src/greeter.ts",
        "file:src/broken.ts",
    ] {
        assert!(
            node_ids.contains(id),
            "{id} must still be a node — only the tour skips it"
        );
        assert!(
            !steps.contains(&id),
            "file with no place in the architecture is on the tour: {id}"
        );
    }
}

#[test]
fn the_tour_stays_newcomer_sized_however_many_files_the_repo_holds() {
    let repo = materialize("simple");
    // Far more connected files than a tour may hold — plus files nothing
    // imports, that import nothing, and that start no call chain.
    for i in 0..30 {
        fs::write(
            repo.path().join(format!("src/chain{i}.ts")),
            format!(
                "import {{ greet }} from \"./util\";\n\n\
                 export function chain{i}(): string {{\n  return greet(\"{i}\");\n}}\n"
            ),
        )
        .unwrap();
    }
    for i in 0..5 {
        fs::write(
            repo.path().join(format!("src/lonely{i}.ts")),
            format!("export const lonely{i} = {i};\n"),
        )
        .unwrap();
    }
    scan(repo.path());
    let map = read_map(repo.path());

    let tour = map["tour"].as_array().expect("map must carry a tour");
    let files = map["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|n| n["kind"] == "file")
        .count();
    assert!(
        files > 40,
        "the fixture must outgrow the tour bound to prove anything"
    );
    assert_eq!(
        tour.len(),
        codeatlas::semantics::TOUR_MAX_STEPS,
        "a {files}-file repo must still produce a newcomer-sized tour"
    );

    let steps: Vec<&str> = tour.iter().map(|s| s["node"].as_str().unwrap()).collect();
    for i in 0..5 {
        let lonely = format!("file:src/lonely{i}.ts");
        assert!(
            !steps.contains(&lonely.as_str()),
            "isolated file on the tour: {lonely} ({steps:?})"
        );
    }
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
fn python_from_package_import_module_reaches_the_module() {
    // `from pkg import util` binds either a module file or a symbol defined
    // in the package, and the statement itself does not say which. Resolving
    // the specifier alone answers "the package initialiser" every time,
    // which is the wrong answer whenever the name is a module — and no
    // answer at all when the package is a PEP 420 namespace package with no
    // `__init__.py`.
    let repo = materialize("pypkgs");
    scan(repo.path());
    let map = read_map(repo.path());

    // The whole set, so this pins the absences as tightly as the presences:
    // a module import must *not* also drag in the package initialiser, and
    // nothing unresolvable may invent an edge.
    let mut found: Vec<(String, String)> = map["edges"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["kind"] == "imports")
        .map(|e| {
            (
                e["source"].as_str().unwrap().to_string(),
                e["target"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    found.sort();
    let expected: Vec<(String, String)> = [
        // `from . import util` inside a package with an initialiser.
        ("file:pkg/inside.py", "file:pkg/util.py"),
        // The same relative form inside a namespace package, where the
        // initialiser that used to absorb the edge does not exist.
        ("file:ns/emit.py", "file:ns/parse.py"),
        // A module name and a symbol name in one statement resolve apart.
        ("file:uses_both.py", "file:pkg/__init__.py"),
        ("file:uses_both.py", "file:pkg/util.py"),
        // The bound name may be aliased; the module is named by `imported`,
        // never by the local alias.
        ("file:uses_alias.py", "file:pkg/util.py"),
        ("file:uses_dotted.py", "file:pkg/util.py"),
        // `import pkg.util as pu` — the alias changes what a call site may
        // write, not where the statement points.
        ("file:uses_dotted_alias.py", "file:pkg/util.py"),
        ("file:uses_module.py", "file:pkg/util.py"),
        ("file:uses_ns.py", "file:ns/parse.py"),
        // Both candidates exist: `pkg/shadow.py` and a `shadow` symbol in
        // `pkg/__init__.py`. The module wins, and this is the one case where
        // the two candidate orders disagree.
        ("file:uses_shadow.py", "file:pkg/shadow.py"),
        // Script style: neither `local` nor a root-level anchor exists, so
        // this resolves only by trying the name as a module beside the
        // importer.
        ("file:scripts/tool.py", "file:scripts/local/render.py"),
        // Preserved: a symbol the package defines still lands on the
        // initialiser, and so does a wildcard, which binds no name to try.
        ("file:uses_star.py", "file:pkg/__init__.py"),
        ("file:uses_symbol.py", "file:pkg/__init__.py"),
        // A name that is neither module nor symbol is indistinguishable from
        // a symbol without reading the initialiser, so it falls back the
        // same way. The statement does execute the initialiser.
        ("file:uses_unknown.py", "file:pkg/__init__.py"),
    ]
    .iter()
    .map(|(s, t)| (s.to_string(), t.to_string()))
    .collect();
    let mut expected_sorted = expected;
    expected_sorted.sort();
    assert_eq!(
        found, expected_sorted,
        "the import edges of the pypkgs fixture are not what the resolution rules say"
    );

    // Calls still cross into the initialiser when the bound name really is a
    // symbol there — including from the statement that also binds a module.
    for caller in ["uses_symbol.py:boot", "uses_both.py:both"] {
        assert!(
            has_edge(
                &map,
                "calls",
                &format!("function:{caller}"),
                "function:pkg/__init__.py:api"
            ),
            "a symbol bound by a from-import lost its call edge: {caller}"
        );
    }

    // The two rules pull opposite ways on the same statement, and both have
    // to hold at once. `from pkg import shadow` points its *edge* at
    // `pkg/shadow.py`, asserted above — but `shadow()` is a bare call, which
    // a module can never answer, so the *call* must still find the symbol in
    // the package initialiser. Resolving the name to one file and stopping
    // there silently drops this edge.
    assert!(
        has_edge(
            &map,
            "calls",
            "function:uses_shadow.py:ambiguous",
            "function:pkg/__init__.py:shadow"
        ),
        "a call to a symbol shadowed by a module of the same name was dropped"
    );

    // Referential integrity over the whole fixture: resolving names as
    // modules must not name a file that is not in the map.
    let ids: std::collections::HashSet<String> = node_ids(&map).into_iter().collect();
    for edge in map["edges"].as_array().unwrap() {
        for end in ["source", "target"] {
            let id = edge[end].as_str().unwrap();
            assert!(ids.contains(id), "dangling edge {end} {id}: {edge:?}");
        }
    }
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
fn c_files_yield_symbols_with_linkage_exports() {
    let repo = materialize("cproj");
    scan(repo.path());
    let map = read_map(repo.path());
    let ids = node_ids(&map);
    let edges = map["edges"].as_array().unwrap();

    // Function definitions with line ranges; named structs (and typedef
    // structs) are the C analog of classes.
    assert!(
        ids.contains(&"function:main.c:main".to_string()),
        "ids: {ids:?}"
    );
    assert!(ids.contains(&"function:util.c:util_greet".to_string()));
    assert!(ids.contains(&"function:util.c:decorate".to_string()));
    assert!(ids.contains(&"class:util.c:point".to_string()));
    assert!(ids.contains(&"class:util.h:frame".to_string()));

    let nodes = map["nodes"].as_array().unwrap();
    let greet = nodes
        .iter()
        .find(|n| n["id"] == "function:util.c:util_greet")
        .unwrap();
    assert_eq!(greet["range"]["start_line"], 17);
    assert_eq!(greet["range"]["end_line"], 19);

    let util = nodes.iter().find(|n| n["id"] == "file:util.c").unwrap();
    let summary = util["summary"].as_str().unwrap();
    assert!(
        summary.starts_with("C file") && summary.contains("2 functions"),
        "summary: {summary}"
    );

    // Linkage is the export convention: non-static file-scope functions are
    // exported, `static` ones are not.
    assert!(has_edge(
        &map,
        "exports",
        "file:util.c",
        "function:util.c:util_greet"
    ));
    assert!(
        !edges
            .iter()
            .any(|e| e["kind"] == "exports" && e["target"] == "function:util.c:decorate"),
        "static fn got an exports edge: {edges:?}"
    );
    assert!(
        !edges
            .iter()
            .any(|e| e["kind"] == "exports" && e["target"] == "function:main.c:local_note"),
        "static fn got an exports edge: {edges:?}"
    );
    // A header's prototypes are declarations, not definitions: no function
    // node for a bare prototype.
    assert!(
        !ids.contains(&"function:util.h:util_greet".to_string()),
        "prototype became a function node: {ids:?}"
    );
}

#[test]
fn quoted_includes_resolve_to_imports_edges_and_pair_header_with_source() {
    let repo = materialize("cproj");
    scan(repo.path());
    let map = read_map(repo.path());
    let edges = map["edges"].as_array().unwrap();

    // Includer-dir-relative resolution, including `../` traversal, and the
    // repo-internal include chain main.c → app/app.h → util.h.
    assert!(
        has_edge(&map, "imports", "file:main.c", "file:app/app.h"),
        "edges: {edges:?}"
    );
    assert!(
        has_edge(&map, "imports", "file:app/app.h", "file:util.h"),
        "edges: {edges:?}"
    );
    assert!(
        has_edge(&map, "imports", "file:main.c", "file:util.h"),
        "edges: {edges:?}"
    );
    // Repo-root-relative fallback: app/app.c includes "util.h" which is not
    // next to it but at the root (the -I<root> build convention).
    assert!(
        has_edge(&map, "imports", "file:app/app.c", "file:util.h"),
        "edges: {edges:?}"
    );
    // Header/source pairing: the implementation includes its own header.
    assert!(
        has_edge(&map, "imports", "file:util.c", "file:util.h"),
        "edges: {edges:?}"
    );
    assert!(
        has_edge(&map, "imports", "file:app/app.c", "file:app/app.h"),
        "edges: {edges:?}"
    );
    // System includes are ignored: no edge, and certainly never a dangling
    // one.
    assert!(
        !edges.iter().any(|e| {
            e["target"]
                .as_str()
                .is_some_and(|t| t.contains("stdio") || t.contains("string.h"))
        }),
        "system include leaked into the map: {edges:?}"
    );
}

#[test]
fn c_calls_resolve_through_included_headers_to_the_implementation() {
    let repo = materialize("cproj");
    scan(repo.path());
    let map = read_map(repo.path());
    let edges = map["edges"].as_array().unwrap();

    // Same-file call, static callee included.
    assert!(
        has_edge(
            &map,
            "calls",
            "function:util.c:util_greet",
            "function:util.c:decorate"
        ),
        "edges: {edges:?}"
    );
    assert!(
        has_edge(
            &map,
            "calls",
            "function:main.c:main",
            "function:main.c:local_note"
        ),
        "edges: {edges:?}"
    );
    // Cross-file: the call lands on the implementation, not the header —
    // `#include "util.h"` routes util_greet() to util.c where it is defined.
    assert!(
        has_edge(
            &map,
            "calls",
            "function:main.c:main",
            "function:util.c:util_greet"
        ),
        "edges: {edges:?}"
    );
    assert!(
        has_edge(
            &map,
            "calls",
            "function:app/app.c:app_run",
            "function:util.c:util_greet"
        ),
        "edges: {edges:?}"
    );
    assert!(
        has_edge(
            &map,
            "calls",
            "function:main.c:main",
            "function:app/app.c:app_run"
        ),
        "edges: {edges:?}"
    );
    // Out-of-repo callees (printf, puts, strcpy) never become edges.
    assert!(
        !edges.iter().any(|e| {
            e["kind"] == "calls"
                && e["target"].as_str().is_some_and(|t| {
                    t.contains("printf") || t.contains("puts") || t.contains("strcpy")
                })
        }),
        "libc call leaked into the map: {edges:?}"
    );
}

#[test]
fn cpp_files_yield_classes_methods_includes_and_calls() {
    let repo = materialize("cppproj");
    scan(repo.path());
    let map = read_map(repo.path());
    let ids = node_ids(&map);
    let edges = map["edges"].as_array().unwrap();

    // Classes, inline methods scope-qualified, and out-of-class qualified
    // definitions (`Circle::area`) landing in the implementation file.
    assert!(
        ids.contains(&"class:geometry.hpp:Circle".to_string()),
        "ids: {ids:?}"
    );
    assert!(ids.contains(&"function:geometry.hpp:Circle.radius".to_string()));
    assert!(ids.contains(&"function:geometry.cpp:Circle.area".to_string()));
    assert!(ids.contains(&"function:geometry.cpp:tau".to_string()));
    assert!(ids.contains(&"function:main.cpp:main".to_string()));
    // The `.cc` extension is C++ too.
    assert!(ids.contains(&"function:report.cc:report".to_string()));

    let geom = map["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == "file:geometry.cpp")
        .unwrap();
    assert!(
        geom["summary"].as_str().unwrap().starts_with("C++ file"),
        "summary: {}",
        geom["summary"]
    );

    // Linkage exports: free functions yes, static and methods no.
    assert!(has_edge(
        &map,
        "exports",
        "file:geometry.cpp",
        "function:geometry.cpp:tau"
    ));
    assert!(
        !edges
            .iter()
            .any(|e| e["kind"] == "exports" && e["target"] == "function:geometry.cpp:square"),
        "static fn got an exports edge: {edges:?}"
    );
    assert!(
        !edges
            .iter()
            .any(|e| e["kind"] == "exports" && e["target"] == "function:geometry.cpp:Circle.area"),
        "method got an exports edge: {edges:?}"
    );

    // Includes and pairing, across .cpp and .cc implementations.
    assert!(
        has_edge(&map, "imports", "file:main.cpp", "file:geometry.hpp"),
        "edges: {edges:?}"
    );
    assert!(
        has_edge(&map, "imports", "file:geometry.cpp", "file:geometry.hpp"),
        "edges: {edges:?}"
    );
    assert!(
        has_edge(&map, "imports", "file:report.cc", "file:report.hpp"),
        "edges: {edges:?}"
    );

    // Calls resolve through headers to the implementation: a .cpp pair, a
    // .cc pair, and a C-parsed `.h` header fronting a C++ source.
    assert!(
        has_edge(
            &map,
            "calls",
            "function:main.cpp:main",
            "function:report.cc:report"
        ),
        "edges: {edges:?}"
    );
    assert!(
        has_edge(
            &map,
            "calls",
            "function:main.cpp:main",
            "function:legacy.cpp:legacy_go"
        ),
        "edges: {edges:?}"
    );
    // Same-file calls from a qualified method definition.
    assert!(
        has_edge(
            &map,
            "calls",
            "function:geometry.cpp:Circle.area",
            "function:geometry.cpp:tau"
        ),
        "edges: {edges:?}"
    );
    assert!(
        has_edge(
            &map,
            "calls",
            "function:geometry.cpp:Circle.area",
            "function:geometry.cpp:square"
        ),
        "edges: {edges:?}"
    );
}

#[test]
fn external_go_modules_with_colliding_package_suffixes_produce_no_edge() {
    let repo = materialize("goproj");
    scan(repo.path());
    let map = read_map(repo.path());
    let edges = map["edges"].as_array().unwrap();

    // external.go imports github.com/external/lib/util — an external module
    // whose trailing segment collides with the in-repo util/ package. The
    // go.mod module line (example.com/demo) says it is not ours: no edge.
    assert!(
        !edges
            .iter()
            .any(|e| e["kind"] == "imports" && e["source"] == "file:external.go"),
        "external module import leaked into the map: {edges:?}"
    );
    // The genuine module-path import still resolves.
    assert!(
        has_edge(&map, "imports", "file:main.go", "file:util/util.go"),
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

#[test]
fn scanning_a_polyglot_repo_twice_is_byte_identical() {
    // Determinism across every supported language in one map: TS, Rust,
    // Python, Go, C, C++, and Markdown.
    let repo = materialize("polyglot");
    scan(repo.path());
    let first = fs::read(repo.path().join(".codeatlas/knowledge-graph.json")).unwrap();
    scan(repo.path());
    let second = fs::read(repo.path().join(".codeatlas/knowledge-graph.json")).unwrap();
    assert_eq!(first, second);

    // The fixture is honest: every language actually contributed symbols.
    let map: serde_json::Value = serde_json::from_slice(&first).unwrap();
    let ids = node_ids(&map);
    for expected in [
        "function:hello.ts:greet",
        "function:hello.rs:greet",
        "function:hello.py:greet",
        "function:hello.go:Greet",
        "function:hello.c:hello_greet",
        "function:shape.cpp:Shape.area",
        "file:README.md",
    ] {
        assert!(ids.contains(&expected.to_string()), "ids: {ids:?}");
    }
}

// ---------------------------------------------------------------------------
// Qualified calls (ticket 21). A call reaching a function *through the module
// that holds it* — `util.helper()`, `crate::util::helper()` — is the ordinary
// way most code calls across a file boundary. Every one of these forms used to
// resolve to nothing, in every language, because the callee was only ever
// looked up as a bare name bound directly by an import.
//
// One test per language, covering every form that language writes, because a
// language's call conventions are a checklist and the fixture exercising one
// of them is not evidence for the rest.
// ---------------------------------------------------------------------------

#[test]
fn rust_qualified_calls_resolve_through_the_module_that_holds_them() {
    let repo = materialize("rustroot");
    scan(repo.path());
    let map = read_map(repo.path());
    let edges = map["edges"].as_array().unwrap();

    let helper = "function:src/util.rs:helper";
    for (caller, form) in [
        ("function:src/lib.rs:from_crate_root", "crate::util::helper()"),
        ("function:src/lib.rs:from_bare_module", "util::helper()"),
        ("function:src/lib.rs:through_alias", "use crate::util as u; u::helper()"),
        ("function:src/lib.rs:from_bound_name", "use util::helper; helper()"),
        ("function:src/lib.rs:from_self", "self::util::helper()"),
        ("function:src/deep/mod.rs:up_and_across", "super::util::helper()"),
    ] {
        assert!(
            has_edge(&map, "calls", caller, helper),
            "`{form}` produced no call edge: {edges:?}"
        );
    }

    // A qualified call through a module bound by `use crate::deep::leaf`.
    assert!(
        has_edge(
            &map,
            "calls",
            "function:src/lib.rs:tip",
            "function:src/deep/leaf.rs:tip"
        ),
        "`leaf::tip()` produced no call edge: {edges:?}"
    );

    // Deliberately *not* asserted here: that `use util::helper;` makes the
    // file edge. `pub mod util;` in the same file already makes it, and Rust
    // requires that declaration to exist for the `use` to be legal at all —
    // so the assertion would pass whether or not the bare path resolved, and
    // prove nothing. What the bare path is needed for is the *binding*, and
    // `from_bound_name` above is what fails when it is missing.

    // Resolving more must not invent edges. `serde_json` is outside the tree.
    assert!(
        !edges.iter().any(|e| {
            e["kind"] == "calls" && e["source"] == "function:src/lib.rs:external"
        }),
        "a call into a crate outside the map produced an edge: {edges:?}"
    );
}

#[test]
fn python_qualified_calls_resolve_through_the_module_that_holds_them() {
    let repo = materialize("pypkgs");
    scan(repo.path());
    let map = read_map(repo.path());
    let edges = map["edges"].as_array().unwrap();

    let helper = "function:pkg/util.py:helper";
    for (caller, form) in [
        ("function:uses_module.py:run", "from pkg import util; util.helper()"),
        ("function:uses_dotted.py:dotted", "import pkg.util; pkg.util.helper()"),
        ("function:uses_alias.py:aliased", "from pkg import util as u; u.helper()"),
        ("function:uses_dotted_alias.py:dotted_alias", "import pkg.util as pu; pu.helper()"),
        ("function:pkg/inside.py:use", "from . import util; util.helper()"),
        ("function:uses_both.py:both", "one statement binding module and symbol"),
    ] {
        assert!(
            has_edge(&map, "calls", caller, helper),
            "`{form}` produced no call edge: {edges:?}"
        );
    }

    // A namespace package (no `__init__.py`), relative and absolute.
    for caller in ["function:ns/emit.py:emit", "function:uses_ns.py:run_ns"] {
        assert!(
            has_edge(&map, "calls", caller, "function:ns/parse.py:parse_it"),
            "a namespace-package qualified call produced no edge: {edges:?}"
        );
    }

    // Script style: resolved only by trying the name as a module beside the
    // importer, and the call must follow the import.
    assert!(
        has_edge(
            &map,
            "calls",
            "function:scripts/tool.py:main",
            "function:scripts/local/render.py:text"
        ),
        "`from local import render; render.text()` produced no edge: {edges:?}"
    );

    // The standard library is outside the map: `os.helper()` invents nothing,
    // though `helper` is a name this map really does export.
    assert!(
        !edges
            .iter()
            .any(|e| e["kind"] == "calls" && e["source"] == "function:uses_absent.py:nothing"),
        "a call into an unmapped module produced an edge: {edges:?}"
    );

    // The receiver of a dotted call is usually a *value*, and a value that
    // happens to share a name with a module beside it is an everyday shape —
    // `logger.info()`, `config.get()`, `parser.parse()`. Following a dotted
    // receiver that no import introduced turns every one of those into a
    // fabricated edge between two files with no relationship at all.
    assert!(
        !edges.iter().any(
            |e| e["kind"] == "calls" && e["source"] == "function:pkg/uses_value.py:call_on_a_value"
        ),
        "a dotted call on a plain value resolved to the module beside it: {edges:?}"
    );
}

#[test]
fn typescript_namespace_imports_resolve_qualified_calls() {
    let repo = materialize("simple");
    scan(repo.path());
    let map = read_map(repo.path());
    let edges = map["edges"].as_array().unwrap();

    let greet = "function:src/util.ts:greet";
    for (caller, form) in [
        ("function:src/namespace.ts:viaNamespace", "import * as util; util.greet()"),
        ("function:src/namespace.ts:viaAlias", "the same module under another local name"),
    ] {
        assert!(
            has_edge(&map, "calls", caller, greet),
            "`{form}` produced no call edge: {edges:?}"
        );
    }

    // A namespace import still makes the file edge it always did.
    assert!(
        has_edge(&map, "imports", "file:src/namespace.ts", "file:src/util.ts"),
        "`import * as util from \"./util\"` did not resolve: {edges:?}"
    );

    // `node:util` is outside the map, and `greet` is a name inside it — so a
    // resolver matching on the callee's name alone would wire this up.
    assert!(
        !edges.iter().any(|e| {
            e["kind"] == "calls" && e["source"] == "function:src/namespace.ts:viaExternal"
        }),
        "a call into a builtin package produced an edge: {edges:?}"
    );
}
