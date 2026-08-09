//! Tests of the share artifact (ticket 14, ADR-0006, spec stories 8 and 10)
//! at the artifact seam: a map file goes in, one self-contained redacted
//! HTML file comes out. Plus the audit lynchpin: the schema-derived
//! exhaustiveness test that forbids any contract field from shipping
//! unclassified.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::Command;

use codeatlas::map::MAP_CONTRACT_VERSION;
use codeatlas::share::{FIELD_CLASSIFICATIONS, REDACTION_MARKER, redact};
use serde_json::{Value, json};

// ---------------------------------------------------------------------------
// Schema-derived exhaustiveness (ADR-0006): every property path in the map
// contract must be classified share-safe or redacted. This walks the schema
// itself — $defs, properties, items, anyOf/oneOf/allOf, inline nesting —
// so a new field cannot ship until someone adds it to the table.
// ---------------------------------------------------------------------------

/// Collects `Type.property` paths from one schema object, descending into
/// inline compositions so a nested inline object yields `Type.outer.inner`.
fn collect_paths(type_name: &str, schema: &Value, out: &mut BTreeSet<String>) {
    let Some(props) = schema.get("properties").and_then(Value::as_object) else {
        return;
    };
    for (prop, sub) in props {
        let path = format!("{type_name}.{prop}");
        collect_nested(&path, sub, out);
        out.insert(path);
    }
}

/// Descends through the schema keywords that can hide inline `properties`.
/// `$ref` targets are deliberately not followed: every `$defs` entry is
/// walked under its own type name, so refs are covered exactly once.
fn collect_nested(path: &str, schema: &Value, out: &mut BTreeSet<String>) {
    if let Some(props) = schema.get("properties").and_then(Value::as_object) {
        for (prop, sub) in props {
            let nested = format!("{path}.{prop}");
            collect_nested(&nested, sub, out);
            out.insert(nested);
        }
    }
    if let Some(items) = schema.get("items") {
        collect_nested(path, items, out);
    }
    for keyword in ["anyOf", "oneOf", "allOf"] {
        if let Some(variants) = schema.get(keyword).and_then(Value::as_array) {
            for variant in variants {
                collect_nested(path, variant, out);
            }
        }
    }
}

fn schema_property_paths() -> BTreeSet<String> {
    let schema = codeatlas::map::contract_schema();
    let mut paths = BTreeSet::new();
    let root = schema["title"].as_str().expect("schema has a title");
    collect_paths(root, &schema, &mut paths);
    for (name, def) in schema["$defs"].as_object().expect("schema has $defs") {
        collect_paths(name, def, &mut paths);
    }
    paths
}

#[test]
fn every_schema_field_is_classified_and_no_classification_is_stale() {
    let schema_paths = schema_property_paths();
    let table_paths: BTreeSet<String> = FIELD_CLASSIFICATIONS
        .iter()
        .map(|(path, _)| (*path).to_string())
        .collect();

    let unclassified: Vec<_> = schema_paths.difference(&table_paths).collect();
    assert!(
        unclassified.is_empty(),
        "contract fields missing from the share classification table \
         (classify each as share-safe or redacted in src/share.rs): \
         {unclassified:?}"
    );

    let stale: Vec<_> = table_paths.difference(&schema_paths).collect();
    assert!(
        stale.is_empty(),
        "share classification table entries no longer in the contract \
         schema: {stale:?}"
    );
}

#[test]
fn the_walker_itself_sees_the_contract() {
    // A walker that returns nothing would make the exhaustiveness test pass
    // vacuously; pin a few known paths so the walk is proven real.
    let paths = schema_property_paths();
    for expected in [
        "KnowledgeGraph.version",
        "Node.summary",
        "Range.start_line",
        "TourStep.label",
    ] {
        assert!(paths.contains(expected), "walker missed {expected}");
    }
}

// ---------------------------------------------------------------------------
// Redaction behavior through the public redact() seam.
// ---------------------------------------------------------------------------

fn fixture_map() -> Value {
    json!({
        "version": MAP_CONTRACT_VERSION,
        "project": { "name": "share-fixture" },
        "nodes": [
            {
                "id": "file:src/main.rs",
                "kind": "file",
                "name": "main.rs",
                "path": "src/main.rs",
                "summary": "Rust file, 10 lines: 1 function",
                "layer": "src",
                "provenance": "structural"
            },
            {
                "id": "function:src/main.rs:main",
                "kind": "function",
                "name": "main",
                "path": "src/main.rs",
                "summary": "SECRET-ENRICHED-SUMMARY paraphrasing proprietary logic",
                "range": { "start_line": 1, "end_line": 9 },
                "provenance": "llm"
            }
        ],
        "edges": [
            {
                "source": "file:src/main.rs",
                "target": "function:src/main.rs:main",
                "kind": "contains",
                "weight": 1.0
            }
        ],
        "layers": [
            { "id": "src", "name": "SECRET-LAYER-NAME", "provenance": "llm" },
            { "id": "docs", "name": "docs", "provenance": "structural" }
        ],
        "domain_flows": [
            {
                "id": "flow:function:src/main.rs:main",
                "name": "SECRET-FLOW-NAME",
                "domain": "src",
                "steps": ["function:src/main.rs:main"],
                "provenance": "llm"
            }
        ],
        "tour": [
            {
                "node": "file:src/main.rs",
                "label": "SECRET-TOUR-LABEL",
                "provenance": "llm"
            },
            {
                "node": "file:src/main.rs",
                "label": "Stop 2: src/main.rs (mechanical)",
                "provenance": "structural"
            }
        ]
    })
}

