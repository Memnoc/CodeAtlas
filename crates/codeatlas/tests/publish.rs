//! Story 18 (ADR-0007, ticket 30): enrichment somebody else already paid for
//! arrives with the repository, so cloning is all a colleague has to do.
//!
//! The mechanism is one nested ignore file. A scan writes
//! `.codeatlas/.gitignore`, which ignores everything CodeAtlas regenerates —
//! the ~790 KB map above all — and publishes the annotation store. One person
//! enriches and commits; everyone else clones and runs a plain `codeatlas
//! scan` with no credential, no network and no flags, and the store re-attaches
//! its prose for free.
//!
//! Two shapes of test, both at seam 1 (the map contract — run the binary,
//! assert on what lands on disk):
//!
//! - **`git check-ignore` is the instrument** for the classification claims.
//!   The criterion is about what git actually does with the file, and a test
//!   that string-matched the ignore file would pass just as readily on a
//!   pattern git reads differently than the author expected.
//! - **A real local clone** carries the end-to-end claim. Enriching and then
//!   deleting the map file would prove less: the map has to be absent
//!   *because git never took it*, and the store present *because git did*.
//!
//! No test here performs network I/O: the only backend selected anywhere in
//! this file is `fake:`, compiled in by the `test-provider` feature, and the
//! runs that matter select no backend at all — a test build has no default
//! provider, so a run that reached for one would fail rather than spend
//! anything.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use codeatlas::enrich::ANNOTATIONS_FILE;
use codeatlas::scan::{DEFAULT_IGNORE, IGNORE_FILE, OUTPUT_DIR};

/// The provider-selection env var the test-built binary honors. Removed from
/// every run below that is meant to have no backend.
const PROVIDER_ENV: &str = "CODEATLAS_ENRICH_PROVIDER";

/// The regenerated map. Ignored, and the whole reason the ignore file exists.
const MAP_FILE: &str = "knowledge-graph.json";

/// A node in the `simple` fixture, and the prose the fake provider buys for
/// it. One node is enough: what is under test is whether the prose travels,
/// not how much of it there is.
const ENRICHED_NODE: &str = "function:src/util.ts:greet";
const PURCHASED_PROSE: &str = "Builds the greeting string shown to a caller.";

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
/// (committed under a neutral name so it cannot affect this repository), and
/// makes it a git repository — every claim here is about what git does.
fn materialize(name: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    copy_tree(&fixture_dir(name), dir.path());
    let inert = dir.path().join("_gitignore");
    if inert.exists() {
        fs::rename(inert, dir.path().join(".gitignore")).unwrap();
    }
    git(dir.path(), &["init", "-q"]);
    git(dir.path(), &["config", "user.email", "fixture@example.com"]);
    git(dir.path(), &["config", "user.name", "Fixture"]);
    git(dir.path(), &["config", "commit.gpgsign", "false"]);
    dir
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

/// What `git check-ignore` says about a repo-relative path — the instrument
/// the acceptance criterion names, because the claim is about git's behaviour
/// and not about the text of a file.
///
/// `--no-index` asks the ignore rules alone. Without it git reports a tracked
/// path as un-ignored no matter what the rules say, which would turn the
/// second half of the clone test into a tautology.
fn ignored_by_git(repo: &Path, path: &str) -> bool {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["check-ignore", "--no-index", "-q", path])
        .output()
        .unwrap();
    match output.status.code() {
        Some(0) => true,
        Some(1) => false,
        other => panic!(
            "git check-ignore {path} answered neither ignored nor not ({other:?}): {}",
            String::from_utf8_lossy(&output.stderr)
        ),
    }
}

/// Writes a canned-responses file OUTSIDE the scanned repo and returns the
/// `fake:` provider spec selecting it.
fn canned_provider(dir: &Path, answers: &[(&str, &str)]) -> String {
    let map: serde_json::Map<String, serde_json::Value> = answers
        .iter()
        .map(|(key, text)| (key.to_string(), serde_json::Value::from(*text)))
        .collect();
    let path = dir.join("canned.json");
    fs::write(&path, serde_json::to_string_pretty(&map).unwrap()).unwrap();
    format!("fake:{}", path.display())
}

/// A plain `codeatlas scan`: no `--enrich`, no `--provider`, and the
/// provider env var explicitly removed. This is the colleague's run, and a
/// test build has no default provider, so anything that reached for one here
/// would fail loudly rather than quietly spend money.
fn plain_scan(repo: &Path) -> assert_cmd::assert::Assert {
    assert_cmd::Command::cargo_bin("codeatlas")
        .unwrap()
        .arg("scan")
        .current_dir(repo)
        .env_remove(PROVIDER_ENV)
        .assert()
}

