//! `codeatlas diff` (spec story 7): git diff in, overlay artifact out.
//! Entirely deterministic — zero LLM, zero network. The only external
//! process is `git` itself, shelled out to rather than linked (libgit2 would
//! be a large dependency for three read-only plumbing calls, and the
//! system's git is already the source of truth for what changed).
//!
//! # Git semantics
//!
//! "Changed" means the working tree differs from `HEAD`:
//!
//! - `git diff --name-only HEAD` — staged and unstaged edits, additions,
//!   and deletions of tracked files, in one pass;
//! - plus `git ls-files --others --exclude-standard` — untracked files that
//!   are not ignored.
//!
//! In a repository with no commits yet (unborn `HEAD`) there is no baseline
//! to diff against, so every file git knows about — staged or untracked,
//! not ignored — counts as changed.
//!
//! Both commands run with `-z` (NUL-separated, unquoted paths) and relative
//! to the scanned root, so the paths line up with the map's repo-relative
//! node paths even when the root is a subdirectory of the git work tree.
//! Paths under `.codeatlas/` are excluded: the artifact directory is never
//! mapped, and the overlay must not report the map or itself as a change.
//!
//! # Overlay artifact
//!
//! Written to `.codeatlas/diff-overlay.json`. Internal format — NOT part of
//! the map contract — but versioned and deterministic (sorted arrays,
//! pretty-printed, trailing newline), matching the annotation store's
//! conventions:
//!
//! - `changed`: node IDs of file nodes for changed paths plus their
//!   contained symbol nodes, sorted;
//! - `affected`: the one-hop blast radius — every node connected to a
//!   changed node by any edge, in either direction, excluding the changed
//!   set itself — sorted;
//! - `unmapped_paths`: changed paths with no node in the map (files the
//!   scan skipped, or paths deleted since the scan), noted rather than
//!   silently dropped so nothing dangles and nothing disappears.
//!
//! Map staleness is deliberately not this module's problem: the overlay
//! projects changed paths onto whatever map exists.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};

use crate::map::KnowledgeGraph;
use crate::scan::OUTPUT_DIR;

/// The overlay's file name under [`OUTPUT_DIR`].
pub const OVERLAY_FILE: &str = "diff-overlay.json";

/// Bumped whenever the overlay format changes; consumers ignore overlays
/// with a version they do not know.
const OVERLAY_VERSION: u32 = 1;

/// The diff impact overlay: changed nodes, their one-hop blast radius, and
/// changed paths the map does not cover. Internal format, not the map
/// contract; every array is sorted for deterministic output.
#[derive(Debug, Serialize, Deserialize)]
pub struct DiffOverlay {
    pub version: u32,
    pub changed: Vec<String>,
    pub affected: Vec<String>,
    pub unmapped_paths: Vec<String>,
}

/// Runs the whole diff pipeline for `root`: load the map, derive changed
/// paths from git, project them onto the graph, write the overlay. Returns
/// the overlay for reporting.
pub fn run(root: &Path) -> Result<DiffOverlay> {
    let root = root
        .canonicalize()
        .with_context(|| format!("cannot diff {}", root.display()))?;

    let map_path = root.join(OUTPUT_DIR).join("knowledge-graph.json");
    let raw = fs::read_to_string(&map_path).map_err(|_| {
        anyhow!(
            "no map at {} — run `codeatlas scan {}` first",
            map_path.display(),
            root.display()
        )
    })?;
    let graph: KnowledgeGraph = serde_json::from_str(&raw)
        .with_context(|| format!("cannot parse the map at {}", map_path.display()))?;

    let changed_paths = changed_paths(&root)?;
    let overlay = compute(&graph, &changed_paths);
    save(&root, &overlay)?;
    Ok(overlay)
}

