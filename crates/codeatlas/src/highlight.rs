//! Server-side syntax highlighting for open code (ADR-0013, V3 ticket 03).
//!
//! The seven grammars the scanner already vendors — C, C++, Go, JavaScript,
//! Python, Rust, TypeScript — drive their own bundled highlight queries
//! in-process, through upstream's `tree-sitter-highlight` pinned in lockstep
//! with the repository's `tree-sitter` (0.26.12 beside 0.26.12). Nothing is
//! downloaded, nothing leaves the host, and no new grammar rides in: this is
//! the stale-blocker correction ADR-0013 records, made code.
//!
//! The seam is one function: [`highlight`] takes a repo-relative path and
//! the text actually being served — the *clipped* text, when the size cap
//! cut it, so a truncated file is highlighted exactly as served — and
//! returns HTML plus the language it decided on. A grammar-covered extension
//! comes back as escaped text wrapped in `<span class="hl-…">` tokens; any
//! other extension falls back to escaped plain text with the language stated
//! as [`PLAIN_TEXT`], so highlighting never gates what can be opened.
//!
//! Three properties the tests hold, because the dashboard builds on them:
//!
//! - **Everything is escaped.** Source content cannot smuggle markup to the
//!   browser: `<`, `>`, `&`, `'` and `"` arrive as entities on both paths —
//!   the renderer escapes on the grammar path, [`escape`] on the fallback.
//! - **Every line is self-contained.** The renderer closes open spans at
//!   each newline and reopens them after, so the dashboard can split on
//!   `\n` and render line by line — the lit-range mechanics of ticket 02
//!   keep working on highlighted lines.
//! - **The text is exactly what was given.** Tags aside, the visible text
//!   round-trips: no invented characters, and no invented trailing newline
//!   (the renderer appends one; [`spans`] takes it back off when the input
//!   did not end with it — a truncated file's cut must stay exactly the cut).
//!
//! Carriage returns are the one deliberate exception to the round-trip: the
//! renderer drops `\r` rather than paint it, so a CRLF file displays as its
//! lines. The bytes on disk are untouched — this module only ever renders.
//!
//! The `hl-…` class names are this module's half of a contract whose other
//! half is the dashboard's stylesheet (`dashboard/src/app/styles.css`),
//! which binds each class to a colour legible in both themes; a drift test
//! below reads the stylesheet so the two cannot part silently.

use std::path::Path;
use std::sync::OnceLock;

use tree_sitter_highlight::{HighlightConfiguration, Highlighter, HtmlRenderer};

/// What the envelope's `language` says when no vendored grammar covers the
/// file: the fallback is stated, never silent, so a reader of the envelope
/// can tell "unhighlighted because uncovered" from a grammar going quiet.
pub const PLAIN_TEXT: &str = "plain text";

/// The highlight names this server recognises, each becoming the CSS class
/// `hl-<name>` on a span. Grammar queries capture finer names — a
/// `@function.method` or `@punctuation.bracket` — and
/// `tree-sitter-highlight` resolves them to the longest matching prefix in
/// this list, so the base names here catch the whole family. Names are kept
/// dot-free so the class is always the name verbatim; captures matching
/// nothing here render as plain text inside the line, which is the honest
/// default for a token the stylesheet could not colour anyway.
const HIGHLIGHT_NAMES: [&str; 14] = [
    "attribute",
    "comment",
    "constant",
    "constructor",
    "function",
    "keyword",
    "label",
    "number",
    "operator",
    "property",
    "punctuation",
    "string",
    "tag",
    "type",
];

/// One vendored grammar, ready to answer: the language name the envelope
/// will carry, the extensions the scanner maps to it (`parsers::registry`
/// is the authority this table mirrors), and the compiled configuration —
/// query parsing is the expensive part, done once at first use and shared
/// (upstream documents `HighlightConfiguration` as immutable and
/// thread-shareable; the per-request state lives in [`Highlighter`]).
struct Grammar {
    language: &'static str,
    extensions: &'static [&'static str],
    config: HighlightConfiguration,
}

