//! The subscription-billed enrichment backend (ADR-0008): spawns the user's
//! already-authenticated `claude` CLI as a one-shot, schema-constrained
//! completion.
//!
//! The point is credentials CodeAtlas never touches. An API key is the only
//! way in for anyone outside Anthropic, and in many organisations only
//! administrators can obtain one — so the whole explanatory half of the
//! product is unreachable for most of a team. Here the CLI authenticates
//! itself and this process never sees a secret, which is also why
//! `ANTHROPIC_API_KEY` is deliberately *removed* from the child's
//! environment: `cli:claude` must mean the subscription, not a silent
//! fallback to per-token API billing.
//!
//! # The child is a completion, not an agent
//!
//! `docs/SECURITY.md` promises the model receives node ids, names, paths and
//! mechanical summaries — never file contents. A `claude` process with its
//! ordinary tools and its ordinary configuration could read the repository,
//! or run a shell command through a hook, and void that. Four flags prevent
//! it, and each is the documented mechanism rather than an approximation:
//!
//! 1. **`--tools=`** — the empty value the CLI documents as "disable all
//!    tools". This is the guarantee; an enumerated `--disallowed-tools` list
//!    was the first attempt and is strictly worse, because it silently stops
//!    covering a tool the day a new one is added.
//! 2. **`--safe-mode`** — no `CLAUDE.md`, no skills, no plugins, **no
//!    hooks**, no MCP servers, no custom agents. Hooks matter most: they run
//!    shell commands, and `HOME` is on the environment allowlist, so without
//!    this the user's own hooks would fire on every enrichment call.
//!    Authentication is explicitly unaffected, which is what makes the flag
//!    usable here at all.
//! 3. **An empty, strict MCP config** — nothing configured elsewhere loads.
//! 4. **A fresh empty working directory**, created per call and removed
//!    after, and never widened with `--add-dir`. Belt and braces behind
//!    axis 1: with no tools there is nothing to read a directory with.
//!
//! Every flag is passed in `--flag=value` form, and the prompt is separated
//! by `--`. That is not style. Several of these options are variadic in the
//! CLI's argument parser, and a variadic option in space-separated form eats
//! following arguments until the next option — which would silently swallow
//! the prompt and run the model on nothing at all.
//!
//! # Two traps in the flag surface
//!
//! `--bare` looks like the right flag for a minimal invocation and is exactly
//! wrong: it *"skips keychain reads"* and takes authentication strictly from
//! `ANTHROPIC_API_KEY`, the credential this backend exists to avoid needing.
//!
//! `--append-system-prompt` *appends* to the CLI's own agent prompt rather
//! than replacing it, so the child is not prompted identically to the API
//! backend even though both send the same instructions. `--safe-mode` is what
//! keeps the difference to the CLI's built-in prompt rather than the built-in
//! prompt plus whatever the user has configured.
//!
//! Everything except [`run`] is a pure function unit-tested below. No test
//! spawns the real CLI.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use anyhow::{Context, Result, anyhow, bail};

use super::{EnrichmentProvider, EnrichmentRequest, EnrichmentResponse, ask, prompt};

/// The one program this backend will run. Not configurable: a general
/// `cli:<program>` spec would make "CodeAtlas executes whatever you name it"
/// a true sentence, which is a far worse claim to defend than "CodeAtlas can
/// invoke `claude`" (ADR-0008).
pub const PROGRAM: &str = "claude";

/// The provider spec that selects this backend.
pub const SPEC: &str = "cli:claude";

/// Environment variables the child keeps. Everything else is dropped, so
/// `docs/SECURITY.md` can state what the subprocess receives as a list rather
/// than as "whatever this process had".
///
/// `HOME` and `XDG_CONFIG_HOME` are how the CLI finds the credentials that
/// are the entire point; `PATH` is how the OS finds the CLI. `ANTHROPIC_API_KEY`
/// is conspicuously absent — see the module header.
const INHERITED_VARS: &[&str] = &["PATH", "HOME", "XDG_CONFIG_HOME", "XDG_DATA_HOME"];

