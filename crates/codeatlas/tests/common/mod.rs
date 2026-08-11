//! Helpers shared by the integration tests that enforce ADR-0006's egress
//! guarantee and ADR-0008's subprocess lockdown.
//!
//! Both the URL allowlist and the stand-in executable are policy, not
//! convenience, so they live in one place: two copies of a security check
//! drifting apart is how one quietly stops agreeing with the other. The
//! stand-in in particular is now used from two directions — `scan --enrich`
//! and `serve --ask` — and the whole point is that both spawn the same
//! locked-down child.

// Each integration-test binary compiles its own copy of this module, so a
// helper only one of them needs looks dead to the others.
#![allow(dead_code)]

use std::fs;
use std::path::Path;

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
