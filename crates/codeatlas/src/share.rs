//! `codeatlas share`: one self-contained, redacted HTML file (ticket 14,
//! ADR-0006, spec stories 8 and 10).
//!
//! The artifact is the embedded dashboard (the same production build
//! `codeatlas serve` ships) with its script and stylesheet inlined and the
//! redacted map embedded as an inline JSON `<script>` — the app detects the
//! embedded payload and never fetches, so double-clicking the file works
//! from `file://` with zero external requests and zero servers. The diff
//! overlay is a live-workspace feature (`/api/diff`) and is deliberately
//! absent from share artifacts; the in-artifact banner says so.
//!
//! # Redaction is an allowlist
//!
//! Every property path of every map-contract type is classified in
//! [`FIELD_CLASSIFICATIONS`] — the one table auditors read. The
//! schema-derived exhaustiveness test (tests/share.rs) walks the contract
//! schema and fails when any path is missing from the table, so a new field
//! cannot ship unclassified; any field the table does not know is dropped at
//! redaction time (deny by default).
//!
//! # V1 classification posture
//!
//! Structure — IDs, kinds, edges, weights, ranges, layer membership, flow
//! steps, provenance — is share-safe: it names code locations without
//! quoting or paraphrasing code. `project.name`, file `path`s and `name`s
//! are structural and share-safe under the spec's story-8 assumption that
//! the recipient is a colleague with repository access; a future audience
//! knob can revisit that. Prose slots (`Node.summary`, `Layer.name`,
//! `DomainFlow.name`, `TourStep.label`) are provenance-conditional:
//! mechanical prose ("Rust file, 214 lines: 3 functions") only restates
//! structure and passes through, while LLM-enriched prose may paraphrase
//! proprietary logic and is replaced with [`REDACTION_MARKER`]. Missing or
//! unreadable provenance fails closed (redacted).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

use crate::scan::OUTPUT_DIR;

/// File name of the artifact, written under `.codeatlas/`.
pub const SHARE_FILE: &str = "share.html";

/// What a redacted string value is replaced with — replacement, not
/// removal, keeps the map valid against the contract schema.
pub const REDACTION_MARKER: &str = "[redacted]";

/// How a contract field is treated by the share artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Classification {
    /// Ships verbatim: structural data that locates code without quoting it.
    ShareSafe,
    /// Enrichable prose slot: ships when provenance is `structural`,
    /// replaced with [`REDACTION_MARKER`] when provenance is `llm` (or
    /// unreadable — fail closed).
    RedactedWhenLlm,
}

use Classification::{RedactedWhenLlm, ShareSafe};

/// THE redaction allowlist: every `Type.property` path of the map contract,
/// classified. The exhaustiveness test derives the required set of paths
/// from the contract schema itself, so editing map.rs without editing this
/// table fails `cargo test`.
pub const FIELD_CLASSIFICATIONS: &[(&str, Classification)] = &[
    // Top-level structure: the version string and the collections are
    // containers; their elements are governed by their own rows.
    ("KnowledgeGraph.version", ShareSafe),
    ("KnowledgeGraph.project", ShareSafe),
    ("KnowledgeGraph.nodes", ShareSafe),
    ("KnowledgeGraph.edges", ShareSafe),
    ("KnowledgeGraph.layers", ShareSafe),
    ("KnowledgeGraph.domain_flows", ShareSafe),
    ("KnowledgeGraph.tour", ShareSafe),
    // The project name is structural (story-8 assumption: the recipient
    // already has repository access).
    ("Project.name", ShareSafe),
    // Node structure is safe; the summary is the enrichable prose slot.
    ("Node.id", ShareSafe),
    ("Node.kind", ShareSafe),
    ("Node.name", ShareSafe),
    ("Node.path", ShareSafe),
    ("Node.summary", RedactedWhenLlm),
    ("Node.range", ShareSafe),
    ("Node.layer", ShareSafe),
    ("Node.provenance", ShareSafe),
    // Edges are pure structure.
    ("Edge.source", ShareSafe),
    ("Edge.target", ShareSafe),
    ("Edge.kind", ShareSafe),
    ("Edge.weight", ShareSafe),
    // Layer membership and IDs are directory-derived structure; the display
    // name is the enrichable slot.
    ("Layer.id", ShareSafe),
    ("Layer.name", RedactedWhenLlm),
    ("Layer.provenance", ShareSafe),
    // Flow IDs, domains, and steps are projected from the call graph; the
    // display name is the enrichable slot.
    ("DomainFlow.id", ShareSafe),
    ("DomainFlow.name", RedactedWhenLlm),
    ("DomainFlow.domain", ShareSafe),
    ("DomainFlow.steps", ShareSafe),
    ("DomainFlow.provenance", ShareSafe),
    // Tour order is topology-derived; the label is the enrichable slot.
    ("TourStep.node", ShareSafe),
    ("TourStep.label", RedactedWhenLlm),
    ("TourStep.provenance", ShareSafe),
    // Line ranges are structural.
    ("Range.start_line", ShareSafe),
    ("Range.end_line", ShareSafe),
];

