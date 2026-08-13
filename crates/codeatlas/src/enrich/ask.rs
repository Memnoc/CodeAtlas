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
//! step is the obvious way to lose that by accident, so every limit is
//! enforced here and nowhere else: [`MAX_QUESTION_CHARS`] on what the reader
//! may send, [`CONTEXT_NODES`] on how many nodes may accompany it,
//! [`MAX_SUMMARY_CHARS`] on how large one of those nodes may be, and — since
//! ADR-0012 let a request carry its conversation — [`MAX_TURNS`] on how many
//! previous turns ride along, with a carried question clamped to
//! [`MAX_QUESTION_CHARS`] and a carried answer to [`MAX_TURN_ANSWER_CHARS`].
//! All are hard caps rather than targets — [`select_context`] truncates on
//! both axes and [`build`] clamps the history, so no phrasing of a question,
//! no map, and no client bookkeeping bug can widen the slice or the prompt.
//!
//! The summary cap exists because the first two bound the prompt in *nodes*
//! and not in *bytes*: every summary CodeAtlas writes is a sentence or less,
//! but a map from another producer (spec story 16) may carry any string the
//! schema allows. The turn bounds clamp rather than refuse, and that split
//! is ADR-0012's: the reader typed the question and can rephrase it, so its
//! bound refuses; the dashboard assembled the history, so its bounds degrade
//! the answer instead of erroring the reader's question.
//!
//! What rides along is what the enrichment prompt already carries: a node's
//! ID, kind, name, repo-relative path, and the summary the map already holds
//! (mechanical or enriched). Never file contents — this module never reads a
//! file, and takes the graph as its only source.
//!
//! Compiled unconditionally. Selection is pure and opens nothing; a build
//! with no provider simply has no way to reach [`answer`] (`serve --ask`
//! refuses at startup).

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, bail};

use super::EnrichmentProvider;
use crate::map::{KnowledgeGraph, Node, NodeKind};

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

/// The most previous turns a request may carry (ADR-0012). History past the
/// bound is clamped oldest-first rather than refused: the reader typed the
/// question but the dashboard assembled the history, so a 400 would punish
/// the wrong party. The dashboard enforces the same bound itself, making
/// this clamp a backstop rather than the mechanism.
pub const MAX_TURNS: usize = 6;

/// The longest carried answer one turn may bring back. The ask prompt
/// demands "a short paragraph", so 2000 characters holds any answer this
/// route has ever produced with room to spare — while capping what
/// [`MAX_TURNS`] answers add to the prompt at 12,000 characters, smaller
/// than the slice itself (40 nodes × 400-character summaries). Clamped
/// rather than refused, like a summary and unlike the current question: the
/// reader can rephrase a question, and can do nothing about what an earlier
/// answer said.
pub const MAX_TURN_ANSWER_CHARS: usize = 2000;

/// One previous turn as the client carries it (ADR-0012): the reader's
/// question, the answer, and the node IDs that answer cited. Deserialized
/// straight off the wire by `POST /api/ask`, and clamped by [`build`] before
/// anything downstream sees it. Citations are input here, not a promise —
/// only IDs naming real nodes reach the slice (see [`select_context`]), so a
/// client cannot smuggle an invented node ID into the prompt's node set.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Turn {
    pub question: String,
    pub answer: String,
    #[serde(default)]
    pub citations: Vec<String>,
}

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
    /// The carried conversation, oldest first: at most [`MAX_TURNS`] turns,
    /// each field clamped. Empty for a bare question, which stays a valid
    /// request answered exactly as before ADR-0012.
    pub turns: Vec<Turn>,
}

/// An answer and the nodes it was drawn from. Every ID in `citations` is
/// one the provider was actually shown (see [`verified`]), so a reader
/// following a citation always lands somewhere real.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Answer {
    pub text: String,
    pub citations: Vec<String>,
    /// What the exchange spent, when the backend's envelope reported it.
    /// `None` is a backend that reported nothing, and stays `None` all the
    /// way to the wire — the response simply has no usage field.
    pub usage: Option<Usage>,
}

