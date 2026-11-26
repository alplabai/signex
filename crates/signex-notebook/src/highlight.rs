//! Typst highlighting for the notebook editor (mockup row 195, "Code
//! editor"): headings, function calls, strings, math, labels, comments and
//! the design references, read with the Typst parser over the whole text so
//! a raw block, an equation or a comment that spans lines is coloured on
//! every line it covers. The editor asks per line; [`Highlights::line`]
//! answers from the one parse.

use std::ops::Range;

use typst::syntax::{LinkedNode, SyntaxKind, Tag, ast};

/// What a span of the source is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Token {
    Heading,
    /// A function name: `#figure`, `report.with`.
    Function,
    /// A `#component`, `#pin` or `#net` call, as a whole.
    Reference,
    String,
    Number,
    /// An equation between `$`, as a whole.
    Math,
    /// `<sec:power>` and `@sec:power`.
    Label,
    Comment,
    /// `let`, `show`, `import`, `include`, `set`, ...
    Keyword,
    /// Raw text between backticks.
    Raw,
    /// `*strong*` and `_emphasis_`.
    Emphasis,
}

/// The coloured spans of one text, by 0-based line, each a byte range of
/// that line.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Highlights {
    lines: Vec<Vec<(Range<usize>, Token)>>,
}

impl Highlights {
    /// The spans of `text`, which is Typst source (the notebook body, the
    /// header left out).
    pub fn of(text: &str) -> Self {
        let root = typst::syntax::parse(text);
        let mut spans = Vec::new();
        visit(&LinkedNode::new(&root), &mut spans);
        Self {
            lines: split_lines(text, spans),
        }
    }

    /// The spans of line `index`, in order; empty past the last line.
    pub fn line(&self, index: usize) -> &[(Range<usize>, Token)] {
        self.lines.get(index).map_or(&[], Vec::as_slice)
    }

    /// How many lines the text had.
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }
}

/// Collect the spans under `node`. A node that is one token as a whole
/// (a heading, an equation, a reference call, a raw block, a string) is
/// not looked into.
fn visit(node: &LinkedNode<'_>, out: &mut Vec<(Range<usize>, Token)>) {
    if let Some(token) = whole_token(node) {
        out.push((node.range(), token));
        return;
    }
    if node.get().children().next().is_none() {
        if let Some(token) = typst::syntax::highlight(node).and_then(token_of) {
            out.push((node.range(), token));
        }
        return;
    }
    for child in node.children() {
        visit(&child, out);
    }
}

/// The token a node is as a whole, if it is one.
fn whole_token(node: &LinkedNode<'_>) -> Option<Token> {
    match node.kind() {
        SyntaxKind::Heading => Some(Token::Heading),
        SyntaxKind::Equation => Some(Token::Math),
        SyntaxKind::Raw => Some(Token::Raw),
        SyntaxKind::Str => Some(Token::String),
        SyntaxKind::LineComment | SyntaxKind::BlockComment => Some(Token::Comment),
        SyntaxKind::Label | SyntaxKind::Ref => Some(Token::Label),
        SyntaxKind::FuncCall if is_reference_call(node) => Some(Token::Reference),
        _ => None,
    }
}

fn is_reference_call(node: &LinkedNode<'_>) -> bool {
    node.get().cast::<ast::FuncCall>().is_some_and(|call| {
        matches!(call.callee(), ast::Expr::Ident(name)
            if matches!(name.as_str(), "component" | "pin" | "net"))
    })
}

/// The token a leaf's Typst highlighting tag is drawn as.
fn token_of(tag: Tag) -> Option<Token> {
    match tag {
        Tag::Function => Some(Token::Function),
        Tag::Keyword => Some(Token::Keyword),
        Tag::Number => Some(Token::Number),
        Tag::String => Some(Token::String),
        Tag::Comment => Some(Token::Comment),
        Tag::Label | Tag::Ref => Some(Token::Label),
        Tag::Heading => Some(Token::Heading),
        Tag::Raw => Some(Token::Raw),
        Tag::Strong | Tag::Emph => Some(Token::Emphasis),
        Tag::MathDelimiter | Tag::MathOperator => Some(Token::Math),
        _ => None,
    }
}

