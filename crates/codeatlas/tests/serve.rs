//! End-to-end tests for `codeatlas serve`: scan a fixture, run the real
//! binary as a child process, speak HTTP/1.1 to it over 127.0.0.1, and
//! assert on what comes back. Never on internals.

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use assert_cmd::cargo::CommandCargoExt;
use predicates::prelude::PredicateBooleanExt;

mod common;

use common::materialize;

/// The provider-selection env var the test-built binary honors. Cleared on
/// every child, so a value in the developer's shell cannot decide what these
/// tests exercise.
const PROVIDER_ENV: &str = "CODEATLAS_ENRICH_PROVIDER";

fn scan(repo: &Path) {
    assert_cmd::Command::cargo_bin("codeatlas")
        .unwrap()
        .arg("scan")
        .current_dir(repo)
        .env_remove(PROVIDER_ENV)
        .assert()
        .success();
}

/// Writes a canned-answer file for the `fake:` backend and returns its path.
/// It lives outside the repository so it can never be scanned into the map
/// it is answering questions about.
///
/// No usage keys, deliberately: this scripts a backend that reports nothing,
/// which is what every pre-usage test already meant and what the absence
/// tests rely on. [`canned_reporting`] scripts the measuring backend.
fn canned(dir: &Path, answer: &str, citations: &[&str]) -> PathBuf {
    let path = dir.join("canned-ask.json");
    let body = serde_json::json!({
        "ask:answer": answer,
        "ask:citations": citations.join(" "),
    });
    fs::write(&path, serde_json::to_string(&body).unwrap()).unwrap();
    path
}

/// [`canned`], for a backend that also reports what the exchange spent
/// (ticket 09): the scripted token counts ride the `fake:` backend's
/// reserved usage keys.
fn canned_reporting(
    dir: &Path,
    answer: &str,
    citations: &[&str],
    input_tokens: u64,
    output_tokens: u64,
) -> PathBuf {
    let path = dir.join("canned-ask.json");
    let body = serde_json::json!({
        "ask:answer": answer,
        "ask:citations": citations.join(" "),
        "ask:input_tokens": input_tokens.to_string(),
        "ask:output_tokens": output_tokens.to_string(),
    });
    fs::write(&path, serde_json::to_string(&body).unwrap()).unwrap();
    path
}

/// A running `codeatlas serve` child, killed on drop so a failing assertion
/// never leaks a listener.
struct Server {
    child: Child,
    port: u16,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Starts `codeatlas serve --port 0` on the repo and returns once the child
/// has printed the URL it bound. `--port 0` asks the OS for a free port, so
/// parallel tests never collide.
fn serve(repo: &Path) -> Server {
    serve_with(repo, &[])
}

/// Starts the server with extra arguments — `--ask` and its backend.
fn serve_with(repo: &Path, extra: &[&str]) -> Server {
    let child = Command::cargo_bin("codeatlas")
        .unwrap()
        .args(["serve", "--port", "0"])
        .args(extra)
        .current_dir(repo)
        .env_remove(PROVIDER_ENV)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    adopt(child)
}

/// [`serve`], under a lowered file-descriptor budget: the shell's own
/// `ulimit` sets it — no library, no new machinery — and `exec` replaces
/// the shell, so the child PID whose `/proc` is measured is the server's
/// own. Starving the server is how a sustained accept-error condition is
/// forced on loopback: every accept past the budget fails with EMFILE, and
/// a failed accept leaves the connection queued, so the next accept fails
/// the same way for as long as the pressure holds.
fn serve_starved(repo: &Path, fd_budget: u32) -> Server {
    let child = Command::new("sh")
        .args([
            "-c",
            &format!("ulimit -n {fd_budget}; exec \"$0\" serve --port 0"),
        ])
        .arg(env!("CARGO_BIN_EXE_codeatlas"))
        .current_dir(repo)
        .env_remove(PROVIDER_ENV)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    adopt(child)
}

/// Reads the startup URL a spawned `serve` prints and wraps the child.
fn adopt(mut child: Child) -> Server {
    let stdout = child.stdout.take().unwrap();
    let mut line = String::new();
    BufReader::new(stdout).read_line(&mut line).unwrap();
    // The URL line is the startup contract: loopback host, actual port.
    let url_start = line
        .find("http://")
        .unwrap_or_else(|| panic!("serve printed no URL, got: {line:?}"));
    let url = line[url_start..].trim();
    let host_port = url.strip_prefix("http://").unwrap();
    let (host, rest) = host_port.split_once(':').unwrap();
    assert_eq!(host, "127.0.0.1", "serve must bind loopback only");
    let port: u16 = rest
        .trim_end_matches('/')
        .parse()
        .unwrap_or_else(|_| panic!("unparseable port in URL {url:?}"));
    Server { child, port }
}

/// Reads one response off a socket a request has already been written to:
/// everything up to EOF, split at the header/body separator into (status
/// line, headers, body). Every helper below is this plus a request, which is
/// the only reason a hand-rolled client is affordable — keeping it hand-rolled
/// adds no HTTP dependency and exercises the wire format directly.
///
/// The `Result` is the point rather than a formality: a response that never
/// arrives is what ticket 35 is about, so the caller decides whether a reset
/// is a failure or the thing under test.
fn read_response(stream: &mut TcpStream) -> std::io::Result<(String, Vec<String>, Vec<u8>)> {
    let mut response = Vec::new();
    stream.read_to_end(&mut response)?;
    let split = response
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(|| std::io::Error::other("no header/body separator in response"))?;
    let head = String::from_utf8_lossy(&response[..split]).to_string();
    let body = response[split + 4..].to_vec();
    let mut lines = head.lines().map(str::to_string);
    let status = lines.next().unwrap_or_default();
    Ok((status, lines.collect(), body))
}

/// Minimal HTTP/1.1 GET over a raw socket; returns (status line, headers,
/// body).
fn http_get(port: u16, path: &str) -> (String, Vec<String>, Vec<u8>) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    read_response(&mut stream).unwrap()
}

/// Minimal HTTP/1.1 HEAD over a raw socket — [`http_get`]'s twin, because
/// that is what story 19 says HEAD is.
fn http_head(port: u16, path: &str) -> (String, Vec<String>, Vec<u8>) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    write!(
        stream,
        "HEAD {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    read_response(&mut stream).unwrap()
}

/// Writes `bytes` verbatim to a fresh connection and reads one response —
/// for requests no honest helper here would form. The `Result` is the
/// point: the silent close story 19 removes surfaces here as an error.
fn raw_request(port: u16, bytes: &[u8]) -> std::io::Result<(String, Vec<String>, Vec<u8>)> {
    let mut stream = TcpStream::connect(("127.0.0.1", port))?;
    stream.write_all(bytes)?;
    stream.flush()?;
    read_response(&mut stream)
}

/// Minimal HTTP/1.1 POST over a raw socket. Hand-rolled for the same reason
/// [`http_get`] is, and because the body framing — a real `Content-Length`
/// with real bytes after it — is part of what this ticket added.
fn http_post(port: u16, path: &str, body: &str) -> (String, Vec<String>, Vec<u8>) {
    http_post_as(port, path, body, "application/json")
}

/// A POST with a chosen `Content-Type` — the header the question route uses
/// to refuse requests a browser could make cross-origin without a preflight.
fn http_post_as(
    port: u16,
    path: &str,
    body: &str,
    content_type: &str,
) -> (String, Vec<String>, Vec<u8>) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    write!(
        stream,
        "POST {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: {content_type}\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .unwrap();
    read_response(&mut stream).unwrap()
}

/// A body comfortably past what the server's first read can swallow.
/// `BufReader`'s default buffer is 8 KiB, so any body larger than that is
/// certain to be still in the socket when the header block has been read,
/// whatever the network does with the segments; four times over is margin
/// against that buffer size changing, not a threshold of its own.
const LARGER_THAN_ONE_READ: usize = 32 * 1024;

/// A POST that is still arriving when the server decides how to answer it,
/// returning a `Result` rather than unwrapping one.
///
/// A refusal is decided from the request line and the headers alone, so the
/// server answers with most of a [`LARGER_THAN_ONE_READ`] body still sitting
/// in the socket's receive queue. Closing a socket in that state closes it
/// with an RST rather than a FIN, and an RST tells the peer's kernel to
/// abandon what it has buffered — including the response written a moment
/// earlier. That is ticket 35's defect, and sizing the body past one buffered
/// read is what makes it certain rather than one run in twenty-five.
///
/// The error is returned rather than unwrapped because a reset *is* a result
/// under test here — the caller names the round it happened on.
fn http_post_still_arriving(
    port: u16,
    path: &str,
    content_type: &str,
    body: &str,
) -> std::io::Result<(String, Vec<u8>)> {
    assert!(
        body.len() > LARGER_THAN_ONE_READ,
        "this helper proves nothing with a body the server reads in one go"
    );
    let mut stream = TcpStream::connect(("127.0.0.1", port))?;
    write!(
        stream,
        "POST {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: {content_type}\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body.as_bytes())?;
    stream.flush()?;

    let (status, _, body) = read_response(&mut stream)?;
    Ok((status, body))
}

/// Asks a question and returns (status line, parsed JSON body).
fn ask(port: u16, question: &str) -> (String, serde_json::Value) {
    let body = serde_json::json!({ "question": question }).to_string();
    let (status, _, raw) = http_post(port, "/api/ask", &body);
    let parsed = serde_json::from_slice(&raw).unwrap_or_else(|e| {
        panic!(
            "ask response was not JSON ({e}): {:?}",
            String::from_utf8_lossy(&raw)
        )
    });
    (status, parsed)
}

/// Asks a question carrying previous turns (ADR-0012): the same POST with
/// the body's optional `turns` array populated.
fn ask_carrying(
    port: u16,
    question: &str,
    turns: serde_json::Value,
) -> (String, serde_json::Value) {
    let body = serde_json::json!({ "question": question, "turns": turns }).to_string();
    let (status, _, raw) = http_post(port, "/api/ask", &body);
    let parsed = serde_json::from_slice(&raw).unwrap_or_else(|e| {
        panic!(
            "ask response was not JSON ({e}): {:?}",
            String::from_utf8_lossy(&raw)
        )
    });
    (status, parsed)
}

/// A node in [`wide_repo`] whose only road into the slice is a carried
/// citation, which is what makes conversation state observable on the wire:
/// the fake backend's canned citation of it survives the server's
/// validation exactly when the carried turns put it in the slice.
const TARGET: &str = "file:src/zzz/target.ts";

/// A question matching nothing in [`wide_repo`], so slice selection falls
/// back to files in ID order — and [`TARGET`] sorts after all sixty gadget
/// files, outside the 40-node bound.
const NO_MATCH_QUESTION: &str = "does the quokka wander at midnight?";

/// A repository bigger than the slice bound: sixty files whose IDs sort
/// before `src/zzz/target.ts`, so on [`NO_MATCH_QUESTION`] the fallback
/// top-40 never includes [`TARGET`] and only a carried citation can.
fn wide_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    fs::create_dir_all(src.join("zzz")).unwrap();
    for i in 0..60 {
        fs::write(
            src.join(format!("gadget{i:02}.ts")),
            format!("export const gadget{i:02} = {i};\n"),
        )
        .unwrap();
    }
    fs::write(src.join("zzz/target.ts"), "export const target = 1;\n").unwrap();
    dir
}

/// ADR-0012 on the wire: a request may carry previous turns, and the slice
/// is built citations-first from them. The proof rides the citation
/// validation the route already has — the fake backend's canned citation of
/// [`TARGET`] survives `verified` exactly when the carried turns put that
/// node in the slice, so the bare ask is the control and the carried ask is
/// the behaviour.
#[test]
fn a_carried_turn_steers_the_slice_the_next_answer_is_drawn_from() {
    let repo = wide_repo();
    scan(repo.path());
    let outside = tempfile::tempdir().unwrap();
    let spec = format!(
        "fake:{}",
        canned(outside.path(), "The target does the work.", &[TARGET]).display()
    );
    let server = serve_with(repo.path(), &["--ask", "--provider", &spec]);

    // Control: a bare question stays a valid request answered exactly as
    // today — and its slice cannot hold the target, so the canned citation
    // is dropped.
    let (status, body) = ask(server.port, NO_MATCH_QUESTION);
    assert!(status.contains("200"), "bare ask status: {status} {body}");
    assert_eq!(body["answer"], "The target does the work.");
    assert_eq!(
        body["citations"],
        serde_json::json!([]),
        "a bare question's slice must not hold the target: {body}"
    );

    // The same question carrying one turn whose answer cited the target:
    // the citation puts the node in the slice, so the same canned citation
    // now checks out.
    let turns = serde_json::json!([{
        "question": "what is the target?",
        "answer": "src/zzz/target.ts is.",
        "citations": [TARGET],
    }]);
    let (status, body) = ask_carrying(server.port, NO_MATCH_QUESTION, turns);
    assert!(
        status.contains("200"),
        "carried ask status: {status} {body}"
    );
    assert_eq!(
        body["citations"],
        serde_json::json!([TARGET]),
        "the carried citation must steer the slice: {body}"
    );
}

