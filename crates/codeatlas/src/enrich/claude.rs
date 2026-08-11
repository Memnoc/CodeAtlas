//! The real enrichment provider (ADR-0004): direct HTTPS to the Claude
//! Messages API — the binary's only possible egress destination (ADR-0006),
//! hardcoded, no override.
//!
//! Rust has no official Anthropic SDK; the sanctioned path is raw HTTPS
//! against `POST /v1/messages`. Requests use the API's structured-outputs
//! mechanism (`output_config.format` with a JSON schema), so a successful
//! response is guaranteed schema-valid JSON: the reply is parsed exactly
//! once into typed structs, and there is deliberately no repair, retry-on-
//! parse, or output-massaging code path — any deviation is an ordinary
//! provider error that degrades the run and leaves the structural map
//! intact (spec story 14).
//!
//! Prompts are bounded: [`super::fill_slots`] batches at most
//! [`super::BATCH_SIZE`] slots per request, and [`build_request_body`]
//! serializes only those slots — a node's id/kind/name/path and mechanical
//! summary, a layer's directory and file count, a flow's domain and opening
//! step names, a tour stop's path and fan-in/out — plus the project name.
//! Never the graph: no edges, no member lists.
//!
//! Credentials resolve like the SDKs (ADR-0004): `ANTHROPIC_API_KEY`
//! first (sent as `x-api-key`), then the `ant auth login` OAuth profile —
//! a short-lived bearer token obtained by running
//! `ant auth print-credentials --access-token`, sent as
//! `Authorization: Bearer` with the `oauth-2025-04-20` beta header.
//! Missing credentials are a clear error before any socket is opened.
//!
//! Everything except the single [`post`] shim is a pure function
//! unit-tested below — no test opens a socket.

use std::process::Command;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;
use serde_json::json;

use super::{EnrichmentProvider, EnrichmentRequest, EnrichmentResponse, ask, prompt};

/// The one URL this binary can ever talk to (ADR-0006). Hardcoded on
/// purpose: no env var, flag, or config may redirect enrichment traffic —
/// which [`agent`] enforces at the transport level too, by refusing to
/// follow redirects and ignoring proxy env vars.
const API_URL: &str = "https://api.anthropic.com/v1/messages";

/// Spec decision: the default enrichment model, overridable with `--model`.
pub const DEFAULT_MODEL: &str = "claude-opus-5";

const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Beta header required when authenticating with an `ant` OAuth bearer
/// token instead of an API key (ADR-0004).
const OAUTH_BETA: &str = "oauth-2025-04-20";

/// Output budget per request. A batch is at most [`super::BATCH_SIZE`]
/// one-sentence summaries — well under this even with the model's internal
/// reasoning counted against the same cap.
const MAX_TOKENS: u32 = 8192;

/// The Claude Messages API provider behind the [`EnrichmentProvider`]
/// trait. Holds only the model name; credentials are resolved per call so
/// a missing key fails the enrichment step, never construction.
pub struct ClaudeProvider {
    model: String,
}

impl ClaudeProvider {
    pub fn new(model: Option<&str>) -> Self {
        Self {
            model: model.unwrap_or(DEFAULT_MODEL).to_string(),
        }
    }
}

impl EnrichmentProvider for ClaudeProvider {
    fn enrich(&self, request: &EnrichmentRequest) -> Result<EnrichmentResponse> {
        prompt::parse_answers(self.complete(&prompt::for_enrichment(request))?)
    }

    fn ask(&self, question: &ask::Question) -> Result<ask::Answer> {
        prompt::parse_ask_answer(self.complete(&prompt::for_question(question))?)
    }
}

impl ClaudeProvider {
    /// One schema-constrained request over HTTPS. Both trait methods go
    /// through here so credential resolution, the POST and the envelope
    /// checks exist once — the CLI backend next door has the same shape for
    /// the same reason.
    fn complete(&self, completion: &prompt::Completion) -> Result<serde_json::Value> {
        let credentials = resolve_credentials()?;
        let body = build_request_body(&self.model, completion);
        let raw = post(&body, &credentials)?;
        structured_output(&raw)
    }
}

