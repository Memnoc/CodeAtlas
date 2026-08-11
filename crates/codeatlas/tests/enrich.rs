//! Tests of enrichment behaviour at the two agreed seams: the map contract
//! (run the binary, assert on the emitted map) and the enrichment provider
//! trait (a fake provider returns canned typed responses — see
//! `src/enrich.rs` unit tests for the in-process side). No test here ever
//! performs network I/O: the binary under test selects its provider through
//! `--provider` or the `CODEATLAS_ENRICH_PROVIDER` env var, whose fake/fail
//! backends are compiled in only for test builds (the `test-provider`
//! feature).

use std::fs;
use std::path::{Path, PathBuf};

mod common;

/// The provider-selection env var the test-built binary honors.
const PROVIDER_ENV: &str = "CODEATLAS_ENRICH_PROVIDER";

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

/// Runs `codeatlas scan [--enrich]` in `repo` with the provider env var
/// either cleared (`None`) or set to the given spec.
fn scan(repo: &Path, enrich: bool, provider: Option<&str>) -> assert_cmd::assert::Assert {
    let mut cmd = assert_cmd::Command::cargo_bin("codeatlas").unwrap();
    cmd.arg("scan").current_dir(repo).env_remove(PROVIDER_ENV);
    if enrich {
        cmd.arg("--enrich");
    }
    if let Some(spec) = provider {
        cmd.env(PROVIDER_ENV, spec);
    }
    cmd.assert()
}

/// Runs `codeatlas scan --enrich` with the provider chosen through either
/// surface, or both, so precedence between them is expressible (ticket 29).
fn scan_selecting(
    repo: &Path,
    env: Option<&str>,
    flag: Option<&str>,
) -> assert_cmd::assert::Assert {
    let mut cmd = assert_cmd::Command::cargo_bin("codeatlas").unwrap();
    cmd.arg("scan")
        .arg("--enrich")
        .current_dir(repo)
        .env_remove(PROVIDER_ENV);
    if let Some(spec) = env {
        cmd.env(PROVIDER_ENV, spec);
    }
    if let Some(spec) = flag {
        cmd.args(["--provider", spec]);
    }
    cmd.assert()
}

fn read_map(repo: &Path) -> serde_json::Value {
    let raw = fs::read_to_string(repo.join(".codeatlas/knowledge-graph.json")).unwrap();
    serde_json::from_str(&raw).unwrap()
}

fn node<'m>(map: &'m serde_json::Value, id: &str) -> &'m serde_json::Value {
    map["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == id)
        .unwrap_or_else(|| panic!("node {id} missing from the map"))
}

fn assert_schema_valid(map: &serde_json::Value) {
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
    let errors: Vec<String> = validator.iter_errors(map).map(|e| e.to_string()).collect();
    assert!(errors.is_empty(), "map violates the schema: {errors:?}");
}

fn layer<'m>(map: &'m serde_json::Value, id: &str) -> &'m serde_json::Value {
    map["layers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["id"] == id)
        .unwrap_or_else(|| panic!("layer {id} missing from the map"))
}

fn flow<'m>(map: &'m serde_json::Value, id: &str) -> &'m serde_json::Value {
    map["domain_flows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["id"] == id)
        .unwrap_or_else(|| panic!("flow {id} missing from the map"))
}

fn tour_step<'m>(map: &'m serde_json::Value, node: &str) -> &'m serde_json::Value {
    map["tour"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["node"] == node)
        .unwrap_or_else(|| panic!("tour step for {node} missing from the map"))
}

/// Writes a canned-responses file (slot key → text; keys are prefixed by
/// slot kind: `summary:<node-id>`, `layer-name:<layer-id>`,
/// `flow-name:<flow-id>`, `tour-label:<node-id>`) OUTSIDE the scanned repo
/// and returns the provider spec selecting it.
fn canned_provider(dir: &Path, answers: &[(&str, &str)]) -> String {
    let map: serde_json::Map<String, serde_json::Value> = answers
        .iter()
        .map(|(id, summary)| (id.to_string(), serde_json::Value::from(*summary)))
        .collect();
    let path = dir.join("canned.json");
    fs::write(&path, serde_json::to_string_pretty(&map).unwrap()).unwrap();
    format!("fake:{}", path.display())
}