/// Token counts a provider's response envelope reported for one exchange
/// (ADR-0012). Measured or absent — never estimated, never a zero stand-in,
/// and never a price: the CLI envelope also carries `total_cost_usd`, which
/// is deliberately not read here, because on subscription billing that
/// number is notional and a wrong price is worse than no price.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

impl Usage {
    /// Reads both token counts out of an envelope's `usage` object, or
    /// nothing. Anything short of two measured counts — no object at all, a
    /// missing field, a value that is not an unsigned integer — is `None`,
    /// so what reaches the reader is measured or absent, never a fabricated
    /// zero.
    ///
    /// Shared by both backends because their envelopes agree on this one
    /// shape: `usage.input_tokens` and `usage.output_tokens` as integers,
    /// in the Messages API response and the CLI's result envelope alike.
    pub fn from_envelope(usage: Option<&serde_json::Value>) -> Option<Self> {
        let usage = usage?;
        Some(Self {
            input_tokens: usage.get("input_tokens")?.as_u64()?,
            output_tokens: usage.get("output_tokens")?.as_u64()?,
        })
    }
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

/// One node of the graph as the prompt will carry it — the only constructor
/// of a [`NodeContext`], so the summary clamp cannot be forgotten on one of
/// two paths into the slice.
fn context_for(node: &Node) -> NodeContext {
    NodeContext {
        id: node.id.as_str().to_string(),
        kind: node.kind,
        name: node.name.clone(),
        path: node.path.clone(),
        summary: clamp(&node.summary, MAX_SUMMARY_CHARS),
    }
}

/// The slice of the map that accompanies a question: the carried citations
/// first, then the best-scoring nodes, **hard-capped at [`CONTEXT_NODES`]**.
///
/// Citations-first is ADR-0012's retrieval rule: a follow-up like "what
/// calls it?" carries no searchable terms, so continuity comes from the
/// nodes the conversation is provably about — the citations earlier answers
/// earned — never from folding earlier questions into term scoring. Newest
/// turn first, so when the bound cuts, the oldest conversation's nodes are
/// the ones to go. A citation is only an ID until it names a real node:
/// one the map does not contain selects nothing, which is what keeps a
/// client from smuggling an invented node into the slice.
///
/// The truncation is the enforcement point for ADR-0004's bound on the
/// question path. It is deliberately unconditional on both paths — the
/// citation loop stops at the bound and the scored remainder is cut to what
/// is left — so a question engineered to match the whole repository, or a
/// history citing all of it, produces exactly the same amount of context as
/// a question that matches nothing.
///
/// Ordering is total: cited nodes in carried order, then score, node kind,
/// ID. Two runs of the same question against the same map select the same
/// nodes in the same order.
pub fn select_context(graph: &KnowledgeGraph, question: &str, turns: &[Turn]) -> Vec<NodeContext> {
    let by_id: BTreeMap<&str, &Node> = graph.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    let mut taken = BTreeSet::new();
    let mut context: Vec<NodeContext> = Vec::new();
    'cited: for turn in turns.iter().rev() {
        for id in &turn.citations {
            if context.len() == CONTEXT_NODES {
                break 'cited;
            }
            // An invented citation names no node and selects nothing.
            let Some(node) = by_id.get(id.as_str()) else {
                continue;
            };
            if taken.insert(id.as_str()) {
                context.push(context_for(node));
            }
        }
    }

    let terms = terms(question);
    let mut ranked: Vec<(u32, u8, NodeContext)> = graph
        .nodes
        .iter()
        .filter(|node| !taken.contains(node.id.as_str()))
        .map(|node| {
            let entry = context_for(node);
            let score = score(&entry, &terms);
            (score, kind_rank(node.kind), entry)
        })
        .collect();
    ranked.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| a.1.cmp(&b.1))
            .then_with(|| a.2.id.cmp(&b.2.id))
    });
    ranked.truncate(CONTEXT_NODES - context.len());
    context.extend(ranked.into_iter().map(|(_, _, entry)| entry));
    context
}

