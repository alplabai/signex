//! The named arguments of a block call, read with the Typst parser as the
//! literals they are written as: strings, numbers, percentages, design
//! references, dictionaries, arrays and content. A computed argument
//! (`value: x * 2`) is kept as its source text, so the reader can say what
//! it could not use instead of guessing.

use typst::syntax::ast::{self, AstNode};
use typst::syntax::{SyntaxKind, SyntaxNode};

use crate::reference::Reference;
use crate::syntax::reference_of;

/// One argument value as written.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Literal {
    Str(String),
    /// A number and its source text (`3.310` keeps its trailing zero).
    Number(f64, String),
    /// A percentage, `2%`, as the number before the sign.
    Percent(f64),
    None,
    Reference(Reference),
    Array(Vec<Literal>),
    /// Keys as written: an identifier or a string.
    Dict(Vec<(String, Literal)>),
    /// A content block's source, brackets dropped and trimmed.
    Content(String),
    /// Anything else, as its source text.
    Other(String),
}

impl Literal {
    /// The text of a string or of a content block.
    pub(crate) fn text(&self) -> Option<&str> {
        match self {
            Literal::Str(text) | Literal::Content(text) => Some(text),
            _ => None,
        }
    }

    pub(crate) fn number(&self) -> Option<f64> {
        match self {
            Literal::Number(value, _) => Some(*value),
            _ => None,
        }
    }

    /// The references a `target:` names: one, or an array of them. Other
    /// items are skipped.
    pub(crate) fn references(&self) -> Vec<Reference> {
        match self {
            Literal::Reference(reference) => vec![reference.clone()],
            Literal::Array(items) => items
                .iter()
                .filter_map(|item| match item {
                    Literal::Reference(reference) => Some(reference.clone()),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    /// The entry `key` of a dictionary.
    pub(crate) fn get(&self, key: &str) -> Option<&Literal> {
        match self {
            Literal::Dict(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
}

/// A call found in a source: its arguments, the byte range of each named
/// argument's value, and the line it starts on.
#[derive(Debug, Clone)]
pub(crate) struct FoundCall {
    pub named: Vec<NamedArg>,
    pub positional: Vec<Literal>,
    pub line: u32,
    /// The index, in the same list, of the call of the same name this one
    /// sits in.
    pub parent: Option<usize>,
}

#[derive(Debug, Clone)]
pub(crate) struct NamedArg {
    pub name: String,
    pub value: Literal,
    /// Byte range of the value in the compiled text (the file's text with
    /// its header lines blanked).
    pub span: std::ops::Range<usize>,
}

impl FoundCall {
    pub(crate) fn arg(&self, name: &str) -> Option<&Literal> {
        self.named
            .iter()
            .find(|arg| arg.name == name)
            .map(|arg| &arg.value)
    }

    pub(crate) fn arg_span(&self, name: &str) -> Option<std::ops::Range<usize>> {
        self.named
            .iter()
            .find(|arg| arg.name == name)
            .map(|arg| arg.span.clone())
    }

    /// The trailing content block, when the call has one.
    pub(crate) fn body(&self) -> Option<&str> {
        self.positional.iter().rev().find_map(|value| match value {
            Literal::Content(text) => Some(text.as_str()),
            _ => None,
        })
    }
}

/// Every call of `function` in `text` (Typst source, header blanked), in
/// source order, in markup (`#measurement(..)`) and in code alike.
pub(crate) fn calls_of(text: &str, function: &str) -> Vec<FoundCall> {
    let root = typst::syntax::parse(text);
    let mut found = Vec::new();
    collect(&root, 0, function, None, text, &mut found);
    found
}

fn collect(
    node: &SyntaxNode,
    offset: usize,
    function: &str,
    parent: Option<usize>,
    text: &str,
    found: &mut Vec<FoundCall>,
) {
    let mut parent = parent;
    if let Some(call) = node.cast::<ast::FuncCall>()
        && matches!(call.callee(), ast::Expr::Ident(name) if name.as_str() == function)
    {
        found.push(read_call(call, offset, parent, text));
        parent = Some(found.len() - 1);
    }
    if node.kind() == SyntaxKind::Error {
        return;
    }
    let mut at = offset;
    for child in node.children() {
        collect(child, at, function, parent, text, found);
        at += child.len();
    }
}

fn read_call(
    call: ast::FuncCall<'_>,
    offset: usize,
    parent: Option<usize>,
    text: &str,
) -> FoundCall {
    let mut named = Vec::new();
    let mut positional = Vec::new();
    let args = call.args();
    // Where the arguments start: the callee comes first.
    let mut at = offset + call.callee().to_untyped().len();
    let args_node = args.to_untyped();
    for child in args_node.children() {
        let start = at;
        at += child.len();
        if child.kind().is_trivia() {
            continue;
        }
        if let Some(arg) = child.cast::<ast::Named>() {
            let value_len = arg.expr().to_untyped().len();
            named.push(NamedArg {
                name: arg.name().as_str().to_string(),
                value: literal(arg.expr()),
                span: (start + child.len() - value_len)..(start + child.len()),
            });
        } else if let Some(expr) = child.cast::<ast::Expr>() {
            positional.push(literal(expr));
        }
    }
    FoundCall {
        named,
        positional,
        line: crate::syntax::line_of(text, offset),
        parent,
    }
}

/// `expr` as the literal it is written as.
pub(crate) fn literal(expr: ast::Expr<'_>) -> Literal {
    let source = || expr.to_untyped().clone().into_text().to_string();
    match expr {
        ast::Expr::Str(text) => Literal::Str(text.get().to_string()),
        ast::Expr::Int(int) => Literal::Number(int.get() as f64, source()),
        ast::Expr::Float(float) => Literal::Number(float.get(), source()),
        ast::Expr::Numeric(numeric) => match numeric.get() {
            (value, ast::Unit::Percent) => Literal::Percent(value),
            _ => Literal::Other(source()),
        },
        ast::Expr::None(_) => Literal::None,
        ast::Expr::Unary(unary) if unary.op() == ast::UnOp::Neg => match literal(unary.expr()) {
            Literal::Number(value, text) => Literal::Number(-value, format!("-{text}")),
            Literal::Percent(value) => Literal::Percent(-value),
            _ => Literal::Other(source()),
        },
        ast::Expr::Unary(unary) if unary.op() == ast::UnOp::Pos => literal(unary.expr()),
        ast::Expr::Parenthesized(inner) => literal(inner.expr()),
        ast::Expr::FuncCall(call) => match reference_of(call) {
            Some(reference) => Literal::Reference(reference),
            None => Literal::Other(source()),
        },
        ast::Expr::Array(array) => Literal::Array(
            array
                .items()
                .filter_map(|item| match item {
                    ast::ArrayItem::Pos(expr) => Some(literal(expr)),
                    ast::ArrayItem::Spread(_) => None,
                })
                .collect(),
        ),
        ast::Expr::Dict(dict) => Literal::Dict(
            dict.items()
                .filter_map(|item| match item {
                    ast::DictItem::Named(named) => {
                        Some((named.name().as_str().to_string(), literal(named.expr())))
                    }
                    ast::DictItem::Keyed(keyed) => match keyed.key() {
                        ast::Expr::Str(key) => Some((key.get().to_string(), literal(keyed.expr()))),
                        _ => None,
                    },
                    ast::DictItem::Spread(_) => None,
                })
                .collect(),
        ),
        ast::Expr::Content(block) => {
            let text = block.body().to_untyped().clone().into_text();
            Literal::Content(text.trim().to_string())
        }
        _ => Literal::Other(source()),
    }
}