#[test]
fn fake_provider_fills_summary_slots_and_flips_provenance_to_llm() {
    let repo = materialize("simple");
    let canned = tempfile::tempdir().unwrap();
    let provider = canned_provider(
        canned.path(),
        &[
            (
                "summary:function:src/util.ts:greet",
                "Builds the greeting string shown to a caller.",
            ),
            (
                "summary:file:src/main.ts",
                "The entry point: wires the app together.",
            ),
        ],
    );

    scan(repo.path(), true, Some(&provider)).success();
    let map = read_map(repo.path());
    assert_schema_valid(&map);

    let greet = node(&map, "function:src/util.ts:greet");
    assert_eq!(
        greet["summary"],
        "Builds the greeting string shown to a caller."
    );
    assert_eq!(greet["provenance"], "llm");
    let main = node(&map, "file:src/main.ts");
    assert_eq!(main["summary"], "The entry point: wires the app together.");
    assert_eq!(main["provenance"], "llm");

    // A node the provider did not answer for keeps its mechanical summary —
    // the fallback is the structural scan's prose, never a hole.
    let util = node(&map, "file:src/util.ts");
    assert_eq!(util["provenance"], "structural");
    assert!(
        util["summary"].as_str().unwrap().contains("TypeScript"),
        "mechanical summary must survive: {}",
        util["summary"]
    );
}

#[test]
fn annotations_reattach_on_a_plain_rescan_without_any_provider() {
    let repo = materialize("simple");
    let canned = tempfile::tempdir().unwrap();
    let provider = canned_provider(
        canned.path(),
        &[(
            "summary:function:src/util.ts:greet",
            "Builds the greeting string shown to a caller.",
        )],
    );
    scan(repo.path(), true, Some(&provider)).success();

    // Annotations persist in .codeatlas/ (ADR-0005) — internal format, but
    // the store must exist for later runs to carry over.
    assert!(
        repo.path().join(".codeatlas/annotations.json").exists(),
        "annotation store missing from .codeatlas/"
    );

    // A plain rescan — no --enrich, no provider env var, so any provider
    // call would fail — re-attaches the annotation for free.
    scan(repo.path(), false, None).success();
    let map = read_map(repo.path());
    assert_schema_valid(&map);
    let greet = node(&map, "function:src/util.ts:greet");
    assert_eq!(
        greet["summary"],
        "Builds the greeting string shown to a caller."
    );
    assert_eq!(greet["provenance"], "llm");

    // Determinism holds with a store present: two plain rescans are
    // byte-identical.
    let first = fs::read(repo.path().join(".codeatlas/knowledge-graph.json")).unwrap();
    scan(repo.path(), false, None).success();
    let second = fs::read(repo.path().join(".codeatlas/knowledge-graph.json")).unwrap();
    assert_eq!(first, second);
}

#[test]
fn editing_a_file_expires_its_annotations_and_reenrichment_reselects_them() {
    let repo = materialize("simple");
    let canned = tempfile::tempdir().unwrap();
    let provider = canned_provider(
        canned.path(),
        &[
            (
                "summary:function:src/util.ts:greet",
                "Original greet prose.",
            ),
            ("summary:file:src/main.ts", "Original main prose."),
        ],
    );
    scan(repo.path(), true, Some(&provider)).success();

    // Edit util.ts: its annotation must expire; main.ts is untouched.
    let util = repo.path().join("src/util.ts");
    let mut source = fs::read_to_string(&util).unwrap();
    source.push_str("\n// edited\n");
    fs::write(&util, source).unwrap();

    // Plain rescan: the edited file's node reverts to the mechanical
    // summary and structural provenance; the untouched file carries over.
    scan(repo.path(), false, None).success();
    let map = read_map(repo.path());
    let greet = node(&map, "function:src/util.ts:greet");
    assert_eq!(greet["provenance"], "structural");
    assert!(
        greet["summary"].as_str().unwrap().starts_with("Function"),
        "mechanical summary must return: {}",
        greet["summary"]
    );
    let main = node(&map, "file:src/main.ts");
    assert_eq!(main["provenance"], "llm");
    assert_eq!(main["summary"], "Original main prose.");

    // Re-enrich with different canned answers: the expired node is
    // re-selected and refilled; the carried-over node is NOT re-selected,
    // so the provider's new answer for it must not land.
    let provider = canned_provider(
        canned.path(),
        &[
            ("summary:function:src/util.ts:greet", "Updated greet prose."),
            ("summary:file:src/main.ts", "MUST NOT APPLY"),
        ],
    );
    scan(repo.path(), true, Some(&provider)).success();
    let map = read_map(repo.path());
    assert_schema_valid(&map);
    let greet = node(&map, "function:src/util.ts:greet");
    assert_eq!(greet["summary"], "Updated greet prose.");
    assert_eq!(greet["provenance"], "llm");
    let main = node(&map, "file:src/main.ts");
    assert_eq!(
        main["summary"], "Original main prose.",
        "an already-enriched node was re-purchased"
    );
}

