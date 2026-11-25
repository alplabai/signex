//! What a notebook's Typst says, read with the Typst parser itself: the
//! chapter files it includes and every reference it makes, in markup and in
//! code alike.

use typst::syntax::ast;
use typst::syntax::{SyntaxKind, SyntaxNode};

use crate::reference::Reference;
use crate::scan::{FrontMatter, front_matter_end};

/// The Typst the compiler sees for the file text `source`: the header's
/// lines are blanked, so every line keeps its number in the file.
pub fn typst_source(source: &str) -> String {
    match front_matter_end(source) {
        FrontMatter::Closed { body, .. } => {
            let blank = source[..body].matches('\n').count();
            format!("{}{}", "\n".repeat(blank), &source[body..])
        }
        FrontMatter::None | FrontMatter::Unclosed => source.to_string(),
    }
}

/// An `#include "chapters/power.snxnbk"` of a notebook file.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Include {
    /// The path as written, relative to the including file.
    pub path: String,
    /// The 1-based line the include is on.
    pub line: u32,
}

/// A reference and the 1-based line it is on.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ReferenceAt {
    pub reference: Reference,
    pub line: u32,
}

/// The includes of the file text `source` whose path is a string literal,
/// in source order. An include of a computed path is not listed.
pub fn includes(source: &str) -> Vec<Include> {
    let text = typst_source(source);
    let mut found = Vec::new();
    walk(&typst::syntax::parse(&text), 0, &mut |node, offset| {
        let Some(include) = node.cast::<ast::ModuleInclude>() else {
            return;
        };
        if let ast::Expr::Str(path) = include.source() {
            found.push(Include {
                path: path.get().to_string(),
                line: line_of(&text, offset),
            });
        }
    });
    found
}

/// Every `component`, `pin` and `net` call of the file text `source` whose
/// arguments are the quoted names it takes, in source order, repeats kept.
/// `#pin("U3", "3")` and `#pin("U3.3")` both name pin 3 of U3.
pub fn references(source: &str) -> Vec<ReferenceAt> {
    let text = typst_source(source);
    let mut found = Vec::new();
    walk(&typst::syntax::parse(&text), 0, &mut |node, offset| {
        if let Some(call) = node.cast::<ast::FuncCall>()
            && let Some(reference) = reference_of(call)
        {
            found.push(ReferenceAt {
                reference,
                line: line_of(&text, offset),
            });
        }
    });
    found
}

/// A reference call in Typst text and the bytes it covers: from the
/// function name to the closing `)`, without a markup `#`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReferenceSpan {
    pub reference: Reference,
    pub range: std::ops::Range<usize>,
    pub line: u32,
}

/// Every reference call of `text`, plain Typst with no header, in source
/// order: what a fix rewrites.
pub(crate) fn reference_spans(text: &str) -> Vec<ReferenceSpan> {
    let mut found = Vec::new();
    walk(&typst::syntax::parse(text), 0, &mut |node, offset| {
        if let Some(call) = node.cast::<ast::FuncCall>()
            && let Some(reference) = reference_of(call)
        {
            found.push(ReferenceSpan {
                reference,
                range: offset..offset + node.len(),
                line: line_of(text, offset),
            });
        }
    });
    found
}

/// The distinct references of `source`, first mention first.
pub fn distinct_references(source: &str) -> Vec<Reference> {
    let mut distinct: Vec<Reference> = Vec::new();
    for at in references(source) {
        if !distinct.contains(&at.reference) {
            distinct.push(at.reference);
        }
    }
    distinct
}

fn reference_of(call: ast::FuncCall<'_>) -> Option<Reference> {
    let ast::Expr::Ident(name) = call.callee() else {
        return None;
    };
    let mut texts = Vec::new();
    for arg in call.args().items() {
        match arg {
            ast::Arg::Pos(ast::Expr::Str(text)) => texts.push(text.get().to_string()),
            _ => return None,
        }
    }
    let nonempty = texts.iter().all(|text| !text.trim().is_empty());
    match (name.as_str(), texts.as_slice()) {
        _ if !nonempty => None,
        ("component", [name]) => Some(Reference::Component(name.clone())),
        ("net", [name]) => Some(Reference::Net(name.clone())),
        ("pin", [component, pin]) => Some(Reference::Pin {
            component: component.clone(),
            pin: pin.clone(),
        }),
        ("pin", [dotted]) => Reference::dotted_pin(dotted),
        _ => None,
    }
}

/// Visit `node` and its descendants in source order with their byte
/// offsets.
pub(crate) fn walk(node: &SyntaxNode, offset: usize, visit: &mut impl FnMut(&SyntaxNode, usize)) {
    visit(node, offset);
    if node.kind() == SyntaxKind::Error {
        // An error node holds no structure to look into.
        return;
    }
    let mut at = offset;
    for child in node.children() {
        walk(child, at, visit);
        at += child.len();
    }
}

/// The 1-based line byte `offset` of `text` is on.
pub(crate) fn line_of(text: &str, offset: usize) -> u32 {
    let before = text.get(..offset).unwrap_or(text);
    u32::try_from(before.matches('\n').count() + 1).unwrap_or(u32::MAX)
}
