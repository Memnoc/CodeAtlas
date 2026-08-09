//! End-to-end tests for `codeatlas serve`: scan a fixture, run the real
//! binary as a child process, speak HTTP/1.1 to it over 127.0.0.1, and
//! assert on what comes back. Never on internals.

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use assert_cmd::cargo::CommandCargoExt;

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

fn scan(repo: &Path) {
    assert_cmd::Command::cargo_bin("codeatlas")
        .unwrap()
        .arg("scan")
        .current_dir(repo)
        .assert()
        .success();
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
    let mut child = Command::cargo_bin("codeatlas")
        .unwrap()
        .args(["serve", "--port", "0"])
        .current_dir(repo)
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