/// The compiled grammars plus the class attribute each recognised name
/// renders as — precomputed so the render callback writes bytes, not format
/// strings, per token.
struct Registry {
    grammars: Vec<Grammar>,
    class_attrs: Vec<String>,
}

/// Builds one grammar's configuration from its crate's own bundled queries.
/// Where a language is layered — C++ over C, TypeScript over JavaScript —
/// the queries concatenate base first: `tree-sitter-highlight` lets the
/// last pattern matching a node win, so the specific layer must come last
/// to override its base. A rejected query is a build defect in a vendored
/// constant (every grammar below is exercised by this module's tests), so
/// it panics rather than pretending the language is uncovered.
fn grammar(
    language: &'static str,
    extensions: &'static [&'static str],
    ts_language: tree_sitter::Language,
    highlight_queries: &[&str],
    locals_query: &str,
) -> Grammar {
    let query = highlight_queries.join("\n");
    let mut config = HighlightConfiguration::new(ts_language, language, &query, "", locals_query)
        .unwrap_or_else(|error| {
            panic!("the vendored {language} grammar rejected its own highlight query: {error}")
        });
    config.configure(&HIGHLIGHT_NAMES);
    Grammar {
        language,
        extensions,
        config,
    }
}

fn registry() -> &'static Registry {
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let js = tree_sitter_javascript::HIGHLIGHT_QUERY;
        let jsx = tree_sitter_javascript::JSX_HIGHLIGHT_QUERY;
        let ts = tree_sitter_typescript::HIGHLIGHTS_QUERY;
        Registry {
            grammars: vec![
                grammar(
                    "TypeScript",
                    &["ts"],
                    tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
                    &[js, ts],
                    tree_sitter_typescript::LOCALS_QUERY,
                ),
                grammar(
                    "TypeScript",
                    &["tsx"],
                    tree_sitter_typescript::LANGUAGE_TSX.into(),
                    &[js, jsx, ts],
                    tree_sitter_typescript::LOCALS_QUERY,
                ),
                grammar(
                    "JavaScript",
                    &["js", "jsx", "mjs", "cjs"],
                    tree_sitter_javascript::LANGUAGE.into(),
                    &[js, jsx],
                    tree_sitter_javascript::LOCALS_QUERY,
                ),
                grammar(
                    "Rust",
                    &["rs"],
                    tree_sitter_rust::LANGUAGE.into(),
                    &[tree_sitter_rust::HIGHLIGHTS_QUERY],
                    "",
                ),
                grammar(
                    "Python",
                    &["py"],
                    tree_sitter_python::LANGUAGE.into(),
                    &[tree_sitter_python::HIGHLIGHTS_QUERY],
                    "",
                ),
                grammar(
                    "Go",
                    &["go"],
                    tree_sitter_go::LANGUAGE.into(),
                    &[tree_sitter_go::HIGHLIGHTS_QUERY],
                    "",
                ),
                grammar(
                    "C",
                    &["c", "h"],
                    tree_sitter_c::LANGUAGE.into(),
                    &[tree_sitter_c::HIGHLIGHT_QUERY],
                    "",
                ),
                grammar(
                    "C++",
                    &["cpp", "cc", "cxx", "hpp", "hh", "hxx"],
                    tree_sitter_cpp::LANGUAGE.into(),
                    &[
                        tree_sitter_c::HIGHLIGHT_QUERY,
                        tree_sitter_cpp::HIGHLIGHT_QUERY,
                    ],
                    "",
                ),
            ],
            class_attrs: HIGHLIGHT_NAMES
                .iter()
                .map(|name| format!("class=\"hl-{name}\""))
                .collect(),
        }
    })
}

/// One highlighted answer: HTML whose text content is exactly the input
/// (escaped, spans around recognised tokens), and the language that says so.
pub struct Highlighted {
    pub html: String,
    pub language: &'static str,
}

