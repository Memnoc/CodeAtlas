//! Tests of enrichment behaviour at the two agreed seams: the map contract
//! (run the binary, assert on the emitted map) and the enrichment provider
//! trait (a fake provider returns canned typed responses — see
//! `src/enrich.rs` unit tests for the in-process side). No test here ever
//! performs network I/O: the binary under test selects its provider through
//! the `CODEATLAS_ENRICH_PROVIDER` env var, whose fake/fail backends are
//! compiled in only for test builds (the `test-provider` feature).

use std::fs;
use std::path::{Path, PathBuf};

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

/// Writes a canned-responses file (node ID → summary) OUTSIDE the scanned
/// repo and returns the provider spec selecting it.
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
                "function:src/util.ts:greet",
                "Builds the greeting string shown to a caller.",
            ),
            (
                "file:src/main.ts",
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
            "function:src/util.ts:greet",
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
            ("function:src/util.ts:greet", "Original greet prose."),
            ("file:src/main.ts", "Original main prose."),
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
            ("function:src/util.ts:greet", "Updated greet prose."),
            ("file:src/main.ts", "MUST NOT APPLY"),
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
        &[("function:src/util.ts:greet", "Prose after recovery.")],
    );
    scan(fresh.path(), true, Some(&provider)).success();
    let map = read_map(fresh.path());
    assert_eq!(
        node(&map, "function:src/util.ts:greet")["summary"],
        "Prose after recovery."
    );
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
