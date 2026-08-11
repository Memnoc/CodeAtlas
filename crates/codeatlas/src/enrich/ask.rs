//! Questions about a map, answered from a bounded slice of the map alone
//! (ADR-0009, spec story 21).
//!
//! A newcomer does not know what to search *for*, which is exactly when
//! name-matching search is least useful. This module turns a question in
//! ordinary words into a small, mechanically selected set of nodes, hands
//! those to the same [`EnrichmentProvider`] the enrichment path uses, and
//! hands back prose plus the node IDs it was drawn from.
//!
//! # The bound is the point
//!
//! ADR-0004's standing promise is that the model never receives the whole
//! serialized graph. An unbounded question feeding an unbounded retrieval
//! step is the obvious way to lose that by accident, so three limits are
//! enforced here and nowhere else: [`MAX_QUESTION_CHARS`] on what the reader
//! may send, [`CONTEXT_NODES`] on how many nodes may accompany it, and
//! [`MAX_SUMMARY_CHARS`] on how large one of those nodes may be. All are
//! hard caps rather than targets — [`select_context`] truncates on both
//! axes, so no phrasing of a question and no map can widen the slice.
//!
//! The third exists because the first two bound the prompt in *nodes* and
//! not in *bytes*: every summary CodeAtlas writes is a sentence or less, but
//! a map from another producer (spec story 16) may carry any string the
//! schema allows.
//!
//! What rides along is what the enrichment prompt already carries: a node's
//! ID, kind, name, repo-relative path, and the summary the map already holds
//! (mechanical or enriched). Never file contents — this module never reads a
//! file, and takes the graph as its only source.
//!
//! Compiled unconditionally. Selection is pure and opens nothing; a build
//! with no provider simply has no way to reach [`answer`] (`serve --ask`
//! refuses at startup).

use std::collections::BTreeSet;

use anyhow::{Result, bail};

use super::EnrichmentProvider;
use crate::map::{KnowledgeGraph, NodeKind};

/// The most nodes that may accompany a question. Chosen the way
/// [`super::BATCH_SIZE`] was: large enough that a real question about a real
/// repository has its answer somewhere in the slice, small enough that the
/// prompt stays a few KB and cannot grow with the repository. A 40-node
/// slice of id/name/path/summary is comparable in size to one enrichment
/// batch.
pub const CONTEXT_NODES: usize = 40;

/// The longest question accepted. A question is prose from a reader, and
/// prose from a reader is unbounded input reaching a prompt; 1000 characters
/// is several sentences and still a rounding error beside the context slice.
/// Over-long questions are refused rather than truncated — a truncated
/// question is a different question, answered without saying so.
pub const MAX_QUESTION_CHARS: usize = 1000;

/// The longest summary one context entry carries. Capping the node *count*
/// alone bounds the slice only if each node is small, which is true of every
/// summary CodeAtlas writes — mechanical ones are a dozen words and enriched
/// ones are a sentence — but not of a map from another producer (story 16),
/// where a summary is any string the schema allows. Without this the prompt
/// is bounded in nodes and unbounded in bytes.
///
/// Truncated rather than refused, unlike a question: a reader can rephrase a
/// question, and can do nothing about a node's prose.
pub const MAX_SUMMARY_CHARS: usize = 400;

/// One node as it accompanies a question: exactly the fields the enrichment
/// prompt already sends for a summary slot, and no others.
///
/// This list is what `docs/SECURITY.md` states for the question path, under
/// guarantee 2's *What a model receives on the question path* — five fields
/// and no others, bounded by [`CONTEXT_NODES`] and [`MAX_SUMMARY_CHARS`].
/// Changing the shape of this struct changes that claim, so the document
/// changes with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeContext {
    pub id: String,
    pub kind: NodeKind,
    pub name: String,
    /// Repo-relative path with forward slashes.
    pub path: String,
    /// The summary the map already holds — mechanical or enriched. Never
    /// the file's contents.
    pub summary: String,
}

/// A question plus the slice of the map selected to answer it. Constructed
/// only by [`build`], so a `Question` that exists is a bounded one.
#[derive(Debug)]
pub struct Question {
    pub project: String,
    /// The reader's words, trimmed and length-checked.
    pub text: String,
    /// At most [`CONTEXT_NODES`] entries.
    pub context: Vec<NodeContext>,
}

