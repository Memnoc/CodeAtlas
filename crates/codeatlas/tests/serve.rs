//! End-to-end tests for `codeatlas serve`: scan a fixture, run the real
//! binary as a child process, speak HTTP/1.1 to it over 127.0.0.1, and
//! assert on what comes back. Never on internals.

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

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
fn canned(dir: &Path, answer: &str, citations: &[&str]) -> PathBuf {
    let path = dir.join("canned-ask.json");
    let body = serde_json::json!({
        "ask:answer": answer,
        "ask:citations": citations.join(" "),
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
    let mut child = Command::cargo_bin("codeatlas")
        .unwrap()
        .args(["serve", "--port", "0"])
        .args(extra)
        .current_dir(repo)
        .env_remove(PROVIDER_ENV)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
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

/// Minimal HTTP/1.1 GET over a raw socket; returns (status line, headers,
/// body). Keeping the client hand-rolled means the test adds no HTTP
/// dependency and exercises the wire format directly.
fn http_get(port: u16, path: &str) -> (String, Vec<String>, Vec<u8>) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).unwrap();
    let split = response
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("no header/body separator in response");
    let head = String::from_utf8_lossy(&response[..split]).to_string();
    let body = response[split + 4..].to_vec();
    let mut lines = head.lines().map(str::to_string);
    let status = lines.next().unwrap();
    (status, lines.collect(), body)
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
    let mut response = Vec::new();
    stream.read_to_end(&mut response).unwrap();
    let split = response
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("no header/body separator in response");
    let head = String::from_utf8_lossy(&response[..split]).to_string();
    let body = response[split + 4..].to_vec();
    let mut lines = head.lines().map(str::to_string);
    let status = lines.next().unwrap();
    (status, lines.collect(), body)
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
    // absent really is. Deliberately an empty body — a POST that 405s is
    // refused without being read out, so bytes left unread in the socket can
    // reset the connection in the client's face (ticket 35). Nothing about
    // routing needs a body to prove.
    let (status, _, _) = http_post(plain.port, "/api/ask", "");
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

    // Not JSON at all.
    let (status, _, body) = http_post(server.port, "/api/ask", "not json");
    assert!(status.contains("400"), "malformed body status: {status}");
    assert!(
        String::from_utf8_lossy(&body).contains("question"),
        "the refusal must show the shape expected: {:?}",
        String::from_utf8_lossy(&body)
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
