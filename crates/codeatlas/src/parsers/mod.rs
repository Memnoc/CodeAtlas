//! The per-language parser interface. Each supported language implements
//! [`Parser`]; everything else in the pipeline is language-agnostic.

use std::collections::HashSet;
use std::path::Path;

mod c_cpp;
mod go;
mod markdown;
mod python;
mod rust;
mod ts_js;

/// Everything a parser extracts from one file: the structural facts only.
#[derive(Debug, Default)]
pub struct Analysis {
    pub symbols: Vec<Symbol>,
    pub imports: Vec<Import>,
    pub calls: Vec<Call>,
    /// Names published by export clauses that need indirection to resolve:
    /// aliases (`export { internal as external }`) and one-level re-exports
    /// (`export { x } from "./y"`).
    pub reexports: Vec<Reexport>,
}

/// A name an export clause makes available to importers under a possibly
/// different name, possibly from another module.
#[derive(Debug)]
pub struct Reexport {
    /// The name importers see.
    pub exported: String,
    /// The name as defined — in this file when `specifier` is `None`,
    /// otherwise in the module the specifier names.
    pub local: String,
    /// Present for `export … from`: the module the name actually comes from.
    pub specifier: Option<String>,
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
    /// `root` is the scanned repository root, for the rare language whose
    /// resolution is anchored by a config file (Go reads `go.mod` from it);
    /// most parsers resolve from paths alone and ignore it.
    /// `None` when unresolvable (bare package, file outside the map) — the
    /// caller drops the edge rather than emit it dangling.
    fn resolve_import(
        &self,
        importer: &str,
        specifier: &str,
        files: &HashSet<String>,
        root: &Path,
    ) -> Option<String>;
    /// Resolves a *bound name* — `name` as written in the module it comes
    /// from, never the local alias — as a module file in its own right.
    /// `None` means the name is not a module, and so lands wherever the
    /// specifier does; that is the answer for every language whose specifier
    /// always names the module, hence the default. Python overrides it: in
    /// `from pkg import util` the name may be `pkg/util.py`, so one
    /// statement can reach several files.
    fn resolve_name_as_module(
        &self,
        _importer: &str,
        _specifier: &str,
        _name: &str,
        _files: &HashSet<String>,
        _root: &Path,
    ) -> Option<String> {
        None
    }
    /// Whether all files in one directory share a single namespace (Go
    /// packages): a plain-identifier call may then resolve to a function
    /// defined in a sibling file without any import.
    fn directory_shares_scope(&self) -> bool {
        false
    }
}

/// The registry of compiled-in language parsers (ADR-0006: no runtime
/// downloads — grammars are part of the binary).
fn registry() -> &'static [Box<dyn Parser>] {
    use std::sync::OnceLock;
    static REGISTRY: OnceLock<Vec<Box<dyn Parser>>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let mut parsers = ts_js::parsers();
        parsers.extend(rust::parsers());
        parsers.extend(python::parsers());
        parsers.extend(go::parsers());
        parsers.extend(c_cpp::parsers());
        parsers.extend(markdown::parsers());
        parsers
    })
}

/// Finds the parser for a file extension, if the language is supported.
pub fn for_extension(extension: &str) -> Option<&'static dyn Parser> {
    registry()
        .iter()
        .find(|p| p.extensions().contains(&extension))
        .map(|b| b.as_ref())
}