/// `codeatlas scan --enrich` through the fake provider named by `spec`.
fn enriching_scan(repo: &Path, spec: &str) -> assert_cmd::assert::Assert {
    assert_cmd::Command::cargo_bin("codeatlas")
        .unwrap()
        .args(["scan", "--enrich", "--provider", spec])
        .current_dir(repo)
        .env_remove(PROVIDER_ENV)
        .assert()
}

fn map_path(repo: &Path) -> PathBuf {
    repo.join(OUTPUT_DIR).join(MAP_FILE)
}

fn store_path(repo: &Path) -> PathBuf {
    repo.join(OUTPUT_DIR).join(ANNOTATIONS_FILE)
}

fn ignore_path(repo: &Path) -> PathBuf {
    repo.join(OUTPUT_DIR).join(IGNORE_FILE)
}

fn read_json(path: &Path) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

/// The map's node with this id, or a panic naming it.
fn node<'m>(map: &'m serde_json::Value, id: &str) -> &'m serde_json::Value {
    map["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == id)
        .unwrap_or_else(|| panic!("node {id} missing from the map"))
}

/// Enriches `repo` through the fake provider, buying [`PURCHASED_PROSE`] for
/// [`ENRICHED_NODE`]. The canned-answers dir is returned so it outlives the
/// run; dropping it early would delete the file mid-scan.
fn buy_prose(repo: &Path) -> tempfile::TempDir {
    let canned = tempfile::tempdir().unwrap();
    let spec = canned_provider(
        canned.path(),
        &[(&format!("summary:{ENRICHED_NODE}"), PURCHASED_PROSE)],
    );
    enriching_scan(repo, &spec).success();
    assert!(store_path(repo).exists(), "enrichment wrote no store");
    canned
}

/// The ignore file names the store as a literal, because a `const` cannot
/// interpolate one. This is the guard that keeps the two spellings together:
/// renaming the store without renaming it here would publish nothing, and
/// every other test in this file would still pass, because they all ask about
/// the name the ignore file happens to carry.
#[test]
fn the_ignore_file_publishes_the_store_the_code_actually_writes() {
    let lines: Vec<&str> = DEFAULT_IGNORE.lines().collect();
    assert!(
        lines.contains(&format!("!{ANNOTATIONS_FILE}").as_str()),
        "the ignore file must un-ignore {ANNOTATIONS_FILE}: {DEFAULT_IGNORE}"
    );
    assert!(
        lines.contains(&format!("!{IGNORE_FILE}").as_str()),
        "the ignore file must un-ignore itself: {DEFAULT_IGNORE}"
    );
    assert!(
        lines.contains(&"*"),
        "everything else must be ignored by default: {DEFAULT_IGNORE}"
    );
}

#[test]
fn the_ignore_file_publishes_the_store_and_ignores_the_regenerated_map() {
    let repo = materialize("simple");
    let _canned = buy_prose(repo.path());

    assert!(
        ignore_path(repo.path()).exists(),
        "a scan must write {OUTPUT_DIR}/{IGNORE_FILE}"
    );
    assert!(
        ignored_by_git(repo.path(), &format!("{OUTPUT_DIR}/{MAP_FILE}")),
        "the regenerated map must stay ignored — it is rebuilt every run and \
         would be pure diff noise"
    );
    assert!(
        !ignored_by_git(repo.path(), &format!("{OUTPUT_DIR}/{ANNOTATIONS_FILE}")),
        "the annotation store must be committable, or nobody else ever sees \
         the prose"
    );
    assert!(
        !ignored_by_git(repo.path(), &format!("{OUTPUT_DIR}/{IGNORE_FILE}")),
        "the ignore file must travel with the repository too, or the \
         arrangement holds only on the machine that scanned"
    );
    // Anything else CodeAtlas regenerates is ignored by the same rule, so a
    // future artifact is ignored by default rather than by remembering.
    assert!(
        ignored_by_git(repo.path(), &format!("{OUTPUT_DIR}/diff-overlay.json")),
        "the default must be ignored: only the store is published"
    );
}

#[test]
fn a_missing_ignore_file_is_written_by_the_next_scan() {
    let repo = materialize("simple");
    plain_scan(repo.path()).success();
    assert_eq!(
        fs::read_to_string(ignore_path(repo.path())).unwrap(),
        DEFAULT_IGNORE
    );

    fs::remove_file(ignore_path(repo.path())).unwrap();
    plain_scan(repo.path()).success();
    assert_eq!(
        fs::read_to_string(ignore_path(repo.path())).unwrap(),
        DEFAULT_IGNORE,
        "a scan must restore the ignore file when it is absent"
    );
}

