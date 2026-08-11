//! What the model is asked, and how its answer is read — the half of
//! enrichment that is the same whichever backend carries it.
//!
//! Two backends now reach a model: the Messages API over HTTPS
//! ([`super::claude`], ADR-0004) and the `claude` CLI as a subprocess
//! ([`super::agent_cli`], ADR-0008). They differ entirely in *transport* and
//! not at all in *what they say*, so the prompt, the response schema, and the
//! answer parsing live here and neither backend owns a copy.
//!
//! That matters beyond tidiness. `docs/SECURITY.md` states exactly what a
//! model receives — node ids, kinds, names, paths and mechanical summaries;
//! layer directories; flow step names; tour topology; the project name, and
//! never file contents or edges. One [`slot_payload`] is one place for that
//! claim to be true, rather than two that can drift apart.
//!
//! Compiled only when a backend that needs it is.

use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::json;

use super::{EnrichmentRequest, EnrichmentResponse, EnrichmentSlot};

pub const SYSTEM_PROMPT: &str = "You fill labeling slots in a code map. Each slot \
has a kind and a key; echo each slot's key exactly as given and answer for \
every slot. Slot kinds: 'summary' — one concise sentence describing the \
entity's purpose, grounded in its kind, name, path, and mechanical summary; \
'layer-name' — a short human-readable name (a few words) for the group of \
files under the given directory; 'flow-name' — a short business-domain name \
for the call flow described by its entry point and steps; 'tour-label' — one \
engaging sentence narrating this stop on a guided tour of the codebase, \
grounded in the path and its import fan-in/fan-out.";

/// One slot as it rides the prompt: its kind, its response key, and the
/// mechanically summarized topology that slot kind carries — nothing else
/// (spec: bounded prompts).
pub fn slot_payload(slot: &EnrichmentSlot) -> serde_json::Value {
    let key = slot.key();
    match slot {
        EnrichmentSlot::NodeSummary(s) => json!({
            "slot": "summary",
            "key": key,
            "kind": s.kind,
            "name": s.name,
            "path": s.path,
            "mechanical_summary": s.mechanical_summary,
        }),
        EnrichmentSlot::LayerName(s) => json!({
            "slot": "layer-name",
            "key": key,
            "directory": s.id,
            "member_files": s.member_files,
        }),
        EnrichmentSlot::FlowName(s) => json!({
            "slot": "flow-name",
            "key": key,
            "domain": s.domain,
            "entry_point": s.entry,
            "steps": s.step_names,
            "step_count": s.step_count,
        }),
        EnrichmentSlot::TourLabel(s) => json!({
            "slot": "tour-label",
            "key": key,
            "path": s.path,
            "fan_in": s.fan_in,
            "fan_out": s.fan_out,
            "mechanical_label": s.mechanical_label,
        }),
    }
}

/// The user turn for one batch: the project name and the slots being filled.
/// Nothing else ever goes into it.
pub fn user_message(request: &EnrichmentRequest) -> String {
    let slots: Vec<serde_json::Value> = request.slots.iter().map(slot_payload).collect();
    format!(
        "Project: {}\n\nSlots to fill:\n{}",
        request.project,
        serde_json::to_string_pretty(&slots).expect("slots serialize"),
    )
}

/// The JSON Schema every backend constrains its response with. A map with
/// dynamic keys is not expressible under structured outputs
/// (`additionalProperties` must be `false`), so answers arrive as an array of
/// `{key, text}` objects.
pub fn answers_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "answers": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "key": {
                            "type": "string",
                            "description": "The slot key, exactly as given",
                        },
                        "text": {
                            "type": "string",
                            "description": "The text filling the slot",
                        },
                    },
                    "required": ["key", "text"],
                    "additionalProperties": false,
                },
            },
        },
        "required": ["answers"],
        "additionalProperties": false,
    })
}

/// The structured-outputs answer shape — mirrors [`answers_schema`].
#[derive(Deserialize)]
struct Answers {
    answers: Vec<Answer>,
}

#[derive(Deserialize)]
struct Answer {
    key: String,
    text: String,
}

/// Reads a structured output into typed answers.
///
/// Both backends call this, so the "did not match the requested schema"
/// wording is written once. There is deliberately no repair path: structured
/// outputs are schema-guaranteed, and anything that does not deserialize is
/// an ordinary provider error that leaves the structural map intact (spec
/// stories 13 and 14).
pub fn parse_answers(structured: serde_json::Value) -> Result<EnrichmentResponse> {
    let answers: Answers = serde_json::from_value(structured)
        .context("the structured output did not match the requested schema")?;
    Ok(EnrichmentResponse {
        answers: answers
            .answers
            .into_iter()
            .map(|answer| (answer.key, answer.text))
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    //! The bounded-prompt invariant lives here rather than in either
    //! backend's tests: it is a property of what is *said*, and both
    //! transports say the same thing.

    use super::*;
    use crate::enrich::SummarySlot;
    use crate::map::{NodeId, NodeKind};

    fn summary_slot() -> EnrichmentSlot {
        EnrichmentSlot::NodeSummary(SummarySlot {
            node: NodeId::file("src/main.ts"),
            kind: NodeKind::File,
            name: "main.ts".into(),
            path: "src/main.ts".into(),
            mechanical_summary: "TypeScript file, 3 lines".into(),
        })
    }

    /// `docs/SECURITY.md` states what a model receives. A slot payload that
    /// grew a field would make that statement false in both backends at
    /// once, which is the reason this module exists.
    #[test]
    fn a_summary_slot_carries_exactly_the_documented_fields() {
        let payload = slot_payload(&summary_slot());
        let fields: Vec<&str> = payload.as_object().unwrap().keys().map(|k| &**k).collect();

        assert_eq!(
            fields,
            ["key", "kind", "mechanical_summary", "name", "path", "slot"],
            "the documented set of fields, and nothing else"
        );
    }

    #[test]
    fn the_message_carries_the_project_and_its_slots_and_no_more() {
        let request = EnrichmentRequest {
            project: "demo".into(),
            slots: vec![summary_slot()],
        };
        let message = user_message(&request);

        assert!(message.contains("demo"));
        assert!(message.contains("summary:file:src/main.ts"));
        // Never the graph: no edges, no member lists (ADR-0004).
        assert!(!message.contains("edges"), "{message}");
        assert!(!message.contains("imports"), "{message}");
    }

    #[test]
    fn answers_parse_once_and_never_get_repaired() {
        let good = serde_json::json!({
            "answers": [{"key": "summary:file:src/main.ts", "text": "The entry point."}]
        });
        let parsed = parse_answers(good).unwrap();
        assert_eq!(
            parsed.answers.get("summary:file:src/main.ts").unwrap(),
            "The entry point."
        );

        // Story 13: no repair machinery. A shape that does not match is an
        // error, not something to be coaxed into place.
        let bad = serde_json::json!({"answers": "not an array"});
        assert!(parse_answers(bad).is_err());
    }
}