/// Highlights `source` as the language `path`'s extension names, falling
/// back to escaped plain text — stated as such — for anything the seven
/// vendored grammars do not cover. `source` must be the text actually being
/// served: the caller clips first, so a truncated file is highlighted
/// exactly as served.
pub fn highlight(path: &str, source: &str) -> Highlighted {
    let extension = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default();
    let registry = registry();
    let Some(grammar) = registry
        .grammars
        .iter()
        .find(|g| g.extensions.contains(&extension))
    else {
        return plain(source);
    };
    match spans(registry, grammar, source) {
        Ok(html) => Highlighted {
            html,
            language: grammar.language,
        },
        // Highlighting is decoration, never a gate: tree-sitter parses
        // error-tolerantly, so this arm is for the truly exceptional —
        // and the honest answer then is readable text that says plain.
        Err(_) => plain(source),
    }
}

/// The grammar path: parse, run the bundled query, render as HTML — one
/// line per source line, spans closed at each newline and reopened after,
/// all text escaped by the renderer. The renderer also terminates its
/// output with a newline the input may not have had; that is taken back
/// off, because serving it would invent a character — on a truncated file,
/// precisely the "ended cleanly" the cut did not.
fn spans(
    registry: &Registry,
    grammar: &Grammar,
    source: &str,
) -> Result<String, tree_sitter_highlight::Error> {
    let mut highlighter = Highlighter::new();
    let events = highlighter.highlight(&grammar.config, source.as_bytes(), None, |_| None)?;
    let mut renderer = HtmlRenderer::new();
    renderer.render(events, source.as_bytes(), &|token, output| {
        output.extend_from_slice(registry.class_attrs[token.0].as_bytes());
    })?;
    let mut html = String::from_utf8(renderer.html)
        .expect("the renderer emits UTF-8: its input was a str and its markup is ASCII");
    if !source.ends_with('\n') && html.ends_with('\n') {
        html.pop();
    }
    Ok(html)
}

/// The uncovered-language answer, and the safety net under a grammar error:
/// the text escaped whole, no spans, the fallback stated.
fn plain(source: &str) -> Highlighted {
    Highlighted {
        html: escape(source),
        language: PLAIN_TEXT,
    }
}

