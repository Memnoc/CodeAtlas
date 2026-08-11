//! Helpers shared by the integration tests: the fixture scaffolding every
//! suite starts from, and the policy pieces that enforce ADR-0006's egress
//! guarantee and ADR-0008's subprocess lockdown.
//!
//! Both the URL allowlist and the stand-in executable are policy, not
//! convenience, so they live in one place: two copies of a security check
//! drifting apart is how one quietly stops agreeing with the other. The
//! stand-in in particular is now used from two directions — `scan --enrich`
//! and `serve --ask` — and the whole point is that both spawn the same
//! locked-down child.
//!
//! The fixture scaffolding is here for a duller reason: [`materialize`] had
//! six byte-identical copies, and the `_gitignore` rename inside it is the
//! kind of shared convention that stops being one the moment a copy is
//! updated alone. What stays in each suite is anything a reader has to see
//! next to the assertion to know the assertion can fail — a `plain_scan` that
//! clears the provider environment, say, whose whole point is visible only
//! where it is used.

// Each integration-test binary compiles its own copy of this module, so a
// helper only one of them needs looks dead to the others.
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// A committed fixture tree, by name, under `tests/fixtures`.
pub fn fixture_dir(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// Recursive copy, used to get a fixture out of the source tree before a test
/// writes into it.
pub fn copy_tree(from: &Path, to: &Path) {
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
pub fn materialize(name: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    copy_tree(&fixture_dir(name), dir.path());
    let inert = dir.path().join("_gitignore");
    if inert.exists() {
        fs::rename(inert, dir.path().join(".gitignore")).unwrap();
    }
    dir
}

/// Runs git in `repo` and insists it succeeded, quoting stderr if not.
pub fn git(repo: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Turns a materialized fixture into a git repository. Identity and signing
/// are configured locally in the tempdir, so the machine's global git config
/// can never affect a test.
pub fn git_init(repo: &Path) {
    git(repo, &["init", "-q"]);
    git(repo, &["config", "user.email", "fixture@example.com"]);
    git(repo, &["config", "user.name", "Fixture"]);
    git(repo, &["config", "commit.gpgsign", "false"]);
}

/// Parses a JSON file, naming it if it cannot be read or parsed.
pub fn read_json(path: &Path) -> serde_json::Value {
    let raw =
        fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("{} is not valid JSON: {e}", path.display()))
}

/// The map a scan wrote under `repo`.
pub fn read_map(repo: &Path) -> serde_json::Value {
    read_json(&repo.join(".codeatlas/knowledge-graph.json"))
}

/// The map's node with this id, or a panic naming it.
pub fn node<'m>(map: &'m serde_json::Value, id: &str) -> &'m serde_json::Value {
    map["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == id)
        .unwrap_or_else(|| panic!("node {id} missing from the map"))
}

/// Writes a canned-responses file (slot key → text; keys are prefixed by slot
/// kind: `summary:<node-id>`, `layer-name:<layer-id>`, `flow-name:<flow-id>`,
/// `tour-label:<node-id>`) OUTSIDE the scanned repo and returns the `fake:`
/// provider spec selecting it.
pub fn canned_provider(dir: &Path, answers: &[(&str, &str)]) -> String {
    let map: serde_json::Map<String, serde_json::Value> = answers
        .iter()
        .map(|(id, text)| (id.to_string(), serde_json::Value::from(*text)))
        .collect();
    let path = dir.join("canned.json");
    fs::write(&path, serde_json::to_string_pretty(&map).unwrap()).unwrap();
    format!("fake:{}", path.display())
}

/// Writes an executable stand-in for the Claude CLI. It records the working
/// directory, selected environment variables and its whole argv to
/// `<dir>/record.txt`, then prints `envelope` on stdout and exits with
/// `code`. The record path is baked in rather than passed through the
/// environment, because the environment is exactly what is under test.
///
/// Newlines inside an argument become carriage returns before recording, so
/// one argument is always one line. Without that, a multi-line prompt is
/// filed as one `arg=` line plus untagged continuation lines, and every
/// assertion about the prompt silently sees only its first line — which is
/// how a prompt body can look checked while nothing checks it.
///
/// Returns the `cli-exec:` provider spec that selects it — an injection
/// point compiled in only under `test-provider`, so no shipped binary gains
/// a way to run an arbitrary program.
pub fn fake_cli(dir: &Path, envelope: &str, code: i32) -> String {
    fs::create_dir_all(dir).unwrap();
    let record = dir.join("record.txt");
    let program = dir.join("fake-claude");
    let script = format!(
        r#"#!/bin/sh
{{
  printf 'cwd=%s\n' "$(pwd)"
  printf 'api-key=%s\n' "${{ANTHROPIC_API_KEY-<unset>}}"
  printf 'secret=%s\n' "${{CODEATLAS_TEST_SECRET-<unset>}}"
  printf 'home=%s\n' "${{HOME-<unset>}}"
  for a in "$@"; do printf 'arg=%s\n' "$(printf '%s' "$a" | tr '\n' '\r')"; done
}} > '{record}'
cat <<'CODEATLAS_ENVELOPE'
{envelope}
CODEATLAS_ENVELOPE
exit {code}
"#,
        record = record.display(),
    );
    fs::write(&program, script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).unwrap();
    }
    format!("cli-exec:{}", program.display())
}

/// What [`fake_cli`]'s stand-in recorded about the call it received.
pub fn record_lines(dir: &Path) -> Vec<String> {
    fs::read_to_string(dir.join("record.txt"))
        .expect("the stand-in CLI recorded nothing — it was never run")
        .lines()
        .map(str::to_string)
        .collect()
}

/// The arguments the stand-in was invoked with, in order.
pub fn recorded_args(dir: &Path) -> Vec<String> {
    record_lines(dir)
        .into_iter()
        .filter_map(|line| line.strip_prefix("arg=").map(str::to_string))
        .collect()
}

/// Every `http(s)` URL in `text`, delimited the way a URL ends in source or
/// markup.
pub fn urls_in(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    for scheme in ["http://", "https://"] {
        for (pos, _) in text.match_indices(scheme) {
            found.push(
                text[pos..]
                    .chars()
                    .take_while(|c| !c.is_whitespace() && !"\"'`\\)<>".contains(*c))
                    .collect(),
            );
        }
    }
    found
}

/// URLs that are string literals by construction, never requests — the same
/// allowlist the dashboard's zero-egress test documents: XML namespace
/// identifiers, React's minified-error text, and React Flow's doc links
/// including its attribution `<a href>` (kept deliberately: a plain anchor
/// performs no request until the reader chooses to click it).
pub fn is_inert(url: &str) -> bool {
    url.starts_with("http://www.w3.org/")
        || url.starts_with("https://www.w3.org/")
        || url.starts_with("https://react.dev/errors/")
        || url.starts_with("https://reactflow.dev")
        || (url.starts_with("https://${") && url.contains("flow.dev"))
}

/// The URLs in `text` that would be real egress — everything [`urls_in`]
/// finds that [`is_inert`] does not excuse.
pub fn external_urls(text: &str) -> Vec<String> {
    urls_in(text)
        .into_iter()
        .filter(|url| !is_inert(url))
        .collect()
}