fn classification_for(path: &str) -> Option<Classification> {
    FIELD_CLASSIFICATIONS
        .iter()
        .find(|(p, _)| *p == path)
        .map(|(_, c)| *c)
}

/// The outcome of redacting a map: the redacted map plus the disclosure —
/// which fields lost values and how many, sorted by field path. Dropped
/// unknown fields are counted here too.
pub struct Redaction {
    pub map: Value,
    pub redacted: Vec<(String, u64)>,
}

/// Applies the allowlist to a map, walking it with the contract schema so
/// object types are identified by their `$defs` entry — the same shape the
/// exhaustiveness test walks.
pub fn redact(map: &Value) -> Redaction {
    let schema = crate::map::contract_schema();
    let root = schema["title"].as_str().expect("contract schema has title");
    let mut map = map.clone();
    let mut counts = BTreeMap::new();
    redact_object(root, &mut map, &schema, &mut counts);
    Redaction {
        map,
        redacted: counts.into_iter().collect(),
    }
}

/// Resolves the `$defs` type a property's values have, if any: through
/// direct `$ref`, array `items`, or `anyOf` (Option fields).
fn ref_target<'s>(prop_schema: &'s Value) -> Option<&'s str> {
    let deref = |v: &'s Value| v["$ref"].as_str()?.strip_prefix("#/$defs/");
    deref(prop_schema)
        .or_else(|| prop_schema.get("items").and_then(deref))
        .or_else(|| prop_schema.get("anyOf")?.as_array()?.iter().find_map(deref))
}

fn redact_object(
    type_name: &str,
    value: &mut Value,
    schema: &Value,
    counts: &mut BTreeMap<String, u64>,
) {
    let type_schema = if type_name == schema["title"].as_str().unwrap_or_default() {
        schema
    } else {
        &schema["$defs"][type_name]
    };
    let Some(object) = value.as_object_mut() else {
        return;
    };
    // Prose slots fail closed: anything but literal `structural` provenance
    // is treated as enriched.
    let mechanical = object.get("provenance").and_then(Value::as_str) == Some("structural");

    let fields: Vec<String> = object.keys().cloned().collect();
    for field in fields {
        let path = format!("{type_name}.{field}");
        match classification_for(&path) {
            // Deny by default: a field the table has never classified is
            // dropped from the artifact and disclosed.
            None => {
                object.remove(&field);
                *counts.entry(path).or_insert(0) += 1;
            }
            Some(RedactedWhenLlm) if !mechanical => {
                object[&field] = Value::String(REDACTION_MARKER.to_string());
                *counts.entry(path).or_insert(0) += 1;
            }
            Some(RedactedWhenLlm) => {}
            Some(ShareSafe) => {
                let Some(child) = ref_target(&type_schema["properties"][&field]) else {
                    continue;
                };
                match &mut object[&field] {
                    Value::Array(items) => {
                        for item in items {
                            redact_object(child, item, schema, counts);
                        }
                    }
                    item => redact_object(child, item, schema, counts),
                }
            }
        }
    }
}

/// What `share` did, for the CLI to report.
pub struct Summary {
    pub path: PathBuf,
    pub redacted: Vec<(String, u64)>,
}