/// Cut document byte ranges into per-line ranges. A `\r\n` ending is left
/// out of the line, as the editor shows it.
fn split_lines(text: &str, spans: Vec<(Range<usize>, Token)>) -> Vec<Vec<(Range<usize>, Token)>> {
    let mut starts = vec![0];
    starts.extend(text.match_indices('\n').map(|(at, _)| at + 1));
    let mut lines: Vec<Vec<(Range<usize>, Token)>> = vec![Vec::new(); starts.len()];
    for (range, token) in spans {
        let first = starts.partition_point(|start| *start <= range.start) - 1;
        for (line, start) in starts.iter().enumerate().skip(first) {
            let end_of_line = starts.get(line + 1).map_or(text.len(), |next| next - 1);
            let end_of_line = if text[..end_of_line].ends_with('\r') {
                end_of_line - 1
            } else {
                end_of_line
            };
            let from = range.start.max(*start);
            let to = range.end.min(end_of_line);
            if to > from {
                lines[line].push((from - start..to - start, token));
            }
            if range.end <= end_of_line {
                break;
            }
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn of_line(text: &str, line: usize) -> Vec<(String, Token)> {
        let lines: Vec<&str> = text.split('\n').collect();
        Highlights::of(text)
            .line(line)
            .iter()
            .map(|(range, token)| (lines[line][range.clone()].to_string(), *token))
            .collect()
    }

    #[test]
    fn a_heading_is_one_span() {
        assert_eq!(
            of_line("= Power stage\nText.", 0),
            [("= Power stage".to_string(), Token::Heading)]
        );
    }

    #[test]
    fn a_reference_call_is_one_span_and_other_calls_colour_their_name() {
        let spans = of_line("See #component(\"R7\") and #figure(image(\"a.png\"))", 0);
        let has = |want: &str, token: Token| {
            spans
                .iter()
                .any(|(text, found)| text.ends_with(want) && *found == token)
        };
        assert!(has("component(\"R7\")", Token::Reference), "{spans:?}");
        assert!(has("figure", Token::Function), "{spans:?}");
        assert!(has("\"a.png\"", Token::String), "{spans:?}");
        assert!(
            !has("\"R7\"", Token::String),
            "inside the reference: {spans:?}"
        );
    }

    #[test]
    fn an_equation_over_two_lines_is_math_on_both() {
        let text = "$ a &= b \\\n  &= c $\nafter";
        assert_eq!(of_line(text, 0), [("$ a &= b \\".to_string(), Token::Math)]);
        assert_eq!(of_line(text, 1), [("  &= c $".to_string(), Token::Math)]);
        assert!(of_line(text, 2).is_empty());
    }

    #[test]
    fn comments_and_labels_are_coloured() {
        let text = "// measured values\n= Ripple <sec:ripple>\nsee @sec:ripple";
        assert_eq!(
            of_line(text, 0),
            [("// measured values".to_string(), Token::Comment)]
        );
        assert!(
            of_line(text, 2).contains(&("@sec:ripple".to_string(), Token::Label)),
            "{:?}",
            of_line(text, 2)
        );
    }

    #[test]
    fn a_line_past_the_end_has_no_spans() {
        let highlights = Highlights::of("= A\n");
        assert_eq!(highlights.line_count(), 2);
        assert!(highlights.line(7).is_empty());
    }

    #[test]
    fn spans_never_cover_a_carriage_return() {
        let text = "= A\r\nb";
        let spans = Highlights::of(text);
        assert_eq!(spans.line(0), &[(0..3, Token::Heading)]);
    }
}