#[test]
fn fake_provider_names_layers_flows_and_tour_and_the_map_stays_valid() {
    let repo = materialize("simple");
    let canned = tempfile::tempdir().unwrap();
    let provider = canned_provider(
        canned.path(),
        &[
            ("layer-name:src", "Application core"),
            (
                "flow-name:flow:function:src/main.ts:main",
                "Greeting delivery",
            ),
            (
                "tour-label:file:src/main.ts",
                "Start here: main wires the app together.",
            ),
            ("summary:file:src/main.ts", "The entry point."),
        ],
    );

    scan(repo.path(), true, Some(&provider)).success();
    let map = read_map(repo.path());
    assert_schema_valid(&map);

    // Each answer landed in its own slot kind and flipped its provenance.
    let src = layer(&map, "src");
    assert_eq!(src["name"], "Application core");
    assert_eq!(src["provenance"], "llm");
    let main_flow = flow(&map, "flow:function:src/main.ts:main");
    assert_eq!(main_flow["name"], "Greeting delivery");
    assert_eq!(main_flow["provenance"], "llm");
    let main_stop = tour_step(&map, "file:src/main.ts");
    assert_eq!(
        main_stop["label"],
        "Start here: main wires the app together."
    );
    assert_eq!(main_stop["provenance"], "llm");
    assert_eq!(
        node(&map, "file:src/main.ts")["summary"],
        "The entry point."
    );

    // Unanswered semantic slots keep their mechanical labels — degrade,
    // never break.
    let root = layer(&map, "root");
    assert_eq!(root["name"], "root");
    assert_eq!(root["provenance"], "structural");
    let app_flow = flow(&map, "flow:function:src/app.ts:app");
    assert_eq!(app_flow["name"], "app → shout → helper → greet");
    assert_eq!(app_flow["provenance"], "structural");
    let util_stop = tour_step(&map, "file:src/util.ts");
    // Shape, not arithmetic. What this test is about is an unanswered slot
    // keeping its mechanical label; whether the cited degree is truthful is
    // checked against the emitted edges in tests/scan.rs. Pinning the number
    // here would make every new fixture file break an enrichment test.
    let util_label = util_stop["label"].as_str().unwrap();
    assert!(
        util_label.starts_with("src/util.ts — fan-in "),
        "expected the mechanical label, got {util_label}"
    );
    assert_eq!(util_stop["provenance"], "structural");
}

#[test]
fn semantic_annotations_reattach_on_plain_rescans_and_survive_content_edits() {
    let repo = materialize("simple");
    let canned = tempfile::tempdir().unwrap();
    let provider = canned_provider(
        canned.path(),
        &[
            ("layer-name:src", "Application core"),
            (
                "flow-name:flow:function:src/main.ts:main",
                "Greeting delivery",
            ),
            ("tour-label:file:src/main.ts", "Start here."),
            ("summary:function:src/util.ts:greet", "Builds the greeting."),
        ],
    );
    scan(repo.path(), true, Some(&provider)).success();

    // A plain rescan — no provider available — re-attaches the semantic
    // annotations for free (ADR-0005).
    scan(repo.path(), false, None).success();
    let map = read_map(repo.path());
    assert_schema_valid(&map);
    assert_eq!(layer(&map, "src")["name"], "Application core");
    assert_eq!(layer(&map, "src")["provenance"], "llm");
    assert_eq!(
        flow(&map, "flow:function:src/main.ts:main")["name"],
        "Greeting delivery"
    );
    assert_eq!(tour_step(&map, "file:src/main.ts")["label"], "Start here.");
    assert_eq!(tour_step(&map, "file:src/main.ts")["provenance"], "llm");

    // Determinism holds with semantic annotations in the store: two plain
    // rescans are byte-identical.
    let first = fs::read(repo.path().join(".codeatlas/knowledge-graph.json")).unwrap();
    scan(repo.path(), false, None).success();
    let second = fs::read(repo.path().join(".codeatlas/knowledge-graph.json")).unwrap();
    assert_eq!(first, second);

    // Editing a file's CONTENT expires its node annotations (hash change)
    // but no semantic derivation: the layer's member set, the flow's step
    // chain, and the tour topology are all unchanged, so their enriched
    // names carry over — nothing is re-purchased for a body edit.
    let util = repo.path().join("src/util.ts");
    let mut source = fs::read_to_string(&util).unwrap();
    source.push_str("\n// edited\n");
    fs::write(&util, source).unwrap();

    scan(repo.path(), false, None).success();
    let map = read_map(repo.path());
    assert_eq!(
        node(&map, "function:src/util.ts:greet")["provenance"],
        "structural",
        "the edited file's node annotation must expire"
    );
    assert_eq!(layer(&map, "src")["name"], "Application core");
    assert_eq!(layer(&map, "src")["provenance"], "llm");
    assert_eq!(
        flow(&map, "flow:function:src/main.ts:main")["name"],
        "Greeting delivery"
    );
    assert_eq!(
        flow(&map, "flow:function:src/main.ts:main")["provenance"],
        "llm"
    );
    assert_eq!(tour_step(&map, "file:src/main.ts")["label"], "Start here.");
    assert_eq!(tour_step(&map, "file:src/main.ts")["provenance"], "llm");

    // On the next --enrich the carried-over semantic slots are not
    // re-selected: a fresh answer for them must not land.
    let provider = canned_provider(
        canned.path(),
        &[
            ("layer-name:src", "MUST NOT APPLY"),
            ("flow-name:flow:function:src/main.ts:main", "MUST NOT APPLY"),
            ("tour-label:file:src/main.ts", "MUST NOT APPLY"),
            (
                "summary:function:src/util.ts:greet",
                "Rebuilt greeting prose.",
            ),
        ],
    );
    scan(repo.path(), true, Some(&provider)).success();
    let map = read_map(repo.path());
    assert_eq!(
        node(&map, "function:src/util.ts:greet")["summary"],
        "Rebuilt greeting prose."
    );
    assert_eq!(
        layer(&map, "src")["name"],
        "Application core",
        "a carried-over layer name was re-purchased"
    );
    assert_eq!(
        flow(&map, "flow:function:src/main.ts:main")["name"],
        "Greeting delivery",
        "a carried-over flow name was re-purchased"
    );
    assert_eq!(
        tour_step(&map, "file:src/main.ts")["label"],
        "Start here.",
        "a carried-over tour label was re-purchased"
    );
}