/// Every flag the child is given except the model, which is optional, and the
/// prompt, which is positional. `--flag=value` form throughout: several of
/// these are variadic in the CLI's parser, and in space-separated form a
/// variadic option consumes following arguments until the next option — which
/// would swallow the prompt.
const LOCKDOWN: &[&str] = &[
    "--print",
    "--output-format=json",
    // The CLI's own documented way to disable every built-in tool.
    "--tools=",
    // No CLAUDE.md, skills, plugins, hooks, MCP servers or custom agents.
    // Hooks are the one that matters: they run shell commands.
    "--safe-mode",
    "--strict-mcp-config",
    "--mcp-config={\"mcpServers\":{}}",
];

/// The CLI backend behind the [`EnrichmentProvider`] trait.
pub struct CliProvider {
    /// `None` leaves the model to the CLI's own default. Deliberately not
    /// pinned the way the API provider pins `claude-opus-5`: a subscription's
    /// entitlement varies, and naming a model the seat cannot use would turn
    /// a working setup into an error.
    model: Option<String>,
    /// The program to spawn. Always [`PROGRAM`] in a shipped binary — the
    /// only way to set anything else is the `test-provider`-gated spec that
    /// points at a fake executable (see [`super::provider_from_spec`]).
    program: String,
}

impl CliProvider {
    pub fn new(model: Option<&str>) -> Self {
        Self {
            model: model.map(str::to_string),
            program: PROGRAM.to_string(),
        }
    }

    /// Points the backend at a stand-in executable so seam 3 can assert what
    /// the child was invoked with. Compiled only for test builds, exactly as
    /// the `fake:` and `fail` backends are, so no shipped binary carries a
    /// way to run an arbitrary program.
    #[cfg(feature = "test-provider")]
    pub fn with_program(program: impl Into<String>, model: Option<&str>) -> Self {
        Self {
            model: model.map(str::to_string),
            program: program.into(),
        }
    }

    /// One locked-down, schema-constrained completion. Both trait methods go
    /// through here so the lockdown flags, the `--` fence and the envelope
    /// checks exist once: a second copy of the argv construction is a second
    /// place for the swallowed-prompt bug to come back, and it would come
    /// back silently.
    fn complete(&self, completion: &prompt::Completion) -> Result<serde_json::Value> {
        // A fresh empty directory per call, removed when `scratch` drops —
        // the child's whole view of the filesystem.
        let scratch = ScratchDir::new()?;
        let args = build_args(completion, self.model.as_deref());
        let output = run(&self.program, &args, scratch.path())?;
        structured_output(&self.program, &output)
    }
}

impl EnrichmentProvider for CliProvider {
    fn enrich(&self, request: &EnrichmentRequest) -> Result<EnrichmentResponse> {
        prompt::parse_answers(self.complete(&prompt::for_enrichment(request))?)
    }

    fn ask(&self, question: &ask::Question) -> Result<ask::Answer> {
        prompt::parse_ask_answer(self.complete(&prompt::for_question(question))?)
    }
}

/// An empty directory that exists for one call and removes itself.
///
/// Hand-rolled rather than pulled from `tempfile`, which this crate has only
/// as a dev-dependency: `agent-cli` must not widen the dependency tree a
/// security review reads (ADR-0006), and "make an empty directory, delete it
/// after" does not need a crate.
struct ScratchDir {
    path: PathBuf,
}

