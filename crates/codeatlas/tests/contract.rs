//! Tests of the published map contract (ADR-0003): the schema file committed
//! at `contract/map.schema.json` is the official artifact external producers
//! target, so these tests validate against that committed file — never against
//! the in-memory structs.

use std::fs;
use std::path::Path;

fn committed_schema() -> serde_json::Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contract/map.schema.json");
    let raw = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("committed contract schema missing at {path:?}: {e}"));
    serde_json::from_str(&raw).unwrap()
}

fn fixture_map(name: &str) -> serde_json::Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/maps")
        .join(name);
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn known_good_fixture_map_validates_against_the_committed_schema() {
    let validator = jsonschema::validator_for(&committed_schema()).unwrap();
    let map = fixture_map("known-good.json");

    let errors: Vec<String> = validator.iter_errors(&map).map(|e| e.to_string()).collect();
    assert!(
        errors.is_empty(),
        "known-good map rejected by the committed contract: {errors:?}"
    );
}

#[test]
fn committed_schema_carries_a_versioned_id() {
    let schema = committed_schema();
    let id = schema["$id"].as_str().expect("schema has no $id");
    assert_eq!(
        id, "urn:codeatlas:map-contract:0.4.0",
        "$id must be the stable contract URI carrying the current version"
    );
}

/// Story 9: `significance` is optional, so a map written before the field
/// existed validates unchanged. The stored fixture is that map — it predates
/// the field and must keep passing.
#[test]
fn a_map_written_before_significance_still_validates() {
    let schema = committed_schema();
    let node = &schema["$defs"]["Node"];
    assert!(
        node["properties"].get("significance").is_some(),
        "the contract must publish `significance`, or its optionality here \
         proves nothing"
    );
    let required: Vec<&str> = node["required"]
        .as_array()
        .expect("Node lists its required properties")
        .iter()
        .map(|r| r.as_str().unwrap())
        .collect();
    assert!(
        !required.contains(&"significance"),
        "`significance` must stay optional — a map written before it cannot \
         carry it: {required:?}"
    );

    let map = fixture_map("known-good.json");
    for node in map["nodes"].as_array().unwrap() {
        assert!(
            node.get("significance").is_none(),
            "the stored fixture must predate `significance`, or validating it \
             proves nothing: {node:?}"
        );
    }
    let validator = jsonschema::validator_for(&schema).unwrap();
    let errors: Vec<String> = validator.iter_errors(&map).map(|e| e.to_string()).collect();
    assert!(
        errors.is_empty(),
        "a map written before `significance` was rejected by the committed \
         contract: {errors:?}"
    );
}

#[test]
fn committed_schema_rejects_ill_formed_scalar_values() {
    // Ticket 14 contract tightenings: version is semver-shaped, node IDs
    // carry a kind prefix, line ranges are 1-based. A permissive schema
    // accepting any string/integer proves none of the doc-comments.
    let validator = jsonschema::validator_for(&committed_schema()).unwrap();
    let good = fixture_map("known-good.json");

    let mut bad_version = good.clone();
    bad_version["version"] = serde_json::json!("not-a-semver");
    assert!(
        !validator.is_valid(&bad_version),
        "schema accepted a non-semver `version`"
    );

    let mut bad_id = good.clone();
    bad_id["nodes"][0]["id"] = serde_json::json!("widget:src/main.ts");
    assert!(
        !validator.is_valid(&bad_id),
        "schema accepted a node ID without a known kind prefix"
    );

    let mut bad_range = good.clone();
    bad_range["nodes"][1]["range"]["start_line"] = serde_json::json!(0);
    assert!(
        !validator.is_valid(&bad_range),
        "schema accepted a 0 start_line despite ranges being 1-based"
    );
}

#[test]
fn committed_schema_rejects_a_map_that_breaks_the_contract() {
    let validator = jsonschema::validator_for(&committed_schema()).unwrap();

    // Strip a required field and corrupt a closed enum: a validator that
    // accepts these proves nothing.
    let mut map = fixture_map("known-good.json");
    map.as_object_mut().unwrap().remove("version");
    map["nodes"][0]["kind"] = serde_json::json!("blob");

    assert!(
        !validator.is_valid(&map),
        "committed schema accepted a map missing `version` with an unknown node kind"
    );
}