/// The changed-path set per the module docs: working tree vs `HEAD` plus
/// untracked files; everything git knows about when `HEAD` is unborn.
fn changed_paths(root: &Path) -> Result<BTreeSet<String>> {
    // A clear refusal beats git's own "fatal:" when the root is no repo.
    let inside = run_git(root, &["rev-parse", "--is-inside-work-tree"])?;
    if !inside.status.success() {
        bail!(
            "{} is not inside a git work tree — `codeatlas diff` derives \
             changes from git (stderr: {})",
            root.display(),
            String::from_utf8_lossy(&inside.stderr).trim()
        );
    }

    let head_exists = run_git(root, &["rev-parse", "--verify", "--quiet", "HEAD"])?
        .status
        .success();

    let mut paths = BTreeSet::new();
    if head_exists {
        paths.extend(git_paths(
            root,
            &["diff", "--name-only", "-z", "--relative", "HEAD"],
        )?);
        paths.extend(git_paths(
            root,
            &["ls-files", "--others", "--exclude-standard", "-z"],
        )?);
    } else {
        // Unborn HEAD: no baseline exists, so everything git knows about
        // (staged or untracked, not ignored) is a change.
        paths.extend(git_paths(
            root,
            &[
                "ls-files",
                "--cached",
                "--others",
                "--exclude-standard",
                "-z",
            ],
        )?);
    }
    // The artifact directory is never mapped and must not report itself.
    let artifact_prefix = format!("{OUTPUT_DIR}/");
    paths.retain(|p| p != OUTPUT_DIR && !p.starts_with(&artifact_prefix));
    Ok(paths)
}

/// Runs one git command in `root`, translating an absent git binary into a
/// clear error instead of a raw io::Error.
fn run_git(root: &Path, args: &[&str]) -> Result<std::process::Output> {
    Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(|err| {
            if err.kind() == std::io::ErrorKind::NotFound {
                anyhow!("`codeatlas diff` needs the `git` binary on PATH, and none was found")
            } else {
                anyhow!("cannot run git: {err}")
            }
        })
}

/// Runs one git command that must succeed and returns its NUL-separated
/// stdout as repo-relative paths with forward slashes.
fn git_paths(root: &Path, args: &[&str]) -> Result<Vec<String>> {
    let output = run_git(root, args)?;
    if !output.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .split('\0')
        .filter(|p| !p.is_empty())
        .map(|p| p.replace('\\', "/"))
        .collect())
}

/// Pure projection of changed paths onto the graph: changed nodes are every
/// node whose path changed (the file node and its contained symbols);
/// affected nodes are the one-hop blast radius over edges of ANY kind, in
/// either direction, minus the changed set. All output sorted.
pub fn compute(graph: &KnowledgeGraph, changed_paths: &BTreeSet<String>) -> DiffOverlay {
    let mut nodes_by_path: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for node in &graph.nodes {
        nodes_by_path
            .entry(node.path.as_str())
            .or_default()
            .push(node.id.as_str());
    }

    let mut changed: BTreeSet<&str> = BTreeSet::new();
    let mut unmapped: BTreeSet<&str> = BTreeSet::new();
    for path in changed_paths {
        match nodes_by_path.get(path.as_str()) {
            Some(ids) => changed.extend(ids),
            None => {
                unmapped.insert(path);
            }
        }
    }

    let mut affected: BTreeSet<&str> = BTreeSet::new();
    for edge in &graph.edges {
        let (source, target) = (edge.source.as_str(), edge.target.as_str());
        if changed.contains(source) && !changed.contains(target) {
            affected.insert(target);
        }
        if changed.contains(target) && !changed.contains(source) {
            affected.insert(source);
        }
    }

    let owned = |set: BTreeSet<&str>| set.into_iter().map(str::to_string).collect();
    DiffOverlay {
        version: OVERLAY_VERSION,
        changed: owned(changed),
        affected: owned(affected),
        unmapped_paths: owned(unmapped),
    }
}

/// Writes the overlay deterministically (sorted arrays already, pretty,
/// trailing newline) to `.codeatlas/diff-overlay.json`.
fn save(root: &Path, overlay: &DiffOverlay) -> Result<()> {
    let dir = root.join(OUTPUT_DIR);
    fs::create_dir_all(&dir)?;
    let mut json = serde_json::to_string_pretty(overlay)?;
    json.push('\n');
    fs::write(dir.join(OVERLAY_FILE), json)?;
    Ok(())
}