impl ScratchDir {
    fn new() -> Result<Self> {
        let base = std::env::temp_dir();
        let pid = std::process::id();
        // `create_dir` fails rather than succeeding on an existing path, so
        // the loop is a race-free claim on a name rather than a check
        // followed by a create.
        for attempt in 0..64 {
            let path = base.join(format!("codeatlas-cli-{pid}-{attempt}"));
            let mut builder = std::fs::DirBuilder::new();
            // 0700, because the system temp directory is world-writable and
            // the default 0755 would let any local user read whatever the
            // child leaves in its working directory.
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            match builder.create(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(err) => {
                    return Err(err).with_context(|| {
                        format!("could not create a working directory at {}", path.display())
                    });
                }
            }
        }
        bail!(
            "could not create a working directory under {}: 64 candidate \
             names were already taken",
            base.display()
        )
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        // Best effort: a leftover empty directory in the system temp folder
        // is untidy, and failing an enrichment run over it would be worse.
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// The argv after the program name. Pure, so seam 3 can assert every part of
/// it without spawning anything.
///
/// Takes a [`prompt::Completion`] rather than an enrichment request:
/// enrichment and questions differ in what is asked and in nothing about how
/// the child is run, so the lockdown below applies to both by construction.
fn build_args(completion: &prompt::Completion, model: Option<&str>) -> Vec<String> {
    let mut args: Vec<String> = LOCKDOWN.iter().map(|flag| (*flag).to_string()).collect();
    args.push(format!("--json-schema={}", completion.schema));
    args.push(format!(
        "--append-system-prompt={}",
        completion.system_prompt
    ));
    if let Some(model) = model {
        args.push(format!("--model={model}"));
    }
    // `--` ends option parsing. Without it the prompt is just another
    // argument, and a preceding variadic option would take it as one more of
    // its own values — leaving the model with no prompt at all and this
    // backend with no way to notice.
    args.push("--".to_string());
    args.push(completion.user_message.clone());
    args
}

/// The variables the child is given, resolved from this process. Absent
/// variables are simply not passed on.
fn child_env() -> Vec<(String, String)> {
    INHERITED_VARS
        .iter()
        .filter_map(|name| {
            std::env::var(name)
                .ok()
                .map(|value| ((*name).into(), value))
        })
        .collect()
}

/// The single subprocess spawn (ADR-0008). Deliberately a thin shim —
/// everything worth testing is pure. `env_clear` first, so the allowlist is
/// the whole environment rather than an addition to it.
fn run(program: &str, args: &[String], cwd: &Path) -> Result<Output> {
    Command::new(program)
        .args(args.iter().map(OsStr::new))
        .current_dir(cwd)
        .env_clear()
        .envs(child_env())
        .output()
        .map_err(|err| {
            anyhow!(
                "could not run `{program}`: {err}. The {SPEC} backend needs \
                 the Claude CLI on PATH and logged in (`claude` then \
                 `/login`)"
            )
        })
}

/// The `--output-format json` envelope, reduced to what this backend reads.
/// A completed run is `{"type":"result","subtype":"success",...}` and carries
/// the schema-constrained answer in `structured_output`.
#[derive(serde::Deserialize)]
struct CliResult {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    subtype: Option<String>,
    #[serde(default)]
    is_error: bool,
    #[serde(default)]
    structured_output: Option<serde_json::Value>,
    /// The plain-text result, read only to quote back what went wrong.
    #[serde(default)]
    result: Option<String>,
}

/// The schema-constrained payload inside a finished process. Like the API
/// backend there is no repair path: a non-zero exit, an error envelope, or a
/// missing `structured_output` is an ordinary provider error, which leaves
/// the structural map intact (spec story 14).
///
/// Shared by both request kinds, which differ in the schema they demanded,
/// not in how the CLI reports having satisfied it.
fn structured_output(program: &str, output: &Output) -> Result<serde_json::Value> {
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "`{program}` exited with {}: {}",
            output.status,
            first_line(stderr.trim()).unwrap_or_else(|| "no diagnostic on stderr".to_string()),
        );
    }
    let result: CliResult = serde_json::from_str(stdout.trim())
        .with_context(|| format!("`{program}` did not print a JSON result envelope"))?;
    if result.kind != "result" {
        bail!(
            "`{program}` printed a {:?} message rather than a result",
            result.kind
        );
    }
    if result.is_error || result.subtype.as_deref() != Some("success") {
        bail!(
            "`{program}` did not complete: {}",
            result
                .result
                .as_deref()
                .and_then(first_line)
                .unwrap_or_else(|| {
                    result
                        .subtype
                        .as_deref()
                        .unwrap_or("no reason given")
                        .to_string()
                }),
        );
    }
    result.structured_output.ok_or_else(|| {
        anyhow!(
            "`{program}` completed without structured output; the response \
             did not satisfy the requested schema"
        )
    })
}

/// Diagnostics from another program can be arbitrarily long; one line of it
/// is a clue, all of it is a wall. Bounded by characters as well as by lines,
/// because nothing obliges another program to emit newlines.
fn first_line(text: &str) -> Option<String> {
    const MOST: usize = 120;
    let line = text.lines().next()?.trim();
    if line.is_empty() {
        return None;
    }
    match line.char_indices().nth(MOST) {
        Some((cut, _)) => Some(format!("{}…", &line[..cut])),
        None => Some(line.to_string()),
    }
}

#[cfg(test)]
mod tests {
    //! Seam 3 — the process interface. These assert what the child would be
    //! invoked with and what this backend makes of what comes back; the
    //! spawn itself is exercised end to end in `tests/enrich.rs` against a
    //! fake executable. Nothing here runs the real CLI.