/// An answer and the nodes it was drawn from. Every ID in `citations` is
/// one the provider was actually shown (see [`verified`]), so a reader
/// following a citation always lands somewhere real.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Answer {
    pub text: String,
    pub citations: Vec<String>,
}

/// English function words, which appear in nearly every mechanical summary
/// and so rank every node equally while ranking none usefully. Kept short
/// and boring on purpose: this is noise removal, not language understanding.
const STOPWORDS: &[&str] = &[
    "and", "any", "are", "but", "can", "did", "does", "for", "from", "get", "has", "have", "how",
    "into", "its", "not", "the", "that", "them", "then", "there", "these", "they", "this", "was",
    "were", "what", "when", "where", "which", "who", "why", "will", "with", "you", "your",
];

/// Terms shorter than this match too much to be evidence of anything.
const MIN_TERM_CHARS: usize = 3;

/// A term in a node's name is stronger evidence than the same term in its
/// path, which is stronger than in its prose. A file called `auth.ts` is a
/// better answer to "how does auth work" than one merely mentioning auth.
const NAME_WEIGHT: u32 = 3;
const PATH_WEIGHT: u32 = 2;
const SUMMARY_WEIGHT: u32 = 1;

/// The distinct, meaningful terms in a question. Distinct, so repeating a
/// word cannot inflate a node's score; lowercased, so matching is
/// case-blind.
fn terms(question: &str) -> Vec<String> {
    question
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|term| term.chars().count() >= MIN_TERM_CHARS && !STOPWORDS.contains(term))
        .map(str::to_string)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// How well one node answers to a set of terms. Substring matching, so
/// "auth" finds `authenticate` — a newcomer asking about a concept rarely
/// spells an identifier exactly.
///
/// A term scores **where it appears most strongly, once** — not once per
/// field. Summing the fields looks natural and is wrong here, because the
/// three fields are not independent: a file's name is always inside its
/// path, and a symbol's mechanical summary is literally
/// `"Function <name>, lines a-b"`. Under a sum, one term in a symbol's name
/// scores 4 while three distinct terms spread through a file's prose score
/// 3 — which is how a question about "the sealed build" ranked every
/// `build_*` function in the repository above the file whose summary
/// actually discussed sealed builds and egress. Measured on this
/// repository; the spec left the ranking rule to be settled that way.
fn score(node: &NodeContext, terms: &[String]) -> u32 {
    let name = node.name.to_lowercase();
    let path = node.path.to_lowercase();
    let summary = node.summary.to_lowercase();
    terms
        .iter()
        .map(|term| {
            if name.contains(term) {
                NAME_WEIGHT
            } else if path.contains(term) {
                PATH_WEIGHT
            } else if summary.contains(term) {
                SUMMARY_WEIGHT
            } else {
                0
            }
        })
        .sum()
}

/// `text` cut to at most `most` characters, by character rather than by
/// byte so a multi-byte boundary cannot be split.
fn clamp(text: &str, most: usize) -> String {
    match text.char_indices().nth(most) {
        Some((cut, _)) => format!("{}…", &text[..cut]),
        None => text.to_string(),
    }
}

/// Files before classes before functions, so a question that matches nothing
/// still yields the map's skeleton rather than an arbitrary handful of
/// symbols.
fn kind_rank(kind: NodeKind) -> u8 {
    match kind {
        NodeKind::File => 0,
        NodeKind::Class => 1,
        NodeKind::Function => 2,
    }
}

/// The slice of the map that accompanies a question: the best-scoring nodes,
/// **hard-capped at [`CONTEXT_NODES`]**.
///
/// This truncation is the enforcement point for ADR-0004's bound on the
/// question path. It is deliberately unconditional — every node is scored
/// and the list is then cut, so a question engineered to match the whole
/// repository produces exactly the same amount of context as one that
/// matches nothing.
///
/// Ordering is total: score, then node kind, then ID. Two runs of the same
/// question against the same map select the same nodes in the same order.
pub fn select_context(graph: &KnowledgeGraph, question: &str) -> Vec<NodeContext> {
    let terms = terms(question);
    let mut ranked: Vec<(u32, u8, NodeContext)> = graph
        .nodes
        .iter()
        .map(|node| {
            let context = NodeContext {
                id: node.id.as_str().to_string(),
                kind: node.kind,
                name: node.name.clone(),
                path: node.path.clone(),
                summary: clamp(&node.summary, MAX_SUMMARY_CHARS),
            };
            let score = score(&context, &terms);
            (score, kind_rank(node.kind), context)
        })
        .collect();
    ranked.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| a.1.cmp(&b.1))
            .then_with(|| a.2.id.cmp(&b.2.id))
    });
    ranked.truncate(CONTEXT_NODES);
    ranked.into_iter().map(|(_, _, context)| context).collect()
}