/// How a request authenticates, in ADR-0004 resolution order.
enum Credentials {
    /// From `ANTHROPIC_API_KEY`.
    ApiKey(String),
    /// A short-lived bearer token from the `ant` OAuth profile.
    OauthToken(String),
}

/// Resolves credentials: `ANTHROPIC_API_KEY` first, then the `ant` OAuth
/// profile via `ant auth print-credentials --access-token` (which refreshes
/// the token if needed — reading the profile file directly could hand back
/// a stale token). No credentials is a clear, actionable error.
fn resolve_credentials() -> Result<Credentials> {
    if let Ok(key) = std::env::var("ANTHROPIC_API_KEY")
        && !key.trim().is_empty()
    {
        return Ok(Credentials::ApiKey(key.trim().to_string()));
    }
    if let Ok(output) = Command::new("ant")
        .args(["auth", "print-credentials", "--access-token"])
        .output()
        && output.status.success()
    {
        let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !token.is_empty() {
            return Ok(Credentials::OauthToken(token));
        }
    }
    Err(anyhow!(
        "no Claude API credentials: set ANTHROPIC_API_KEY, or log in with \
         `ant auth login` (the ant OAuth profile)"
    ))
}

/// The auth headers for a credential, per ADR-0004: API keys ride
/// `x-api-key`; `ant` OAuth tokens ride `Authorization: Bearer` and must
/// carry the [`OAUTH_BETA`] header.
fn auth_headers(credentials: &Credentials) -> Vec<(&'static str, String)> {
    match credentials {
        Credentials::ApiKey(key) => vec![("x-api-key", key.clone())],
        Credentials::OauthToken(token) => vec![
            ("authorization", format!("Bearer {token}")),
            ("anthropic-beta", OAUTH_BETA.to_string()),
        ],
    }
}

/// Builds the Messages API request body for one completion — whichever kind
/// it is. `output_config.format` carries the completion's schema, so a
/// successful response is guaranteed to deserialize (ADR-0004).
fn build_request_body(model: &str, completion: &prompt::Completion) -> serde_json::Value {
    json!({
        "model": model,
        "max_tokens": MAX_TOKENS,
        "system": completion.system_prompt,
        "messages": [{
            "role": "user",
            "content": completion.user_message,
        }],
        "output_config": {
            "format": {
                "type": "json_schema",
                "schema": completion.schema,
            },
        },
    })
}

/// A Messages API response, reduced to the fields this provider reads.
#[derive(Deserialize)]
struct ApiMessage {
    #[serde(default)]
    content: Vec<ContentBlock>,
    #[serde(default)]
    stop_reason: Option<String>,
}

#[derive(Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    text: String,
}

/// The structured payload inside a successful (HTTP 2xx) Messages API
/// response. Structured outputs guarantee the text block is schema-valid
/// JSON, so this parses exactly once; anything else — a refusal or
/// truncation `stop_reason`, a missing text block, text that is not JSON —
/// is an error, never a repair attempt.
///
/// Shared by both request kinds: enrichment and questions differ in the
/// schema they demand, not in how a Messages API response is read.
fn structured_output(raw: &str) -> Result<serde_json::Value> {
    let message: ApiMessage =
        serde_json::from_str(raw).context("malformed Claude Messages API response")?;
    if let Some(reason) = message.stop_reason.as_deref()
        && reason != "end_turn"
    {
        bail!(
            "the model stopped with stop_reason {reason:?} instead of \
             completing the structured response"
        );
    }
    let text = message
        .content
        .iter()
        .find(|block| block.kind == "text" && !block.text.is_empty())
        .map(|block| block.text.as_str())
        .ok_or_else(|| anyhow!("the Claude API response carries no text content"))?;
    serde_json::from_str(text).context("the structured output was not JSON")
}