#[test]
fn changed_derivations_expire_semantic_names_and_reenrichment_reselects_them() {
    let repo = materialize("simple");
    let canned = tempfile::tempdir().unwrap();
    let provider = canned_provider(
        canned.path(),
        &[
            ("layer-name:src", "Application core"),
            (
                "flow-name:flow:function:src/main.ts:main",
                "Greeting delivery",
            ),
            ("tour-label:file:src/main.ts", "Start here."),
        ],
    );
    scan(repo.path(), true, Some(&provider)).success();

    // Change the derivations: a new file in src/ changes the layer's
    // member set, and rewiring main changes the flow's step chain and
    // main.ts's fan-out (the tour label's mechanical inputs).
    fs::write(
        repo.path().join("src/extra.ts"),
        "export function extra(): void {}\n",
    )
    .unwrap();
    fs::write(
        repo.path().join("src/main.ts"),
        r#"import { greet } from "./util";
import { shout } from "./arrow";

export function main(): void {
  console.log(greet("atlas"));
  console.log(shout("atlas"));
}
"#,
    )
    .unwrap();

    // Plain rescan: every changed derivation reverts to its mechanical
    // label and structural provenance — stale prose never describes a new
    // shape.
    scan(repo.path(), false, None).success();
    let map = read_map(repo.path());
    assert_schema_valid(&map);
    assert_eq!(
        layer(&map, "src")["name"],
        "src",
        "a layer whose member set changed must revert to its mechanical name"
    );
    assert_eq!(layer(&map, "src")["provenance"], "structural");
    let main_flow = flow(&map, "flow:function:src/main.ts:main");
    assert_eq!(
        main_flow["name"], "main → shout → greet",
        "a flow whose steps changed must revert to its mechanical name"
    );
    assert_eq!(main_flow["provenance"], "structural");
    let main_stop = tour_step(&map, "file:src/main.ts");
    assert_eq!(
        main_stop["label"], "Entry point: src/main.ts — fan-in 0, fan-out 2",
        "a tour step whose topology changed must revert to its mechanical label"
    );
    assert_eq!(main_stop["provenance"], "structural");

    // The next --enrich re-selects exactly the expired slots.
    let provider = canned_provider(
        canned.path(),
        &[
            ("layer-name:src", "Platform core"),
            ("flow-name:flow:function:src/main.ts:main", "Loud greeting"),
            ("tour-label:file:src/main.ts", "Fresh narration."),
        ],
    );
    scan(repo.path(), true, Some(&provider)).success();
    let map = read_map(repo.path());
    assert_schema_valid(&map);
    assert_eq!(layer(&map, "src")["name"], "Platform core");
    assert_eq!(layer(&map, "src")["provenance"], "llm");
    assert_eq!(
        flow(&map, "flow:function:src/main.ts:main")["name"],
        "Loud greeting"
    );
    assert_eq!(
        tour_step(&map, "file:src/main.ts")["label"],
        "Fresh narration."
    );
}

