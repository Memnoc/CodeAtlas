//! Egress tests (ticket 15, ADR-0006): the default path — scan, diff,
//! share, serve — must open no non-loopback sockets. The mechanism is a
//! network namespace: each command runs under `unshare -r -n` (an
//! unprivileged user namespace plus a fresh netns whose only interface is
//! an initially-down loopback). Inside that namespace no route to anywhere
//! but 127.0.0.1 exists, so a command that *succeeds* there has proven it
//! needs no egress — stronger than snapshotting /proc/net, which can miss
//! short-lived sockets.
//!
//! The serve test additionally speaks HTTP to the server over 127.0.0.1
//! *inside* the namespace and asserts the printed URL is loopback: the
//! listener works with no network in existence beyond lo.
//!
//! Two counter-tests pin the other side of the surface. Pointed at the real
//! Claude provider with dummy credentials inside the namespace, `scan
//! --enrich` must FAIL (no route out) while leaving the structural map intact
//! (spec story 14); and `serve --ask` — the second egress route, added by
//! ADR-0009 — must answer its question route 502 while the same server, in
//! the same namespace, still answers `/api/map` 200. Together with the tests
//! above that is spec story 9's sentence as an executable claim: the two ways
//! to reach a model are `scan --enrich` and `serve --ask`, and nothing else
//! here needs a route off the host.
//!
//! # Honest skip
//!
//! `unshare -r -n` requires unprivileged user namespaces (standard on
//! desktop Linux and GitHub's ubuntu-latest runners, which these tests
//! assume; often disabled inside containers/sandboxes). When unavailable,
//! every test here skips with a message instead of failing — the CI run on
//! ubuntu-latest is the enforcing environment. Non-Linux platforms compile
//! this file to nothing.
#![cfg(target_os = "linux")]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn fixture_dir(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn copy_tree(from: &Path, to: &Path) {
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
fn materialize(name: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    copy_tree(&fixture_dir(name), dir.path());
    let inert = dir.path().join("_gitignore");
    if inert.exists() {
        fs::rename(inert, dir.path().join(".gitignore")).unwrap();
    }
    dir
}