/// The carried conversation cut to what ADR-0012 admits: the newest
/// [`MAX_TURNS`] turns, a carried question clamped to [`MAX_QUESTION_CHARS`]
/// and a carried answer to [`MAX_TURN_ANSWER_CHARS`]. Clamped, never
/// refused — over-bound history is the dashboard's bookkeeping slipping, and
/// erroring the reader's question would punish the wrong party (story 14).
fn clamped_turns(turns: &[Turn]) -> Vec<Turn> {
    turns[turns.len().saturating_sub(MAX_TURNS)..]
        .iter()
        .map(|turn| Turn {
            question: clamp(&turn.question, MAX_QUESTION_CHARS),
            answer: clamp(&turn.answer, MAX_TURN_ANSWER_CHARS),
            citations: turn.citations.clone(),
        })
        .collect()
}

/// Builds a bounded question, or explains why the text is not one. Blank and
/// over-long questions are refused here rather than deeper down, so nothing
/// downstream has to wonder whether a `Question` is fit to send. The carried
/// turns take the opposite treatment — clamped, never refused (see
/// [`clamped_turns`]) — and the clamp runs before the slice is built, so a
/// dropped turn's citations cannot still steer it.
pub fn build(graph: &KnowledgeGraph, text: &str, turns: &[Turn]) -> Result<Question> {
    let text = text.trim();
    if text.is_empty() {
        bail!("the question is empty");
    }
    let length = text.chars().count();
    if length > MAX_QUESTION_CHARS {
        bail!("the question is {length} characters; the limit is {MAX_QUESTION_CHARS}");
    }
    let turns = clamped_turns(turns);
    Ok(Question {
        project: graph.project.name.clone(),
        text: text.to_string(),
        context: select_context(graph, text, &turns),
        turns,
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
        // Verification is about citations; what the exchange measurably
        // spent is not diminished by an invented citation being dropped.
        usage: answer.usage,
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
            significance: None,
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
                usage: self.answer.usage,
            })
        }
    }

    #[test]
    fn the_context_is_capped_however_large_the_map_is() {
        let graph = wide_graph(200);
        let context = select_context(&graph, "how does the widget run", &[]);

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
        let context = select_context(&graph, &everything, &[]);

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
        let context = select_context(&graph, "widget", &[]);

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

        let context = select_context(&graph, "where are login sessions validated?", &[]);

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
        let entry = select_context(&graph, "widget", &[])
            .into_iter()
            .next()
            .unwrap();

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
        let context = select_context(&graph, "quantum entanglement thermodynamics", &[]);

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

        let entry = select_context(&graph, "prose", &[])
            .into_iter()
            .next()
            .unwrap();
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

    /// ADR-0012's retrieval rule, the reason a follow-up that says "it"
    /// works: the nodes the conversation already earned lead the slice. The
    /// cited node here is a *function* on a map of 150 nodes and the current
    /// question matches nothing, so the fallback (files first) would never
    /// select it — only the carried citation can put it there.
    #[test]
    fn a_carried_citation_selects_a_node_the_question_alone_never_would() {
        let graph = wide_graph(50);
        let cited = "function:src/module3/widget3.ts:run3";
        let question = "quantum entanglement thermodynamics";

        let bare = select_context(&graph, question, &[]);
        assert!(
            !bare.iter().any(|c| c.id == cited),
            "this test is only meaningful when a bare question omits the node"
        );

        let turns = vec![Turn {
            question: "what runs third?".into(),
            answer: "run3 does.".into(),
            citations: vec![cited.into()],
        }];
        let context = select_context(&graph, question, &turns);

        assert_eq!(
            context.first().map(|c| c.id.as_str()),
            Some(cited),
            "the carried citation must lead the slice: {:?}",
            context.iter().map(|c| &c.id).take(5).collect::<Vec<_>>()
        );
        // Term scoring fills the remainder exactly as it would have filled a
        // bare question's slice, one seat shorter.
        assert_eq!(context.len(), CONTEXT_NODES);
        assert_eq!(context[1..], bare[..CONTEXT_NODES - 1]);
    }

    /// "It" in a follow-up almost always means the *last* answer's nodes, so
    /// when the bound has to cut, the newest turn's citations are the ones
    /// that must survive — the mirror of dropping whole turns oldest-first.
    /// A node two answers cited enters once.
    #[test]
    fn the_newest_turns_citations_lead_and_a_repeat_enters_once() {
        let graph = wide_graph(50);
        let older = Turn {
            question: "what runs third?".into(),
            answer: "run3.".into(),
            citations: vec![
                "function:src/module3/widget3.ts:run3".into(),
                // Also cited by the newer turn below: kept once, at the
                // position the newer turn earned it.
                "function:src/module7/widget7.ts:run7".into(),
            ],
        };
        let newer = Turn {
            question: "and seventh?".into(),
            answer: "run7.".into(),
            citations: vec!["function:src/module7/widget7.ts:run7".into()],
        };

        let context = select_context(&graph, "quantum entanglement", &[older, newer]);

        let leading: Vec<&str> = context.iter().take(2).map(|c| c.id.as_str()).collect();
        assert_eq!(
            leading,
            [
                "function:src/module7/widget7.ts:run7",
                "function:src/module3/widget3.ts:run3",
            ],
            "the newest turn's citations lead, and the repeat is not doubled"
        );
        assert_eq!(
            context
                .iter()
                .filter(|c| c.id == "function:src/module7/widget7.ts:run7")
                .count(),
            1
        );
    }

    /// The hard case for the bound: a history citing more real nodes than
    /// the slice may hold. The cap holds, the newest turns' citations are
    /// the ones kept, and term scoring adds nothing on top.
    #[test]
    fn citations_alone_cannot_widen_the_slice_past_the_bound() {
        let graph = wide_graph(50);
        // Six turns citing 8 distinct real files each: 48 valid citations
        // for 40 seats.
        let turns: Vec<Turn> = (0..MAX_TURNS)
            .map(|t| Turn {
                question: format!("turn {t}?"),
                answer: format!("answer {t}."),
                citations: (0..8)
                    .map(|i| {
                        let n = t * 8 + i;
                        format!("file:src/module{n}/widget{n}.ts")
                    })
                    .collect(),
            })
            .collect();

        let context = select_context(&graph, "widget", &turns);

        assert_eq!(context.len(), CONTEXT_NODES, "the bound is never exceeded");
        // The newest turn's citations all survived…
        for id in &turns[MAX_TURNS - 1].citations {
            assert!(
                context.iter().any(|c| c.id == *id),
                "a newest-turn citation was cut: {id}"
            );
        }
        // …and what the bound cut was the oldest turn's, entirely.
        for id in &turns[0].citations {
            assert!(
                !context.iter().any(|c| c.id == *id),
                "an oldest-turn citation survived past the bound: {id}"
            );
        }
    }

    #[test]
    fn an_invented_carried_citation_selects_nothing() {
        let graph = wide_graph(50);
        let turns = vec![Turn {
            question: "what handles login?".into(),
            answer: "A file I made up.".into(),
            citations: vec![
                "file:src/does/not/exist.ts".into(),
                "function:src/module3/widget3.ts:run3".into(),
            ],
        }];

        let context = select_context(&graph, "quantum entanglement", &turns);

        assert!(
            !context.iter().any(|c| c.id == "file:src/does/not/exist.ts"),
            "an invented node ID must never enter the slice"
        );
        // The real citation still leads, and the invented one cost nothing:
        // the slice is as full as a bare question's would be.
        assert_eq!(context[0].id, "function:src/module3/widget3.ts:run3");
        assert_eq!(context.len(), CONTEXT_NODES);
    }

    #[test]
    fn ranking_is_deterministic_for_the_same_question_and_map() {
        let graph = wide_graph(200);
        let once = select_context(&graph, "widget gadget run", &[]);
        let twice = select_context(&graph, "widget gadget run", &[]);

        assert_eq!(once, twice);
    }

    #[test]
    fn a_blank_or_oversized_question_is_refused_before_any_provider_is_asked() {
        let graph = wide_graph(2);

        for blank in ["", "   ", "\n\t "] {
            assert!(
                build(&graph, blank, &[]).is_err(),
                "a blank question must not reach a provider: {blank:?}"
            );
        }

        let long = "a".repeat(MAX_QUESTION_CHARS + 1);
        let err = build(&graph, &long, &[]).unwrap_err().to_string();
        assert!(
            err.contains(&MAX_QUESTION_CHARS.to_string()),
            "the refusal must state the limit: {err}"
        );

        // The boundary itself is accepted, so the limit is a limit and not
        // an off-by-one.
        assert!(build(&graph, &"a".repeat(MAX_QUESTION_CHARS), &[]).is_ok());
    }

    /// Story 14: over-bound history is clamped mechanically, oldest turns
    /// first — the reader typed the question, the dashboard assembled the
    /// history, and an error would punish the wrong party.
    #[test]
    fn history_beyond_the_turn_bound_is_dropped_oldest_first() {
        let graph = wide_graph(50);
        // Citing *functions*, which a no-match question's fallback (files
        // first) never selects: the only road into the slice for these is
        // the citation, so a dropped turn is observable there.
        let turns: Vec<Turn> = (0..MAX_TURNS + 1)
            .map(|t| Turn {
                question: format!("turn {t}?"),
                answer: format!("answer {t}."),
                citations: vec![format!("function:src/module{t}/widget{t}.ts:run{t}")],
            })
            .collect();

        let question = build(&graph, "quantum entanglement", &turns).unwrap();

        assert_eq!(question.turns.len(), MAX_TURNS, "the bound is the bound");
        assert_eq!(
            question.turns.first().map(|t| t.question.as_str()),
            Some("turn 1?"),
            "the oldest turn is the one dropped"
        );
        // And the dropped turn's citation dropped out of the slice with it.
        assert!(
            !question
                .context
                .iter()
                .any(|c| c.id == "function:src/module0/widget0.ts:run0"),
            "a dropped turn must not still steer the slice"
        );
        assert!(
            question
                .context
                .iter()
                .any(|c| c.id == "function:src/module1/widget1.ts:run1"),
            "a surviving turn's citation must still steer the slice"
        );

        // The boundary itself is kept whole, so the bound is a bound and
        // not an off-by-one.
        let exactly = build(&graph, "quantum entanglement", &turns[1..]).unwrap();
        assert_eq!(exactly.turns.len(), MAX_TURNS);
        assert_eq!(
            exactly.turns.first().map(|t| t.question.as_str()),
            Some("turn 1?")
        );
    }

    /// The per-field bounds on carried turns clamp rather than refuse —
    /// unlike the current question, whose refusal stands: the reader can
    /// rephrase what they are typing, and can do nothing about what an
    /// earlier answer said.
    #[test]
    fn carried_fields_are_clamped_rather_than_refused() {
        let graph = wide_graph(2);
        let turns = vec![Turn {
            question: "q".repeat(MAX_QUESTION_CHARS + 50),
            answer: "a".repeat(MAX_TURN_ANSWER_CHARS + 50),
            citations: Vec::new(),
        }];

        let question = build(&graph, "what runs first?", &turns)
            .expect("over-bound carried fields must never refuse the request");

        let carried = &question.turns[0];
        assert_eq!(
            carried.question.chars().count(),
            MAX_QUESTION_CHARS + 1,
            "clamped to the bound plus the ellipsis"
        );
        assert_eq!(carried.answer.chars().count(), MAX_TURN_ANSWER_CHARS + 1);

        // Fields at the bound pass untouched — the clamp is a ceiling, not
        // a rewrite.
        let at_bound = vec![Turn {
            question: "q".repeat(MAX_QUESTION_CHARS),
            answer: "a".repeat(MAX_TURN_ANSWER_CHARS),
            citations: Vec::new(),
        }];
        let question = build(&graph, "what runs first?", &at_bound).unwrap();
        assert_eq!(question.turns[0].question, at_bound[0].question);
        assert_eq!(question.turns[0].answer, at_bound[0].answer);

        // And the reader's own bound is unchanged by history being present:
        // an over-long *current* question is still refused.
        let long = "a".repeat(MAX_QUESTION_CHARS + 1);
        assert!(build(&graph, &long, &at_bound).is_err());
    }

    #[test]
    fn a_question_is_trimmed_and_carries_the_project() {
        let graph = wide_graph(1);
        let question = build(&graph, "  what runs first?  ", &[]).unwrap();

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
            select_context(&graph, question, &[])
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
            select_context(&graph, "invoice", &[])
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
            select_context(&graph, "session", &[])
                .first()
                .map(|c| &*c.id),
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
            select_context(&graph, "auth", &[]).first().map(|c| &*c.id),
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
            select_context(&graph, "how are sessions validated?", &[])
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
            select_context(&graph, question, &[])
                .first()
                .map(|c| &*c.id),
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
            usage: None,
        });

        let question = build(&graph, "what starts things off?", &[]).unwrap();
        let answer = super::answer(&provider, &question).unwrap();

        assert_eq!(answer.text, "Widget zero starts things off.");
        assert_eq!(
            answer.citations,
            vec!["file:src/module0/widget0.ts".to_string()],
            "only checkable citations survive, and only once"
        );
    }

    /// Usage rides through citation verification untouched: dropping an
    /// invented citation says nothing about what the exchange spent.
    #[test]
    fn usage_survives_citation_verification() {
        let graph = wide_graph(2);
        let provider = Recording::new(Answer {
            text: "Widget zero starts things off.".into(),
            citations: vec!["file:src/does/not/exist.ts".into()],
            usage: Some(Usage {
                input_tokens: 1200,
                output_tokens: 80,
            }),
        });

        let question = build(&graph, "what starts things off?", &[]).unwrap();
        let answer = super::answer(&provider, &question).unwrap();

        assert_eq!(
            answer.usage,
            Some(Usage {
                input_tokens: 1200,
                output_tokens: 80,
            }),
            "the measured counts must survive verification"
        );
        assert_eq!(answer.citations, Vec::<String>::new());
    }

    /// The reading rule both backends share: two measured counts or nothing.
    /// A missing object, a missing field, or a count that is not an unsigned
    /// integer is absence — never a zero standing in for a measurement.
    #[test]
    fn envelope_usage_is_two_measured_counts_or_nothing() {
        let full = serde_json::json!({"input_tokens": 3, "output_tokens": 7});
        assert_eq!(
            Usage::from_envelope(Some(&full)),
            Some(Usage {
                input_tokens: 3,
                output_tokens: 7,
            })
        );

        assert_eq!(Usage::from_envelope(None), None, "no object at all");
        for (label, partial) in [
            ("output only", serde_json::json!({"output_tokens": 7})),
            ("input only", serde_json::json!({"input_tokens": 3})),
            (
                "a count that is not an unsigned integer",
                serde_json::json!({"input_tokens": "3", "output_tokens": 7}),
            ),
            (
                "a negative count",
                serde_json::json!({"input_tokens": -3, "output_tokens": 7}),
            ),
            ("not an object", serde_json::json!("3 in, 7 out")),
        ] {
            assert_eq!(
                Usage::from_envelope(Some(&partial)),
                None,
                "{label} must read as absent, never as zero"
            );
        }
    }

    #[test]
    fn the_provider_never_sees_more_than_the_bound() {
        let graph = wide_graph(200);
        let provider = Recording::new(Answer::default());

        let question = build(&graph, "widget gadget run module", &[]).unwrap();
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
        let question = build(&graph, "anything", &[]).unwrap();
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
        let question = build(&graph, "anything", &[]).unwrap();
        let err = super::answer(&EnrichOnly, &question).unwrap_err();
        assert!(
            err.to_string().contains("question"),
            "the refusal must say what is missing: {err}"
        );
    }
}