    use super::*;
    use crate::enrich::{EnrichmentSlot, SummarySlot};
    use crate::map::{NodeId, NodeKind};

    fn request() -> EnrichmentRequest {
        EnrichmentRequest {
            project: "demo".into(),
            slots: vec![EnrichmentSlot::NodeSummary(SummarySlot {
                node: NodeId::file("src/main.ts"),
                kind: NodeKind::File,
                name: "main.ts".into(),
                path: "src/main.ts".into(),
                mechanical_summary: "TypeScript file, 3 lines".into(),
            })],
        }
    }

    fn question() -> ask::Question {
        ask::Question {
            project: "demo".into(),
            text: "what runs first?".into(),
            context: vec![ask::NodeContext {
                id: "file:src/main.ts".into(),
                kind: NodeKind::File,
                name: "main.ts".into(),
                path: "src/main.ts".into(),
                summary: "TypeScript file, 3 lines".into(),
            }],
        }
    }

    fn enrich_args(model: Option<&str>) -> Vec<String> {
        build_args(&prompt::for_enrichment(&request()), model)
    }

    fn ask_args(model: Option<&str>) -> Vec<String> {
        build_args(&prompt::for_question(&question()), model)
    }

    fn args() -> Vec<String> {
        enrich_args(None)
    }

    /// Envelope handling plus the enrichment schema — the pairing the
    /// `enrich` method performs, so these tests read as they did before the
    /// envelope step was shared with the question path.
    fn parse_output(program: &str, output: &Output) -> Result<EnrichmentResponse> {
        prompt::parse_answers(structured_output(program, output)?)
    }

    /// The value of a `--flag=value` argument. Only the `=` form is
    /// recognised, deliberately: the space-separated form is what the prompt
    /// bug came from, so a test that accepted it would accept the bug back.
    fn value_of(args: &[String], flag: &str) -> Option<String> {
        let prefix = format!("{flag}=");
        args.iter()
            .find_map(|a| a.strip_prefix(&prefix))
            .map(str::to_string)
    }