#[test]
fn provider_failure_mid_run_leaves_a_complete_schema_valid_structural_map() {
    let repo = materialize("simple");

    // Reference run: what a plain structural scan of this fixture yields.
    scan(repo.path(), false, None).success();
    let structural = read_map(repo.path());

    // Failing provider: the enrichment step errors, the exit code says so,
    // and the map on disk is the complete structural map — never corrupted,
    // never truncated (spec story 14).
    let fresh = materialize("simple");
    let assert = scan(fresh.path(), true, Some("fail")).failure();
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    assert!(
        stderr.contains("injected provider failure"),
        "failure must surface the provider error: {stderr}"
    );

    let map = read_map(fresh.path());
    assert_schema_valid(&map);
    assert_eq!(
        map["nodes"].as_array().unwrap().len(),
        structural["nodes"].as_array().unwrap().len(),
        "map must be complete despite the failure"
    );
    assert_eq!(
        map["edges"].as_array().unwrap().len(),
        structural["edges"].as_array().unwrap().len()
    );
    for n in map["nodes"].as_array().unwrap() {
        assert_eq!(n["provenance"], "structural");
    }

    // A failed run purchased nothing, so it must not fabricate a store
    // claiming otherwise; the next --enrich selects everything again.
    let canned = tempfile::tempdir().unwrap();
    let provider = canned_provider(
        canned.path(),
        &[(
            "summary:function:src/util.ts:greet",
            "Prose after recovery.",
        )],
    );
    scan(fresh.path(), true, Some(&provider)).success();
    let map = read_map(fresh.path());
    assert_eq!(
        node(&map, "function:src/util.ts:greet")["summary"],
        "Prose after recovery."
    );
}

#[test]
fn an_old_version_annotation_store_degrades_to_no_carry_over() {
    // The store format is internal and versioned; a store written by an
    // older binary (version 1, before the semantic sections) is ignored
    // wholesale — the scan never breaks, it merely re-purchases.
    let repo = materialize("simple");
    fs::create_dir_all(repo.path().join(".codeatlas")).unwrap();
    fs::write(
        repo.path().join(".codeatlas/annotations.json"),
        r#"{
  "version": 1,
  "annotations": {
    "function:src/util.ts:greet": {
      "content_hash": "fnv1a64:0000000000000000",
      "summary": "Stale v1 prose."
    }
  }
}
"#,
    )
    .unwrap();

    scan(repo.path(), false, None).success();
    let map = read_map(repo.path());
    assert_schema_valid(&map);
    for n in map["nodes"].as_array().unwrap() {
        assert_eq!(n["provenance"], "structural", "v1 store must not attach");
    }
    for l in map["layers"].as_array().unwrap() {
        assert_eq!(l["provenance"], "structural");
    }
}

#[test]
fn enrich_with_nothing_to_enrich_succeeds_without_any_provider() {
    // An empty repo yields zero summary slots, so --enrich has nothing to
    // purchase: it must succeed without resolving a provider at all (no
    // env var is set here, and in a network build the default provider
    // would demand credentials).
    let repo = tempfile::tempdir().unwrap();

    let assert = scan(repo.path(), true, None).success();
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    assert!(
        stderr.contains("nothing to enrich"),
        "must explain that no slots needed enrichment: {stderr}"
    );
}

/// Sealed builds only (ticket 15, ADR-0006): with the `network` feature off
/// the Claude provider does not merely refuse to run — it does not exist to
/// be selected. Asking for it by name fails cleanly, and the structural map
/// still ships. (The genuinely sealed *binary* — no dev-deps, so also no
/// test-provider — reports its own "compiled without the `network` feature"
/// message; CI's `scripts/sealed-probe.sh` asserts that one, since every
/// `cargo test` build carries test-provider via the self dev-dependency.)
#[cfg(not(feature = "network"))]
#[test]
fn the_claude_provider_does_not_exist_in_sealed_builds() {
    let repo = materialize("simple");
    let assert = scan(repo.path(), true, Some("claude")).failure();
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    assert!(
        stderr.contains("unknown enrichment provider"),
        "sealed build must not know a provider named claude: {stderr}"
    );

    // Spec story 14 holds even here: the structural map survives.
    let map = read_map(repo.path());
    assert_schema_valid(&map);
    assert!(!map["nodes"].as_array().unwrap().is_empty());
}

#[test]
fn enrich_without_a_provider_fails_cleanly_but_writes_the_structural_map() {
    let repo = materialize("simple");
    let assert = scan(repo.path(), true, None).failure();
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    assert!(
        stderr.contains("provider"),
        "failure must explain that no provider is available: {stderr}"
    );

    // Spec story 14: the structural map survives, complete and valid.
    let map = read_map(repo.path());
    assert_schema_valid(&map);
    assert!(!map["nodes"].as_array().unwrap().is_empty());
    node(&map, "file:src/main.ts");
    for n in map["nodes"].as_array().unwrap() {
        assert_eq!(n["provenance"], "structural", "no enrichment ran: {n:?}");
    }
}

