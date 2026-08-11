//! Markdown link edges. Markdown files contribute no symbols — only edges to
//! the in-map files their relative links reference (ticket 04).
//!
//! Decision: link extraction is a small hand parser, not a tree-sitter
//! grammar. The ticket mandates grammars for the programming languages;
//! for Markdown the only fact worth extracting is `[text](target)` (plus
//! reference definitions `[label]: target`), and ADR-0006's constraint —
//! nothing downloaded at runtime — is satisfied trivially by having no
//! grammar at all.

use std::collections::HashSet;

use super::{Analysis, Import, Parser};

pub(super) struct Markdown;

pub(super) fn parsers() -> Vec<Box<dyn Parser>> {
    vec![Box::new(Markdown)]
}

impl Parser for Markdown {
    fn language_name(&self) -> &'static str {
        "Markdown"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["md", "markdown"]
    }

    fn parse(&self, source: &str) -> Analysis {
        let mut analysis = Analysis::default();
        let mut in_fence = false;
        for line in source.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
                in_fence = !in_fence;
                continue;
            }
            if in_fence {
                continue;
            }
            for target in line_link_targets(line) {
                if let Some(specifier) = file_target(target) {
                    analysis.imports.push(Import {
                        specifier,
                        names: Vec::new(),
                        namespaces: Vec::new(),
                    });
                }
            }
        }
        analysis
    }

    /// Relative-path resolution to any file in the map — markdown links may
    /// point at code and documents alike. Exact matches only: no extension
    /// inference, no index convention.
    fn resolve_import(
        &self,
        importer: &str,
        specifier: &str,
        files: &HashSet<String>,
        _root: &std::path::Path,
    ) -> Option<String> {
        let mut parts: Vec<&str> = importer.split('/').collect();
        parts.pop(); // drop the linking file's name
        for segment in specifier.split('/') {
            match segment {
                "" | "." => {}
                ".." => {
                    parts.pop()?; // escaping the repo root is unresolvable
                }
                s => parts.push(s),
            }
        }
        let candidate = parts.join("/");
        files.contains(&candidate).then_some(candidate)
    }
}

/// The raw targets of inline links `[text](target ...)` and reference
/// definitions `[label]: target` on one line.
fn line_link_targets(line: &str) -> Vec<&str> {
    let mut targets = Vec::new();

    // Reference definition: `[label]: target` at the start of the line.
    let trimmed = line.trim_start();
    if trimmed.starts_with('[')
        && let Some(close) = trimmed.find("]:")
    {
        let target = trimmed[close + 2..].trim();
        let target = target.split_whitespace().next().unwrap_or("");
        if !target.is_empty() {
            targets.push(target);
        }
    }

    // Inline links: every `](…)` on the line.
    let mut rest = line;
    while let Some(open) = rest.find("](") {
        let after = &rest[open + 2..];
        let Some(close) = after.find(')') else {
            break;
        };
        // An optional title (`(target "title")`) follows whitespace.
        let target = after[..close].split_whitespace().next().unwrap_or("");
        if !target.is_empty() {
            targets.push(target);
        }
        rest = &after[close + 1..];
    }
    targets
}

/// Filters a raw link target down to a file path candidate: URLs, absolute
/// paths, and pure anchors are discarded; `<…>` wrappers and `#fragment` /
/// `?query` suffixes are stripped.
fn file_target(raw: &str) -> Option<String> {
    let raw = raw.trim_matches(['<', '>']);
    let path = raw.split(['#', '?']).next().unwrap_or_default().trim();
    if path.is_empty() || path.starts_with('/') {
        return None;
    }
    // A scheme (`https://…`, `mailto:…`) marks an external target.
    if path
        .split_once(':')
        .is_some_and(|(scheme, _)| !scheme.contains('/'))
    {
        return None;
    }
    Some(path.to_string())
}