/// Builds a bounded question, or explains why the text is not one. Blank and
/// over-long questions are refused here rather than deeper down, so nothing
/// downstream has to wonder whether a `Question` is fit to send.
pub fn build(graph: &KnowledgeGraph, text: &str) -> Result<Question> {
    let text = text.trim();
    if text.is_empty() {
        bail!("the question is empty");
    }
    let length = text.chars().count();
    if length > MAX_QUESTION_CHARS {
        bail!("the question is {length} characters; the limit is {MAX_QUESTION_CHARS}");
    }
    Ok(Question {
        project: graph.project.name.clone(),
        text: text.to_string(),
        context: select_context(graph, text),
    })
}

/// Drops citations the provider was never shown, and collapses repeats.
///
/// A citation is a promise that the reader can go and look. A model asked
/// for node IDs can produce a plausible one it invented, and there is no
/// repair to attempt — the honest handling is to keep what is checkable and
/// discard the rest, which leaves the prose intact and the links live.
fn verified(answer: Answer, question: &Question) -> Answer {
    let shown: BTreeSet<&str> = question.context.iter().map(|c| c.id.as_str()).collect();
    let mut seen = BTreeSet::new();
    Answer {
        text: answer.text,
        citations: answer
            .citations
            .into_iter()
            .filter(|id| shown.contains(id.as_str()) && seen.insert(id.clone()))
            .collect(),
    }
}

/// Asks a built question and keeps the citations that check out.
///
/// Separate from [`build`] so a caller can tell the two failures apart
/// without reading an error message: an unaskable question is the reader's
/// to fix, a provider that would not answer is not. `POST /api/ask` maps
/// them to different statuses on exactly that distinction.
pub fn answer(provider: &dyn EnrichmentProvider, question: &Question) -> Result<Answer> {
    Ok(verified(provider.ask(question)?, question))
}

#[cfg(test)]
mod tests {
    //! Seam 2 for the question path: what reaches the provider, and what is
    //! made of what comes back.

    use std::cell::RefCell;

    use super::*;
    use crate::map::{KnowledgeGraph, Node, NodeId, Project, Provenance};

    fn node(id: NodeId, kind: NodeKind, name: &str, path: &str, summary: &str) -> Node {
        Node {
            id,
            kind,
            name: name.into(),
            path: path.into(),
            summary: summary.into(),
            range: None,
            layer: None,
            provenance: Provenance::Structural,
        }
    }

    fn empty_graph() -> KnowledgeGraph {
        KnowledgeGraph {
            version: crate::map::MAP_CONTRACT_VERSION.into(),
            project: Project {
                name: "demo".into(),
            },
            nodes: Vec::new(),
            edges: Vec::new(),
            layers: Vec::new(),
            domain_flows: Vec::new(),
            tour: Vec::new(),
        }
    }

    /// A graph of `count` files, each with a distinct name, path and prose,
    /// plus a class and a function inside each — comfortably more nodes than
    /// the context bound.
    ///
    /// All three kinds are present on purpose. Node IDs are prefixed by kind
    /// (`class:` < `file:` < `function:` alphabetically), so a fixture of
    /// files and functions alone would put files first under plain ID
    /// ordering and make the kind tiebreak untestable — it would look
    /// enforced while doing nothing.
    fn wide_graph(count: usize) -> KnowledgeGraph {
        let mut graph = empty_graph();
        for i in 0..count {
            let path = format!("src/module{i}/widget{i}.ts");
            graph.nodes.push(node(
                NodeId::file(&path),
                NodeKind::File,
                &format!("widget{i}.ts"),
                &path,
                &format!("TypeScript file defining gadget{i} behaviour."),
            ));
            // Named so it does not share a term with its file: these
            // fixtures are also used to assert which node ranks first.
            graph.nodes.push(node(
                NodeId::symbol(NodeKind::Class, &path, &format!("Gadget{i}")),
                NodeKind::Class,
                &format!("Gadget{i}"),
                &path,
                &format!("Class Gadget{i} with two methods."),
            ));
            graph.nodes.push(node(
                NodeId::symbol(NodeKind::Function, &path, &format!("run{i}")),
                NodeKind::Function,
                &format!("run{i}"),
                &path,
                &format!("Function run{i}, called by nothing."),
            ));
        }
        graph
    }