/// V3 story 20: carried citations are bounded per turn, and the bound is a
/// clamp — excess dropped from the tail, never a refusal — like every
/// carried field (ADR-0012: the history is the dashboard's bookkeeping).
/// Observable exactly as the steering test above is: the fake backend's
/// canned citation of [`TARGET`] survives `verified` only if the carried
/// turn put that node in the slice. Only fabricated history can feel this
/// bound — an honest turn's citations came from an answer, which cited at
/// most the slice it was shown — but invented IDs fill no seats in the
/// selection, so without the clamp a turn padding the bound's worth of
/// them ahead of a real ID would steer the slice from past the bound.
#[test]
fn a_citation_past_the_per_turn_bound_stops_steering_the_slice() {
    use codeatlas::enrich::ask::MAX_TURN_CITATIONS;

    let repo = wide_repo();
    scan(repo.path());
    let outside = tempfile::tempdir().unwrap();
    let spec = format!(
        "fake:{}",
        canned(outside.path(), "The target does the work.", &[TARGET]).display()
    );
    let server = serve_with(repo.path(), &["--ask", "--provider", &spec]);

    let invented =
        |n: usize| -> Vec<String> { (0..n).map(|i| format!("file:src/invented{i}.ts")).collect() };
    let carrying = |citations: Vec<String>| {
        serde_json::json!([{
            "question": "what is the target?",
            "answer": "src/zzz/target.ts is.",
            "citations": citations,
        }])
    };

    // The boundary control first: [`TARGET`] in the bound's last seat still
    // steers, so the clamp is a bound and not an off-by-one — and the
    // citation channel is proven live before the refusal below leans on it.
    let mut at_bound = invented(MAX_TURN_CITATIONS - 1);
    at_bound.push(TARGET.into());
    let (status, body) = ask_carrying(server.port, NO_MATCH_QUESTION, carrying(at_bound));
    assert!(status.contains("200"), "at-bound ask: {status} {body}");
    assert_eq!(
        body["citations"],
        serde_json::json!([TARGET]),
        "a citation within the bound must steer the slice: {body}"
    );

    // One seat past the bound: the request still succeeds — clamped, never
    // refused — and the smuggled citation no longer reaches the provider,
    // so the canned citation of it is dropped as unshown.
    let mut past_bound = invented(MAX_TURN_CITATIONS);
    past_bound.push(TARGET.into());
    let (status, body) = ask_carrying(server.port, NO_MATCH_QUESTION, carrying(past_bound));
    assert!(status.contains("200"), "past-bound ask: {status} {body}");
    assert_eq!(body["answer"], "The target does the work.");
    assert_eq!(
        body["citations"],
        serde_json::json!([]),
        "a citation past the per-turn bound must stop steering the slice: {body}"
    );
}

/// The other axis of story 20's citation bound: each carried citation is
/// cut at [`ask::MAX_CITATION_CHARS`] like every carried field, and a cut
/// ID names no node, so an over-length citation selects nothing — while
/// the request it rode in on succeeds untouched. The probe is what no
/// fixture had: a real node whose own ID outgrows the bound, minted here
/// from a path deep enough to exceed it — which is what makes the clamp
/// falsifiable, because without it that citation is real and steers.
#[test]
fn an_over_length_citation_is_cut_and_steers_nothing() {
    use codeatlas::enrich::ask::MAX_CITATION_CHARS;

    let repo = wide_repo();
    // Five directory levels of 120 characters under `src/zzz/`, so the file
    // sorts after the sixty gadgets — outside the fallback top-40, like
    // [`TARGET`] — and its ID passes the bound with room to spare.
    let level = "z".repeat(120);
    let deep_rel = format!("src/zzz/{}", [level.as_str(); 5].join("/"));
    fs::create_dir_all(repo.path().join(&deep_rel)).unwrap();
    fs::write(
        repo.path().join(&deep_rel).join("deep.ts"),
        "export const deep = 1;\n",
    )
    .unwrap();
    let deep_id = format!("file:{deep_rel}/deep.ts");
    assert!(
        deep_id.chars().count() > MAX_CITATION_CHARS,
        "this test is only meaningful when the real ID exceeds the bound: \
         {} chars",
        deep_id.chars().count()
    );
    scan(repo.path());

    let outside = tempfile::tempdir().unwrap();
    let spec = format!(
        "fake:{}",
        canned(
            outside.path(),
            "The target does the work.",
            &[TARGET, &deep_id],
        )
        .display()
    );
    let server = serve_with(repo.path(), &["--ask", "--provider", &spec]);

    // The premise: the deep file really is a node in the map, so its
    // absence below is the clamp's doing and never the scanner's.
    let (_, _, raw) = http_get(server.port, "/api/map");
    let map: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    assert!(
        map["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n["id"] == deep_id.as_str()),
        "the deep file must be a node in the map for this test to mean anything"
    );

    // One turn citing both: the short citation is the live control — it
    // must steer — and the over-length one, though it names a real node,
    // arrives cut into an ID that names nothing.
    let turns = serde_json::json!([{
        "question": "what is the target?",
        "answer": "The target, and something very deep.",
        "citations": [TARGET, deep_id],
    }]);
    let (status, body) = ask_carrying(server.port, NO_MATCH_QUESTION, turns);
    assert!(status.contains("200"), "carried ask: {status} {body}");
    assert_eq!(body["answer"], "The target does the work.");
    assert_eq!(
        body["citations"],
        serde_json::json!([TARGET]),
        "the over-length citation must be cut and steer nothing, while the \
         one within the bound steers: {body}"
    );
}

/// V3 story 20's second half: a structurally-wrong turn — a turn missing a
/// field, citations that are not an array, turns that are not a list — draws
/// the 400 the route has drawn since ticket 08 grew the body, pinned now so
/// tolerating a malformed turn becomes a test failure instead of a
/// discovery. The refusal is the JSON parse's, decided before any provider
/// is consulted, so the scripted answer must never appear in it. Distinct
/// from clamping on purpose: an over-*bound* turn is well-formed input the
/// dashboard assembled and is clamped (ADR-0012), while a turn that is not
/// even the documented shape came from no version of the dashboard at all.
#[test]
fn a_structurally_wrong_turn_draws_the_400_the_route_always_drew() {
    let repo = materialize("simple");
    scan(repo.path());
    let outside = tempfile::tempdir().unwrap();
    let spec = format!(
        "fake:{}",
        canned(
            outside.path(),
            "Only a well-formed request reaches me.",
            &[]
        )
        .display()
    );
    let server = serve_with(repo.path(), &["--ask", "--provider", &spec]);

    for (label, body) in [
        (
            "a turn missing its answer",
            serde_json::json!({"question": "q?", "turns": [
                {"question": "earlier?", "citations": []}
            ]}),
        ),
        (
            "a turn missing its question",
            serde_json::json!({"question": "q?", "turns": [
                {"answer": "Earlier prose.", "citations": []}
            ]}),
        ),
        (
            "citations that are not an array",
            serde_json::json!({"question": "q?", "turns": [
                {"question": "earlier?", "answer": "a.", "citations": "file:src/main.ts"}
            ]}),
        ),
        (
            "turns that are not a list",
            serde_json::json!({"question": "q?", "turns":
                {"question": "earlier?", "answer": "a.", "citations": []}
            }),
        ),
    ] {
        let (status, _, raw) = http_post(server.port, "/api/ask", &body.to_string());
        let text = String::from_utf8_lossy(&raw);
        assert!(
            status.contains("400"),
            "{label} must draw a 400: {status} {text}"
        );
        assert!(
            !text.contains("Only a well-formed request reaches me."),
            "{label} reached the backend anyway: {text}"
        );
        assert!(
            text.contains("turns"),
            "{label}'s refusal must describe the accepted shape: {text}"
        );
    }

    // The live control: the same server answers a well-formed carrying
    // request, so the refusals above are the parse's judgement rather than
    // a server that refuses everything.
    let turns = serde_json::json!([
        {"question": "earlier?", "answer": "Earlier prose.", "citations": []}
    ]);
    let (status, body) = ask_carrying(server.port, "what does this project do?", turns);
    assert!(status.contains("200"), "the control ask: {status} {body}");
    assert_eq!(body["answer"], "Only a well-formed request reaches me.");
}

fn header<'a>(headers: &'a [String], name: &str) -> Option<&'a str> {
    headers.iter().find_map(|h| {
        let (key, value) = h.split_once(':')?;
        key.eq_ignore_ascii_case(name).then_some(value.trim())
    })
}