#[test]
fn an_edited_ignore_file_is_never_clobbered_and_its_decision_stands() {
    let repo = materialize("simple");
    let _canned = buy_prose(repo.path());

    // Someone decides their prose is not going into git. Overwriting that
    // every scan would silently discard a real decision — ADR-0007 says every
    // scan writes the file and does not settle this case; ticket 30 settles
    // it here.
    let mine = "# not publishing this\n*\n";
    fs::write(ignore_path(repo.path()), mine).unwrap();
    plain_scan(repo.path()).success();

    assert_eq!(
        fs::read_to_string(ignore_path(repo.path())).unwrap(),
        mine,
        "a scan clobbered an edited ignore file"
    );
    assert!(
        ignored_by_git(repo.path(), &format!("{OUTPUT_DIR}/{ANNOTATIONS_FILE}")),
        "surviving on disk is not enough: the edit has to be what git obeys"
    );
}

#[test]
fn a_clone_gets_the_prose_with_no_credential_and_no_provider() {
    let origin = materialize("simple");
    let _canned = buy_prose(origin.path());

    // What one person commits. `git add -A` obeys the ignore file just
    // written, so which artifacts travel is the mechanism's decision and not
    // the test's.
    git(origin.path(), &["add", "-A"]);
    git(origin.path(), &["commit", "-qm", "enriched"]);

    let elsewhere = tempfile::tempdir().unwrap();
    let clone = elsewhere.path().join("clone");
    let output = Command::new("git")
        .args(["clone", "-q"])
        .arg(origin.path())
        .arg(&clone)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git clone failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // The colleague's starting position: the prose, and no map at all.
    assert!(
        store_path(&clone).exists(),
        "the annotation store did not travel — the whole story is that it does"
    );
    assert!(
        !map_path(&clone).exists(),
        "the regenerated map travelled; it must not, at ~790 KB per commit"
    );

    // No --enrich, no --provider, no env var, no network: exactly what
    // someone with no credential can run.
    plain_scan(&clone).success();

    let map = read_json(&map_path(&clone));
    let greet = node(&map, ENRICHED_NODE);
    assert_eq!(
        greet["summary"], PURCHASED_PROSE,
        "the clone's plain scan did not re-attach the purchased prose"
    );
    assert_eq!(
        greet["provenance"], "llm",
        "prose that came from a model must still say so in the clone"
    );
}

#[test]
fn the_store_records_what_produced_its_prose() {
    let repo = materialize("simple");
    let _canned = buy_prose(repo.path());
    let store = read_json(&store_path(repo.path()));

    assert_eq!(
        store["version"], 2,
        "the provenance fields are additive: bumping the store version would \
         throw away the carry-over of every store already written"
    );
    assert_eq!(
        store["produced_by"]["provider"], "fake",
        "the store must name the backend that produced its prose: {store}"
    );
    let date = store["produced_by"]["date"]
        .as_str()
        .unwrap_or_else(|| panic!("the store must date its prose: {store}"));
    let parts: Vec<&str> = date.split('-').collect();
    assert!(
        parts.len() == 3
            && parts[0].len() == 4
            && parts[1].len() == 2
            && parts[2].len() == 2
            && parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit())),
        "the date must be an ISO calendar date a reviewer can read: {date:?}"
    );
    assert!(
        date >= "2026-01-01",
        "the date must be the run's, not an epoch fallback: {date:?}"
    );
}

#[test]
fn a_store_written_before_the_provenance_fields_still_reattaches() {
    let repo = materialize("simple");
    let _canned = buy_prose(repo.path());

    // Roll the store back to the shape a binary before ticket 30 wrote:
    // everything as it is now, minus the fields this ticket added. Testing
    // the real file rather than reasoning about serde defaults is the point —
    // a store already in someone's repository has to keep working.
    let mut store = read_json(&store_path(repo.path()));
    assert!(
        store
            .as_object_mut()
            .unwrap()
            .remove("produced_by")
            .is_some(),
        "nothing was rolled back, so this proves nothing: {store}"
    );
    fs::write(
        store_path(repo.path()),
        serde_json::to_string_pretty(&store).unwrap(),
    )
    .unwrap();

    // And discard the map, as a clone of a repository holding that store
    // would: the prose has to come from the store alone.
    fs::remove_file(map_path(repo.path())).unwrap();
    plain_scan(repo.path()).success();

    let map = read_json(&map_path(repo.path()));
    let greet = node(&map, ENRICHED_NODE);
    assert_eq!(
        greet["summary"], PURCHASED_PROSE,
        "an old-shaped store stopped re-attaching"
    );
    assert_eq!(greet["provenance"], "llm");
}