    /// A small graph whose summaries read like real mechanical prose — full
    /// of the function words that would otherwise match every node in the
    /// map. `wide_graph`'s terse summaries happen to contain none of them,
    /// which makes them useless for testing stopword removal.
    fn prose_graph() -> KnowledgeGraph {
        let mut graph = empty_graph();
        for (i, subject) in ["session", "invoice", "printer"].iter().enumerate() {
            let path = format!("src/mod{i}/thing{i}.ts");
            graph.nodes.push(node(
                NodeId::file(&path),
                NodeKind::File,
                &format!("thing{i}.ts"),
                &path,
                &format!(
                    "This is the file that defines what the {subject} does \
                     and how they are used."
                ),
            ));
        }
        graph
    }

    /// Records the question it was asked and returns a canned answer.
    struct Recording {
        answer: Answer,
        seen: RefCell<Vec<usize>>,
    }

    impl Recording {
        fn new(answer: Answer) -> Self {
            Self {
                answer,
                seen: RefCell::new(Vec::new()),
            }
        }
    }

    impl EnrichmentProvider for Recording {
        fn enrich(
            &self,
            _request: &super::super::EnrichmentRequest,
        ) -> Result<super::super::EnrichmentResponse> {
            unreachable!("the question path never enriches")
        }

        fn ask(&self, question: &Question) -> Result<Answer> {
            self.seen.borrow_mut().push(question.context.len());
            Ok(Answer {
                text: self.answer.text.clone(),
                citations: self.answer.citations.clone(),
            })
        }
    }

    #[test]
    fn the_context_is_capped_however_large_the_map_is() {
        let graph = wide_graph(200);
        let context = select_context(&graph, "how does the widget run");

        assert_eq!(
            context.len(),
            CONTEXT_NODES,
            "the slice must be capped at the stated bound"
        );
    }

    /// The adversarial phrasing the criterion names: a question assembled
    /// from every identifier in the repository, so every node scores. The
    /// cap is what has to hold, not the ranking.
    #[test]
    fn a_question_crafted_to_match_everything_still_gets_one_slice() {
        let graph = wide_graph(200);
        let everything: String = graph
            .nodes
            .iter()
            .map(|n| format!("{} {} {} ", n.name, n.path, n.summary))
            .collect();
        // Long enough that `build` would refuse it; `select_context` is the
        // enforcement point being tested, so it is called directly.
        let context = select_context(&graph, &everything);

        assert_eq!(context.len(), CONTEXT_NODES);
        assert!(
            context.iter().all(|c| score(c, &terms(&everything)) > 0),
            "every node matched, which is what makes this the hard case"
        );
    }

    /// A map smaller than the bound sends what it has — the cap is a
    /// ceiling, not a quota to pad out to.
    #[test]
    fn a_small_map_sends_only_the_nodes_it_has() {
        let graph = wide_graph(3);
        let context = select_context(&graph, "widget");

        assert_eq!(
            context.len(),
            9,
            "three files, three classes, three functions"
        );
    }

    #[test]
    fn a_node_the_question_names_is_selected_out_of_a_large_map() {
        let mut graph = wide_graph(200);
        graph.nodes.push(node(
            NodeId::file("src/auth/session.ts"),
            NodeKind::File,
            "session.ts",
            "src/auth/session.ts",
            "Issues and validates login sessions.",
        ));

        let context = select_context(&graph, "where are login sessions validated?");

        assert_eq!(
            context.first().map(|c| c.id.as_str()),
            Some("file:src/auth/session.ts"),
            "the node the question describes must rank first: {:?}",
            context.iter().map(|c| &c.id).take(5).collect::<Vec<_>>()
        );
    }