    #[test]
    fn the_child_is_asked_for_one_schema_constrained_json_result() {
        let args = args();
        assert!(args.contains(&"--print".to_string()), "{args:?}");
        assert_eq!(value_of(&args, "--output-format").as_deref(), Some("json"));

        let schema = value_of(&args, "--json-schema").expect("a schema is passed");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&schema).unwrap(),
            prompt::answers_schema(),
            "the CLI must be constrained by the same schema as the API backend"
        );
    }

    /// The bug this exists to prevent is invisible and total: several of the
    /// CLI's options are variadic, and a variadic option in space-separated
    /// form swallows following arguments until the next option. A prompt
    /// passed as a bare trailing positional therefore becomes one more value
    /// of whatever flag precedes it, the model is asked nothing, and this
    /// backend has no way to tell.
    ///
    /// No fake-executable test can catch that — a stand-in shell script has
    /// no argument parser to be confused. What is checkable is the shape:
    /// every flag carries its own value, and `--` ends option parsing before
    /// the prompt.
    #[test]
    fn no_flag_can_swallow_the_prompt() {
        // Both request kinds, because both go out as a trailing positional
        // and either would be swallowed alone.
        let expected = [
            (enrich_args(None), prompt::user_message(&request())),
            (
                enrich_args(Some("claude-sonnet-5")),
                prompt::user_message(&request()),
            ),
            (ask_args(None), prompt::ask_user_message(&question())),
            (
                ask_args(Some("claude-sonnet-5")),
                prompt::ask_user_message(&question()),
            ),
        ];
        for (args, message) in expected {
            let (prompt_arg, rest) = args.split_last().expect("there are arguments");
            assert_eq!(
                rest.last().map(String::as_str),
                Some("--"),
                "the prompt must be fenced off from option parsing: {args:?}"
            );
            assert_eq!(prompt_arg, &message);

            // Every flag before the fence carries its value itself. A bare
            // `--flag value` pair is the shape that eats the next argument.
            for arg in rest {
                assert!(
                    arg.starts_with("--"),
                    "{arg:?} is a loose value; it belongs in --flag=value form: \
                     {args:?}"
                );
            }
        }
    }

    #[test]
    fn the_prompt_carries_the_slots_and_nothing_from_the_repository() {
        let args = args();
        let prompt_arg = args.last().expect("the prompt is the final argument");

        assert!(
            prompt_arg.contains("demo"),
            "the project name: {prompt_arg}"
        );
        assert!(
            prompt_arg.contains("summary:file:src/main.ts"),
            "the slot key: {prompt_arg}"
        );
        assert!(
            prompt_arg.contains("TypeScript file, 3 lines"),
            "the mechanical summary it would replace: {prompt_arg}"
        );
        // The prompt is built by `prompt::user_message`, the same function the
        // API backend uses, so what a model receives is stated once.
        assert_eq!(prompt_arg, &prompt::user_message(&request()));
    }

    #[test]
    fn the_child_gets_no_tools_no_mcp_servers_and_no_extra_directory() {
        let args = args();

        assert!(
            args.contains(&"--strict-mcp-config".to_string()),
            "nothing configured elsewhere may load: {args:?}"
        );
        assert_eq!(
            value_of(&args, "--mcp-config").as_deref(),
            Some("{\"mcpServers\":{}}"),
            "the MCP config must be empty, not merely strict"
        );

        // The CLI's documented way to disable every built-in tool. An
        // enumerated deny-list was the first attempt and stops covering a
        // tool the day a new one is added.
        assert_eq!(
            value_of(&args, "--tools").as_deref(),
            Some(""),
            "every built-in tool must be disabled: {args:?}"
        );
        // Hooks run shell commands, and HOME is on the environment
        // allowlist, so without this the reader's own hooks fire on every
        // enrichment call.
        assert!(
            args.contains(&"--safe-mode".to_string()),
            "no CLAUDE.md, skills, plugins or hooks: {args:?}"
        );

        // Widening the child's view to the repository is the one thing that
        // would void the never-file-contents guarantee outright.
        assert!(
            !args.iter().any(|a| a == "--add-dir"),
            "the child's scope must never be widened: {args:?}"
        );
        // `--bare` would force ANTHROPIC_API_KEY authentication, defeating the
        // entire purpose of this backend.
        assert!(
            !args.iter().any(|a| a == "--bare"),
            "--bare skips keychain reads and demands an API key: {args:?}"
        );
    }

    /// The lockdown is a property of the child, not of what it is asked.
    /// A question answered by a `claude` process with its tools enabled
    /// could read the repository, which is precisely what ADR-0009's "from
    /// the map alone" cannot survive — and questions arrive from a network
    /// route, unlike enrichment.
    #[test]
    fn a_question_is_locked_down_exactly_as_an_enrichment_call_is() {
        let args = ask_args(None);

        for flag in LOCKDOWN {
            assert!(
                args.contains(&(*flag).to_string()),
                "the question path dropped {flag}: {args:?}"
            );
        }
        assert!(!args.iter().any(|a| a == "--add-dir"), "{args:?}");

        let schema = value_of(&args, "--json-schema").expect("a schema is passed");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&schema).unwrap(),
            prompt::ask_answer_schema(),
            "a question must be constrained by the answer schema, not the \
             enrichment one"
        );
        assert_eq!(
            value_of(&args, "--append-system-prompt").as_deref(),
            Some(prompt::ASK_SYSTEM_PROMPT),
        );
    }

    #[test]
    fn the_model_is_the_cli_s_own_unless_one_is_asked_for() {
        // Asserted on the provider as well as on the argv, because the two
        // are separate places a default could creep in — and the API backend
        // next door does pin `claude-opus-5`, which makes copying it the
        // obvious mistake. A subscription's entitlement varies, so naming a
        // model the seat cannot use turns a working setup into an error.
        assert!(
            CliProvider::new(None).model.is_none(),
            "the provider must not invent a model when none was asked for"
        );
        assert!(
            !args().iter().any(|a| a.starts_with("--model")),
            "an unasked-for model must not reach the child"
        );

        assert_eq!(
            CliProvider::new(Some("claude-sonnet-5")).model.as_deref(),
            Some("claude-sonnet-5")
        );
        let chosen = enrich_args(Some("claude-sonnet-5"));
        assert_eq!(
            value_of(&chosen, "--model").as_deref(),
            Some("claude-sonnet-5")
        );
    }

    #[test]
    fn the_api_key_is_never_handed_to_the_child() {
        // The allowlist is what the child gets; the assertion that matters is
        // about what it does not.
        assert!(
            !INHERITED_VARS.contains(&"ANTHROPIC_API_KEY"),
            "cli:claude must mean the CLI's own credential, not silent API \
             billing through a subprocess"
        );
        for name in child_env().iter().map(|(name, _)| name.as_str()) {
            assert!(
                INHERITED_VARS.contains(&name),
                "{name} reached the child without being on the allowlist"
            );
        }
    }

    /// The child's whole view of the filesystem. It lives in the system temp
    /// directory, which is world-writable, so the default 0755 would let any
    /// local user read whatever the child leaves behind — and it must not
    /// outlive the call that made it.
    #[cfg(unix)]
    #[test]
    fn the_scratch_directory_is_private_and_temporary() {
        use std::os::unix::fs::PermissionsExt;

        let scratch = ScratchDir::new().unwrap();
        let path = scratch.path().to_path_buf();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700, "readable by other users: {mode:o}");

        drop(scratch);
        assert!(!path.exists(), "the directory outlived the call: {path:?}");
    }

    fn output(code: i32, stdout: &str, stderr: &str) -> Output {
        use std::os::unix::process::ExitStatusExt;
        Output {
            status: std::process::ExitStatus::from_raw(code << 8),
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.as_bytes().to_vec(),
        }
    }

    fn success_envelope(answers: &str) -> String {
        format!(
            r#"{{"type":"result","subtype":"success","is_error":false,
                "structured_output":{answers},"total_cost_usd":0.01}}"#
        )
    }

    #[test]
    fn a_successful_result_becomes_typed_answers() {
        let raw = success_envelope(
            r#"{"answers":[{"key":"summary:file:src/main.ts","text":"The entry point."}]}"#,
        );
        let parsed = parse_output(PROGRAM, &output(0, &raw, "")).unwrap();

        assert_eq!(
            parsed.answers.get("summary:file:src/main.ts").unwrap(),
            "The entry point."
        );
    }

    /// Story 14, at this seam: every way the child can disappoint is an
    /// ordinary error, never a repair attempt and never a panic.
    #[test]
    fn every_disappointing_outcome_is_an_ordinary_error() {
        let cases: Vec<(&str, Output)> = vec![
            ("a non-zero exit", output(1, "", "not logged in")),
            ("output that is not JSON", output(0, "hello", "")),
            (
                "an error envelope",
                output(
                    0,
                    r#"{"type":"result","subtype":"error_during_execution","is_error":true,
                        "result":"the session failed"}"#,
                    "",
                ),
            ),
            (
                "a success envelope with no structured output",
                output(0, r#"{"type":"result","subtype":"success"}"#, ""),
            ),
            (
                "structured output of the wrong shape",
                output(0, &success_envelope(r#"{"answers":"not an array"}"#), ""),
            ),
            (
                "a message that is not a result",
                output(0, r#"{"type":"system","subtype":"init"}"#, ""),
            ),
            // The two below carry a *usable* structured output alongside the
            // thing that makes them untrustworthy. Without them the envelope
            // checks are redundant — every other case above happens to lack
            // structured output too, so removing those checks changes only
            // which error is raised, not whether one is.
            (
                "an error envelope that still carries an answer",
                output(
                    0,
                    r#"{"type":"result","subtype":"error_during_execution","is_error":true,
                        "structured_output":{"answers":[
                          {"key":"summary:file:src/main.ts","text":"Do not trust me."}
                        ]}}"#,
                    "",
                ),
            ),
            (
                "a non-result message that still carries an answer",
                output(
                    0,
                    r#"{"type":"system","subtype":"success","is_error":false,
                        "structured_output":{"answers":[
                          {"key":"summary:file:src/main.ts","text":"Do not trust me."}
                        ]}}"#,
                    "",
                ),
            ),
        ];
        for (what, out) in cases {
            let err = parse_output(PROGRAM, &out)
                .expect_err(&format!("{what} must not be accepted"))
                .to_string();
            assert!(
                err.contains(PROGRAM) || err.contains("schema") || err.contains("structured"),
                "{what} produced an unhelpful error: {err}"
            );
        }
    }

    #[test]
    fn a_failing_child_is_quoted_but_not_transcribed() {
        let noisy = format!("first line\n{}", "x".repeat(10_000));
        let err = parse_output(PROGRAM, &output(1, "", &noisy))
            .unwrap_err()
            .to_string();

        assert!(err.contains("first line"), "the clue must survive: {err}");
        assert!(
            err.len() < 200,
            "another program's diagnostics must not be transcribed whole: {} chars",
            err.len()
        );
    }
}