#[test]
fn serve_delivers_the_dashboard_index_over_loopback() {
    let repo = materialize("simple");
    scan(repo.path());
    let server = serve(repo.path());

    let (status, headers, body) = http_get(server.port, "/");
    assert!(status.contains("200"), "index status: {status}");
    assert_eq!(
        header(&headers, "Content-Type"),
        Some("text/html; charset=utf-8")
    );
    let html = String::from_utf8_lossy(&body);
    assert!(
        html.contains(r#"<div id="root">"#),
        "index must contain the app mount, got: {html}"
    );
}

#[test]
fn serve_delivers_the_map_as_json() {
    let repo = materialize("simple");
    scan(repo.path());
    let server = serve(repo.path());

    let (status, headers, body) = http_get(server.port, "/api/map");
    assert!(status.contains("200"), "map status: {status}");
    assert_eq!(header(&headers, "Content-Type"), Some("application/json"));
    let map: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let version = map["version"].as_str().unwrap();
    let parts: Vec<_> = version.split('.').collect();
    assert!(
        parts.len() == 3 && parts.iter().all(|p| p.parse::<u64>().is_ok()),
        "version must be semver, got {version}"
    );
    assert!(!map["nodes"].as_array().unwrap().is_empty());
}

#[test]
fn serve_delivers_assets_with_correct_mime_types() {
    let repo = materialize("simple");
    scan(repo.path());
    let server = serve(repo.path());

    // The index names the hashed bundle files; fetch what it references so
    // the test survives every rebuild of the dashboard.
    let (_, _, index) = http_get(server.port, "/");
    let html = String::from_utf8_lossy(&index);

    let js = html
        .split("src=\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .expect("index references a script");
    let (status, headers, _) = http_get(server.port, js);
    assert!(status.contains("200"), "js status: {status}");
    assert_eq!(header(&headers, "Content-Type"), Some("text/javascript"));

    let css = html
        .split("href=\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .expect("index references a stylesheet");
    let (status, headers, _) = http_get(server.port, css);
    assert!(status.contains("200"), "css status: {status}");
    assert_eq!(header(&headers, "Content-Type"), Some("text/css"));
}

#[test]
fn serve_delivers_the_diff_overlay_as_json() {
    let repo = materialize("simple");
    scan(repo.path());
    // The route serves whatever overlay `codeatlas diff` left on disk; the
    // seam is the artifact, so writing it directly is equivalent and keeps
    // this test free of git setup.
    fs::write(
        repo.path().join(".codeatlas/diff-overlay.json"),
        "{\"version\":1,\"changed\":[\"file:src/util.ts\"],\"affected\":[\"file:src/main.ts\"],\"unmapped_paths\":[]}\n",
    )
    .unwrap();
    let server = serve(repo.path());

    let (status, headers, body) = http_get(server.port, "/api/diff");
    assert!(status.contains("200"), "overlay status: {status}");
    assert_eq!(header(&headers, "Content-Type"), Some("application/json"));
    let overlay: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(overlay["changed"][0], "file:src/util.ts");
    assert_eq!(overlay["affected"][0], "file:src/main.ts");
}

#[test]
fn serve_returns_404_when_no_diff_overlay_exists() {
    let repo = materialize("simple");
    scan(repo.path());
    let server = serve(repo.path());

    let (status, _, body) = http_get(server.port, "/api/diff");
    assert!(status.contains("404"), "expected 404, got: {status}");
    let error: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(
        error["error"].as_str().unwrap().contains("codeatlas diff"),
        "error must say how to produce an overlay: {error}"
    );
}

#[test]
fn serve_returns_404_for_unknown_paths() {
    let repo = materialize("simple");
    scan(repo.path());
    let server = serve(repo.path());

    let (status, _, _) = http_get(server.port, "/no-such-asset.js");
    assert!(status.contains("404"), "expected 404, got: {status}");
}

/// Story 19: HEAD is answered wherever GET is — GET's status and headers,
/// the `Content-Length` of the body GET would send (RFC 9110 §9.3.2), and
/// no body. The paths walked are `serve::REGISTRY`'s own GET entries — the
/// server derives HEAD from that table and so does this test, so a GET
/// route added later is covered here without anyone remembering it — plus
/// the asset fallback the registry deliberately leaves out, at its 200 and
/// its 404.
#[test]
fn head_is_answered_wherever_get_is_with_gets_headers_and_no_body() {
    let repo = materialize("simple");
    scan(repo.path());
    let server = serve(repo.path());

    let mut paths: Vec<&str> = codeatlas::serve::REGISTRY
        .iter()
        .filter(|route| route.method == "GET")
        .map(|route| route.path)
        .collect();
    assert!(
        !paths.is_empty(),
        "the registry holds no GET route at all; this test is walking nothing"
    );
    paths.extend(["/", "/no-such-asset.js"]);

    for path in paths {
        let (get_status, get_headers, get_body) = http_get(server.port, path);
        let (head_status, head_headers, head_body) = http_head(server.port, path);
        assert_eq!(
            head_status, get_status,
            "HEAD {path} must carry GET's status"
        );
        for name in ["Content-Type", "Content-Length"] {
            assert_eq!(
                header(&head_headers, name),
                header(&get_headers, name),
                "HEAD {path}: {name} must match GET's"
            );
        }
        // The promised length is the body GET actually sends, never a
        // number invented without it.
        assert_eq!(
            header(&head_headers, "Content-Length").unwrap(),
            get_body.len().to_string(),
            "HEAD {path} must promise exactly the body GET sends"
        );
        assert!(
            head_body.is_empty(),
            "HEAD {path} must send no body, got {} bytes",
            head_body.len()
        );
    }
}

/// What HEAD draws on the one route that answers no GET: `/api/ask` is
/// POST-only, so `GET /api/ask` has always fallen through to the asset
/// lookup and 404ed — and HEAD mirrors GET everywhere, so it draws the
/// same 404 with the same headers, on both server shapes. Not a 405: HEAD
/// is a method this server answers, and refusing it politely on one path
/// would take exactly the second method-aware list the ticket forbids.
/// The POST beside it still answers, so HEAD changed lanes without taking
/// the question route along.
#[test]
fn head_of_the_post_only_question_route_mirrors_gets_404() {
    let repo = materialize("simple");
    scan(repo.path());
    let outside = tempfile::tempdir().unwrap();
    let spec = format!(
        "fake:{}",
        canned(outside.path(), "It starts in main.ts.", &[]).display()
    );
    let asking = serve_with(repo.path(), &["--ask", "--provider", &spec]);

    let (get_status, _, get_body) = http_get(asking.port, "/api/ask");
    assert!(get_status.contains("404"), "GET /api/ask: {get_status}");
    let (status, headers, body) = http_head(asking.port, "/api/ask");
    assert!(status.contains("404"), "HEAD /api/ask: {status}");
    assert_eq!(
        header(&headers, "Content-Length").unwrap(),
        get_body.len().to_string(),
        "HEAD /api/ask must promise the body GET /api/ask sends"
    );
    assert!(body.is_empty(), "HEAD sends no body");
    let (status, _) = ask(asking.port, "where does the program start?");
    assert!(status.contains("200"), "POST /api/ask beside it: {status}");

    // The same 404 on a plain serve, where the POST route does not exist.
    let plain = serve(repo.path());
    let (status, _, _) = http_head(plain.port, "/api/ask");
    assert!(status.contains("404"), "plain HEAD /api/ask: {status}");
}

/// A bodyless request in an arbitrary method — the refusal lanes' own
/// helper, for methods no honest helper above speaks.
fn http_method(port: u16, method: &str, path: &str) -> (String, Vec<String>, Vec<u8>) {
    raw_request(
        port,
        format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n")
            .as_bytes(),
    )
    .unwrap()
}

/// V2 ticket 13 pinned this refusal byte-identical across every refused
/// method, `Allow`-less; this lap supersedes that pin DELIBERATELY (V3,
/// `docs/specs/2026-08-16-codeatlas-v3.md` stories 18–19, ticket 05 — the
/// spec says so in as many words: "that constraint pinned a shape, and this
/// lap changes the shape on purpose"). RFC 9110 §15.5.6 makes `Allow` a
/// MUST on every 405, so the refusal now names what is served in the header
/// machines read as well as the sentence humans do. The sentence itself is
/// unchanged — HEAD left the 405 lane in V2 and nobody else has — and the
/// header is asserted with equality, never `contains`: at a path where only
/// GET (and so HEAD) is served, `Allow` is exactly that, and an over-claim
/// is the same false advertisement as an under-claim.
#[test]
fn other_methods_draw_a_405_whose_allow_header_names_the_served_surface() {
    let repo = materialize("simple");
    scan(repo.path());
    let server = serve(repo.path());

    for method in ["PUT", "DELETE", "OPTIONS", "PATCH"] {
        let (status, headers, body) = http_method(server.port, method, "/api/map");
        assert!(status.contains("405"), "{method} status: {status}");
        assert_eq!(
            header(&headers, "Allow"),
            Some("GET, HEAD"),
            "{method} /api/map: a 405 must carry Allow naming exactly the \
             methods served at the path (RFC 9110 §15.5.6)"
        );
        assert_eq!(
            String::from_utf8_lossy(&body),
            "only GET is served",
            "{method} must keep the refusal that names the served surface"
        );
    }
}

/// The `Allow` list reflects the registry AS CONFIGURED, flags included:
/// POST is served at exactly one path — `/api/ask`, and only while `--ask`
/// holds it registered — so POST appears in exactly that 405's `Allow` and
/// no other's. A plain `serve` never advertises it at all: the flag absent
/// means the method is not served there, and an `Allow` naming it would be
/// the capability route's lie in header form.
#[test]
fn allow_names_post_exactly_where_ask_is_registered() {
    let repo = materialize("simple");
    scan(repo.path());

    // Plain shape: no POST anywhere, the ask path included.
    let plain = serve(repo.path());
    let (status, headers, _) = http_method(plain.port, "PUT", "/api/ask");
    assert!(status.contains("405"), "plain PUT /api/ask: {status}");
    assert_eq!(
        header(&headers, "Allow"),
        Some("GET, HEAD"),
        "without --ask the ask route does not exist, so its path's Allow \
         must not name POST"
    );
    let (status, headers, _) = http_post(
        plain.port,
        "/api/ask",
        &serde_json::json!({"question": "anything?"}).to_string(),
    );
    assert!(status.contains("405"), "plain POST /api/ask: {status}");
    assert_eq!(
        header(&headers, "Allow"),
        Some("GET, HEAD"),
        "the refused POST itself must draw the same honest Allow"
    );

    // The --ask shape: POST beside GET and HEAD at the ask path, and only
    // there — the map path's Allow is unchanged by the flag.
    let outside = tempfile::tempdir().unwrap();
    let spec = format!("fake:{}", canned(outside.path(), "unused", &[]).display());
    let asking = serve_with(repo.path(), &["--ask", "--provider", &spec]);
    let (status, headers, _) = http_method(asking.port, "PUT", "/api/ask");
    assert!(status.contains("405"), "asking PUT /api/ask: {status}");
    assert_eq!(
        header(&headers, "Allow"),
        Some("GET, HEAD, POST"),
        "with --ask the ask path serves POST, and its 405 must say so — \
         exactly GET, HEAD and POST, nothing else"
    );
    let (status, headers, _) = http_method(asking.port, "PUT", "/api/map");
    assert!(status.contains("405"), "asking PUT /api/map: {status}");
    assert_eq!(
        header(&headers, "Allow"),
        Some("GET, HEAD"),
        "POST is served at the ask path, not at this one — Allow is per \
         path, never per server"
    );
}

/// Story 19's second half: a request the parser cannot make sense of draws
/// a 400 saying what was wrong, where it used to draw a closed connection
/// the client could only report as a network failure. Three malformed
/// shapes: a head that is not UTF-8, a request line short of its parts,
/// and a request line with no HTTP version. Size and pace stay out of this
/// lane — the 408/431 tests below pin those refusals separately.
#[test]
fn a_request_that_cannot_be_parsed_draws_a_400_instead_of_a_silent_close() {
    let repo = materialize("simple");
    scan(repo.path());
    let server = serve(repo.path());

    // Not UTF-8: 0xFF begins no UTF-8 sequence.
    let (status, _, body) = raw_request(server.port, b"\xff\xfe GET / HTTP/1.1\r\n\r\n")
        .expect("a malformed request must draw a response, not a closed connection");
    assert!(status.contains("400"), "non-UTF-8 status: {status}");
    assert!(
        String::from_utf8_lossy(&body).contains("UTF-8"),
        "the body must say what was wrong: {:?}",
        String::from_utf8_lossy(&body)
    );

    // A request line short of its three parts.
    let (status, _, body) = raw_request(server.port, b"GARBAGE\r\n\r\n")
        .expect("a malformed request line must draw a response");
    assert!(status.contains("400"), "one-token status: {status}");
    assert!(
        String::from_utf8_lossy(&body).contains("<method> <target> HTTP/<version>"),
        "the body must show the shape a request line has: {:?}",
        String::from_utf8_lossy(&body)
    );

    // Method and target but no version — HTTP/1.1's own grammar, not a
    // nicety: without the third token the line is a different protocol.
    let (status, _, body) = raw_request(server.port, b"GET /api/map\r\n\r\n")
        .expect("a versionless request line must draw a response");
    assert!(status.contains("400"), "no-version status: {status}");
    assert!(
        String::from_utf8_lossy(&body).contains("HTTP/"),
        "the body must name the missing version: {:?}",
        String::from_utf8_lossy(&body)
    );

    // And the server is unharmed by all three.
    let (status, _, _) = http_get(server.port, "/api/map");
    assert!(
        status.contains("200"),
        "map status after the garbage: {status}"
    );
}

/// V3 story 19's grammar half: a request line is exactly three tokens or
/// nothing. Fewer than three has drawn a 400 since V2 (the test above);
/// what V2 left over — its ticket 13 recorded the residual in as many
/// words — was that `starts_with("HTTP/")` silently tolerated a FOURTH
/// token, so `GET / HTTP/1.1 junk` was served as if the junk were not
/// there. Tolerating what the grammar forbids is how parsers drift apart;
/// now the whole line must parse, and the extra token is the fault the 400
/// names.
#[test]
fn a_request_line_of_more_than_three_tokens_is_refused_not_tolerated() {
    let repo = materialize("simple");
    scan(repo.path());
    let server = serve(repo.path());

    let (status, _, body) = raw_request(
        server.port,
        b"GET /api/map HTTP/1.1 junk\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n",
    )
    .expect("a four-token request line must draw a response, not be served");
    assert!(status.contains("400"), "four-token status: {status}");
    assert!(
        String::from_utf8_lossy(&body).contains("<method> <target> HTTP/<version>"),
        "the body must show the shape a request line has: {:?}",
        String::from_utf8_lossy(&body)
    );

    // The control: the same request without the junk is exactly three
    // tokens, and is served — the refusal above is about the count, not
    // the line.
    let (status, _, _) = http_get(server.port, "/api/map");
    assert!(status.contains("200"), "three-token control: {status}");
}

/// V3 story 19's other half, and the refusal taxonomy stated so the next
/// reader knows which refusal means what (RFC 9110 §§9.1, 15.5.6, 15.6.2):
///
/// - **501 Not Implemented** — "this server does not implement the method
///   at all": the method is not one HTTP defines, so no path here or
///   anywhere on this server could ever serve it. No `Allow`, because
///   there is no path where the answer would differ.
/// - **405 Method Not Allowed** — "the method exists here, just not at
///   this path": one of HTTP's own registered methods (GET, HEAD, POST,
///   PUT, DELETE, CONNECT, OPTIONS, TRACE, PATCH), refused at a path that
///   serves other methods — named in the `Allow` the 405 must carry.
///
/// Before this lap `FROB / HTTP/1.1` drew the 405, which claimed a
/// method-of-this-path problem the request does not have — V2 ticket 13
/// recorded exactly that as its residual. The recognised control alongside
/// proves the 501 is about the method being unknown, not about this server
/// having grown a taste for refusing everything harder.
#[test]
fn an_unrecognised_method_draws_501_and_a_recognised_one_stays_405() {
    let repo = materialize("simple");
    scan(repo.path());
    let server = serve(repo.path());

    // The method HTTP never defined: 501, on any path.
    for path in ["/", "/api/map"] {
        let (status, headers, body) = http_method(server.port, "FROB", path);
        assert!(status.contains("501"), "FROB {path} status: {status}");
        assert!(
            String::from_utf8_lossy(&body).contains("FROB"),
            "the refusal must name the method it does not implement: {:?}",
            String::from_utf8_lossy(&body)
        );
        assert_eq!(
            header(&headers, "Allow"),
            None,
            "a 501 makes no claim about this path's methods — Allow is the \
             405's header"
        );
    }

    // The taxonomy's other side, at the same path: methods HTTP does
    // define, this server just does not serve here — 405, with the Allow
    // that refusal owes. TRACE and CONNECT ride beside the everyday four
    // because they are the registered methods most tempting to lump in
    // with FROB: recognised is a fact about HTTP, not about this server.
    for method in ["PUT", "DELETE", "OPTIONS", "PATCH", "TRACE", "CONNECT"] {
        let (status, headers, _) = http_method(server.port, method, "/api/map");
        assert!(
            status.contains("405"),
            "{method} is a method HTTP defines, so its refusal is \
             405-not-at-this-path, never 501-not-implemented: {status}"
        );
        assert_eq!(
            header(&headers, "Allow"),
            Some("GET, HEAD"),
            "{method}: the 405 keeps naming what is served"
        );
    }

    // And the server outlives the whole taxonomy lesson.
    let (status, _, _) = http_get(server.port, "/api/map");
    assert!(status.contains("200"), "map status after FROB: {status}");
}

/// Seam 4 (spec, added 2026-08-11): the real binary, real HTTP/1.1 over
/// 127.0.0.1, assertions on the response. The bounding half of story 21 is
/// asserted at seam 2 in `src/enrich/ask.rs`; what these cover is the route.
#[test]
fn a_question_is_answered_and_cites_only_nodes_that_exist() {
    let repo = materialize("simple");
    scan(repo.path());
    let outside = tempfile::tempdir().unwrap();
    // One real citation and one the model invented: the invented one must
    // not reach the reader, because a citation is a promise it can be
    // followed.
    let spec = format!(
        "fake:{}",
        canned(
            outside.path(),
            "Everything starts in src/main.ts.",
            &["file:src/main.ts", "file:src/invented-by-the-model.ts"],
        )
        .display()
    );
    let server = serve_with(repo.path(), &["--ask", "--provider", &spec]);

    let (status, body) = ask(server.port, "where does the program start?");
    assert!(status.contains("200"), "ask status: {status} {body}");
    assert_eq!(body["answer"], "Everything starts in src/main.ts.");
    assert_eq!(
        body["citations"],
        serde_json::json!(["file:src/main.ts"]),
        "an invented citation must be dropped, a real one kept: {body}"
    );

    // Story 21's promise is that the reader is shown *which nodes* answer
    // the question, so every citation has to resolve in the served map.
    let (_, _, raw) = http_get(server.port, "/api/map");
    let map: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    let ids: Vec<&str> = map["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["id"].as_str().unwrap())
        .collect();
    for citation in body["citations"].as_array().unwrap() {
        assert!(
            ids.contains(&citation.as_str().unwrap()),
            "cited a node the map does not have: {citation}"
        );
    }
}

/// Usage on the wire (ticket 09, stories 12/13): what the backend reported
/// is what the response carries — two token counts, nothing else. The
/// scripted numbers are deliberately unequal and unround, so a transposed
/// field or a fabricated zero cannot pass as the measurement.
#[test]
fn usage_rides_the_answer_exactly_as_the_backend_reported_it() {
    let repo = materialize("simple");
    scan(repo.path());
    let outside = tempfile::tempdir().unwrap();
    let spec = format!(
        "fake:{}",
        canned_reporting(outside.path(), "It starts in main.ts.", &[], 1207, 83).display()
    );
    let server = serve_with(repo.path(), &["--ask", "--provider", &spec]);

    let (status, body) = ask(server.port, "where does the program start?");
    assert!(status.contains("200"), "ask status: {status} {body}");
    assert_eq!(
        body["usage"],
        serde_json::json!({"input_tokens": 1207, "output_tokens": 83}),
        "usage must be the backend's own counts and only them: {body}"
    );
    // No currency anywhere (ADR-0012): the response is two token counts,
    // never a price. Asserted on the raw object so a field added beside the
    // counts cannot ride along unnoticed.
    assert_eq!(
        body["usage"].as_object().unwrap().len(),
        2,
        "the usage object is two counts and nothing else: {body}"
    );
    assert!(
        !body.to_string().contains("cost") && !body.to_string().contains('$'),
        "no cost figure may reach the wire: {body}"
    );
}

/// Story 13 on the wire: a backend that reports nothing produces no usage
/// field at all — the absent display the dashboard renders is the wire's own
/// absence, never a zero the server made up.
#[test]
fn a_backend_reporting_no_usage_produces_no_usage_field() {
    let repo = materialize("simple");
    scan(repo.path());
    let outside = tempfile::tempdir().unwrap();
    let spec = format!(
        "fake:{}",
        canned(outside.path(), "It starts in main.ts.", &[]).display()
    );
    let server = serve_with(repo.path(), &["--ask", "--provider", &spec]);

    let (status, body) = ask(server.port, "where does the program start?");
    assert!(status.contains("200"), "ask status: {status} {body}");
    assert!(
        !body.as_object().unwrap().contains_key("usage"),
        "no measurement means no field — not null, not zero: {body}"
    );
}

/// Story 14 on the wire: over-bound history is clamped mechanically, oldest
/// turns first, and never rejected — the reader typed the question, the
/// dashboard assembled the history, and a 400 would punish the wrong party.
/// Which turns survived is observable exactly as above: the canned citation
/// of [`TARGET`] outlives `verified` only if the turn citing it did.
#[test]
fn history_beyond_the_bound_is_clamped_oldest_first_never_rejected() {
    let repo = wide_repo();
    scan(repo.path());
    let outside = tempfile::tempdir().unwrap();
    let spec = format!(
        "fake:{}",
        canned(outside.path(), "Still the target.", &[TARGET]).display()
    );
    let server = serve_with(repo.path(), &["--ask", "--provider", &spec]);

    // Seven turns where only the OLDEST cites the target: the clamp keeps
    // the newest six, so the citing turn is gone and the citation with it.
    let citing = |t: usize, cites: bool| {
        serde_json::json!({
            "question": format!("turn {t}?"),
            "answer": format!("answer {t}."),
            "citations": if cites { vec![TARGET] } else { vec![] },
        })
    };
    let oldest_cites: Vec<_> = (0..7).map(|t| citing(t, t == 0)).collect();
    let (status, body) = ask_carrying(
        server.port,
        NO_MATCH_QUESTION,
        serde_json::json!(oldest_cites),
    );
    assert!(
        status.contains("200"),
        "over-bound history must never be rejected: {status} {body}"
    );
    assert_eq!(
        body["citations"],
        serde_json::json!([]),
        "the oldest turn must be the one dropped: {body}"
    );

    // The same seven turns except the *second*-oldest cites: within the
    // newest six, so it survives the clamp and steers the slice.
    let second_cites: Vec<_> = (0..7).map(|t| citing(t, t == 1)).collect();
    let (status, body) = ask_carrying(
        server.port,
        NO_MATCH_QUESTION,
        serde_json::json!(second_cites),
    );
    assert!(status.contains("200"), "{status} {body}");
    assert_eq!(
        body["citations"],
        serde_json::json!([TARGET]),
        "a turn inside the bound must survive the clamp: {body}"
    );

    // Over-bound in every dimension a client controls short of MAX_BODY:
    // eight turns of over-long fields, and the answer is still an answer.
    let bloated: Vec<_> = (0..8)
        .map(|t| {
            serde_json::json!({
                "question": "q".repeat(1_500),
                "answer": "a".repeat(2_500),
                "citations": if t == 7 { vec![TARGET] } else { vec![] },
            })
        })
        .collect();
    let (status, body) = ask_carrying(server.port, NO_MATCH_QUESTION, serde_json::json!(bloated));
    assert!(
        status.contains("200"),
        "over-bound fields are clamped, never refused: {status} {body}"
    );
    assert_eq!(body["citations"], serde_json::json!([TARGET]), "{body}");
}

/// Story 15 on the wire: the server holds no conversation state, so two
/// conversations interleaved on one server never see each other. If the
/// server retained anything between requests, the bare conversation's slice
/// would inherit the carried one's cited node and the canned citation would
/// start surviving there too.
#[test]
fn two_conversations_interleaved_on_one_server_never_see_each_other() {
    let repo = wide_repo();
    scan(repo.path());
    let outside = tempfile::tempdir().unwrap();
    let spec = format!(
        "fake:{}",
        canned(outside.path(), "About the target.", &[TARGET]).display()
    );
    let server = serve_with(repo.path(), &["--ask", "--provider", &spec]);

    let carried = serde_json::json!([{
        "question": "what is the target?",
        "answer": "src/zzz/target.ts is.",
        "citations": [TARGET],
    }]);
    for round in 1..=2 {
        let (status, body) = ask_carrying(server.port, NO_MATCH_QUESTION, carried.clone());
        assert!(status.contains("200"), "round {round}: {status} {body}");
        assert_eq!(
            body["citations"],
            serde_json::json!([TARGET]),
            "round {round}: the carried conversation steers its own slice: {body}"
        );

        let (status, body) = ask(server.port, NO_MATCH_QUESTION);
        assert!(status.contains("200"), "round {round}: {status} {body}");
        assert_eq!(
            body["citations"],
            serde_json::json!([]),
            "round {round}: a bare conversation asked between two carried \
             ones must inherit nothing from them: {body}"
        );
    }
}

/// The `Content-Type` gate is what keeps an arbitrary page in the reader's
/// browser from spending their model budget cross-origin, and growing the
/// body's shape must not weaken it (ADR-0012): a request carrying turns is
/// refused on a browser-simple content type exactly as a bare one is.
#[test]
fn a_request_carrying_turns_still_faces_the_content_type_gate() {
    let repo = materialize("simple");
    scan(repo.path());
    let outside = tempfile::tempdir().unwrap();
    let spec = format!(
        "fake:{}",
        canned(outside.path(), "MUST NOT BE REACHED", &[]).display()
    );
    let server = serve_with(repo.path(), &["--ask", "--provider", &spec]);

    let body = serde_json::json!({
        "question": "what is this?",
        "turns": [{"question": "earlier?", "answer": "Earlier.", "citations": []}],
    })
    .to_string();
    let (status, _, raw) = http_post_as(server.port, "/api/ask", &body, "text/plain");
    assert!(
        status.contains("415"),
        "turns must not soften the gate: {status}"
    );
    assert!(
        !String::from_utf8_lossy(&raw).contains("MUST NOT BE REACHED"),
        "the backend was reached anyway: {:?}",
        String::from_utf8_lossy(&raw)
    );

    // The control: the same body with the demanded type is answered.
    let (status, _, _) = http_post_as(server.port, "/api/ask", &body, "application/json");
    assert!(
        status.contains("200"),
        "the honest request failed: {status}"
    );
}

/// ADR-0009: without the flag the server reaches nothing but loopback and
/// local disk, which is what keeps the netns egress test a real subject.
#[test]
fn the_question_route_does_not_exist_without_the_flag() {
    let repo = materialize("simple");
    scan(repo.path());
    let server = serve(repo.path());

    let (status, _, body) = http_post(
        server.port,
        "/api/ask",
        &serde_json::json!({"question": "anything?"}).to_string(),
    );
    assert!(
        status.contains("405"),
        "a plain serve must not accept POST: {status}"
    );
    assert!(
        String::from_utf8_lossy(&body).contains("only GET"),
        "the refusal must say the server has one verb: {:?}",
        String::from_utf8_lossy(&body)
    );

    // And the server is unharmed by having been asked.
    let (status, _, _) = http_get(server.port, "/api/map");
    assert!(status.contains("200"), "map status after POST: {status}");
}

/// Ticket 27: the dashboard is the same embedded bytes whether or not the
/// binary serving it was started with `--ask`, so it has to be told at
/// runtime. The claim is asserted against the route it describes — a
/// capability answer nothing checks against reality is the kind of fact that
/// drifts silently.
#[test]
fn the_capability_route_states_whether_questions_can_be_asked() {
    let repo = materialize("simple");
    scan(repo.path());

    let plain = serve(repo.path());
    let (status, headers, body) = http_get(plain.port, "/api/capabilities");
    assert!(status.contains("200"), "capabilities status: {status}");
    assert_eq!(header(&headers, "Content-Type"), Some("application/json"));
    let said: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(said["ask"], serde_json::json!(false), "{said}");
    // And it is telling the truth about this server: the route it says is
    // absent really is. This carried an empty body until ticket 35, to keep
    // clear of the reset that a refusal with bytes left unread used to cause;
    // a refusal now hangs the connection up rather than dropping it on them,
    // so the question it would really be asked is the honest thing to send.
    let (status, _, _) = http_post(
        plain.port,
        "/api/ask",
        &serde_json::json!({"question": "anything?"}).to_string(),
    );
    assert!(
        status.contains("405"),
        "said it could not answer, then did: {status}"
    );

    let outside = tempfile::tempdir().unwrap();
    let spec = format!(
        "fake:{}",
        canned(outside.path(), "It starts in main.ts.", &[]).display()
    );
    let asking = serve_with(repo.path(), &["--ask", "--provider", &spec]);
    let (status, _, body) = http_get(asking.port, "/api/capabilities");
    assert!(status.contains("200"), "capabilities status: {status}");
    let said: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(said["ask"], serde_json::json!(true), "{said}");
    let (status, _) = ask(asking.port, "where does the program start?");
    assert!(
        status.contains("200"),
        "said it could answer, then did not: {status}"
    );
}

/// ADR-0013's gate, proven on the wire: the source route is registered
/// exactly when `--open-code` was given. Without the flag the route does not
/// exist — the `--ask` pattern — so asking for source draws the very refusal
/// any unregistered GET draws: the asset fallback's 404, indistinguishable
/// from a path nobody ever served. A 403 here would be a route that exists
/// and refuses, which is the shape ADR-0013 rejects.
#[test]
fn the_source_route_exists_exactly_when_open_code_was_given() {
    let repo = materialize("simple");
    scan(repo.path());

    let plain = serve(repo.path());
    let (miss_status, miss_headers, miss_body) = http_get(plain.port, "/no-such-asset.js");
    let (status, headers, body) = http_get(plain.port, "/api/source?id=file%3Asrc%2Fmain.ts");
    assert!(
        status.contains("404"),
        "plain serve source status: {status}"
    );
    assert_eq!(
        (
            status.as_str(),
            header(&headers, "Content-Type"),
            body.clone()
        ),
        (
            miss_status.as_str(),
            header(&miss_headers, "Content-Type"),
            miss_body
        ),
        "without the flag the source route must not exist: the refusal must \
         be the unregistered-route refusal, never a route of its own"
    );

    let open = serve_with(repo.path(), &["--open-code"]);
    let (status, headers, body) = http_get(open.port, "/api/source?id=file%3Asrc%2Fmain.ts");
    assert!(status.contains("200"), "open-code source status: {status}");
    assert_eq!(header(&headers, "Content-Type"), Some("application/json"));
    let envelope: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(envelope["path"], serde_json::json!("src/main.ts"));
    assert_eq!(envelope["truncated"], serde_json::json!(false));
    assert_eq!(
        envelope["language"],
        serde_json::json!("TypeScript"),
        "the envelope must name the language it highlighted as"
    );
    let disk = fs::read_to_string(repo.path().join("src/main.ts")).unwrap();
    assert_eq!(
        source_text(&envelope),
        disk,
        "under the markup, the source must be the file's own bytes"
    );
    assert!(
        envelope["html"]
            .as_str()
            .unwrap()
            .contains("<span class=\"hl-"),
        "a grammar-covered file must arrive with token spans: {}",
        envelope["html"]
    );
}

/// The text a browser would show for the envelope's `html` — tags stripped,
/// the renderer's five entities decoded, `&amp;` last so an escaped
/// ampersand cannot cascade. Literal `<` in source arrives as `&lt;`, so
/// every raw `<…>` in the HTML is markup and stripping it is exact. Written
/// here independently of the server's own escaping, the same way the cap
/// test states the cap: a helper imported from the crate could not catch
/// the crate lying.
fn source_text(envelope: &serde_json::Value) -> String {
    let html = envelope["html"]
        .as_str()
        .unwrap_or_else(|| panic!("the envelope must carry html: {envelope}"));
    let mut text = String::new();
    let mut rest = html;
    while let Some(open) = rest.find('<') {
        text.push_str(&rest[..open]);
        let close = rest[open..].find('>').expect("markup closes its tags");
        rest = &rest[open + close + 1..];
    }
    text.push_str(rest);
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

/// A source URL for one node id, percent-encoded the way a browser's
/// `encodeURIComponent` sends it — `:` and `/` escaped — so these tests
/// speak the encoding the dashboard will.
fn source_url(id: &str) -> String {
    let encoded: String = id
        .chars()
        .map(|c| match c {
            ':' => "%3A".to_string(),
            '/' => "%2F".to_string(),
            other => other.to_string(),
        })
        .collect();
    format!("/api/source?id={encoded}")
}

/// Map membership is the allowlist, proven where it bites: existence on
/// disk buys a request nothing. An unmapped path that really exists inside
/// the root, a symbol id the map really holds, and a traversal-shaped id
/// pointing at a real file outside the root each draw the same 404 for the
/// same stated reason — a lookup that consulted the filesystem would have
/// found all three, so the sameness is what shows the request never
/// reached it. The mapped control alongside proves the 404s are about the
/// ids, not a route that cannot answer.
#[test]
fn only_file_nodes_in_the_map_resolve_and_disk_existence_buys_nothing() {
    // The repo sits one level down so a `../` id has a real target: the
    // secret beside the root exists, and must still be unreachable.
    let outer = tempfile::tempdir().unwrap();
    let repo = outer.path().join("repo");
    common::copy_tree(&common::fixture_dir("simple"), &repo);
    let inert = repo.join("_gitignore");
    if inert.exists() {
        fs::rename(inert, repo.join(".gitignore")).unwrap();
    }
    fs::write(outer.path().join("beside-the-root.txt"), "not yours").unwrap();
    scan(&repo);
    // On disk inside the root, but born after the scan — not in the map.
    fs::write(repo.join("unmapped-after-scan.txt"), "also not yours").unwrap();
    let server = serve_with(&repo, &["--open-code"]);

    // A symbol id the map really holds: file-level is the only granularity
    // the wire speaks (ADR-0013; symbols open client-side via `range`).
    let (_, _, raw) = http_get(server.port, "/api/map");
    let map: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    let symbol = map["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["kind"] != "file")
        .expect("the fixture maps symbols")["id"]
        .as_str()
        .unwrap()
        .to_string();

    for id in [
        "file:unmapped-after-scan.txt",
        symbol.as_str(),
        "file:../beside-the-root.txt",
    ] {
        let (status, _, body) = http_get(server.port, &source_url(id));
        assert!(status.contains("404"), "{id}: {status}");
        let error: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(
            error["error"]
                .as_str()
                .unwrap()
                .contains("not a file node in the map"),
            "{id} must draw the allowlist's own refusal: {error}"
        );
    }
    // The two real files were readable the whole time; only the map said no.
    assert!(repo.join("unmapped-after-scan.txt").exists());
    assert!(outer.path().join("beside-the-root.txt").exists());

    // The control: a mapped file node still answers.
    let (status, _, _) = http_get(server.port, &source_url("file:src/main.ts"));
    assert!(status.contains("200"), "mapped control: {status}");
}

/// Source is read live from disk per request, exactly as the map and the
/// overlay are: an edit after the scan is served as it now stands, and a
/// deletion draws a 404 naming the honest reason — a stale map never
/// fabricates source (ADR-0013, spec story 8).
#[test]
fn source_is_read_live_so_an_edit_shows_and_a_deletion_says_so() {
    let repo = materialize("simple");
    scan(repo.path());
    let server = serve_with(repo.path(), &["--open-code"]);
    let url = source_url("file:src/util.ts");

    let (status, _, body) = http_get(server.port, &url);
    assert!(status.contains("200"), "before the edit: {status}");
    let envelope: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let scanned = fs::read_to_string(repo.path().join("src/util.ts")).unwrap();
    assert_eq!(source_text(&envelope), scanned);

    // Edited after the scan — no re-scan, and the wire serves the edit.
    let edited = "// rewritten after the scan\nexport const shape = 42;\n";
    fs::write(repo.path().join("src/util.ts"), edited).unwrap();
    let (status, _, body) = http_get(server.port, &url);
    assert!(status.contains("200"), "after the edit: {status}");
    let envelope: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        source_text(&envelope),
        edited,
        "an edited file must serve its current contents, never the scan's"
    );

    // Deleted after the scan: an honest 404, not an empty 200 and not the
    // allowlist's sentence — the node is in the map, the file is gone.
    fs::remove_file(repo.path().join("src/util.ts")).unwrap();
    let (status, _, body) = http_get(server.port, &url);
    assert!(status.contains("404"), "after the deletion: {status}");
    let error: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let reason = error["error"].as_str().unwrap();
    assert!(
        reason.contains("no longer on disk"),
        "the refusal must name the honest reason: {reason:?}"
    );
    assert!(
        reason.contains("src/util.ts"),
        "the refusal must name the file: {reason:?}"
    );

    // And the server outlives the stale entry.
    let (status, _, _) = http_get(server.port, "/api/map");
    assert!(status.contains("200"), "map status after the 404: {status}");
}

/// The envelope and its cap: source, path and a truncation flag; content
/// past the named cap arrives cut at it with the flag set, disclosed
/// rather than refused (ADR-0013). The cap is stated here as its own
/// literal — the server's `MAX_SOURCE_BYTES`, written independently so a
/// moved cap is a tripped test, never a silently moved test.
#[test]
fn source_past_the_named_cap_arrives_truncated_and_says_so() {
    /// The server's cap, as an independent literal (see above).
    const CAP: usize = 512 * 1024;

    let repo = materialize("simple");
    // Comment lines, so half a mebibyte of fixture parses as one file node
    // rather than thousands of identically-named symbols.
    let content = "// pad\n".repeat((CAP + 4096) / 7 + 1);
    assert!(content.len() > CAP, "the fixture must exceed the cap");
    fs::write(repo.path().join("src/huge.ts"), &content).unwrap();
    scan(repo.path());
    let server = serve_with(repo.path(), &["--open-code"]);

    let (status, _, body) = http_get(server.port, &source_url("file:src/huge.ts"));
    assert!(status.contains("200"), "over-cap status: {status}");
    let envelope: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(envelope["path"], serde_json::json!("src/huge.ts"));
    assert_eq!(
        envelope["truncated"],
        serde_json::json!(true),
        "content past the cap must say it was cut: {status}"
    );
    assert_eq!(
        source_text(&envelope),
        content[..CAP],
        "the served source must be exactly the file's first {CAP} bytes — \
         the clip runs first and highlighting runs on the clipped bytes, \
         so no character (a tidy trailing newline included) is invented"
    );
    // And cut is not stripped: what was served arrives highlighted, notice
    // intact — the fixture is comment lines, so the spans are comment spans.
    assert!(
        envelope["html"]
            .as_str()
            .unwrap()
            .contains("<span class=\"hl-"),
        "a truncated file must still highlight what is served"
    );

    // The control that makes the flag meaningful: an under-cap file arrives
    // whole, byte for byte, and unflagged.
    let (status, _, body) = http_get(server.port, &source_url("file:src/main.ts"));
    assert!(status.contains("200"), "under-cap status: {status}");
    let envelope: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let disk = fs::read_to_string(repo.path().join("src/main.ts")).unwrap();
    assert_eq!(envelope["truncated"], serde_json::json!(false));
    assert_eq!(
        source_text(&envelope),
        disk,
        "an under-cap file must arrive whole"
    );
}

/// Ticket 03's half of the envelope (ADR-0013): the language is named, an
/// uncovered language arrives as readable plain text that says so, and
/// source content cannot smuggle markup to the dashboard — the module tests
/// in `src/highlight.rs` prove the rule, this proves the plumbing carries
/// it to the wire unbroken.
#[test]
fn the_envelope_names_its_language_and_uncovered_files_arrive_plain_and_escaped() {
    let repo = materialize("simple");
    // A file whose whole point is markup in source: if the escaping ever
    // breaks, this is the payload that would execute in the dashboard.
    fs::write(
        repo.path().join("src/tricky.ts"),
        "export const sneaky = \"<script>alert('pwned')</script>\";\n",
    )
    .unwrap();
    scan(repo.path());
    let server = serve_with(repo.path(), &["--open-code"]);

    // The uncovered language the map really holds: Markdown has a parser
    // but no vendored highlight grammar, so it falls back — readable, no
    // spans, and the fallback stated rather than left to be inferred.
    let (status, _, body) = http_get(server.port, &source_url("file:README.md"));
    assert!(status.contains("200"), "markdown status: {status}");
    let envelope: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        envelope["language"],
        serde_json::json!("plain text"),
        "an uncovered language must state its fallback: {envelope}"
    );
    assert!(
        !envelope["html"].as_str().unwrap().contains("<span"),
        "plain text carries no token spans: {}",
        envelope["html"]
    );
    let disk = fs::read_to_string(repo.path().join("README.md")).unwrap();
    assert_eq!(source_text(&envelope), disk, "plain text is still whole");

    // The escaping, observed on the wire: the payload arrives as entities,
    // under a language that was genuinely highlighted.
    let (status, _, body) = http_get(server.port, &source_url("file:src/tricky.ts"));
    assert!(status.contains("200"), "tricky status: {status}");
    let envelope: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(envelope["language"], serde_json::json!("TypeScript"));
    let html = envelope["html"].as_str().unwrap();
    assert!(
        !html.contains("<script"),
        "markup in source must never reach the dashboard as markup: {html}"
    );
    assert!(
        html.contains("&lt;script&gt;"),
        "the payload must arrive as entities: {html}"
    );
}

/// The capabilities route carries the open-code boolean beside `ask`
/// (ADR-0013, spec story 7), and each answer is held to the route it
/// describes, exactly as the ask boolean is: a capability answer nothing
/// checks against reality is the kind of fact that drifts silently.
#[test]
fn the_capability_route_states_whether_source_can_be_fetched() {
    let repo = materialize("simple");
    scan(repo.path());
    let url = source_url("file:src/main.ts");

    let plain = serve(repo.path());
    let (status, _, body) = http_get(plain.port, "/api/capabilities");
    assert!(status.contains("200"), "capabilities status: {status}");
    let said: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(said["open_code"], serde_json::json!(false), "{said}");
    assert_eq!(
        said["ask"],
        serde_json::json!(false),
        "the ask boolean must keep riding beside it: {said}"
    );
    // Held to reality: the route it said is absent really is — the
    // unregistered-route 404, even for a mapped id.
    let (status, _, body) = http_get(plain.port, &url);
    assert!(status.contains("404"), "said no source, then: {status}");
    assert_eq!(
        String::from_utf8_lossy(&body),
        "not found",
        "the absent route must refuse as any unregistered path does"
    );

    let open = serve_with(repo.path(), &["--open-code"]);
    let (status, _, body) = http_get(open.port, "/api/capabilities");
    assert!(status.contains("200"), "capabilities status: {status}");
    let said: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(said["open_code"], serde_json::json!(true), "{said}");
    assert_eq!(
        said["ask"],
        serde_json::json!(false),
        "open code must not claim questions: {said}"
    );
    let (status, _, _) = http_get(open.port, &url);
    assert!(
        status.contains("200"),
        "said it serves source, then did not: {status}"
    );
}

/// HEAD mirrors GET on the source route through the registry's existing
/// derivation — no second list, so nothing here was added for HEAD to work.
/// The registry-walking HEAD test above covers the plain-serve shape (the
/// route unregistered, the asset fallback's 404); this holds the flag-on
/// shape, at the envelope's 200 and the allowlist's 404 alike.
#[test]
fn head_mirrors_get_on_the_source_route() {
    let repo = materialize("simple");
    scan(repo.path());
    let server = serve_with(repo.path(), &["--open-code"]);

    for (name, url) in [
        ("a mapped file", source_url("file:src/main.ts")),
        ("an unmapped id", source_url("file:not-in-the-map.ts")),
    ] {
        let (get_status, get_headers, get_body) = http_get(server.port, &url);
        let (head_status, head_headers, head_body) = http_head(server.port, &url);
        assert_eq!(
            head_status, get_status,
            "{name}: HEAD must carry GET's status"
        );
        for header_name in ["Content-Type", "Content-Length"] {
            assert_eq!(
                header(&head_headers, header_name),
                header(&get_headers, header_name),
                "{name}: {header_name} must match GET's"
            );
        }
        assert_eq!(
            header(&head_headers, "Content-Length").unwrap(),
            get_body.len().to_string(),
            "{name}: HEAD must promise exactly the body GET sends"
        );
        assert!(
            head_body.is_empty(),
            "{name}: HEAD must send no body, got {} bytes",
            head_body.len()
        );
    }
}

/// Story 14's rule, applied to a route: the backend failing is a response,
/// not the end of serving.
#[test]
fn a_failing_backend_answers_cleanly_and_the_server_keeps_serving() {
    let repo = materialize("simple");
    scan(repo.path());
    let server = serve_with(repo.path(), &["--ask", "--provider", "fail"]);

    let (status, body) = ask(server.port, "what is this?");
    assert!(
        status.contains("502"),
        "a backend failure is not the reader's fault: {status}"
    );
    assert!(
        body["error"].as_str().unwrap().contains("injected"),
        "the response must carry the reason: {body}"
    );

    // Still serving: the map route works, and a second question fails the
    // same clean way rather than hanging or connecting to a dead process.
    let (status, _, _) = http_get(server.port, "/api/map");
    assert!(status.contains("200"), "map status after failure: {status}");
    let (status, _) = ask(server.port, "and again?");
    assert!(status.contains("502"), "second ask status: {status}");
}

/// Criterion: gating questions on enrichment would add a way to fail for a
/// reason the reader cannot see. The map here has never been enriched — the
/// assertion below proves that rather than assuming it.
#[test]
fn a_question_is_answered_from_a_map_that_was_never_enriched() {
    let repo = materialize("simple");
    scan(repo.path());
    let outside = tempfile::tempdir().unwrap();
    let spec = format!(
        "fake:{}",
        canned(outside.path(), "Answered from mechanical prose.", &[]).display()
    );
    let server = serve_with(repo.path(), &["--ask", "--provider", &spec]);

    let (_, _, raw) = http_get(server.port, "/api/map");
    let map: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    assert!(
        map["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .all(|n| n["provenance"] == "structural"),
        "this test is only meaningful on an unenriched map"
    );

    let (status, body) = ask(server.port, "what does this project do?");
    assert!(status.contains("200"), "ask status: {status} {body}");
    assert_eq!(body["answer"], "Answered from mechanical prose.");
}

#[test]
fn an_unusable_question_is_refused_with_a_reason() {
    let repo = materialize("simple");
    scan(repo.path());
    let outside = tempfile::tempdir().unwrap();
    let spec = format!(
        "fake:{}",
        canned(outside.path(), "unreachable", &[]).display()
    );
    let server = serve_with(repo.path(), &["--ask", "--provider", &spec]);

    // Not JSON at all. The hint has to describe the whole accepted shape —
    // ticket 08 grew the body an optional `turns` array, and its crosscheck
    // handed this ticket the stale hint that still described only the bare
    // form.
    let (status, _, body) = http_post(server.port, "/api/ask", "not json");
    assert!(status.contains("400"), "malformed body status: {status}");
    let hint = String::from_utf8_lossy(&body);
    assert!(
        hint.contains("question"),
        "the refusal must show the shape expected: {hint:?}"
    );
    assert!(
        hint.contains("turns"),
        "the refusal must mention the optional turns the body accepts: {hint:?}"
    );

    // JSON, but no question in it.
    let (status, body) = ask(server.port, "   ");
    assert!(status.contains("400"), "blank question status: {status}");
    assert!(
        body["error"].as_str().unwrap().contains("empty"),
        "the refusal must say what was wrong: {body}"
    );

    // A question far past the bound the module states.
    let (status, body) = ask(server.port, &"a".repeat(5_000));
    assert!(status.contains("400"), "long question status: {status}");
    assert!(
        body["error"].as_str().unwrap().contains("limit"),
        "the refusal must state the limit: {body}"
    );
}

/// While `serve --ask` runs, any page the reader happens to have open could
/// otherwise POST to 127.0.0.1 and spend their model budget. A cross-origin
/// `fetch` or form post can only set the three "simple" content types
/// without a CORS preflight, and this server answers no `OPTIONS` — so
/// demanding `application/json` is what makes the route unreachable from
/// another origin. Same-origin callers are unaffected.
#[test]
fn a_question_sent_as_a_browser_simple_request_is_refused() {
    let repo = materialize("simple");
    scan(repo.path());
    let outside = tempfile::tempdir().unwrap();
    let spec = format!(
        "fake:{}",
        canned(outside.path(), "MUST NOT BE REACHED", &[]).display()
    );
    let server = serve_with(repo.path(), &["--ask", "--provider", &spec]);

    let body = serde_json::json!({"question": "what is this?"}).to_string();
    // The three content types a cross-origin request can set unaided.
    for simple in [
        "text/plain",
        "application/x-www-form-urlencoded",
        "multipart/form-data",
    ] {
        let (status, _, raw) = http_post_as(server.port, "/api/ask", &body, simple);
        assert!(
            status.contains("415"),
            "{simple} must be refused, got: {status}"
        );
        assert!(
            !String::from_utf8_lossy(&raw).contains("MUST NOT BE REACHED"),
            "the backend was reached anyway: {:?}",
            String::from_utf8_lossy(&raw)
        );
    }

    // The control: the same body with the demanded type is answered, so the
    // assertions above are about the header and not about the request.
    let (status, _, _) = http_post_as(server.port, "/api/ask", &body, "application/json");
    assert!(
        status.contains("200"),
        "the honest request failed: {status}"
    );
}

/// With `--ask` the server has two verbs, so the flat "only GET" refusal
/// stops being true. A reader who mistyped the route should be told the
/// right one rather than that POST is unsupported.
#[test]
fn the_method_refusal_describes_the_server_it_came_from() {
    let repo = materialize("simple");
    scan(repo.path());
    let outside = tempfile::tempdir().unwrap();
    let spec = format!("fake:{}", canned(outside.path(), "unused", &[]).display());
    let server = serve_with(repo.path(), &["--ask", "--provider", &spec]);

    let (status, _, body) = http_post(server.port, "/api/asked", "{}");
    assert!(status.contains("405"), "unknown POST status: {status}");
    let text = String::from_utf8_lossy(&body);
    assert!(
        text.contains("/api/ask"),
        "the refusal must name the route that does exist: {text:?}"
    );
}

/// Without `--ask` nothing about request handling changed. A GET carrying a
/// declared body was ignored before ADR-0009 and must still be — the body
/// cap belongs to the question route, not to the server.
#[test]
fn a_plain_serve_still_ignores_a_body_it_was_never_going_to_read() {
    let repo = materialize("simple");
    scan(repo.path());
    let server = serve(repo.path());

    // A declared body far past the question route's cap, with almost none
    // of it actually sent. A server that applied the cap before routing
    // would block draining it and then answer 413; this one never looks,
    // because a GET has no body to it, and answers the map immediately.
    let mut stream = TcpStream::connect(("127.0.0.1", server.port)).unwrap();
    write!(
        stream,
        "GET /api/map HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 200000\r\n\
         Connection: close\r\n\r\nxx"
    )
    .unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).unwrap();
    let head = String::from_utf8_lossy(&response);
    assert!(
        head.starts_with("HTTP/1.1 200"),
        "a GET must be served as before, got: {}",
        head.lines().next().unwrap_or_default()
    );
}

/// A hand-rolled server that allocates whatever a client declares is a
/// hand-rolled server with a memory bug. The cap is refused, not truncated.
#[test]
fn an_oversized_request_body_is_refused() {
    let repo = materialize("simple");
    scan(repo.path());
    let outside = tempfile::tempdir().unwrap();
    let spec = format!(
        "fake:{}",
        canned(outside.path(), "unreachable", &[]).display()
    );
    let server = serve_with(repo.path(), &["--ask", "--provider", &spec]);

    let huge = serde_json::json!({ "question": "a".repeat(200_000) }).to_string();
    let (status, _, body) = http_post(server.port, "/api/ask", &huge);
    assert!(status.contains("413"), "oversized body status: {status}");
    assert!(
        String::from_utf8_lossy(&body).contains("at most"),
        "the refusal must state the cap: {:?}",
        String::from_utf8_lossy(&body)
    );

    // The connection was read out rather than reset, and the server lives.
    let (status, _, _) = http_get(server.port, "/api/map");
    assert!(status.contains("200"), "map status after 413: {status}");
}

/// Ticket 35: a refusal is only a refusal if it arrives.
///
/// The 405 branch writes its reason and closes. A socket closed with unread
/// bytes still in its receive queue closes with an RST, and an RST tells the
/// peer's kernel to throw away what it has buffered — so the reader gets
/// `ConnectionReset` instead of the sentence naming the routes that do exist.
///
/// Repetition is the assertion, not decoration. The defect that filed this
/// ticket passed twenty-four runs in twenty-five, so a single green run says
/// nothing; [`http_post_still_arriving`] makes each round deterministic and
/// the loop makes a rare survivor visible.
#[test]
fn a_refused_method_reaches_the_client_that_asked_for_it() {
    let repo = materialize("simple");
    scan(repo.path());
    let server = serve(repo.path());

    let question = serde_json::json!({"question": "a".repeat(LARGER_THAN_ONE_READ)}).to_string();
    for round in 1..=25 {
        let (status, body) =
            http_post_still_arriving(server.port, "/api/ask", "application/json", &question)
                .unwrap_or_else(|e| panic!("round {round}: the refusal never arrived: {e}"));
        assert!(status.contains("405"), "round {round} status: {status}");
        assert!(
            String::from_utf8_lossy(&body).contains("only GET"),
            "round {round}: the refusal must say which verbs are served: {:?}",
            String::from_utf8_lossy(&body)
        );
    }

    // And the server is unharmed by twenty-five of them.
    let (status, _, _) = http_get(server.port, "/api/map");
    assert!(status.contains("200"), "map status after POSTs: {status}");
}

/// Ticket 35, the other two refusals that answer from the header block alone.
///
/// 415 is decided by `Content-Type` and 413 by the declared length, so both
/// now reply while the body is still arriving, and both need the hang-up for
/// the same reason the 405 above does. What this guards is that pairing, not
/// the race itself: before ticket 35 these two branches read the body *before*
/// deciding, so the receive queue was empty at close and this test passes
/// against the unfixed server. It fails if the hang-up is dropped while the
/// reorder stays — which is the combination a later change is most likely to
/// reach for, since the reorder is the part that looks like an optimisation.
/// The 413 round doubles as the proof that an over-long request is refused on
/// its declared length rather than after the server has read it: the body here
/// is written only after the refusal has been decided.
#[test]
fn the_question_routes_refusals_reach_the_client_too() {
    let repo = materialize("simple");
    scan(repo.path());
    let outside = tempfile::tempdir().unwrap();
    let spec = format!(
        "fake:{}",
        canned(outside.path(), "MUST NOT BE REACHED", &[]).display()
    );
    let server = serve_with(repo.path(), &["--ask", "--provider", &spec]);

    let question = serde_json::json!({"question": "a".repeat(LARGER_THAN_ONE_READ)}).to_string();
    let huge = serde_json::json!({ "question": "a".repeat(200_000) }).to_string();
    for round in 1..=10 {
        let (status, body) =
            http_post_still_arriving(server.port, "/api/ask", "text/plain", &question)
                .unwrap_or_else(|e| panic!("round {round}: the 415 never arrived: {e}"));
        assert!(status.contains("415"), "round {round} status: {status}");
        assert!(
            String::from_utf8_lossy(&body).contains("application/json"),
            "round {round}: the refusal must name the type demanded: {:?}",
            String::from_utf8_lossy(&body)
        );

        let (status, body) =
            http_post_still_arriving(server.port, "/api/ask", "application/json", &huge)
                .unwrap_or_else(|e| panic!("round {round}: the 413 never arrived: {e}"));
        assert!(status.contains("413"), "round {round} status: {status}");
        assert!(
            String::from_utf8_lossy(&body).contains("at most"),
            "round {round}: the refusal must state the cap: {:?}",
            String::from_utf8_lossy(&body)
        );
    }

    let (status, _, _) = http_get(server.port, "/api/map");
    assert!(
        status.contains("200"),
        "map status after refusals: {status}"
    );
}

/// One byte at a time, slower than the drain's per-read timeout and forever.
///
/// Ticket 35's hang-up gave the drain a timeout per read and a cap in bytes,
/// and neither bounds a loop: a client whose every read lands inside the
/// timeout simply gets another read, up to a megabyte of them. This dribbles
/// at `TRICKLE` below — well inside the 500 ms per-read bound — so every read
/// succeeds and only a deadline across the whole loop can end it. Against the
/// server that shipped, the handler thread outlives its response for days,
/// unauthenticated, on a route that needs no flag.
///
/// The trickle has to start before the response is read and continue through
/// it. A client that pauses to read is a client that has gone quiet, and going
/// quiet is the one case the per-read timeout already handled — which would
/// make this a test that passes either way.
#[test]
fn a_client_that_keeps_sending_is_hung_up_on_rather_than_drained_forever() {
    let repo = materialize("simple");
    scan(repo.path());
    let server = serve(repo.path());

    /// Between dribbled bytes: comfortably inside the server's 500 ms
    /// per-read timeout, so no read of the drain ever times out.
    const TRICKLE: Duration = Duration::from_millis(200);
    /// How long the client is willing to keep dribbling. Far past the
    /// server's one-second deadline; reaching the end of it is the failure.
    const PATIENCE: Duration = Duration::from_secs(6);
    /// The bound asserted. Not the server's deadline — the client cannot see
    /// that — but a number a loaded machine still beats and an unbounded
    /// drain never does.
    const HUNG_UP_WITHIN: Duration = Duration::from_secs(4);

    let mut stream = TcpStream::connect(("127.0.0.1", server.port)).unwrap();
    stream.set_nodelay(true).unwrap();
    // A modest declared body, sent one byte at a time and never finished: the
    // refusal is decided from the request line alone, so the server answers
    // and then drains whatever this keeps feeding it.
    write!(
        stream,
        "POST /api/ask HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\n\
         Content-Length: 4096\r\nConnection: close\r\n\r\n"
    )
    .unwrap();

    let mut dribbling = stream.try_clone().unwrap();
    let began = Instant::now();
    let trickler = thread::spawn(move || {
        while began.elapsed() < PATIENCE {
            // The server closing on us is what this is waiting for: the next
            // write after that fails, which is the only way a client learns
            // the drain has stopped.
            if dribbling
                .write_all(b"a")
                .and_then(|()| dribbling.flush())
                .is_err()
            {
                return Some(began.elapsed());
            }
            thread::sleep(TRICKLE);
        }
        None
    });

    let (status, _, body) = read_response(&mut stream).expect("the refusal never arrived");
    assert!(status.contains("405"), "status: {status}");
    assert!(
        String::from_utf8_lossy(&body).contains("only GET"),
        "the refusal must say which verbs are served: {:?}",
        String::from_utf8_lossy(&body)
    );

    let hung_up = trickler.join().unwrap();
    let hung_up = hung_up.unwrap_or_else(|| {
        panic!("the server was still draining a trickling client after {PATIENCE:?}")
    });
    assert!(
        hung_up < HUNG_UP_WITHIN,
        "the drain has no deadline: hung up after {hung_up:?}, not within {HUNG_UP_WITHIN:?}"
    );

    // And the server is unharmed by having been dribbled at.
    let (status, _, _) = http_get(server.port, "/api/map");
    assert!(
        status.contains("200"),
        "map status after trickling: {status}"
    );
}

/// The byte bound has a price, and this pins it rather than letting it be
/// discovered.
///
/// `MAX_DRAIN` stops the close-time drain at a megabyte, so a client that
/// sends more than that is still sending when the server stops reading and
/// closes — which is the reset the drain exists to avoid, back again for the
/// requests that overrun it. What the sender sees is its own write failing
/// partway, not the 413 that named the cap. That is the trade the bound makes:
/// a server that drains whatever it is sent has no bound at all.
///
/// The refusal was written and flushed before the drain began, so on Linux it
/// is usually still in the client's receive buffer afterwards — which is not
/// asserted here, because it is the kernel's ordering of a FIN and an RST
/// rather than anything this server promises, and ticket 35 exists because
/// that ordering was being leaned on.
#[test]
fn a_body_far_past_the_drain_bound_costs_the_client_its_refusal() {
    let repo = materialize("simple");
    scan(repo.path());
    let outside = tempfile::tempdir().unwrap();
    let spec = format!(
        "fake:{}",
        canned(outside.path(), "MUST NOT BE REACHED", &[]).display()
    );
    let server = serve_with(repo.path(), &["--ask", "--provider", &spec]);

    /// Past `MAX_DRAIN` (a megabyte, in `crates/codeatlas/src/serve.rs`) by a
    /// margin no pair of socket buffers can absorb — measured here, a client
    /// gets about 2.8 MB out before the write fails.
    const PAST_ANY_DRAIN: usize = 16 * 1024 * 1024;

    let mut stream = TcpStream::connect(("127.0.0.1", server.port)).unwrap();
    write!(
        stream,
        "POST /api/ask HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\n\
         Content-Length: {PAST_ANY_DRAIN}\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let sent = stream
        .write_all(&vec![b'a'; PAST_ANY_DRAIN])
        .and_then(|()| stream.flush());
    let err = sent.expect_err(
        "the whole body was accepted, so the drain read past its bound — \
         either MAX_DRAIN grew or nothing stops it",
    );
    assert!(
        matches!(
            err.kind(),
            std::io::ErrorKind::BrokenPipe | std::io::ErrorKind::ConnectionReset
        ),
        "expected the server to have stopped reading and closed, got: {err:?}"
    );

    // The server is unharmed, and a request inside the bound still gets the
    // 413 that names the cap — the limitation is the size, not the route.
    let huge = serde_json::json!({ "question": "a".repeat(200_000) }).to_string();
    let (status, _, body) = http_post(server.port, "/api/ask", &huge);
    assert!(status.contains("413"), "oversized body status: {status}");
    assert!(
        String::from_utf8_lossy(&body).contains("at most"),
        "the refusal must state the cap: {:?}",
        String::from_utf8_lossy(&body)
    );
}

/// Reads the child's kernel thread count from `/proc` — the state V1's
/// `TCPAbortOnClose` lesson says to count, because a "dropped" connection
/// that leaves a thread parked is the same defect wearing a passing test.
/// Linux-only, like the netns egress suite; CI's `ubuntu-latest` is the
/// enforcing environment.
fn thread_count(pid: u32) -> usize {
    let status = fs::read_to_string(format!("/proc/{pid}/status"))
        .expect("this test counts kernel state via /proc and needs Linux");
    status
        .lines()
        .find_map(|line| line.strip_prefix("Threads:"))
        .expect("a Threads: line in /proc/<pid>/status")
        .trim()
        .parse()
        .expect("the thread count parses")
}

/// The child's CPU spent so far — utime + stime, in clock ticks — from
/// `/proc/<pid>/stat`, parsed after the comm field's closing paren, the one
/// field allowed to contain anything. Kernel state again, like
/// [`thread_count`], and Linux-only like it: a loop that claims to pause is
/// measured by the clock the kernel charges it, never by its own green
/// assertions.
fn cpu_ticks(pid: u32) -> u64 {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat"))
        .expect("this test measures CPU via /proc and needs Linux");
    let after_comm = &stat[stat.rfind(')').expect("a comm field") + 1..];
    let fields: Vec<&str> = after_comm.split_whitespace().collect();
    // After the comm, state is field 0, so utime and stime are 11 and 12.
    let utime: u64 = fields[11].parse().expect("utime parses");
    let stime: u64 = fields[12].parse().expect("stime parses");
    utime + stime
}

/// The thread count once it has stopped moving: handler threads from earlier
/// requests die asynchronously, so a baseline is only a baseline after two
/// readings 200 ms apart agree.
fn settled_thread_count(pid: u32) -> usize {
    let mut last = thread_count(pid);
    for _ in 0..25 {
        thread::sleep(Duration::from_millis(200));
        let now = thread_count(pid);
        if now == last {
            return now;
        }
        last = now;
    }
    last
}

/// Story 18, the bound itself (ticket 12; deferred V1 ticket 38). A per-read
/// timeout is not a bound: a client that trickles one header line every few
/// seconds beats the ten-second `READ_TIMEOUT` on every read and, on the
/// server V1 shipped, held a handler thread for as long as it cared to. The
/// whole-read `REQUEST_DEADLINE` is what ends it, with a 408 naming the
/// deadline.
///
/// The proof counts kernel state, not green assertions: the child's own
/// `/proc` thread count shows a handler thread parked while the trickle runs
/// and released after the refusal, the trickler's write fails — the only way
/// a client learns its socket is gone — and the server answers a fresh
/// request promptly afterwards.
#[test]
fn a_client_that_trickles_header_lines_is_dropped_at_the_request_deadline() {
    let repo = materialize("simple");
    scan(repo.path());
    let server = serve(repo.path());
    let pid = server.child.id();

    /// Between trickled header lines: comfortably inside the server's
    /// ten-second per-read timeout, so no single read ever times out and
    /// only a deadline across the whole read can end the request.
    const TRICKLE: Duration = Duration::from_millis(600);
    /// How long the client keeps trickling — far past the server's
    /// twenty-second `REQUEST_DEADLINE`, and at one line per 600 ms it also
    /// stays under the 64-line header cap, so the deadline is the only bound
    /// this connection can trip.
    const PATIENCE: Duration = Duration::from_secs(35);
    /// The give-up margin asserted, measured from just before the request
    /// line went out. Not the server's own deadline — the client cannot see
    /// that — but a window a loaded machine still lands in and an unbounded
    /// read never does: at or after the deadline, within six seconds of it.
    const REFUSED_AFTER: Duration = Duration::from_secs(19);
    const REFUSED_WITHIN: Duration = Duration::from_secs(26);

    // Warm up, then take the thread baseline the release is measured against.
    let (status, _, _) = http_get(server.port, "/api/map");
    assert!(status.contains("200"), "warm-up status: {status}");
    let baseline = settled_thread_count(pid);

    let mut stream = TcpStream::connect(("127.0.0.1", server.port)).unwrap();
    stream.set_nodelay(true).unwrap();
    // A hang must be a failed test, not a stuck suite: if the server never
    // gives up, this read times out and the expect below names the missing
    // refusal.
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .unwrap();
    let began = Instant::now();
    write!(stream, "GET /api/map HTTP/1.1\r\nHost: 127.0.0.1\r\n").unwrap();

    let mut dribbling = stream.try_clone().unwrap();
    let trickler = thread::spawn(move || {
        let mut line = 0u32;
        while began.elapsed() < PATIENCE {
            line += 1;
            if write!(dribbling, "X-Drip-{line}: {line}\r\n")
                .and_then(|()| dribbling.flush())
                .is_err()
            {
                return Some(began.elapsed());
            }
            thread::sleep(TRICKLE);
        }
        None
    });

    // While the trickle runs, a handler thread is parked on it — counted, so
    // the release below is a release of something demonstrably held.
    thread::sleep(Duration::from_secs(2));
    assert!(
        thread_count(pid) > baseline,
        "no handler thread is holding this connection; its release would prove nothing"
    );

    let (status, _, body) = read_response(&mut stream)
        .expect("the 408 never arrived: nothing bounds the whole request read");
    let refused = began.elapsed();
    assert!(status.contains("408"), "status: {status}");
    assert!(
        String::from_utf8_lossy(&body).contains("20 seconds"),
        "the refusal must name the deadline that tripped: {:?}",
        String::from_utf8_lossy(&body)
    );
    assert!(
        refused >= REFUSED_AFTER,
        "gave up before the deadline: {refused:?}"
    );
    assert!(
        refused < REFUSED_WITHIN,
        "the deadline overshot its stated margin: refused after {refused:?}"
    );

    let write_failed = trickler.join().unwrap().unwrap_or_else(|| {
        panic!("the server was still reading a trickled head after {PATIENCE:?}")
    });
    assert!(
        write_failed < REFUSED_WITHIN + Duration::from_secs(3),
        "the socket outlived the refusal: writes still landing at {write_failed:?}"
    );
    eprintln!("trickler refused after {refused:?}, its write failed at {write_failed:?}");

    // The kernel-state half: the handler thread is gone, not merely quiet.
    let mut released = false;
    for _ in 0..50 {
        if thread_count(pid) <= baseline {
            released = true;
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    assert!(
        released,
        "the handler thread was never released — a drop that leaves a thread \
         parked is the same defect wearing a passing test"
    );

    // And the server keeps serving other requests promptly.
    let asked = Instant::now();
    let (status, _, _) = http_get(server.port, "/api/map");
    assert!(
        status.contains("200"),
        "map status after the trickler: {status}"
    );
    assert!(
        asked.elapsed() < Duration::from_secs(5),
        "the map took {:?} after a trickler was dropped",
        asked.elapsed()
    );
}

/// Story 18's second bound: an over-long header line ends the request rather
/// than growing a buffer, with a 431 naming the line cap — its own failure,
/// never folded into the count cap's or the deadline's, because the operator
/// reading a refusal wants to know which bound tripped.
#[test]
fn an_over_long_header_line_ends_the_request_instead_of_growing_a_buffer() {
    let repo = materialize("simple");
    scan(repo.path());
    let server = serve(repo.path());

    // Politely over the cap: one 9 000-byte line, past the server's 8 KiB
    // `MAX_HEADER_LINE`, sent whole so the refusal itself is readable.
    let mut stream = TcpStream::connect(("127.0.0.1", server.port)).unwrap();
    write!(
        stream,
        "GET /api/map HTTP/1.1\r\nHost: 127.0.0.1\r\nX-Longing: {}\r\n\r\n",
        "a".repeat(9_000)
    )
    .unwrap();
    let (status, _, body) = read_response(&mut stream).expect("the 431 never arrived");
    assert!(status.contains("431"), "status: {status}");
    let reason = String::from_utf8_lossy(&body);
    assert!(
        reason.contains("header line may be at most 8192 bytes"),
        "the refusal must name the line cap, and name it as its own failure: {reason:?}"
    );

    // Hostile: one line, sixteen megabytes, never a newline. The server must
    // stop reading at the cap and close — observed from here as the client's
    // own write failing partway, which a server buffering the line whole
    // could not produce.
    let mut endless = TcpStream::connect(("127.0.0.1", server.port)).unwrap();
    write!(endless, "GET /api/map HTTP/1.1\r\nX-Endless: ").unwrap();
    let err = endless
        .write_all(&vec![b'a'; 16 * 1024 * 1024])
        .and_then(|()| endless.flush())
        .expect_err("sixteen megabytes of one header line were accepted — nothing caps the line");
    assert!(
        matches!(
            err.kind(),
            std::io::ErrorKind::BrokenPipe | std::io::ErrorKind::ConnectionReset
        ),
        "expected the server to stop reading and close, got: {err:?}"
    );

    // And the server is unharmed by both.
    let (status, _, _) = http_get(server.port, "/api/map");
    assert!(
        status.contains("200"),
        "map status after the long line: {status}"
    );
}

/// Story 18's third bound: a client that sends header lines forever is told
/// to stop — a 431 naming the count cap, distinct from the line cap's
/// refusal, and past the refusal the server stops reading rather than
/// parsing headers for as long as they come.
#[test]
fn a_header_block_of_too_many_lines_is_told_to_stop() {
    let repo = materialize("simple");
    scan(repo.path());
    let server = serve(repo.path());

    // Politely over the cap: 200 short lines against `MAX_HEADER_LINES`, 64.
    let mut stream = TcpStream::connect(("127.0.0.1", server.port)).unwrap();
    let mut request = String::from("GET /api/map HTTP/1.1\r\nHost: 127.0.0.1\r\n");
    for line in 0..200 {
        request.push_str(&format!("X-Count-{line}: {line}\r\n"));
    }
    request.push_str("\r\n");
    stream.write_all(request.as_bytes()).unwrap();
    let (status, _, body) = read_response(&mut stream).expect("the 431 never arrived");
    assert!(status.contains("431"), "status: {status}");
    let reason = String::from_utf8_lossy(&body);
    assert!(
        reason.contains("at most 64 header lines"),
        "the refusal must name the count cap, its own failure and not the line cap's: {reason:?}"
    );

    // Hostile: four megabytes of header lines and never a blank one. The
    // write failing partway is the server refusing and closing; a server
    // that reads headers forever accepts every byte of this.
    let mut endless = TcpStream::connect(("127.0.0.1", server.port)).unwrap();
    write!(endless, "GET /api/map HTTP/1.1\r\n").unwrap();
    let line = format!("X-Forever: {}\r\n", "b".repeat(100));
    let mut wrote = Ok(());
    for _ in 0..40_000 {
        wrote = endless
            .write_all(line.as_bytes())
            .and_then(|()| endless.flush());
        if wrote.is_err() {
            break;
        }
    }
    let err =
        wrote.expect_err("four megabytes of header lines were accepted — nothing caps the count");
    assert!(
        matches!(
            err.kind(),
            std::io::ErrorKind::BrokenPipe | std::io::ErrorKind::ConnectionReset
        ),
        "expected the server to stop reading and close, got: {err:?}"
    );

    let (status, _, _) = http_get(server.port, "/api/map");
    assert!(
        status.contains("200"),
        "map status after the endless headers: {status}"
    );
}

/// Story 20: accept errors back off rather than spin. The forced error is
/// descriptor exhaustion, arriving the way it actually arrives: the server
/// runs under a lowered fd budget ([`serve_starved`]), more connections are
/// parked on it than the budget holds, and every accept past it fails with
/// EMFILE while the failed connection stays queued — a sustained error. The
/// spinning loop this replaces answered that with accept-fail-continue at
/// syscall speed, a full core; the backoff answers it with a bounded pause.
/// The proof is kernel state, not a green sleep: the child's own `/proc`
/// CPU clock across the window — then, because back-off is a pause and not
/// a shutdown, the pressure is lifted and the same server must answer
/// again.
#[test]
fn accept_errors_back_off_instead_of_burning_a_core() {
    let repo = materialize("simple");
    scan(repo.path());

    /// The child's whole descriptor budget: stdio, the listener, and room
    /// for a few dozen accepted connections before EMFILE.
    const FD_BUDGET: u32 = 40;
    /// Connections parked on the server: past the budget by enough that
    /// the accept queue never empties during the window.
    const PARKED: usize = 80;
    /// How long the sustained-error condition is watched.
    const WINDOW: Duration = Duration::from_secs(3);
    /// The most CPU the child may spend across the window, in clock ticks
    /// (USER_HZ is 100, so 100 ticks is one second — a third of the
    /// window). Measured 2026-08-14: the spinning loop burned the whole
    /// window, 300 ticks; the backed-off loop spent 0. The bound sits
    /// where a loaded machine cannot blur the two.
    const MAX_TICKS: u64 = 100;

    let server = serve_starved(repo.path(), FD_BUDGET);
    let pid = server.child.id();

    // Inside its budget, the starved server is an ordinary server.
    let (status, _, _) = http_get(server.port, "/api/map");
    assert!(status.contains("200"), "warm-up status: {status}");

    // Park connections until the budget is far exceeded. `connect` succeeds
    // from here the moment the kernel queues it — acceptance is what the
    // server can no longer afford.
    let held: Vec<TcpStream> = (0..PARKED)
        .map(|i| {
            TcpStream::connect(("127.0.0.1", server.port))
                .unwrap_or_else(|e| panic!("parked connection {i} refused: {e}"))
        })
        .collect();

    // Let the exhaustion establish, then read the CPU clock across the
    // window it sustains.
    thread::sleep(Duration::from_millis(500));
    let before = cpu_ticks(pid);
    thread::sleep(WINDOW);
    let burned = cpu_ticks(pid) - before;
    eprintln!("accept-error window: {burned} clock ticks over {WINDOW:?}");
    assert!(
        burned < MAX_TICKS,
        "the accept loop spent {burned} clock ticks over {WINDOW:?} under \
         sustained accept errors — spinning, not backing off"
    );

    // Back-off is a pause, never a shutdown: release the descriptors and
    // the same server must answer again.
    drop(held);
    let patience = Instant::now() + Duration::from_secs(10);
    let mut recovered = false;
    while Instant::now() < patience {
        if let Ok(mut stream) = TcpStream::connect(("127.0.0.1", server.port))
            && write!(
                stream,
                "GET /api/map HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n"
            )
            .is_ok()
            && let Ok((status, _, _)) = read_response(&mut stream)
            && status.contains("200")
        {
            recovered = true;
            break;
        }
        thread::sleep(Duration::from_millis(200));
    }
    assert!(
        recovered,
        "the server never came back once the descriptor pressure lifted — \
         back-off must degrade service, never end it"
    );
}

/// Seams 3 and 4 together: a question travelling all the way from HTTP to a
/// spawned child, asserted on the argv that child actually received.
///
/// This is the only place `CliProvider::ask`'s own choices are visible. A
/// unit test can assert what `build_args` produces when handed the answer
/// schema, but not that `ask` is the caller handing it over — the same trap
/// ticket 31 hit, where an assertion watched `build_args` while the defect
/// lived in the constructor.
#[cfg(feature = "agent-cli")]
#[test]
fn a_question_reaches_a_spawned_cli_locked_down_and_correctly_framed() {
    let repo = materialize("simple");
    scan(repo.path());
    let outside = tempfile::tempdir().unwrap();
    let spec = common::fake_cli(
        outside.path(),
        r#"{"type":"result","subtype":"success","is_error":false,
            "structured_output":{"answer":"It starts in main.ts.",
              "citations":["file:src/main.ts"]}}"#,
        0,
    );
    let server = serve_with(repo.path(), &["--ask", "--provider", &spec]);

    let (status, body) = ask(server.port, "where does the program start?");
    assert!(status.contains("200"), "ask status: {status} {body}");
    assert_eq!(body["answer"], "It starts in main.ts.");
    assert_eq!(body["citations"], serde_json::json!(["file:src/main.ts"]));

    let args = common::recorded_args(outside.path());
    let value_of = |flag: &str| -> Option<String> {
        let prefix = format!("{flag}=");
        args.iter()
            .find_map(|a| a.strip_prefix(&prefix))
            .map(str::to_string)
    };

    // The answer schema, not the enrichment one: `ask` must not have been
    // wired to whichever schema was nearest.
    let schema: serde_json::Value =
        serde_json::from_str(&value_of("--json-schema").expect("a schema was passed")).unwrap();
    assert_eq!(
        schema["required"],
        serde_json::json!(["answer", "citations"]),
        "the child was constrained by the wrong schema: {schema}"
    );

    // The lockdown holds on this path too — questions arrive over a socket,
    // so a tool-enabled child here would be reachable from the network.
    assert_eq!(value_of("--tools").as_deref(), Some(""), "{args:?}");
    assert!(args.iter().any(|a| a == "--safe-mode"), "{args:?}");
    assert!(!args.iter().any(|a| a == "--add-dir"), "{args:?}");

    // The prompt survived option parsing and carries the reader's question.
    let (prompt, rest) = args.split_last().expect("there are arguments");
    assert_eq!(rest.last().map(String::as_str), Some("--"), "{args:?}");
    assert!(
        prompt.contains("where does the program start?"),
        "the question never reached the child: {prompt}"
    );
    // And the map's nodes, but nothing from the files themselves.
    assert!(prompt.contains("file:src/main.ts"), "{prompt}");

    // ADR-0008's credential rule is not relaxed for questions.
    let lines = common::record_lines(outside.path());
    assert!(
        lines.contains(&"api-key=<unset>".to_string()),
        "the API key reached the child: {lines:?}"
    );
}

/// The transcript half of ADR-0012, observed where it lands: the spawned
/// CLI's argv. The citation tests above prove carried turns steer the
/// *slice*; this proves the turns themselves reach the model — clamped,
/// oldest first, ahead of the question — because a follow-up that says "it"
/// needs the earlier turns for "it" to mean anything.
///
/// The newline-sensitive assertions read `\r` where the prompt had `\n`:
/// the stand-in CLI records argv with newlines translated so one argument
/// stays one line (see `common::fake_cli`).
#[cfg(feature = "agent-cli")]
#[test]
fn carried_turns_reach_the_model_clamped_and_in_order() {
    let repo = materialize("simple");
    scan(repo.path());
    let outside = tempfile::tempdir().unwrap();
    let spec = common::fake_cli(
        outside.path(),
        r#"{"type":"result","subtype":"success","is_error":false,
            "structured_output":{"answer":"It is called from main.","citations":[]}}"#,
        0,
    );
    let server = serve_with(repo.path(), &["--ask", "--provider", &spec]);

    // Seven turns: the oldest must be clamped away, the survivors must ride
    // oldest-first, and the newest turn's over-long answer must arrive cut
    // to the stated bound.
    let turns: Vec<serde_json::Value> = (0..7)
        .map(|t| {
            serde_json::json!({
                "question": format!("MARK{t} what about step {t}?"),
                "answer": if t == 6 { "x".repeat(3_000) } else { format!("Step {t} answers.") },
                "citations": [],
            })
        })
        .collect();
    let (status, body) = ask_carrying(server.port, "what calls it?", serde_json::json!(turns));
    assert!(status.contains("200"), "ask status: {status} {body}");

    let args = common::recorded_args(outside.path());
    let (prompt, _) = args.split_last().expect("there are arguments");

    // The clamp, visible in what the child was handed: six turns, not seven.
    assert!(
        !prompt.contains("MARK0"),
        "the oldest turn must be clamped away, oldest first: {prompt}"
    );
    for t in 1..=6 {
        assert!(
            prompt.contains(&format!("MARK{t}")),
            "turn {t} must survive the clamp: {prompt}"
        );
    }
    // Oldest first, and the whole transcript ahead of the question.
    let position = |needle: &str| prompt.find(needle).unwrap();
    assert!(position("MARK1") < position("MARK6"), "{prompt}");
    assert!(
        position("MARK6") < position("Question: what calls it?"),
        "the transcript must ride ahead of the question: {prompt}"
    );
    // The carried answer arrives clamped to its bound plus the ellipsis —
    // 2000 x's and one `…`, never 2001.
    assert!(
        prompt.contains(&format!("{}…", "x".repeat(2_000))),
        "the over-long answer must arrive clamped"
    );
    assert!(
        !prompt.contains(&"x".repeat(2_001)),
        "the answer's bound did not hold"
    );
}

/// Usage end-to-end on the subscription path (ticket 09): the stand-in CLI
/// prints a result envelope carrying both the measured counts and the
/// `total_cost_usd` the real CLI reports — and the response carries the
/// counts while the price never reaches the wire (ADR-0012: on subscription
/// billing that number is notional, and a wrong price is worse than none).
#[cfg(feature = "agent-cli")]
#[test]
fn the_cli_envelopes_counts_reach_the_wire_and_its_price_never_does() {
    let repo = materialize("simple");
    scan(repo.path());
    let outside = tempfile::tempdir().unwrap();
    let spec = common::fake_cli(
        outside.path(),
        r#"{"type":"result","subtype":"success","is_error":false,
            "structured_output":{"answer":"It starts in main.ts.","citations":[]},
            "usage":{"input_tokens":4213,"output_tokens":57,
                     "cache_creation_input_tokens":0,"cache_read_input_tokens":9},
            "total_cost_usd":0.0731}"#,
        0,
    );
    let server = serve_with(repo.path(), &["--ask", "--provider", &spec]);

    let (status, body) = ask(server.port, "where does the program start?");
    assert!(status.contains("200"), "ask status: {status} {body}");
    assert_eq!(
        body["usage"],
        serde_json::json!({"input_tokens": 4213, "output_tokens": 57}),
        "the CLI's measured counts must arrive, and only them: {body}"
    );
    let raw = body.to_string();
    assert!(
        !raw.contains("cost") && !raw.contains("0.0731") && !raw.contains('$'),
        "the envelope's price must never reach the wire: {body}"
    );
}

/// `--ask` resolves its backend before binding, so a binary or a machine
/// that cannot answer questions says so once at startup rather than 502-ing
/// every question at a reader who has already opened the dashboard.
///
/// The sealed build's own version of this is in `scripts/sealed-probe.sh`:
/// every `cargo test` build carries `test-provider`, so the no-backend-at-all
/// branch cannot be reached from here.
#[test]
fn serve_ask_refuses_at_startup_when_no_backend_resolves() {
    let repo = materialize("simple");
    scan(repo.path());
    assert_cmd::Command::cargo_bin("codeatlas")
        .unwrap()
        .args(["serve", "--port", "0", "--ask"])
        .current_dir(repo.path())
        .env_remove(PROVIDER_ENV)
        .assert()
        .failure()
        .stderr(predicates::str::contains("serve --ask"))
        .stderr(predicates::str::contains("no enrichment provider"));

    // And it is the *first* check: which backends exist is a property of the
    // binary, so it is answered before anything about the repository.
    let bare = tempfile::tempdir().unwrap();
    assert_cmd::Command::cargo_bin("codeatlas")
        .unwrap()
        .args(["serve", "--port", "0", "--ask"])
        .current_dir(bare.path())
        .env_remove(PROVIDER_ENV)
        .assert()
        .failure()
        .stderr(predicates::str::contains("serve --ask"))
        .stderr(predicates::str::contains("codeatlas scan").not());
}

/// Spawns `serve`, reads the two lines it writes to stderr at startup, and
/// stops it. `serve_with` sends stderr to `/dev/null` to keep test output
/// readable, which is right for every test but this one.
///
/// The lines are read *before* the kill. Killing on the strength of the stdout
/// URL and then reading what survived is a race, and not a theoretical one:
/// the first version of this did exactly that and caught
/// `answering questions at POST http://127.0.0.1` with the port sliced off
/// mid-write.
///
/// Reading is bounded by quiet rather than by a line count, which matters more
/// than it looks. A count is the obvious way to write this and it *hangs* when
/// a line stops being printed — which is precisely the regression these tests
/// exist to catch. The second version of this helper deadlocked the suite
/// instead of failing it. `serve` writes its whole banner in consecutive
/// statements and then blocks accepting connections, so a second of silence
/// means there is nothing more coming.
fn startup_banner(repo: &Path, extra: &[&str]) -> String {
    let mut child = Command::cargo_bin("codeatlas")
        .unwrap()
        .args(["serve", "--port", "0"])
        .args(extra)
        .current_dir(repo)
        .env_remove(PROVIDER_ENV)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let stderr = child.stderr.take().unwrap();
    let (lines, received) = std::sync::mpsc::channel();
    thread::spawn(move || {
        let mut reader = BufReader::new(stderr);
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) | Err(_) => return,
                Ok(_) => {
                    if lines.send(line).is_err() {
                        return;
                    }
                }
            }
        }
    });
    let mut banner = String::new();
    while let Ok(line) = received.recv_timeout(Duration::from_secs(1)) {
        banner.push_str(&line);
    }
    child.kill().unwrap();
    let _ = child.wait();
    banner
}

/// Plain `serve` hides the question feature completely — no Ask button, no
/// hint in the search field, no walkthrough step — because the dashboard is
/// told by `GET /api/capabilities` that this server cannot answer. That is the
/// right call in the browser and it leaves nobody anywhere to learn the
/// feature exists, so the terminal says it, where `--ask` can be acted on.
///
/// The sealed case — a build with no backend at all, which must *not* be
/// pointed at `--ask` — is in `scripts/sealed-probe.sh` for the same reason
/// the test above is: every `cargo test` build carries `test-provider`, so
/// `recognised_specs()` is never empty here and that branch is unreachable.
#[test]
fn a_server_that_cannot_answer_says_what_would_make_one_that_can() {
    let repo = materialize("simple");
    scan(repo.path());
    assert!(
        !codeatlas::enrich::recognised_specs().is_empty(),
        "this assertion is about the branch taken when a backend exists",
    );

    let plain = startup_banner(repo.path(), &[]);
    assert!(
        plain.contains("--ask"),
        "plain serve leaves no way to discover questions: {plain}"
    );

    // And the pointer is gone once it would be noise: a server that already
    // answers must not tell the reader to restart it.
    let canned_path = canned(repo.path().parent().unwrap(), "ok", &[]);
    let spec = format!("fake:{}", canned_path.display());
    let asking = startup_banner(repo.path(), &["--ask", "--provider", &spec]);
    assert!(
        asking.contains(codeatlas::serve::ASK_ROUTE),
        "a serving --ask does not say where questions go: {asking}"
    );
    assert!(
        !asking.contains("restart with --ask"),
        "a server that answers still asks to be restarted: {asking}"
    );
}

#[test]
fn serve_refuses_to_start_without_a_map() {
    let repo = tempfile::tempdir().unwrap();
    assert_cmd::Command::cargo_bin("codeatlas")
        .unwrap()
        .args(["serve", "--port", "0"])
        .current_dir(repo.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("codeatlas scan"));
}