#[test]
fn redaction_replaces_llm_prose_and_keeps_mechanical_prose() {
    let redaction = redact(&fixture_map());
    let map = &redaction.map;

    // Every LLM-provenance prose slot carries the marker…
    assert_eq!(map["nodes"][1]["summary"], REDACTION_MARKER);
    assert_eq!(map["layers"][0]["name"], REDACTION_MARKER);
    assert_eq!(map["domain_flows"][0]["name"], REDACTION_MARKER);
    assert_eq!(map["tour"][0]["label"], REDACTION_MARKER);

    // …while mechanical prose and structure pass through untouched.
    assert_eq!(
        map["nodes"][0]["summary"],
        "Rust file, 10 lines: 1 function"
    );
    assert_eq!(map["layers"][1]["name"], "docs");
    assert_eq!(map["tour"][1]["label"], "Stop 2: src/main.rs (mechanical)");
    assert_eq!(map["project"]["name"], "share-fixture");
    assert_eq!(map["nodes"][1]["range"]["start_line"], 1);

    // The disclosure counts exactly what happened, per field.
    let counts: Vec<(String, u64)> = redaction.redacted.clone();
    assert_eq!(
        counts,
        vec![
            ("DomainFlow.name".to_string(), 1),
            ("Layer.name".to_string(), 1),
            ("Node.summary".to_string(), 1),
            ("TourStep.label".to_string(), 1),
        ]
    );
}

#[test]
fn redaction_denies_by_default() {
    // A field the classification table has never heard of is dropped, not
    // shipped — the allowlist posture (ADR-0006).
    let mut map = fixture_map();
    map["nodes"][0]
        .as_object_mut()
        .unwrap()
        .insert("secret_extra".into(), json!("smuggled prose"));
    // Prose slots with unreadable provenance fail closed.
    map["layers"][1]
        .as_object_mut()
        .unwrap()
        .remove("provenance");

    let redaction = redact(&map);
    assert!(redaction.map["nodes"][0].get("secret_extra").is_none());
    assert_eq!(redaction.map["layers"][1]["name"], REDACTION_MARKER);
    assert!(
        redaction
            .redacted
            .iter()
            .any(|(field, count)| field == "Node.secret_extra" && *count == 1)
    );
}