// ── Provider selection (ticket 29) ───────────────────────────────────────
//
// Until this ticket the only way to choose a backend was an environment
// variable, which nobody finds — so in practice there was one provider and no
// way to learn otherwise. These tests are about the selection surface itself,
// not about what any backend does with the slots it is given.

#[test]
fn the_provider_flag_selects_a_backend() {
    let repo = materialize("simple");
    let outside = tempfile::tempdir().unwrap();
    let canned = canned_provider(
        outside.path(),
        &[("summary:file:src/main.ts", "The entry point.")],
    );

    scan_selecting(repo.path(), None, Some(&canned)).success();

    let map = read_map(repo.path());
    assert_eq!(
        node(&map, "file:src/main.ts")["summary"],
        "The entry point."
    );
    assert_eq!(node(&map, "file:src/main.ts")["provenance"], "llm");
}

#[test]
fn the_flag_beats_the_environment_variable() {
    let repo = materialize("simple");
    let outside = tempfile::tempdir().unwrap();
    let canned = canned_provider(
        outside.path(),
        &[("summary:file:src/main.ts", "Chosen by flag.")],
    );

    // The env var names the provider that errors on every call; the flag
    // names one that answers. If the variable won, this run would fail.
    scan_selecting(repo.path(), Some("fail"), Some(&canned)).success();

    let map = read_map(repo.path());
    assert_eq!(node(&map, "file:src/main.ts")["summary"], "Chosen by flag.");
}

#[test]
fn the_flag_beats_the_environment_variable_in_the_other_direction_too() {
    let repo = materialize("simple");
    let outside = tempfile::tempdir().unwrap();
    let canned = canned_provider(
        outside.path(),
        &[("summary:file:src/main.ts", "Never used.")],
    );

    // The control for the test above. Without this, "the flag wins" would
    // also pass if the flag were the *only* surface read and the variable
    // silently ignored — which is a different, worse behaviour.
    scan_selecting(repo.path(), Some(&canned), Some("fail")).failure();

    let map = read_map(repo.path());
    assert_eq!(node(&map, "file:src/main.ts")["provenance"], "structural");
}

#[test]
fn the_environment_variable_still_selects_when_no_flag_is_given() {
    let repo = materialize("simple");
    let outside = tempfile::tempdir().unwrap();
    let canned = canned_provider(
        outside.path(),
        &[("summary:file:src/main.ts", "Chosen by env.")],
    );

    scan_selecting(repo.path(), Some(&canned), None).success();

    let map = read_map(repo.path());
    assert_eq!(node(&map, "file:src/main.ts")["summary"], "Chosen by env.");
}

#[test]
fn an_unrecognised_provider_names_the_ones_that_exist() {
    let repo = materialize("simple");
    let assert = scan_selecting(repo.path(), None, Some("nope")).failure();
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();

    assert!(
        stderr.contains("unknown enrichment provider"),
        "must say the spec was not recognised: {stderr}"
    );
    // Being told a name is wrong without being told any right one is the
    // failure this ticket exists to stop repeating.
    assert!(
        stderr.contains("fake:"),
        "must list what this build does recognise: {stderr}"
    );

    // Spec story 14: the structural map survives a selection failure. The
    // node lookup is load-bearing — a loop over an empty array asserts
    // nothing, and "the map is intact" is exactly the claim that must not be
    // vacuous here.
    let map = read_map(repo.path());
    assert_schema_valid(&map);
    node(&map, "file:src/main.ts");
    for n in map["nodes"].as_array().unwrap() {
        assert_eq!(n["provenance"], "structural", "no enrichment ran: {n:?}");
    }
}