/// The single network call in the binary (ADR-0006): one blocking POST to
/// [`API_URL`]. Deliberately a thin, untested shim — everything worth
/// testing lives in the pure functions above. Any transport or HTTP-status
/// failure degrades the enrichment step with a clear message.
/// The one place the HTTP agent is configured; [`post`] uses it for every
/// call and a unit test pins the egress-critical settings via
/// `Agent::config()`.
///
/// Two ureq defaults are explicitly overridden because they would break
/// ADR-0006's single-egress-destination guarantee:
///
/// - `max_redirects(0)` — ureq follows up to 10 redirects by default, and
///   a 3xx Location may point at any host while `x-api-key` and
///   `anthropic-beta` are retained cross-host (only `authorization` and
///   `cookie` are stripped). A redirecting response must be an ordinary
///   error, never a request to a second destination.
/// - `proxy(None)` — ureq honors `HTTPS_PROXY`/`ALL_PROXY` by default,
///   which would let the environment choose the TCP egress destination.
///   The socket goes direct to [`API_URL`]'s host or nowhere.
fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(300)))
        .max_redirects(0)
        .proxy(None)
        .build()
        .into()
}

fn post(body: &serde_json::Value, credentials: &Credentials) -> Result<String> {
    let mut request = agent()
        .post(API_URL)
        .header("content-type", "application/json")
        .header("anthropic-version", ANTHROPIC_VERSION);
    for (name, value) in auth_headers(credentials) {
        request = request.header(name, &value);
    }
    let mut response = request
        .send(body.to_string())
        .context("cannot reach the Claude API")?;
    let status = response.status();
    let text = response
        .body_mut()
        .read_to_string()
        .context("cannot read the Claude API response")?;
    if !status.is_success() {
        bail!(
            "the Claude API request failed with HTTP {status}: {}",
            text.chars().take(400).collect::<String>()
        );
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    //! Request construction and response handling without sockets: the
    //! HTTP call itself is the one thing these tests do not cover.

    use super::*;
    use crate::enrich::EnrichmentSlot;
    use crate::map::{NodeId, NodeKind};
    use std::collections::BTreeMap;

    /// Envelope handling plus the enrichment schema — the pairing the
    /// `enrich` method performs, named so these tests read as before the
    /// envelope step was shared with the question path.
    fn parse_response(raw: &str) -> Result<EnrichmentResponse> {
        prompt::parse_answers(structured_output(raw)?)
    }

    fn request() -> EnrichmentRequest {
        EnrichmentRequest {
            project: "demo".into(),
            slots: vec![
                EnrichmentSlot::NodeSummary(super::super::SummarySlot {
                    node: NodeId::file("src/a.ts"),
                    kind: NodeKind::File,
                    name: "a.ts".into(),
                    path: "src/a.ts".into(),
                    mechanical_summary: "TypeScript file with 1 function.".into(),
                }),
                EnrichmentSlot::NodeSummary(super::super::SummarySlot {
                    node: NodeId::symbol(NodeKind::Function, "src/a.ts", "go"),
                    kind: NodeKind::Function,
                    name: "go".into(),
                    path: "src/a.ts".into(),
                    mechanical_summary: "Function go, called by main.".into(),
                }),
                EnrichmentSlot::LayerName(super::super::LayerSlot {
                    id: "src".into(),
                    member_files: 12,
                }),
                EnrichmentSlot::FlowName(super::super::FlowSlot {
                    id: "flow:function:src/a.ts:go".into(),
                    domain: "src".into(),
                    entry: "go".into(),
                    step_names: vec!["go".into(), "greet".into()],
                    step_count: 2,
                }),
                EnrichmentSlot::TourLabel(super::super::TourSlot {
                    node: NodeId::file("src/a.ts"),
                    path: "src/a.ts".into(),
                    fan_in: 3,
                    fan_out: 1,
                    mechanical_label: "Entry point: src/a.ts — fan-in 3, fan-out 1".into(),
                }),
            ],
        }
    }

    #[test]
    fn the_default_model_is_claude_opus_5_and_the_flag_overrides_it() {
        assert_eq!(ClaudeProvider::new(None).model, DEFAULT_MODEL);
        assert_eq!(DEFAULT_MODEL, "claude-opus-5");
        assert_eq!(
            ClaudeProvider::new(Some("claude-haiku-4-5")).model,
            "claude-haiku-4-5"
        );
    }

    #[test]
    fn the_request_carries_typed_slots_and_demands_schema_valid_output() {
        let body = build_request_body(DEFAULT_MODEL, &prompt::for_enrichment(&request()));

        assert_eq!(body["model"], "claude-opus-5");
        assert_eq!(body["max_tokens"], MAX_TOKENS);

        // Structured outputs per ADR-0004: the schema rides the request,
        // so the response is guaranteed to deserialize — no repair code.
        let format = &body["output_config"]["format"];
        assert_eq!(format["type"], "json_schema");
        let items = &format["schema"]["properties"]["answers"]["items"];
        assert_eq!(items["required"], json!(["key", "text"]));
        assert_eq!(items["additionalProperties"], json!(false));

        // Bounded prompt: every slot kind rides as its key plus its own
        // mechanically summarized topology, and the project name.
        let content = body["messages"][0]["content"].as_str().unwrap();
        assert!(content.contains("Project: demo"));
        for needle in [
            // node summary slots
            "summary:file:src/a.ts",
            "summary:function:src/a.ts:go",
            "TypeScript file with 1 function.",
            "Function go, called by main.",
            "\"kind\": \"function\"",
            // layer slot: directory + member count, no member list
            "layer-name:src",
            "\"member_files\": 12",
            // flow slot: domain, entry point, step names
            "flow-name:flow:function:src/a.ts:go",
            "\"entry_point\": \"go\"",
            "greet",
            "\"step_count\": 2",
            // tour slot: path + fan-in/out numbers
            "tour-label:file:src/a.ts",
            "\"fan_in\": 3",
            "\"fan_out\": 1",
        ] {
            assert!(content.contains(needle), "missing from prompt: {needle}");
        }
        // Never the serialized graph: no edge objects, no adjacency.
        for forbidden in [
            "\"edges\"",
            "\"nodes\"",
            "\"source\"",
            "\"target\"",
            "\"weight\"",
        ] {
            assert!(
                !content.contains(forbidden),
                "the prompt must never carry the graph: found {forbidden:?}"
            );
        }
    }

    #[test]
    fn a_schema_valid_response_becomes_typed_answers() {
        // Thinking blocks (empty text) precede the structured text block —
        // the parser must skip them.
        let raw = json!({
            "content": [
                {"type": "thinking", "thinking": "..."},
                {"type": "text", "text": r#"{"answers": [
                    {"key": "summary:file:src/a.ts", "text": "The entry file."},
                    {"key": "layer-name:src", "text": "Application core"},
                    {"key": "tour-label:file:src/a.ts", "text": "Start here."}
                ]}"#},
            ],
            "stop_reason": "end_turn",
        })
        .to_string();

        let response = parse_response(&raw).unwrap();
        assert_eq!(
            response.answers,
            BTreeMap::from([
                (
                    "summary:file:src/a.ts".to_string(),
                    "The entry file.".to_string()
                ),
                ("layer-name:src".to_string(), "Application core".to_string()),
                (
                    "tour-label:file:src/a.ts".to_string(),
                    "Start here.".to_string()
                ),
            ])
        );
    }

    #[test]
    fn a_refusal_or_truncation_is_an_error_not_a_repair_attempt() {
        let refusal = json!({"content": [], "stop_reason": "refusal"}).to_string();
        let err = parse_response(&refusal).unwrap_err();
        assert!(err.to_string().contains("refusal"), "{err}");

        let truncated = json!({
            "content": [{"type": "text", "text": "{\"summaries\": ["}],
            "stop_reason": "max_tokens",
        })
        .to_string();
        let err = parse_response(&truncated).unwrap_err();
        assert!(err.to_string().contains("max_tokens"), "{err}");
    }

    #[test]
    fn text_that_violates_the_schema_is_an_error() {
        let raw = json!({
            "content": [{"type": "text", "text": "{\"unexpected\": true}"}],
            "stop_reason": "end_turn",
        })
        .to_string();
        let err = parse_response(&raw).unwrap_err();
        assert!(err.to_string().contains("schema"), "{err}");
    }

    /// The question path over this transport: the same bounded-prompt claim
    /// as enrichment, plus the schema that makes citations parseable.
    #[test]
    fn a_question_rides_with_its_nodes_and_demands_a_cited_answer() {
        let question = ask::Question {
            project: "demo".into(),
            text: "where are sessions validated?".into(),
            context: vec![ask::NodeContext {
                id: "file:src/auth/session.ts".into(),
                kind: NodeKind::File,
                name: "session.ts".into(),
                path: "src/auth/session.ts".into(),
                summary: "Issues and validates login sessions.".into(),
            }],
        };
        let body = build_request_body(DEFAULT_MODEL, &prompt::for_question(&question));

        let format = &body["output_config"]["format"];
        assert_eq!(format["type"], "json_schema");
        assert_eq!(
            format["schema"]["required"],
            json!(["answer", "citations"]),
            "an answer without citations is not an answer (ADR-0009)"
        );

        let content = body["messages"][0]["content"].as_str().unwrap();
        assert!(content.contains("where are sessions validated?"));
        assert!(content.contains("file:src/auth/session.ts"));
        assert!(content.contains("Issues and validates login sessions."));
        // The map's prose and topology, never the file behind it.
        for forbidden in ["\"edges\"", "\"source\"", "\"target\"", "\"content\""] {
            assert!(
                !content.contains(forbidden),
                "a question must never carry the graph or a file: {forbidden:?}"
            );
        }
    }

    #[test]
    fn an_answer_and_its_citations_are_read_from_the_structured_output() {
        let raw = json!({
            "content": [{"type": "text", "text": r#"{
                "answer": "Sessions are validated in session.ts.",
                "citations": ["file:src/auth/session.ts"]
            }"#}],
            "stop_reason": "end_turn",
        })
        .to_string();

        let answer = prompt::parse_ask_answer(structured_output(&raw).unwrap()).unwrap();
        assert_eq!(answer.text, "Sessions are validated in session.ts.");
        assert_eq!(answer.citations, vec!["file:src/auth/session.ts"]);
    }

    #[test]
    fn the_agent_can_neither_follow_redirects_nor_use_an_env_proxy() {
        // ADR-0006: api.anthropic.com is the binary's only possible egress
        // destination. Two ureq defaults would break that guarantee —
        // following 3xx redirects to an attacker-chosen Location (with the
        // x-api-key header retained cross-host), and honoring
        // HTTPS_PROXY/ALL_PROXY so the TCP destination becomes
        // env-controlled. Declare a proxy here to prove the agent ignores
        // it: ureq reads these env vars when the config is built, so a
        // pre-fix agent would show Some(proxy).
        //
        // SAFETY: set_var is process-global; no other test in this crate
        // reads proxy env vars, and nothing else runs `post()` in tests.
        unsafe {
            std::env::set_var("HTTPS_PROXY", "http://127.0.0.1:39999");
            std::env::set_var("ALL_PROXY", "http://127.0.0.1:39999");
        }
        let agent = agent();
        let config = agent.config();
        assert_eq!(
            config.max_redirects(),
            0,
            "redirects must be hard-disabled: a 3xx must never move the \
             request (or the API key) to another host"
        );
        assert!(
            config.proxy().is_none(),
            "the agent must ignore proxy env vars: the socket goes direct \
             to api.anthropic.com or nowhere"
        );
        unsafe {
            std::env::remove_var("HTTPS_PROXY");
            std::env::remove_var("ALL_PROXY");
        }
    }

    #[test]
    fn api_keys_and_oauth_tokens_ride_different_headers() {
        let key = auth_headers(&Credentials::ApiKey("sk-ant-x".into()));
        assert_eq!(key, vec![("x-api-key", "sk-ant-x".to_string())]);

        let oauth = auth_headers(&Credentials::OauthToken("tok".into()));
        assert_eq!(
            oauth,
            vec![
                ("authorization", "Bearer tok".to_string()),
                ("anthropic-beta", OAUTH_BETA.to_string()),
            ]
        );
    }
}