/// The same five characters `tree-sitter-highlight`'s renderer escapes, so
/// both paths make one promise to the dashboard: source content cannot
/// smuggle markup, whichever way it arrived.
fn escape(source: &str) -> String {
    let mut escaped = String::with_capacity(source.len());
    for c in source.chars() {
        match c {
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '&' => escaped.push_str("&amp;"),
            '\'' => escaped.push_str("&#39;"),
            '"' => escaped.push_str("&quot;"),
            other => escaped.push(other),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The text a browser would show for `html`: tags dropped, entities
    /// decoded. Any literal `<` in source text arrives as `&lt;`, so every
    /// raw `<…>` in the HTML is a tag of ours and stripping to the next `>`
    /// is exact, not heuristic. The entity set is the renderer's five;
    /// `&amp;` decodes last so an escaped ampersand cannot cascade.
    fn visible_text(html: &str) -> String {
        let mut text = String::new();
        let mut rest = html;
        while let Some(open) = rest.find('<') {
            text.push_str(&rest[..open]);
            let close = rest[open..]
                .find('>')
                .expect("every tag the renderer opens, it closes");
            rest = &rest[open + close + 1..];
        }
        text.push_str(rest);
        text.replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&#39;", "'")
            .replace("&amp;", "&")
    }

    /// One snippet per vendored grammar, each holding at least a keyword
    /// and a string or type so the bundled queries have something to catch.
    /// The paths carry the extensions the scanner itself maps (see
    /// `parsers::registry`), so this table is the module's coverage claim.
    fn seven_grammars() -> Vec<(&'static str, &'static str, &'static str)> {
        vec![
            (
                "C",
                "src/main.c",
                "#include <stdio.h>\nint main(void) {\n  return 0; /* done */\n}\n",
            ),
            (
                "C++",
                "src/greeter.cpp",
                "class Greeter {\npublic:\n  int greet() { return 42; }\n};\n",
            ),
            (
                "Go",
                "cmd/main.go",
                "package main\n\nfunc main() {\n\tprintln(\"hi\")\n}\n",
            ),
            (
                "JavaScript",
                "src/main.js",
                "const greeting = \"hi\";\nfunction main() {\n  return greeting;\n}\n",
            ),
            ("Python", "src/main.py", "def main():\n    return \"hi\"\n"),
            (
                "Rust",
                "src/main.rs",
                "fn main() {\n    println!(\"hi\");\n}\n",
            ),
            (
                "TypeScript",
                "src/main.ts",
                "const answer: number = 42;\nexport function main(): number {\n  return answer;\n}\n",
            ),
        ]
    }

    #[test]
    fn every_vendored_grammar_yields_spans_and_names_its_language() {
        // The rule beside the wire (spec seam 2): each of the seven grammars
        // produces token spans through its own bundled highlight query, the
        // envelope-bound language is the grammar's name, and the text under
        // the markup is exactly the source that went in.
        for (language, path, source) in seven_grammars() {
            let highlighted = highlight(path, source);
            assert_eq!(
                highlighted.language, language,
                "{path} must be highlighted as {language}"
            );
            assert!(
                highlighted.html.contains("<span class=\"hl-"),
                "{language} must yield at least one token span, got: {}",
                highlighted.html
            );
            assert_eq!(
                visible_text(&highlighted.html),
                source,
                "{language}: the text under the spans must be the source itself"
            );
        }
    }

    #[test]
    fn the_sibling_extensions_land_on_their_family_grammar() {
        // The scanner's own extension families (parsers::registry): a
        // header is C, the C++ spellings are C++, `.tsx` is TypeScript and
        // the JS family is JavaScript — same grammar, same spans, whichever
        // spelling the repository uses.
        for (path, source, language) in [
            ("include/api.h", "int add(int a, int b);\n", "C"),
            (
                "src/impl.cc",
                "int add(int a, int b) { return a + b; }\n",
                "C++",
            ),
            ("include/api.hpp", "class Api {};\n", "C++"),
            (
                "src/App.tsx",
                "export const App = () => <div>hi</div>;\n",
                "TypeScript",
            ),
            ("src/legacy.jsx", "const x = <b>hi</b>;\n", "JavaScript"),
            ("src/mod.mjs", "export const x = 1;\n", "JavaScript"),
        ] {
            let highlighted = highlight(path, source);
            assert_eq!(highlighted.language, language, "{path}");
            assert!(
                highlighted.html.contains("<span class=\"hl-"),
                "{path} must reach the {language} grammar, got: {}",
                highlighted.html
            );
        }
    }

    #[test]
    fn an_uncovered_language_falls_back_to_plain_text_and_says_so() {
        // Highlighting never gates what can be opened: a mapped Markdown
        // file (a parser without a grammar) and an extension nothing maps
        // both arrive readable — escaped text, no spans — with the fallback
        // stated in the language, never a guess and never a refusal.
        for path in ["README.md", "notes.txt", "Makefile"] {
            let source = "# a heading\nplain prose, `code`, and 2 < 3.\n";
            let highlighted = highlight(path, source);
            assert_eq!(highlighted.language, PLAIN_TEXT, "{path}");
            assert!(
                !highlighted.html.contains("<span"),
                "{path} must carry no token spans: {}",
                highlighted.html
            );
            assert_eq!(visible_text(&highlighted.html), source, "{path}");
        }
    }

    #[test]
    fn source_content_cannot_smuggle_markup_on_either_path() {
        // The safety the dashboard's renderer leans on: it injects this HTML
        // verbatim, so a file containing markup must arrive as entities on
        // the grammar path and the fallback path alike. `<script>` is the
        // canonical payload — if it survives to the browser unescaped, the
        // opened file executes in the reader's dashboard.
        let payload = "const x = \"<script>alert('pwned')</script>\";\n";
        for path in ["src/evil.ts", "evil.weird"] {
            let highlighted = highlight(path, payload);
            assert!(
                !highlighted.html.contains("<script"),
                "{path}: markup in source must not survive as markup: {}",
                highlighted.html
            );
            assert!(
                highlighted.html.contains("&lt;script&gt;"),
                "{path}: the payload must arrive as entities: {}",
                highlighted.html
            );
            assert_eq!(
                visible_text(&highlighted.html),
                payload,
                "{path}: escaping must lose nothing"
            );
        }
    }

    #[test]
    fn a_clipped_prefix_highlights_exactly_as_served() {
        // Truncation composes with highlighting (the ticket's rule: clip
        // first, highlight the clipped bytes). A cap landing mid-string is
        // the worst case — an unterminated literal — and tree-sitter parses
        // error-tolerantly, so the prefix still highlights; what it must
        // never do is pad or trim what was served, and in particular the
        // renderer's synthetic trailing newline must not survive to claim
        // the file ended cleanly.
        let whole = "fn main() {\n    let s = \"a string the cap will cut in half\";\n}\n";
        let cut = &whole[..whole.find("cap").expect("the marker is in the string")];
        assert!(!cut.ends_with('\n'), "the fixture must cut mid-line");

        let highlighted = highlight("src/main.rs", cut);
        assert_eq!(highlighted.language, "Rust");
        assert_eq!(
            visible_text(&highlighted.html),
            cut,
            "the highlighted text must be exactly the served prefix — \
             nothing invented, nothing dropped"
        );
        assert!(
            highlighted.html.contains("<span class=\"hl-"),
            "a syntactically cut file still highlights: {}",
            highlighted.html
        );
    }

    #[test]
    fn every_line_is_self_contained_so_the_dashboard_can_split_on_newlines() {
        // Ticket 02's panel renders one element per line and lights ranges
        // by line; this module's HTML keeps that possible by never letting a
        // span straddle a newline. A multi-line token — a block comment — is
        // the case that would break it.
        let source = "/* a comment\n   that spans\n   three lines */\nfn main() {}\n";
        let highlighted = highlight("src/lib.rs", source);

        let lines: Vec<&str> = highlighted.html.split('\n').collect();
        assert_eq!(
            lines.len(),
            source.split('\n').count(),
            "lines must map one to one onto the source's"
        );
        for (number, line) in lines.iter().enumerate() {
            assert_eq!(
                line.matches("<span").count(),
                line.matches("</span>").count(),
                "line {} must open exactly the spans it closes: {line}",
                number + 1
            );
        }
        // And the comment really was highlighted across those lines — the
        // balance above must not be the balance of nothing happening.
        assert!(
            highlighted.html.contains("hl-comment"),
            "the block comment must carry its span: {}",
            highlighted.html
        );
    }

    #[test]
    fn every_class_the_server_can_emit_is_bound_by_the_dashboard_stylesheet() {
        // The other half of the class contract. The dashboard styles these
        // spans with its own stylesheet (ADR-0013: no client-side highlight
        // library, no bundle growth beyond styles); a name added here
        // without a `.hl-…` rule there would ship unreadable-in-no-theme
        // tokens silently. Same shape as the route drift gates: the code's
        // own list, checked against the document that must follow it.
        let stylesheet = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../dashboard/src/app/styles.css"
        ))
        .expect("the dashboard stylesheet is part of this repository");
        for name in HIGHLIGHT_NAMES {
            assert!(
                stylesheet.contains(&format!(".hl-{name}")),
                "styles.css binds no colour to .hl-{name}; every class the \
                 server can emit must be styled (legibly, in both themes — \
                 the stylesheet contract test holds that half)"
            );
        }
    }
}
