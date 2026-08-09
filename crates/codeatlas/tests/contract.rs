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