    /// The whole selection is a projection of the graph. There is no path
    /// from here to a file's bytes — the function takes no root and opens
    /// nothing — and the field list is the claim `docs/SECURITY.md` makes.
    #[test]
    fn a_context_entry_carries_the_documented_fields_and_no_contents() {
        let graph = wide_graph(1);
        let entry = select_context(&graph, "widget").into_iter().next().unwrap();

        assert_eq!(
            entry,
            NodeContext {
                id: "file:src/module0/widget0.ts".into(),
                kind: NodeKind::File,
                name: "widget0.ts".into(),
                path: "src/module0/widget0.ts".into(),
                summary: "TypeScript file defining gadget0 behaviour.".into(),
            },
            "the documented fields, carrying only what the map already holds"
        );
    }

    /// A question that matches nothing still has to send something useful.
    /// Files are the map's skeleton, so they go first — the alternative is
    /// an arbitrary handful of functions chosen by ID order, which reads to
    /// the model as though it were told something.
    #[test]
    fn a_question_matching_nothing_falls_back_to_the_map_s_skeleton() {
        let graph = wide_graph(200);
        let context = select_context(&graph, "quantum entanglement thermodynamics");

        assert!(
            context.iter().all(|c| score(c, &terms("quantum")) == 0),
            "this test is only meaningful when nothing matches"
        );
        // Every score is zero, so ID order alone would decide — and `class:`
        // sorts before `file:`. Files first is therefore evidence that kind
        // is doing the ranking, not the alphabet.
        assert!(
            context.iter().all(|c| c.kind == NodeKind::File),
            "a no-match question must fall back to files, not symbols: {:?}",
            context.iter().map(|c| &c.id).take(5).collect::<Vec<_>>()
        );
    }

    /// Bounding the node count bounds the prompt only if a node is small.
    /// A map from another producer (story 16) may carry any string the
    /// schema allows, so the slice is bounded in bytes as well as in nodes.
    #[test]
    fn one_enormous_summary_cannot_inflate_the_slice() {
        let mut graph = empty_graph();
        graph.nodes.push(node(
            NodeId::file("src/huge.ts"),
            NodeKind::File,
            "huge.ts",
            "src/huge.ts",
            &"prose ".repeat(20_000),
        ));

        let entry = select_context(&graph, "prose").into_iter().next().unwrap();
        assert!(
            entry.summary.chars().count() <= MAX_SUMMARY_CHARS + 1,
            "a summary reached the prompt at {} characters",
            entry.summary.chars().count()
        );
    }

    /// A multi-byte character must not be sliced in half by the cap.
    #[test]
    fn clamping_a_summary_never_splits_a_character() {
        let text = "é".repeat(MAX_SUMMARY_CHARS + 50);
        let cut = clamp(&text, MAX_SUMMARY_CHARS);
        assert_eq!(
            cut.chars().count(),
            MAX_SUMMARY_CHARS + 1,
            "plus the ellipsis"
        );
        assert!(
            clamp("short", MAX_SUMMARY_CHARS) == "short",
            "short text is untouched"
        );
    }

    #[test]
    fn ranking_is_deterministic_for_the_same_question_and_map() {
        let graph = wide_graph(200);
        let once = select_context(&graph, "widget gadget run");
        let twice = select_context(&graph, "widget gadget run");

        assert_eq!(once, twice);
    }

    #[test]
    fn a_blank_or_oversized_question_is_refused_before_any_provider_is_asked() {
        let graph = wide_graph(2);

        for blank in ["", "   ", "\n\t "] {
            assert!(
                build(&graph, blank).is_err(),
                "a blank question must not reach a provider: {blank:?}"
            );
        }

        let long = "a".repeat(MAX_QUESTION_CHARS + 1);
        let err = build(&graph, &long).unwrap_err().to_string();
        assert!(
            err.contains(&MAX_QUESTION_CHARS.to_string()),
            "the refusal must state the limit: {err}"
        );

        // The boundary itself is accepted, so the limit is a limit and not
        // an off-by-one.
        assert!(build(&graph, &"a".repeat(MAX_QUESTION_CHARS)).is_ok());
    }

    #[test]
    fn a_question_is_trimmed_and_carries_the_project() {
        let graph = wide_graph(1);
        let question = build(&graph, "  what runs first?  ").unwrap();

        assert_eq!(question.text, "what runs first?");
        assert_eq!(question.project, "demo");
    }