/// True when the test cannot run here: prints the honest skip message.
/// Callers `return` on true, so the test passes without asserting anything —
/// CI on ubuntu-latest (where user namespaces work) is the enforcing run.
fn netns_unavailable() -> bool {
    let works = Command::new("unshare")
        .args(["-r", "-n", "true"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !works {
        eprintln!(
            "SKIPPED: `unshare -r -n` does not work here (unprivileged user \
             namespaces disabled — common in containers), so the netns egress \
             assertion did NOT run; it is enforced by CI on ubuntu-latest"
        );
    }
    !works
}

/// Runs the built codeatlas binary inside a fresh network namespace whose
/// only interface is a down loopback. Success inside proves zero egress.
fn in_netns(args: &[&str], cwd: &Path, envs: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new("unshare");
    cmd.args(["-r", "-n", "--", env!("CARGO_BIN_EXE_codeatlas")])
        .args(args)
        .current_dir(cwd);
    for (key, value) in envs {
        cmd.env(key, value);
    }
    cmd.output().expect("unshare runs")
}

fn assert_success(output: &Output, what: &str) {
    assert!(
        output.status.success(),
        "{what} failed inside the network namespace (does it need egress?): {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn git(repo: &Path, args: &[&str]) {
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

#[test]
fn scan_succeeds_with_no_network_beyond_loopback() {
    if netns_unavailable() {
        return;
    }
    let repo = materialize("simple");
    let output = in_netns(&["scan", "."], repo.path(), &[]);
    assert_success(&output, "codeatlas scan");
    assert!(
        repo.path().join(".codeatlas/knowledge-graph.json").exists(),
        "scan wrote no map"
    );
}

#[test]
fn diff_succeeds_with_no_network_beyond_loopback() {
    if netns_unavailable() {
        return;
    }
    let repo = materialize("diffrepo");
    git(repo.path(), &["init", "-q"]);
    git(
        repo.path(),
        &["config", "user.email", "fixture@example.com"],
    );
    git(repo.path(), &["config", "user.name", "Fixture"]);
    git(repo.path(), &["config", "commit.gpgsign", "false"]);
    git(repo.path(), &["add", "-A"]);
    git(repo.path(), &["commit", "-q", "-m", "fixture baseline"]);
    let output = in_netns(&["scan", "."], repo.path(), &[]);
    assert_success(&output, "codeatlas scan");
    fs::write(
        repo.path().join("src/util.ts"),
        "export function greet(name: string): string {\n  return `hi ${name}`;\n}\n",
    )
    .unwrap();

    let output = in_netns(&["diff", "."], repo.path(), &[]);
    assert_success(&output, "codeatlas diff");
    assert!(
        repo.path().join(".codeatlas/diff-overlay.json").exists(),
        "diff wrote no overlay"
    );
}

#[test]
fn share_succeeds_with_no_network_beyond_loopback() {
    if netns_unavailable() {
        return;
    }
    let repo = materialize("simple");
    let output = in_netns(&["scan", "."], repo.path(), &[]);
    assert_success(&output, "codeatlas scan");

    let output = in_netns(&["share", "."], repo.path(), &[]);
    assert_success(&output, "codeatlas share");
    assert!(
        repo.path().join(".codeatlas/share.html").exists(),
        "share wrote no artifact"
    );
}

/// The serve script run inside the namespace: bring loopback up (the one
/// piece of network the namespace allows), start the server, and speak
/// HTTP/1.1 to it over bash's /dev/tcp — no curl dependency. Asserts the
/// printed URL is loopback and /api/map answers 200 with the map.
const SERVE_SCRIPT: &str = r#"
set -eu
ip_bin=$(command -v ip || true)
[ -n "$ip_bin" ] || ip_bin=/usr/sbin/ip
"$ip_bin" link set lo up
"$BIN" serve --port 0 . >"$OUT" 2>/dev/null &
pid=$!
url=""
for _ in $(seq 1 200); do
  url=$(grep -o 'http://[0-9.:]*' "$OUT" | head -n1 || true)
  [ -n "$url" ] && break
  sleep 0.05
done
[ -n "$url" ] || { echo "serve printed no URL" >&2; kill "$pid"; exit 1; }
case "$url" in
  http://127.0.0.1:*) ;;
  *) echo "serve bound a non-loopback address: $url" >&2; kill "$pid"; exit 1 ;;
esac
port=${url##*:}
port=${port%%/*}
exec 3<>"/dev/tcp/127.0.0.1/$port"
printf 'GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n' >&3
index=$(cat <&3)
exec 3<&-
case "$index" in
  *"200 OK"*"<html"*) ;;
  *) echo "unexpected / response: $index" >&2; kill "$pid"; exit 1 ;;
esac
exec 3<>"/dev/tcp/127.0.0.1/$port"
printf 'GET /api/map HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n' >&3
resp=$(cat <&3)
kill "$pid" 2>/dev/null || true
case "$resp" in
  *"200 OK"*'"nodes"'*) exit 0 ;;
  *) echo "unexpected /api/map response: $resp" >&2; exit 1 ;;
esac
"#;

#[test]
fn serve_binds_loopback_and_answers_with_no_network_beyond_loopback() {
    if netns_unavailable() {
        return;
    }
    let repo = materialize("simple");
    // The map can exist before the namespace does; serve is what is under
    // egress observation here (scan has its own netns test).
    assert_cmd::Command::cargo_bin("codeatlas")
        .unwrap()
        .arg("scan")
        .current_dir(repo.path())
        .assert()
        .success();

    let out_file = repo.path().join("serve-stdout.txt");
    let output = Command::new("unshare")
        .args(["-r", "-n", "--", "bash", "-c", SERVE_SCRIPT])
        .current_dir(repo.path())
        .env("BIN", env!("CARGO_BIN_EXE_codeatlas"))
        .env("OUT", &out_file)
        .output()
        .expect("unshare runs");
    assert!(
        output.status.success(),
        "serve-in-netns script failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// The `serve --ask` script: the same namespace, the same server, two
/// requests. `POST /api/ask` must fail with no route off the host, and
/// `GET /api/map` immediately afterwards must succeed — the live control
/// without which a 502 would be evidence of nothing (a server that never
/// started, a port never bound, a namespace that broke loopback would all
/// produce one just as readily). The control also doubles as story 14's rule
/// for a route: a backend that cannot answer must not take the server down.
///
/// Gated with its test: the only backend safe to point at here is the API
/// one, so a build without `network` has nothing to run this against.
#[cfg(feature = "network")]
const SERVE_ASK_SCRIPT: &str = r#"
set -eu
ip_bin=$(command -v ip || true)
[ -n "$ip_bin" ] || ip_bin=/usr/sbin/ip
"$ip_bin" link set lo up
"$BIN" serve --port 0 --ask . >"$OUT" 2>/dev/null &
pid=$!
url=""
for _ in $(seq 1 200); do
  url=$(grep -o 'http://[0-9.:]*' "$OUT" | head -n1 || true)
  [ -n "$url" ] && break
  sleep 0.05
done
[ -n "$url" ] || { echo "serve --ask printed no URL" >&2; kill "$pid" 2>/dev/null; exit 1; }
port=${url##*:}
port=${port%%/*}

body='{"question":"where does the program start?"}'
exec 3<>"/dev/tcp/127.0.0.1/$port"
printf 'POST /api/ask HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: %s\r\nConnection: close\r\n\r\n%s' "${#body}" "$body" >&3
ask=$(cat <&3)
exec 3<&-
case "$ask" in
  *"200 OK"*)
    echo "POST /api/ask answered 200 with no route off the host: $ask" >&2
    kill "$pid" 2>/dev/null; exit 1 ;;
  *"502 Bad Gateway"*) ;;
  *) echo "unexpected /api/ask response: $ask" >&2; kill "$pid" 2>/dev/null; exit 1 ;;
esac

exec 3<>"/dev/tcp/127.0.0.1/$port"
printf 'GET /api/map HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n' >&3
resp=$(cat <&3)
exec 3<&-
kill "$pid" 2>/dev/null || true
case "$resp" in
  *"200 OK"*'"nodes"'*) exit 0 ;;
  *) echo "the control failed: /api/map did not answer inside the namespace, so the 502 above proves nothing: $resp" >&2; exit 1 ;;
esac
"#;

/// `serve --ask` is the second egress-capable command (ADR-0009), and story
/// 9's sentence names it beside `--enrich`. Inside the namespace the question
/// route must fail for want of a route out, while the plain `serve` test
/// above keeps succeeding in the same conditions — the two together are what
/// make "reachable only from `scan --enrich` and `serve --ask`" a tested
/// claim rather than a described one.
///
/// Network builds only, and deliberately never `cli:claude`: that spec spawns
/// the reader's real Claude CLI, which is their credential and their
/// subscription. The API backend with a dummy key reaches the same verdict
/// with nothing at stake.
#[cfg(feature = "network")]
#[test]
fn serve_ask_needs_egress_and_says_so_without_taking_the_server_down() {
    if netns_unavailable() {
        return;
    }
    let repo = materialize("simple");
    assert_cmd::Command::cargo_bin("codeatlas")
        .unwrap()
        .arg("scan")
        .current_dir(repo.path())
        .assert()
        .success();

    let out_file = repo.path().join("serve-ask-stdout.txt");
    let output = Command::new("unshare")
        .args(["-r", "-n", "--", "bash", "-c", SERVE_ASK_SCRIPT])
        .current_dir(repo.path())
        .env("BIN", env!("CARGO_BIN_EXE_codeatlas"))
        .env("OUT", &out_file)
        // The test binary carries test-provider, which removes the default
        // provider; name the real Claude provider explicitly, exactly as the
        // `--enrich` counter-test below does.
        .env("CODEATLAS_ENRICH_PROVIDER", "claude")
        .env("ANTHROPIC_API_KEY", "dummy-key-for-egress-test")
        .output()
        .expect("unshare runs");
    assert!(
        output.status.success(),
        "serve --ask-in-netns script failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// The first counter-test: `--enrich` against the real Claude provider DOES
/// need egress, so inside the namespace it must fail — and degrade to an
/// intact structural map (spec story 14).
///
/// Named for what it asserts rather than for "the only path", which it was
/// called until ADR-0009 gave `serve --ask` the same property and made the
/// name false. Which paths need egress is pinned by this test and the
/// `serve --ask` one *together* with the four that succeed in here.
/// Network builds only: sealed builds have no `claude` provider to name.
#[cfg(feature = "network")]
#[test]
fn enrich_needs_egress_and_degrades_cleanly_without_it() {
    if netns_unavailable() {
        return;
    }
    let repo = materialize("simple");
    let output = in_netns(
        &["scan", "--enrich", "."],
        repo.path(),
        &[
            // The test binary carries test-provider, which removes the
            // default provider; select the real Claude provider explicitly.
            ("CODEATLAS_ENRICH_PROVIDER", "claude"),
            ("ANTHROPIC_API_KEY", "dummy-key-for-egress-test"),
        ],
    );
    assert!(
        !output.status.success(),
        "--enrich succeeded with no route off the host — it cannot have \
         reached api.anthropic.com, so what did it do?"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("the structural map is intact"),
        "enrichment failure must reassure about the map: {stderr}"
    );
    assert!(
        repo.path().join(".codeatlas/knowledge-graph.json").exists(),
        "the structural map must survive the enrichment failure"
    );
}
