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

use super::ask;
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

/// What the model is told when answering a question about the map
/// (ADR-0009). The instruction to answer only from the given nodes is not a
/// security control — the bound is, and it is enforced by
/// [`ask::select_context`](super::ask::select_context) before this prompt
/// exists. It is here so a thin slice produces "I cannot tell from the map"
/// rather than a confident guess.
pub const ASK_SYSTEM_PROMPT: &str = "You answer questions about a codebase \
from a map of it. You are given a question and a set of nodes, each with an \
id, kind, name, path and summary. Answer only from those nodes: you cannot \
see the source. Cite the ids of the nodes your answer rests on, and cite \
only ids from the given set. If the nodes do not contain the answer, say so \
plainly and cite whatever comes closest. Keep the answer to a short \
paragraph.";

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

/// The user turn for one question: the project, the carried conversation
/// when there is one, the question, and the bounded slice of nodes selected
/// to answer it. Nothing else — in particular no file contents, which this
/// path has no way to reach.
///
/// The transcript is what gives "it" in a follow-up its referent; the
/// *nodes* the conversation earned arrive through the slice instead
/// (ADR-0012's citations-first rule, enforced in `ask::select_context`).
/// Every string in it was clamped by `ask::build`, so the block is bounded
/// exactly as the rest of the prompt is. A bare question renders
/// byte-identically to what it did before conversations existed.
pub fn ask_user_message(question: &ask::Question) -> String {
    let nodes: Vec<serde_json::Value> = question
        .context
        .iter()
        .map(|node| {
            json!({
                "id": node.id,
                "kind": node.kind,
                "name": node.name,
                "path": node.path,
                "summary": node.summary,
            })
        })
        .collect();
    let transcript = if question.turns.is_empty() {
        String::new()
    } else {
        let mut block = String::from("Previous turns of this conversation, oldest first:\n\n");
        for turn in &question.turns {
            block.push_str(&format!("Q: {}\nA: {}\n\n", turn.question, turn.answer));
        }
        block
    };
    format!(
        "Project: {}\n\n{transcript}Question: {}\n\nNodes:\n{}",
        question.project,
        question.text,
        serde_json::to_string_pretty(&nodes).expect("nodes serialize"),
    )
}

/// One schema-constrained exchange, in the form every transport needs it.
///
/// Enrichment and questions differ in exactly these three values and in
/// nothing about how a completion is performed, so each backend has one
/// `complete` and builds it from one of the two constructors below. The
/// three used to travel as separate parameters, which is the shape that let
/// a backend be wired to the wrong schema without anything noticing.
pub struct Completion {
    pub system_prompt: &'static str,
    pub schema: serde_json::Value,
    pub user_message: String,
}

/// Ask a model to fill enrichment slots.
pub fn for_enrichment(request: &EnrichmentRequest) -> Completion {
    Completion {
        system_prompt: SYSTEM_PROMPT,
        schema: answers_schema(),
        user_message: user_message(request),
    }
}

/// Ask a model a question about the map (ADR-0009).
pub fn for_question(question: &ask::Question) -> Completion {
    Completion {
        system_prompt: ASK_SYSTEM_PROMPT,
        schema: ask_answer_schema(),
        user_message: ask_user_message(question),
    }
}

/// The JSON Schema an answer is constrained to: prose plus the node ids it
/// rests on. Citations are a plain string array — the ids are validated
/// against what was actually sent, which a schema cannot express.
///
/// Named apart from [`answers_schema`] deliberately: the two were one
/// character apart, and a backend wired to the wrong one survived a
/// mutation because every assertion about it read the same near-identical
/// name.
pub fn ask_answer_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "answer": {
                "type": "string",
                "description": "The answer, a short paragraph",
            },
            "citations": {
                "type": "array",
                "items": {
                    "type": "string",
                    "description": "A node id from the given set",
                },
            },
        },
        "required": ["answer", "citations"],
        "additionalProperties": false,
    })
}

/// The structured-outputs answer shape — mirrors [`ask_answer_schema`].
#[derive(Deserialize)]
struct AskAnswer {
    answer: String,
    citations: Vec<String>,
}