    /// Stopwords are the reason ranking works at all: mechanical summaries
    /// are English prose, so "the" and "does" match nearly every node and
    /// would drown the terms that carry the question.
    #[test]
    fn function_words_do_not_rank_nodes() {
        let graph = prose_graph();
        let question = "what does this and how are they";

        // The premise: every summary genuinely contains these words, so
        // without stopword removal every node would score and the ranking
        // would be noise.
        for word in ["what", "does", "this", "and", "how", "are", "they"] {
            assert!(
                graph
                    .nodes
                    .iter()
                    .all(|n| n.summary.to_lowercase().contains(word)),
                "the fixture must contain {word} for this test to mean anything"
            );
        }

        assert!(
            terms(question).is_empty(),
            "a question of nothing but function words carries no terms: {:?}",
            terms(question)
        );
        assert!(
            select_context(&graph, question)
                .iter()
                .all(|c| score(c, &terms(question)) == 0),
            "function words must contribute no evidence"
        );
    }

    /// A one- or two-character term is not evidence of anything: `in` is
    /// inside `thing`, `defines` and half the paths in a repository, so
    /// admitting it scores the whole map alike and drowns the term that
    /// actually carries the question.
    #[test]
    fn terms_too_short_to_be_evidence_are_dropped() {
        assert_eq!(terms("is a in of invoice"), vec!["invoice".to_string()]);

        // The premise, demonstrated rather than asserted in a comment.
        let graph = prose_graph();
        let short = vec!["in".to_string()];
        assert!(
            select_context(&graph, "invoice")
                .iter()
                .all(|c| score(c, &short) > 0),
            "a two-character term matches every node, which is the point"
        );
    }

    /// *Where* a term appears says how strongly a node is about it. A symbol
    /// named `session` answers "session" better than a file whose prose
    /// merely mentions sessions twice, and a file under `auth/` answers
    /// "auth" better than one that mentions it in passing.
    #[test]
    fn a_name_or_path_match_outranks_a_mention_in_prose() {
        let mut graph = empty_graph();
        graph.nodes.push(node(
            NodeId::file("src/other.ts"),
            NodeKind::File,
            "other.ts",
            "src/other.ts",
            "Reads the session and writes the session log.",
        ));
        graph.nodes.push(node(
            NodeId::symbol(NodeKind::Function, "src/other.ts", "session"),
            NodeKind::Function,
            "session",
            "src/other.ts",
            "Does a thing.",
        ));
        assert_eq!(
            select_context(&graph, "session").first().map(|c| &*c.id),
            Some("function:src/other.ts:session"),
            "a node named for the term must outrank one that mentions it"
        );

        let mut graph = empty_graph();
        // The decoy sorts *before* the answer by ID, deliberately: if the
        // weights stopped separating them, the alphabetical tiebreak would
        // hand this test the right answer for the wrong reason.
        graph.nodes.push(node(
            NodeId::file("src/aaa/notes.ts"),
            NodeKind::File,
            "notes.ts",
            "src/aaa/notes.ts",
            "Mentions auth once.",
        ));
        graph.nodes.push(node(
            NodeId::file("src/auth/helper.ts"),
            NodeKind::File,
            "helper.ts",
            "src/auth/helper.ts",
            "Does a thing.",
        ));
        assert_eq!(
            select_context(&graph, "auth").first().map(|c| &*c.id),
            Some("file:src/auth/helper.ts"),
            "a node living under the term must outrank one that mentions it"
        );
    }

    /// Prose is evidence too, and on an enriched map it is the *only* place
    /// a concept the reader names appears at all — mechanical summaries read
    /// "Rust file, 378 lines: 7 functions", so nothing but names and paths
    /// carries vocabulary until enrichment has run.
    #[test]
    fn a_node_matching_only_in_prose_outranks_one_matching_nowhere() {
        let mut graph = empty_graph();
        graph.nodes.push(node(
            NodeId::file("src/aaa/first.ts"),
            NodeKind::File,
            "first.ts",
            "src/aaa/first.ts",
            "Rust file, 40 lines: 2 functions.",
        ));
        graph.nodes.push(node(
            NodeId::file("src/zzz/second.ts"),
            NodeKind::File,
            "second.ts",
            "src/zzz/second.ts",
            "Validates login sessions against the store.",
        ));

        // Both are files and the match sorts last by ID, so only the summary
        // weight can put it first.
        assert_eq!(
            select_context(&graph, "how are sessions validated?")
                .first()
                .map(|c| &*c.id),
            Some("file:src/zzz/second.ts"),
        );
    }

