//! TypeScript / JavaScript extraction via compiled-in tree-sitter grammars.

use tree_sitter::{Language, Node as TsNode};

use super::{Parser, Symbol, SymbolKind};

struct TsJs {
    name: &'static str,
    extensions: &'static [&'static str],
    language: Language,
}

pub(super) fn parsers() -> Vec<Box<dyn Parser>> {
    vec![
        Box::new(TsJs {
            name: "TypeScript",
            extensions: &["ts"],
            language: tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        }),
        Box::new(TsJs {
            name: "TypeScript",
            extensions: &["tsx"],
            language: tree_sitter_typescript::LANGUAGE_TSX.into(),
        }),
        Box::new(TsJs {
            name: "JavaScript",
            extensions: &["js", "jsx", "mjs", "cjs"],
            language: tree_sitter_javascript::LANGUAGE.into(),
        }),
    ]
}

impl Parser for TsJs {
    fn language_name(&self) -> &'static str {
        self.name
    }

    fn extensions(&self) -> &'static [&'static str] {
        self.extensions
    }

    fn parse(&self, source: &str) -> Vec<Symbol> {
        let mut parser = tree_sitter::Parser::new();
        if parser.set_language(&self.language).is_err() {
            return Vec::new();
        }
        let Some(tree) = parser.parse(source, None) else {
            return Vec::new();
        };
        let mut symbols = Vec::new();
        collect(tree.root_node(), source.as_bytes(), None, &mut symbols);
        symbols
    }
}

fn collect(node: TsNode, source: &[u8], scope: Option<&str>, out: &mut Vec<Symbol>) {
    let kind = match node.kind() {
        "function_declaration" | "generator_function_declaration" | "method_definition" => {
            Some(SymbolKind::Function)
        }
        "class_declaration" => Some(SymbolKind::Class),
        _ => None,
    };
    let name = node
        .child_by_field_name("name")
        .and_then(|n| n.utf8_text(source).ok());

    if let (Some(kind), Some(name)) = (kind, name) {
        // Members are scope-qualified (`Alpha.run`) so same-named symbols in
        // different scopes get distinct node IDs.
        let qualified = match (kind, scope) {
            (SymbolKind::Function, Some(scope)) => format!("{scope}.{name}"),
            _ => name.to_string(),
        };
        out.push(Symbol {
            kind,
            name: qualified,
            start_line: node.start_position().row as u32 + 1,
            end_line: node.end_position().row as u32 + 1,
        });
    }

    let child_scope = if node.kind() == "class_declaration" {
        name
    } else {
        scope
    };
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect(child, source, child_scope, out);
    }
}