#[test]
fn redacted_map_still_validates_against_the_contract() {
    let committed: Value = serde_json::from_str(
        &fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contract/map.schema.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let validator = jsonschema::validator_for(&committed).unwrap();

    let redaction = redact(&fixture_map());
    let errors: Vec<String> = validator
        .iter_errors(&redaction.map)
        .map(|e| e.to_string())
        .collect();
    assert!(
        errors.is_empty(),
        "redacted map no longer validates against the contract: {errors:?}"
    );
}

// ---------------------------------------------------------------------------
// The artifact seam: `codeatlas share` emits one self-contained HTML file.
// ---------------------------------------------------------------------------

fn run_share(root: &Path) -> Vec<u8> {
    let output = Command::new(env!("CARGO_BIN_EXE_codeatlas"))
        .args(["share", root.to_str().unwrap()])
        .output()
        .expect("codeatlas share runs");
    assert!(
        output.status.success(),
        "share failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::read(root.join(".codeatlas/share.html")).expect("share.html written")
}

fn share_fixture_root() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let atlas = dir.path().join(".codeatlas");
    fs::create_dir_all(&atlas).unwrap();
    fs::write(
        atlas.join("knowledge-graph.json"),
        serde_json::to_string_pretty(&fixture_map()).unwrap(),
    )
    .unwrap();
    dir
}

/// Extracts the embedded share payload from the artifact bytes.
fn embedded_payload(html: &str) -> Value {
    let open = html
        .find(r#"<script id="codeatlas-share-data" type="application/json">"#)
        .expect("artifact embeds the share data script");
    let start = open + html[open..].find('>').unwrap() + 1;
    let end = start + html[start..].find("</script>").unwrap();
    serde_json::from_str(&html[start..end]).expect("embedded payload is valid JSON")
}

#[test]
fn share_emits_one_self_contained_redacted_artifact() {
    let root = share_fixture_root();
    let bytes = run_share(root.path());
    let html = String::from_utf8(bytes).unwrap();

    // Redacted values are absent from the artifact's bytes.
    for secret in [
        "SECRET-ENRICHED-SUMMARY",
        "SECRET-LAYER-NAME",
        "SECRET-FLOW-NAME",
        "SECRET-TOUR-LABEL",
    ] {
        assert!(!html.contains(secret), "artifact leaks {secret}");
    }

    // Share-safe values are present.
    for safe in [
        "share-fixture",
        "src/main.rs",
        "Rust file, 10 lines: 1 function",
        "Stop 2: src/main.rs (mechanical)",
        REDACTION_MARKER,
    ] {
        assert!(
            !safe.is_empty() && html.contains(safe),
            "artifact lost {safe}"
        );
    }

    // Self-contained: nothing references the served asset paths, so the
    // renderer and styles must be inlined.
    assert!(!html.contains("src=\"/assets"), "script not inlined");
    assert!(!html.contains("href=\"/assets"), "stylesheet not inlined");

    // The disclosure ships inside the artifact, field names + counts.
    let payload = embedded_payload(&html);
    assert_eq!(
        payload["redaction"]["redacted"],
        json!([
            { "count": 1, "field": "DomainFlow.name" },
            { "count": 1, "field": "Layer.name" },
            { "count": 1, "field": "Node.summary" },
            { "count": 1, "field": "TourStep.label" },
        ])
    );
    assert_eq!(payload["redaction"]["marker"], REDACTION_MARKER);

    // The embedded map itself still conforms to the published contract.
    let committed: Value = serde_json::from_str(
        &fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contract/map.schema.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let validator = jsonschema::validator_for(&committed).unwrap();
    assert!(
        validator.is_valid(&payload["map"]),
        "embedded map violates the contract"
    );
}

#[test]
fn share_artifact_references_no_external_host() {
    let root = share_fixture_root();
    let html = String::from_utf8(run_share(root.path())).unwrap();

    // Byte-scan for anything fetchable, mirroring the dashboard's
    // zero-egress test: http(s), websockets, and protocol-relative
    // references in markup contexts.
    let mut offenders = Vec::new();
    for scheme in ["http://", "https://"] {
        for (pos, _) in html.match_indices(scheme) {
            let url: String = html[pos..]
                .chars()
                .take_while(|c| !c.is_whitespace() && !"\"'`\\)<>".contains(*c))
                .collect();
            if !is_inert(&url) {
                offenders.push(url);
            }
        }
    }
    assert_eq!(offenders, Vec::<String>::new());

    assert!(!html.contains("ws://"), "artifact references a websocket");
    assert!(!html.contains("wss://"), "artifact references a websocket");
    assert!(
        !html.contains("src=\"//") && !html.contains("href=\"//"),
        "artifact contains a protocol-relative reference"
    );
}

/// URLs that are string literals by construction, never requests — the same
/// allowlist the dashboard's zero-egress test documents: XML namespace
/// identifiers, React's minified-error text, and React Flow's doc links
/// including its attribution `<a href>` (kept deliberately: a plain anchor
/// performs no request until the reader chooses to click it).
fn is_inert(url: &str) -> bool {
    url.starts_with("http://www.w3.org/")
        || url.starts_with("https://www.w3.org/")
        || url.starts_with("https://react.dev/errors/")
        || url.starts_with("https://reactflow.dev")
        || (url.starts_with("https://${") && url.contains("flow.dev"))
}

#[test]
fn share_survives_map_content_that_mentions_asset_like_paths() {
    // A scanned repo can legitimately contain nodes whose paths look like
    // the artifact's own moving parts (an index.html, an assets dir) —
    // found the hard way by sharing CodeAtlas's own map. Inlining must key
    // off the document's real tags, not off byte search over content.
    let root = share_fixture_root();
    let map_path = root.path().join(".codeatlas/knowledge-graph.json");
    let mut map: Value = serde_json::from_str(&fs::read_to_string(&map_path).unwrap()).unwrap();
    map["nodes"][0]["path"] = json!("dashboard/index.html");
    map["nodes"][0]["id"] = json!("file:dashboard/index.html");
    map["nodes"][0]["name"] = json!("index.html");
    map["nodes"][0]["summary"] = json!("mentions /index.html and /assets/index-abc.js");
    fs::write(&map_path, serde_json::to_string(&map).unwrap()).unwrap();

    let html = String::from_utf8(run_share(root.path())).unwrap();
    assert!(html.contains("dashboard/index.html"));
    assert!(!html.contains("src=\"/assets"), "script not inlined");
}

#[test]
fn share_artifact_is_deterministic() {
    let root = share_fixture_root();
    let first = run_share(root.path());
    let second = run_share(root.path());
    assert_eq!(first, second, "share.html differs across identical runs");
}

#[test]
fn share_without_a_map_fails_with_guidance() {
    let dir = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_codeatlas"))
        .args(["share", dir.path().to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("codeatlas scan"),
        "error should tell the user to scan first"
    );
}
