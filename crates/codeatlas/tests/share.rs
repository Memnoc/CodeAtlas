//! Tests of the share artifact (ticket 14, ADR-0006, spec stories 8 and 10)
//! at the artifact seam: a map file goes in, one self-contained redacted
//! HTML file comes out. Plus the audit lynchpin: the schema-derived
//! exhaustiveness test that forbids any contract field from shipping
//! unclassified.

mod common;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use codeatlas::map::MAP_CONTRACT_VERSION;
use codeatlas::share::{FIELD_CLASSIFICATIONS, REDACTION_MARKER, SHARE_CEILING_BYTES, redact};
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

/// Ticket 06: a layer's description carries provenance of its own, and the
/// walker honors it per part — a purchased description on a mechanically
/// named layer is redacted, and a mechanical description on an enriched-name
/// layer ships. One provenance covering both halves of the card would either
/// leak the purchase or redact plain structure.
#[test]
fn layer_descriptions_are_redacted_by_their_own_provenance() {
    let mut map = fixture_map();
    // layers[0]: enriched name, mechanical description.
    map["layers"][0]["description"] = json!({
        "text": "Files under src/",
        "provenance": "structural"
    });
    // layers[1]: mechanical name, purchased description.
    map["layers"][1]["description"] = json!({
        "text": "SECRET-ENRICHED-DESCRIPTION paraphrasing the region",
        "provenance": "llm"
    });

    let redaction = redact(&map);
    // The mechanical sentence ships even though the layer's *name* is
    // enriched: the name's provenance is not the description's.
    assert_eq!(
        redaction.map["layers"][0]["description"]["text"],
        "Files under src/"
    );
    assert_eq!(redaction.map["layers"][0]["name"], REDACTION_MARKER);
    // The purchased sentence is redacted even though the layer's name is
    // mechanical.
    assert_eq!(
        redaction.map["layers"][1]["description"]["text"],
        REDACTION_MARKER
    );
    assert_eq!(redaction.map["layers"][1]["name"], "docs");
    assert!(
        redaction
            .redacted
            .iter()
            .any(|(field, count)| field == "LayerDescription.text" && *count == 1),
        "disclosure must count the redacted description: {:?}",
        redaction.redacted
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
    assert_eq!(common::external_urls(&html), Vec::<String>::new());

    assert!(!html.contains("ws://"), "artifact references a websocket");
    assert!(!html.contains("wss://"), "artifact references a websocket");
    assert!(
        !html.contains("src=\"//") && !html.contains("href=\"//"),
        "artifact contains a protocol-relative reference"
    );
}

#[test]
fn share_artifact_references_no_serve_route() {
    // Ticket 04 of V3 (ADR-0013, spec story 10): a share recipient is
    // precisely someone who does not hold the code, so the artifact must
    // not even carry the source route's name — and the honest way to make
    // that true is structural: every route-speaking function lives in the
    // dashboard's wire module, which only `App`'s served branch reaches,
    // and only by dynamic import. The inliner embeds exactly what
    // `index.html` references, so the wire chunk never rides the artifact.
    //
    // The scan walks `serve::REGISTRY` rather than naming `/api/source`
    // alone: the registry is the one place a route is declared, so a route
    // added tomorrow joins this scan the day it exists, and a static import
    // that drags any route string back into the main chunk trips here.
    let root = share_fixture_root();
    let html = String::from_utf8(run_share(root.path())).unwrap();

    for route in codeatlas::serve::REGISTRY {
        assert!(
            !html.contains(route.path),
            "the share artifact byte-contains {} — a serve route has no \
             business in a file whose recipient has no server (ADR-0013)",
            route.path
        );
    }
    // The registry-walk above can only guard routes it contains; pin the
    // one this ticket exists for, so an emptied registry cannot quietly
    // turn the loop vacuous.
    assert!(
        !html.contains("/api/source"),
        "the share artifact byte-contains the source route"
    );
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
fn share_refuses_a_map_that_does_not_conform_to_the_contract() {
    // Fail closed on malformed input (ticket 15 carry-over): redaction
    // reasons about typed fields, so a map whose shape lies — a string
    // where an object belongs, an unknown node kind — must abort the
    // share, not ship the mystery value verbatim.
    for (label, mutate) in [
        (
            "string where the project object belongs",
            Box::new(|map: &mut Value| map["project"] = json!("just-a-string"))
                as Box<dyn Fn(&mut Value)>,
        ),
        (
            "unknown node kind",
            Box::new(|map: &mut Value| map["nodes"][0]["kind"] = json!("blob")),
        ),
        (
            "string where the range object belongs",
            Box::new(|map: &mut Value| map["nodes"][1]["range"] = json!("1-9")),
        ),
    ] {
        let root = share_fixture_root();
        let map_path = root.path().join(".codeatlas/knowledge-graph.json");
        let mut map: Value = serde_json::from_str(&fs::read_to_string(&map_path).unwrap()).unwrap();
        mutate(&mut map);
        fs::write(&map_path, serde_json::to_string(&map).unwrap()).unwrap();

        let output = Command::new(env!("CARGO_BIN_EXE_codeatlas"))
            .args(["share", root.path().to_str().unwrap()])
            .output()
            .unwrap();
        assert!(
            !output.status.success(),
            "share must fail closed on a map with a {label}"
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("does not conform"),
            "share error must say the map is non-conforming ({label}): {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            !root.path().join(".codeatlas/share.html").exists(),
            "no artifact may be written from a non-conforming map ({label})"
        );
    }
}

// ---------------------------------------------------------------------------
// The ceiling (spec story 23, ADR-0011): the share artifact stays small
// enough to hand to anyone, and passing that size is a decision in a diff
// rather than an accumulation nobody sees.
// ---------------------------------------------------------------------------

/// This repository, from the crate the test is compiled in. The ceiling is
/// about the file a person actually receives, so it is weighed on a real map
/// — CodeAtlas's own — and not on the fixture above, whose artifact would be
/// the embedded dashboard and little else.
fn repository_root() -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the workspace root must exist");
    assert!(
        root.join("CONTEXT.md").is_file() && root.join("dashboard").is_dir(),
        "{} is not this repository's root; the measurement below would weigh \
         the wrong map",
        root.display()
    );
    root
}

#[test]
fn the_share_artifact_stays_under_its_ceiling() {
    // Dogfooded, as the self-scan in tests/scan.rs is: this repository is the
    // largest map CodeAtlas is committed to producing, so its artifact is the
    // one worth weighing. A plain scan — no `--enrich`, so nothing is bought;
    // the committed annotation store is carried over exactly as a colleague's
    // clone carries it (ADR-0005), which is the map a real share starts from.
    let root = repository_root();
    let scan = Command::new(env!("CARGO_BIN_EXE_codeatlas"))
        .args(["scan", root.to_str().unwrap()])
        .output()
        .expect("codeatlas scan runs");
    assert!(
        scan.status.success(),
        "self-scan failed: {}",
        String::from_utf8_lossy(&scan.stderr)
    );

    // What the recipient receives: the artifact's own bytes on disk, after
    // templating and inlining, uncompressed.
    let bytes = run_share(&root);
    let measured = bytes.len() as u64;
    eprintln!("this repository's share artifact measured {measured} bytes");
    assert!(
        measured <= SHARE_CEILING_BYTES,
        "the share artifact is {measured} bytes — {over} bytes over the \
         ceiling of {SHARE_CEILING_BYTES} bytes (ADR-0011). Either shrink \
         what the artifact embeds, or raise SHARE_CEILING_BYTES in \
         src/share.rs and say why in the diff.",
        over = measured - SHARE_CEILING_BYTES
    );

    // Ticket 04 (ADR-0013), on the same artifact — the one built from this
    // repository's own enriched map, annotation store and all, because that
    // is the artifact a person actually hands to a stranger. The fixture
    // tests above prove the same properties on a synthetic map; this proves
    // nothing about the real one smuggles them back in.
    let html = String::from_utf8(bytes).expect("the artifact is UTF-8");
    for route in codeatlas::serve::REGISTRY {
        assert!(
            !html.contains(route.path),
            "this repository's own share artifact byte-contains {}",
            route.path
        );
    }
    // No source rides either: probe with a line of a mapped Rust file —
    // read at test time so the probe cannot go stale. The map may name
    // serve.rs, count its lines and list its symbols; the moment a line of
    // it appears verbatim, the artifact is carrying code. The probe is the
    // longest line containing no `"`, `\` or `<`, because those are the
    // characters the two leak paths rewrite (JSON string escaping in the
    // payload, the inliner's `<`): a line free of them arrives
    // byte-identical whether it leaked raw into the HTML or inside the
    // embedded JSON, so one scan covers both vectors — the first tamper of
    // this guard leaked a quoted line and sailed through as `\"`-escaped
    // bytes, which is exactly the miss this filter closes.
    let serve_rs = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/serve.rs"))
        .expect("serve.rs is readable");
    let probe = serve_rs
        .lines()
        .map(str::trim)
        .filter(|line| !line.contains(['"', '\\', '<']))
        .max_by_key(|line| line.len())
        .expect("serve.rs has lines");
    assert!(
        probe.len() > 60,
        "the probe line is too short to be distinctive: {probe:?}"
    );
    assert!(
        !html.contains(probe),
        "this repository's own share artifact contains a line of serve.rs — \
         source is riding the artifact (ADR-0013): {probe:?}"
    );
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