/// Reads a structured output into an answer. Shared by both backends for the
/// same reason [`parse_answers`] is: one wording for a schema violation, and
/// one place for the shape to be right.
pub fn parse_ask_answer(structured: serde_json::Value) -> Result<ask::Answer> {
    let parsed: AskAnswer = serde_json::from_value(structured)
        .context("the structured output did not match the requested answer schema")?;
    Ok(ask::Answer {
        text: parsed.answer,
        citations: parsed.citations,
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

    /// The question path's counterpart, and the reason it exists separately:
    /// `docs/SECURITY.md` states per-node what a question carries, which is
    /// a claim about the nodes and says nothing about what surrounds them.
    /// The message also carries the project name and the reader's question,
    /// and a third thing appearing beside them would make the document's
    /// "and nothing else" false without any per-node assertion noticing.
    #[test]
    fn a_question_carries_the_project_the_question_and_its_nodes_and_no_more() {
        let question = ask::Question {
            project: "demo".into(),
            text: "where does the program start?".into(),
            context: vec![ask::NodeContext {
                id: "file:src/main.ts".into(),
                kind: NodeKind::File,
                name: "main.ts".into(),
                path: "src/main.ts".into(),
                summary: "TypeScript file, 3 lines".into(),
            }],
            turns: Vec::new(),
        };

        let message = ask_user_message(&question);

        // Everything outside the node slice, asserted whole rather than by
        // `contains`: an extra sentence carrying anything else would survive
        // a contains-check and not this.
        let (preamble, nodes) = message
            .split_once("Nodes:\n")
            .expect("the message has a nodes section");
        assert_eq!(
            preamble, "Project: demo\n\nQuestion: where does the program start?\n\n",
            "the message carries the project and the question and nothing else"
        );

        let nodes: serde_json::Value = serde_json::from_str(nodes).expect("the nodes are JSON");
        let fields: Vec<&str> = nodes[0].as_object().unwrap().keys().map(|k| &**k).collect();
        assert_eq!(
            fields,
            ["id", "kind", "name", "path", "summary"],
            "the documented set of per-node fields, and nothing else"
        );
        // Never the graph and never the repository (ADR-0009).
        assert!(!message.contains("edges"), "{message}");
        assert!(!message.contains("imports"), "{message}");
    }

    /// ADR-0012: carried turns ride the prompt as a transcript, oldest
    /// first, so "it" in the current question has a referent — while a bare
    /// question's message stays byte-identical to what it was before
    /// conversations existed (the test above holds it to that).
    #[test]
    fn carried_turns_ride_between_the_project_and_the_question_oldest_first() {
        let question = ask::Question {
            project: "demo".into(),
            text: "what calls it?".into(),
            context: vec![ask::NodeContext {
                id: "file:src/main.ts".into(),
                kind: NodeKind::File,
                name: "main.ts".into(),
                path: "src/main.ts".into(),
                summary: "TypeScript file, 3 lines".into(),
            }],
            turns: vec![
                ask::Turn {
                    question: "where does the program start?".into(),
                    answer: "In src/main.ts.".into(),
                    citations: vec!["file:src/main.ts".into()],
                },
                ask::Turn {
                    question: "what does it import?".into(),
                    answer: "The util module.".into(),
                    citations: Vec::new(),
                },
            ],
        };

        let message = ask_user_message(&question);

        // Asserted whole, exactly as the bare-question test does: the
        // transcript is one block in a stated place, not sentences scattered
        // wherever an implementation dropped them.
        let (preamble, _) = message
            .split_once("Nodes:\n")
            .expect("the message has a nodes section");
        assert_eq!(
            preamble,
            "Project: demo\n\n\
             Previous turns of this conversation, oldest first:\n\n\
             Q: where does the program start?\n\
             A: In src/main.ts.\n\n\
             Q: what does it import?\n\
             A: The util module.\n\n\
             Question: what calls it?\n\n",
            "the transcript rides between the project and the question"
        );
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