#[test]
fn the_help_names_the_providers_this_build_recognises() {
    let assert = assert_cmd::Command::cargo_bin("codeatlas")
        .unwrap()
        .args(["scan", "--help"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();

    assert!(
        stdout.contains("--provider"),
        "help must offer it: {stdout}"
    );
    // `cli:claude` is the discriminating spec here: it is not a substring of
    // any other, so it can be asked about directly. (`claude` cannot — it is
    // a substring of `cli:claude`, which is how ticket 29's version of this
    // assertion broke the moment ADR-0008's backend arrived. The exact
    // list-versus-build check now lives in a unit test that parses the
    // rendered sentence, where clap's line wrapping cannot interfere.)
    assert_eq!(
        stdout.contains("cli:claude"),
        cfg!(feature = "agent-cli"),
        "help offers the CLI backend exactly when it exists: {stdout}"
    );
    assert!(
        stdout.contains("fake:"),
        "help must name the specs this build accepts: {stdout}"
    );
}

#[test]
fn choosing_a_provider_without_asking_for_enrichment_is_refused() {
    let assert = assert_cmd::Command::cargo_bin("codeatlas")
        .unwrap()
        .args(["scan", "--provider", "fail"])
        .assert()
        .failure();
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();

    assert!(
        stderr.contains("--enrich"),
        "must point at the flag it requires: {stderr}"
    );
}

/// Builds with no enrichment backend at all. The flag must still exist and
/// explain itself — a selection surface that vanishes in one build
/// configuration teaches the reader nothing about why.
///
/// Gated on *both* backends being absent, not on `network` alone: in an
/// `agent-cli`-without-`network` build `--model` is honoured by the CLI
/// backend, so asserting it has nothing to modify would be asserting a
/// falsehood. (As with the sibling sealed test above, a
/// `cargo test` build carries `test-provider`, so `fake:` is selectable here
/// and the genuinely sealed binary is CI's subject, not this one.)
///
/// What the *recognised list* contains is asserted as a unit test on
/// `recognised_specs`, not here: clap wraps help text at the terminal width,
/// so an assertion on the rendered list tests the wrapping as much as the
/// content.
#[cfg(not(any(feature = "network", feature = "agent-cli")))]
#[test]
fn the_provider_flag_exists_with_no_backend_compiled_in_and_refuses_claude() {
    let help = assert_cmd::Command::cargo_bin("codeatlas")
        .unwrap()
        .args(["scan", "--help"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&help.get_output().stdout).into_owned();
    assert!(
        stdout.contains("--provider"),
        "the flag must not silently disappear in a sealed build: {stdout}"
    );
    // `--model` must admit it has nothing to modify rather than describing a
    // provider that is not there. Asserted on the flag's own paragraph, not
    // on the whole page: `/crosscheck` found that a page-wide search for
    // "sealed build" was satisfied by `--model` alone, so `--provider`'s
    // explanation could be deleted entirely and this test would still pass.
    let model_paragraph = stdout
        .split("--model")
        .nth(1)
        .expect("--model must be in the help");
    assert!(
        model_paragraph.contains("compiled none in"),
        "--model must say it has nothing to modify: {model_paragraph}"
    );

    let repo = materialize("simple");
    scan_selecting(repo.path(), None, Some("claude")).failure();
}

// ── The CLI backend (ticket 31, seam 3) ──────────────────────────────────
//
// Seam 3 is the spawned program's process interface. The unit tests in
// `src/enrich/agent_cli.rs` cover argv construction and output parsing as
// pure functions; what only a real spawn can show is that the environment
// was actually cleared, the working directory actually changed, and the whole
// path from `--provider` to a filled slot actually joins up. That is what
// these do, against a stand-in executable — never the real `claude`.

/// Runs `scan --enrich --provider <spec>` with a credential and an unrelated
/// secret in the parent environment, so the child's environment can be
/// checked for both.
#[cfg(feature = "agent-cli")]
fn scan_with_secrets(repo: &Path, spec: &str) -> assert_cmd::assert::Assert {
    assert_cmd::Command::cargo_bin("codeatlas")
        .unwrap()
        .args(["scan", "--enrich", "--provider", spec])
        .current_dir(repo)
        .env_remove(PROVIDER_ENV)
        .env("ANTHROPIC_API_KEY", "sk-ant-must-not-reach-the-child")
        .env("CODEATLAS_TEST_SECRET", "must-not-reach-the-child")
        .assert()
}

#[cfg(feature = "agent-cli")]
#[test]
fn the_cli_backend_fills_slots_through_a_spawned_program() {
    let repo = materialize("simple");
    let outside = tempfile::tempdir().unwrap();
    let spec = common::fake_cli(
        outside.path(),
        r#"{"type":"result","subtype":"success","is_error":false,
            "structured_output":{"answers":[
              {"key":"summary:file:src/main.ts","text":"The entry point."}
            ]}}"#,
        0,
    );

    scan_with_secrets(repo.path(), &spec).success();

    let map = read_map(repo.path());
    assert_schema_valid(&map);
    let main = node(&map, "file:src/main.ts");
    assert_eq!(main["summary"], "The entry point.");
    assert_eq!(main["provenance"], "llm");
}

#[cfg(feature = "agent-cli")]
#[test]
fn the_child_gets_no_credential_no_unrelated_variable_and_no_repository() {
    let repo = materialize("simple");
    let outside = tempfile::tempdir().unwrap();
    let spec = common::fake_cli(
        outside.path(),
        r#"{"type":"result","subtype":"success","is_error":false,
            "structured_output":{"answers":[]}}"#,
        0,
    );

    scan_with_secrets(repo.path(), &spec).success();
    let lines = common::record_lines(outside.path());
    let field = |name: &str| {
        lines
            .iter()
            .find_map(|l| l.strip_prefix(name))
            .unwrap_or_else(|| panic!("{name} missing from {lines:?}"))
            .to_string()
    };

    // `cli:` must mean the CLI's own credential. A key that leaked through
    // would silently bill the API instead — working, and wrong.
    assert_eq!(field("api-key="), "<unset>");
    // The allowlist is a list, not a filter on names that look secret.
    assert_eq!(field("secret="), "<unset>");
    // ...and it is not so aggressive that the CLI cannot find its own
    // credentials, which live under HOME.
    assert_ne!(field("home="), "<unset>");

    // The structural guarantee behind "the model never receives file
    // contents": the child runs somewhere empty, and is never pointed at the
    // repository it is describing.
    let cwd = field("cwd=");
    let repo_path = repo.path().canonicalize().unwrap();
    assert!(
        !Path::new(&cwd).starts_with(&repo_path),
        "the child ran inside the scanned repository: {cwd}"
    );
    assert!(
        !lines
            .iter()
            .any(|l| l.contains(repo_path.to_str().unwrap())),
        "the repository path reached the child: {lines:?}"
    );
}

#[cfg(feature = "agent-cli")]
#[test]
fn the_child_is_invoked_as_a_locked_down_one_shot_completion() {
    let repo = materialize("simple");
    let outside = tempfile::tempdir().unwrap();
    let spec = common::fake_cli(
        outside.path(),
        r#"{"type":"result","subtype":"success","is_error":false,
            "structured_output":{"answers":[]}}"#,
        0,
    );

    scan_with_secrets(repo.path(), &spec).success();
    let args: Vec<String> = common::record_lines(outside.path())
        .iter()
        .filter_map(|l| l.strip_prefix("arg=").map(str::to_string))
        .collect();

    for flag in ["--print", "--safe-mode", "--strict-mcp-config", "--tools="] {
        assert!(args.iter().any(|a| a == flag), "{flag} missing: {args:?}");
    }
    // The prompt reached the child as its own argument rather than being
    // absorbed by a preceding variadic flag. A shell script has no argument
    // parser, so this checks the shape the real parser would act on.
    let fence = args.iter().position(|a| a == "--").expect("a -- fence");
    assert_eq!(
        fence,
        args.len() - 2,
        "exactly one argument follows the fence — the prompt: {args:?}"
    );
    assert!(
        args[fence + 1].starts_with("Project:"),
        "the prompt must be what follows the fence: {args:?}"
    );
    assert!(
        !args.iter().any(|a| a == "--add-dir"),
        "the child's file scope must never be widened: {args:?}"
    );
    assert!(
        !args.iter().any(|a| a == "--bare"),
        "--bare would demand the API key this backend exists to avoid: {args:?}"
    );
}

