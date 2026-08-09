//! The per-language parser interface. Each supported language implements
//! [`Parser`]; everything else in the pipeline is language-agnostic.

use std::collections::HashSet;

mod ts_js;

/// Everything a parser extracts from one file: the structural facts only.
#[derive(Debug, Default)]
pub struct Analysis {
    pub symbols: Vec<Symbol>,
    pub imports: Vec<Import>,
    pub calls: Vec<Call>,
}

/// A symbol extracted from one file.
#[derive(Debug)]
pub struct Symbol {
    pub kind: SymbolKind,
    pub name: String,
    /// 1-based inclusive lines.
    pub start_line: u32,
    pub end_line: u32,
    /// Whether the file exports this symbol (inline modifier or export
    /// clause). Methods of an exported class are not themselves exported.
    pub exported: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Function,
    Class,
}

/// One import statement: the module specifier as written in source.
#[derive(Debug)]
pub struct Import {
    /// e.g. `./util` or `lodash`; resolution happens later, against the
    /// scanned file set.
    pub specifier: String,
    /// Named bindings this import introduces, for call resolution.
    pub names: Vec<ImportedName>,
}

/// A named import binding: `import { imported as local }`.
#[derive(Debug)]
pub struct ImportedName {
    /// The name in the importing file's scope.
    pub local: String,
    /// The name as exported by the target file.
    pub imported: String,
}

/// A function invocation: `caller` (qualified symbol name) invokes `callee`
/// (a plain identifier as written; resolution happens later).
#[derive(Debug)]
pub struct Call {
    pub caller: String,
    pub callee: String,
}

pub trait Parser: Send + Sync {
    /// Language name as it appears in mechanical summaries, e.g. "TypeScript".
    fn language_name(&self) -> &'static str;
    /// File extensions (without dot) this parser handles.
    fn extensions(&self) -> &'static [&'static str];
    /// Extracts structural facts from source. Must tolerate syntax errors:
    /// return whatever is recoverable, never fail the scan.
    fn parse(&self, source: &str) -> Analysis;
    /// Resolves an import specifier written in `importer` (repo-relative
    /// path) to a repo-relative file path, given the set of scanned files.
    /// `None` when unresolvable (bare package, file outside the map) — the
    /// caller drops the edge rather than emit it dangling.
    fn resolve_import(
        &self,
        importer: &str,
        specifier: &str,
        files: &HashSet<String>,
    ) -> Option<String>;
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