    /// The measured defect: a symbol's mechanical summary is
    /// `"Function <name>, lines a-b"`, so under a per-field sum one term in
    /// a symbol's name counted twice — beating a file whose prose genuinely
    /// discussed three separate terms from the question.
    ///
    /// Observed on this repository: "how does the sealed build stop network
    /// egress?" put six `build_*` functions above the file whose summary
    /// named sealed builds, egress *and* the network feature.
    #[test]
    fn one_term_in_a_name_does_not_outweigh_three_terms_in_prose() {
        let mut graph = empty_graph();
        graph.nodes.push(node(
            NodeId::symbol(NodeKind::Function, "src/args.rs", "build_args"),
            NodeKind::Function,
            "build_args",
            "src/args.rs",
            // Exactly what `semantics` emits for a symbol: the name again.
            "Function build_args, lines 240-259",
        ));
        graph.nodes.push(node(
            NodeId::file("src/enrich.rs"),
            NodeKind::File,
            "enrich.rs",
            "src/enrich.rs",
            "A sealed build compiles no backend in, so nothing here can \
             reach a model and the binary contains no egress path.",
        ));

        let question = "how does the sealed build stop network egress?";
        assert_eq!(
            select_context(&graph, question).first().map(|c| &*c.id),
            Some("file:src/enrich.rs"),
            "three terms in prose must outrank one term in a name that its \
             own summary repeats"
        );
    }

    #[test]
    fn an_invented_citation_is_dropped_and_a_real_one_survives() {
        let graph = wide_graph(2);
        let provider = Recording::new(Answer {
            text: "Widget zero starts things off.".into(),
            citations: vec![
                "file:src/module0/widget0.ts".into(),
                "file:src/does/not/exist.ts".into(),
                // A repeat of a real one: kept once.
                "file:src/module0/widget0.ts".into(),
            ],
        });

        let question = build(&graph, "what starts things off?").unwrap();
        let answer = super::answer(&provider, &question).unwrap();

        assert_eq!(answer.text, "Widget zero starts things off.");
        assert_eq!(
            answer.citations,
            vec!["file:src/module0/widget0.ts".to_string()],
            "only checkable citations survive, and only once"
        );
    }

    #[test]
    fn the_provider_never_sees_more_than_the_bound() {
        let graph = wide_graph(200);
        let provider = Recording::new(Answer::default());

        let question = build(&graph, "widget gadget run module").unwrap();
        super::answer(&provider, &question).unwrap();

        assert_eq!(*provider.seen.borrow(), vec![CONTEXT_NODES]);
    }

    #[test]
    fn a_provider_failure_propagates_rather_than_becoming_an_empty_answer() {
        struct Failing;
        impl EnrichmentProvider for Failing {
            fn enrich(
                &self,
                _request: &super::super::EnrichmentRequest,
            ) -> Result<super::super::EnrichmentResponse> {
                unreachable!()
            }
            fn ask(&self, _question: &Question) -> Result<Answer> {
                bail!("boom")
            }
        }

        let graph = wide_graph(2);
        let question = build(&graph, "anything").unwrap();
        let err = super::answer(&Failing, &question).unwrap_err();
        assert!(err.to_string().contains("boom"));
    }

    /// The trait's default: a backend that has not implemented questions
    /// says so, rather than returning nothing and looking like an answer.
    #[test]
    fn a_backend_without_a_question_implementation_says_so() {
        struct EnrichOnly;
        impl EnrichmentProvider for EnrichOnly {
            fn enrich(
                &self,
                _request: &super::super::EnrichmentRequest,
            ) -> Result<super::super::EnrichmentResponse> {
                unreachable!()
            }
        }

        let graph = wide_graph(2);
        let question = build(&graph, "anything").unwrap();
        let err = super::answer(&EnrichOnly, &question).unwrap_err();
        assert!(
            err.to_string().contains("question"),
            "the refusal must say what is missing: {err}"
        );
    }
}