/// Story 14 at seam 3: every way a spawned program can disappoint leaves a
/// complete, schema-valid structural map behind.
#[cfg(feature = "agent-cli")]
#[test]
fn every_way_the_child_can_fail_leaves_the_structural_map_intact() {
    let outside = tempfile::tempdir().unwrap();
    let cases = [
        (
            "not installed",
            "cli-exec:/nonexistent/definitely-not-a-program".to_string(),
        ),
        (
            "a non-zero exit",
            common::fake_cli(&outside.path().join("exit"), "", 1),
        ),
        (
            "output that is not JSON",
            common::fake_cli(&outside.path().join("garbage"), "not json at all", 0),
        ),
        (
            "an error envelope",
            common::fake_cli(
                &outside.path().join("errored"),
                r#"{"type":"result","subtype":"error_during_execution","is_error":true,
                    "result":"the session failed"}"#,
                0,
            ),
        ),
    ];

    for (what, spec) in cases {
        let repo = materialize("simple");
        scan_with_secrets(repo.path(), &spec).failure();

        let map = read_map(repo.path());
        assert_schema_valid(&map);
        node(&map, "file:src/main.ts");
        for n in map["nodes"].as_array().unwrap() {
            assert_eq!(
                n["provenance"], "structural",
                "{what}: no enrichment should have landed: {n:?}"
            );
        }
    }
}

#[cfg(feature = "agent-cli")]
#[test]
fn the_cli_backend_runs_claude_and_refuses_to_run_anything_else() {
    let repo = materialize("simple");
    let assert = scan_selecting(repo.path(), None, Some("cli:sh")).failure();
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();

    assert!(
        stderr.contains("cli:claude"),
        "must name the one CLI backend there is: {stderr}"
    );
    assert!(
        stderr.contains("arbitrary program"),
        "must say why, not merely that it is unknown: {stderr}"
    );

    // Spec story 14 again: refusing to run a program is still a clean
    // degradation.
    let map = read_map(repo.path());
    assert_schema_valid(&map);
    node(&map, "file:src/main.ts");
}
