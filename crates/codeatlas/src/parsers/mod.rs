//! The per-language parser interface. Each supported language implements
//! [`Parser`]; everything else in the pipeline is language-agnostic.

mod ts_js;

/// A symbol extracted from one file: the structural facts only.
#[derive(Debug)]
pub struct Symbol {
    pub kind: SymbolKind,
    pub name: String,
    /// 1-based inclusive lines.
    pub start_line: u32,
    pub end_line: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Function,
    Class,
}

pub trait Parser: Send + Sync {
    /// Language name as it appears in mechanical summaries, e.g. "TypeScript".
    fn language_name(&self) -> &'static str;
    /// File extensions (without dot) this parser handles.
    fn extensions(&self) -> &'static [&'static str];
    /// Extracts symbols from source. Must tolerate syntax errors: return
    /// whatever is recoverable, never fail the scan.
    fn parse(&self, source: &str) -> Vec<Symbol>;
}

/// The registry of compiled-in language parsers (ADR-0006: no runtime
/// downloads — grammars are part of the binary).
fn registry() -> &'static [Box<dyn Parser>] {
    use std::sync::OnceLock;
    static REGISTRY: OnceLock<Vec<Box<dyn Parser>>> = OnceLock::new();
    REGISTRY.get_or_init(ts_js::parsers)
}

/// Finds the parser for a file extension, if the language is supported.
pub fn for_extension(extension: &str) -> Option<&'static dyn Parser> {
    registry()
        .iter()
        .find(|p| p.extensions().contains(&extension))
        .map(|b| b.as_ref())
}