/// Reads `<root>/.codeatlas/knowledge-graph.json`, redacts it, and writes
/// the self-contained artifact to `<root>/.codeatlas/share.html`.
pub fn run(root: &Path) -> Result<Summary> {
    let map_path = root.join(OUTPUT_DIR).join("knowledge-graph.json");
    let raw = fs::read_to_string(&map_path).with_context(|| {
        format!(
            "no map at {} — run `codeatlas scan {}` first",
            map_path.display(),
            root.display()
        )
    })?;
    let map: Value = serde_json::from_str(&raw)
        .with_context(|| format!("invalid map at {}", map_path.display()))?;

    let redaction = redact(&map);
    let policy: Vec<&str> = FIELD_CLASSIFICATIONS
        .iter()
        .filter(|(_, c)| *c == RedactedWhenLlm)
        .map(|(p, _)| *p)
        .collect();
    let payload = json!({
        "map": redaction.map,
        "redaction": {
            "marker": REDACTION_MARKER,
            "policy": policy,
            "redacted": redaction
                .redacted
                .iter()
                .map(|(field, count)| json!({ "field": field, "count": count }))
                .collect::<Vec<_>>(),
        },
    });

    let html = build_html(&payload)?;
    let out = root.join(OUTPUT_DIR).join(SHARE_FILE);
    fs::write(&out, html).with_context(|| format!("cannot write {}", out.display()))?;
    Ok(Summary {
        path: out,
        redacted: redaction.redacted,
    })
}

/// Builds the single-file HTML: the embedded dashboard's `index.html` with
/// the share payload injected and every referenced asset inlined.
///
/// Every edit position is computed against the pristine `index.html` and
/// applied back-to-front, so inlined content — which may legitimately
/// contain strings like `/index.html` or `/assets/…` when the scanned repo
/// has such files — is never re-scanned for tags.
fn build_html(payload: &Value) -> Result<String> {
    let index = crate::serve::ASSETS
        .iter()
        .find(|a| a.path == "index.html")
        .context("embedded dashboard has no index.html")?;
    let html = std::str::from_utf8(index.bytes).context("embedded index.html is not UTF-8")?;

    // (start, end, replacement) edits against the pristine document.
    let mut edits: Vec<(usize, usize, String)> = Vec::new();

    // Escaping `<` (as the JSON string escape <) makes it impossible
    // for map content to smuggle `</script>` or `<!--` into the document.
    let payload_json = serde_json::to_string(payload)?.replace('<', "\\u003c");
    let head_end = html
        .find("</head>")
        .context("embedded index.html has no </head>")?;
    edits.push((
        head_end,
        head_end,
        format!(
            "<script id=\"codeatlas-share-data\" type=\"application/json\">{payload_json}</script>\n  "
        ),
    ));

    for asset in crate::serve::ASSETS {
        if asset.path == "index.html" {
            continue;
        }
        // Only a quoted attribute value is a reference tag; Vite emits
        // exactly this shape for its own assets.
        let url = format!("\"/{}\"", asset.path);
        let Some(pos) = html.find(&url) else { continue };
        let tag_start = html[..pos].rfind('<').context("asset URL outside a tag")?;
        let content = std::str::from_utf8(asset.bytes)
            .with_context(|| format!("embedded asset {} is not UTF-8", asset.path))?;
        if asset.path.ends_with(".js") || asset.path.ends_with(".mjs") {
            // `<\/script` is identical to `</script` inside JS strings and
            // regexes, and a bare `</script` cannot appear in valid module
            // code — so this closes the only script-breakout vector.
            let tag_end = tag_start
                + html[tag_start..]
                    .find("</script>")
                    .context("unclosed script tag")?
                + "</script>".len();
            let js = content.replace("</script", "<\\/script");
            edits.push((
                tag_start,
                tag_end,
                format!("<script type=\"module\">{js}</script>"),
            ));
        } else if asset.path.ends_with(".css") {
            if content.contains("</style") {
                bail!(
                    "embedded stylesheet {} contains '</style' and cannot be inlined",
                    asset.path
                );
            }
            let tag_end = tag_start + html[tag_start..].find('>').context("unclosed tag")? + 1;
            edits.push((tag_start, tag_end, format!("<style>{content}</style>")));
        } else {
            // No other asset kinds exist today; refuse to emit a silently
            // broken artifact if one appears.
            bail!(
                "embedded asset {} has a type the share inliner does not support",
                asset.path
            );
        }
    }

    // Back-to-front so earlier positions stay valid.
    edits.sort_by_key(|(start, _, _)| std::cmp::Reverse(*start));
    let mut out = html.to_string();
    for (start, end, replacement) in edits {
        out.replace_range(start..end, &replacement);
    }
    Ok(out)
}
