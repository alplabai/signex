//! Recursive-descent parser for the sketch expression language.
//! Cleanroom — see crate-level doc comment in `expr/mod.rs`.
//!
//! Reference: standard recursive-descent design per Aho/Sethi/Ullman,
//! *Compilers: Principles, Techniques, and Tools* (Dragon Book).
//! Implementation derived from first principles; no third-party
//! parser source consulted.
//!
//! # Grammar (left-associative unless noted)
//!
//! ```text
//! expr        ::= ternary
//! ternary     ::= or ('?' expr ':' expr)?
//! or          ::= and ('||' and)*
//! and         ::= equality ('&&' equality)*
//! equality    ::= comparison (('==' | '!=') comparison)*
//! comparison  ::= sum (('<' | '<=' | '>' | '>=') sum)*
//! sum         ::= product (('+' | '-') product)*
//! product     ::= power (('*' | '/' | '%') power)*
//! power       ::= unary ('^' unary)?       // RIGHT-associative
//! unary       ::= ('-' | '!')? primary
//! primary     ::= QUANTITY | IDENT | '(' expr ')' | call | array_idx
//! call        ::= 'lookup' '(' expr ',' '[' list ']' ',' '[' list ']' ')'
//! array_idx   ::= 'i' | 'j'                // bare 1-char identifiers only
//! list        ::= expr (',' expr)*
//! ```
//!
//! # Lexing notes
//!
//! - Whitespace between tokens is silently skipped.
//! - A `QUANTITY` lexes greedily as `[0-9.]+[a-zA-Z]*` so suffixes
//!   like `mm`, `mil`, `deg` ride along with the digit run; the
//!   resulting slice is fed to [`crate::unit::parse_quantity`].
//! - Identifiers are `[a-zA-Z_][a-zA-Z0-9_]*`. The reserved word
//!   `lookup` becomes a function call; bare 1-char `i`/`j` become
//!   [`ArrayIndex::I`] / [`ArrayIndex::J`]; everything else is a
//!   [`ExprNode::Ref`].
//! - There are no string or boolean literals — comparison and
//!   logical operators yield dimensionless 0/1 at evaluation time.
//! - There are no comments inside expressions; the source is a
//!   single line.

use crate::expr::ast::{ArrayIndex, BinOp, ExprNode, UnaryOp};
use crate::expr::ExprError;
use crate::unit::parse_quantity;

// =========================================================================
// Tokens
// =========================================================================

/// One lexical token.
///
/// `Quantity` carries the raw source slice (digits + unit suffix) so
/// the parser can hand it to [`crate::unit::parse_quantity`] without
/// re-lexing.
#[derive(Clone, Debug, PartialEq)]
enum Token {
    Quantity(String),
    Ident(String),
    LParen,
    RParen,
    LBrack,
    RBrack,
    Comma,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Caret,
    EqEq,
    NotEq,
    Lt,
    Le,
    Gt,
    Ge,
    AndAnd,
    OrOr,
    Bang,
    Question,
    Colon,
    Eof,
}

/// A token paired with the byte offset of its first character in the
/// source string. The position is used solely for error messages.
#[derive(Clone, Debug)]
struct Spanned {
    tok: Token,
    pos: usize,
}

// =========================================================================
// Lexer
// =========================================================================

/// Scans the source string one token at a time on demand.
struct Lexer<'a> {
    src: &'a str,
    /// Byte offset of the next character to inspect.
    cursor: usize,
}

impl<'a> Lexer<'a> {
    fn new(src: &'a str) -> Self {
        Self { src, cursor: 0 }
    }

    /// Skip ASCII whitespace.
    fn skip_ws(&mut self) {
        while self.cursor < self.src.len() {
            let b = self.src.as_bytes()[self.cursor];
            if b == b' ' || b == b'\t' || b == b'\n' || b == b'\r' {
                self.cursor += 1;
            } else {
                break;
            }
        }
    }

    /// Look at the byte at the current cursor without consuming it.
    fn peek_byte(&self) -> Option<u8> {
        self.src.as_bytes().get(self.cursor).copied()
    }

    /// Look at the byte one past the current cursor.
    fn peek_byte_at(&self, offset: usize) -> Option<u8> {
        self.src.as_bytes().get(self.cursor + offset).copied()
    }

    /// Produce the next token, advancing the cursor.
    fn next_token(&mut self) -> Result<Spanned, ExprError> {
        self.skip_ws();
        let pos = self.cursor;

        let b = match self.peek_byte() {
            None => {
                return Ok(Spanned {
                    tok: Token::Eof,
                    pos,
                });
            }
            Some(b) => b,
        };

        // Quantity: [0-9.]+[a-zA-Z]*
        // We start the greedy run on a digit. A leading '.' (e.g.
        // ".5mm") is rejected — quantities are required to start with
        // a digit, matching the unit parser's accepted forms.
        if b.is_ascii_digit() {
            return Ok(Spanned {
                tok: self.lex_quantity(),
                pos,
            });
        }

        // Identifier: [a-zA-Z_][a-zA-Z0-9_]*
        if b.is_ascii_alphabetic() || b == b'_' {
            return Ok(Spanned {
                tok: self.lex_ident(),
                pos,
            });
        }

        // Punctuation and multi-char operators.
        let tok = match b {
            b'(' => {
                self.cursor += 1;
                Token::LParen
            }
            b')' => {
                self.cursor += 1;
                Token::RParen
            }
            b'[' => {
                self.cursor += 1;
                Token::LBrack
            }
            b']' => {
                self.cursor += 1;
                Token::RBrack
            }
            b',' => {
                self.cursor += 1;
                Token::Comma
            }
            b'+' => {
                self.cursor += 1;
                Token::Plus
            }
            b'-' => {
                self.cursor += 1;
                Token::Minus
            }
            b'*' => {
                self.cursor += 1;
                Token::Star
            }
            b'/' => {
                self.cursor += 1;
                Token::Slash
            }
            b'%' => {
                self.cursor += 1;
                Token::Percent
            }
            b'^' => {
                self.cursor += 1;
                Token::Caret
            }
            b'?' => {
                self.cursor += 1;
                Token::Question
            }
            b':' => {
                self.cursor += 1;
                Token::Colon
            }
            b'=' => {
                if self.peek_byte_at(1) == Some(b'=') {
                    self.cursor += 2;
                    Token::EqEq
                } else {
                    return Err(ExprError::Parse {
                        pos,
                        msg: "lone '=' is not a valid operator (use '==' for equality)".into(),
                    });
                }
            }
            b'!' => {
                if self.peek_byte_at(1) == Some(b'=') {
                    self.cursor += 2;
                    Token::NotEq
                } else {
                    self.cursor += 1;
                    Token::Bang
                }
            }
            b'<' => {
                if self.peek_byte_at(1) == Some(b'=') {
                    self.cursor += 2;
                    Token::Le
                } else {
                    self.cursor += 1;
                    Token::Lt
                }
            }
            b'>' => {
                if self.peek_byte_at(1) == Some(b'=') {
                    self.cursor += 2;
                    Token::Ge
                } else {
                    self.cursor += 1;
                    Token::Gt
                }
            }
            b'&' => {
                if self.peek_byte_at(1) == Some(b'&') {
                    self.cursor += 2;
                    Token::AndAnd
                } else {
                    return Err(ExprError::Parse {
                        pos,
                        msg: "lone '&' is not a valid operator (use '&&' for logical and)".into(),
                    });
                }
            }
            b'|' => {
                if self.peek_byte_at(1) == Some(b'|') {
                    self.cursor += 2;
                    Token::OrOr
                } else {
                    return Err(ExprError::Parse {
                        pos,
                        msg: "lone '|' is not a valid operator (use '||' for logical or)".into(),
                    });
                }
            }
            other => {
                return Err(ExprError::Parse {
                    pos,
                    msg: format!("unexpected character '{}'", other as char),
                });
            }
        };

        Ok(Spanned { tok, pos })
    }

    /// Eat a quantity literal: digits/dot followed by optional letter
    /// suffix. The cursor is positioned at a digit when this is called.
    fn lex_quantity(&mut self) -> Token {
        let start = self.cursor;
        // Digits / dots.
        while let Some(b) = self.peek_byte() {
            if b.is_ascii_digit() || b == b'.' {
                self.cursor += 1;
            } else {
                break;
            }
        }
        // Letters (unit suffix, no underscores or digits — those would
        // be a separate identifier or token).
        while let Some(b) = self.peek_byte() {
            if b.is_ascii_alphabetic() {
                self.cursor += 1;
            } else {
                break;
            }
        }
        let slice = &self.src[start..self.cursor];
        Token::Quantity(slice.to_string())
    }

    /// Eat an identifier: `[a-zA-Z_][a-zA-Z0-9_]*`. The cursor is on
    /// an alpha or underscore when this is called.
    fn lex_ident(&mut self) -> Token {
        let start = self.cursor;
        while let Some(b) = self.peek_byte() {
            if b.is_ascii_alphanumeric() || b == b'_' {
                self.cursor += 1;
            } else {
                break;
            }
        }
        let slice = &self.src[start..self.cursor];
        Token::Ident(slice.to_string())
    }
}

// =========================================================================
// Parser
// =========================================================================

/// Recursive-descent parser. Holds a single token of look-ahead so
/// each grammar method can decide its production by inspecting
/// `self.peek`.
struct Parser<'a> {
    lexer: Lexer<'a>,
    /// One-token look-ahead.
    peek: Spanned,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str) -> Result<Self, ExprError> {
        let mut lexer = Lexer::new(src);
        let peek = lexer.next_token()?;
        Ok(Self { lexer, peek })
    }

    /// Consume the current look-ahead and refill from the lexer.
    fn bump(&mut self) -> Result<Spanned, ExprError> {
        let next = self.lexer.next_token()?;
        Ok(std::mem::replace(&mut self.peek, next))
    }

    /// Consume the current token if it equals `expected`, otherwise
    /// produce a parse error pointing at the look-ahead's position.
    fn expect(&mut self, expected: &Token, what: &str) -> Result<(), ExprError> {
        if &self.peek.tok == expected {
            self.bump()?;
            Ok(())
        } else {
            Err(ExprError::Parse {
                pos: self.peek.pos,
                msg: format!("expected {what}, found {:?}", self.peek.tok),
            })
        }
    }

    // -- expr ----------------------------------------------------------

    fn parse_expr(&mut self) -> Result<ExprNode, ExprError> {
        self.parse_ternary()
    }

    // -- ternary -------------------------------------------------------

    fn parse_ternary(&mut self) -> Result<ExprNode, ExprError> {
        let cond = self.parse_or()?;
        if matches!(self.peek.tok, Token::Question) {
            self.bump()?; // '?'
            let then_branch = self.parse_expr()?;
            self.expect(&Token::Colon, "':' in ternary")?;
            let else_branch = self.parse_expr()?;
            Ok(ExprNode::Ternary(
                Box::new(cond),
                Box::new(then_branch),
                Box::new(else_branch),
            ))
        } else {
            Ok(cond)
        }
    }

    // -- or / and ------------------------------------------------------

    fn parse_or(&mut self) -> Result<ExprNode, ExprError> {
        let mut node = self.parse_and()?;
        while matches!(self.peek.tok, Token::OrOr) {
            self.bump()?;
            let rhs = self.parse_and()?;
            node = ExprNode::Binary(BinOp::Or, Box::new(node), Box::new(rhs));
        }
        Ok(node)
    }

    fn parse_and(&mut self) -> Result<ExprNode, ExprError> {
        let mut node = self.parse_equality()?;
        while matches!(self.peek.tok, Token::AndAnd) {
            self.bump()?;
            let rhs = self.parse_equality()?;
            node = ExprNode::Binary(BinOp::And, Box::new(node), Box::new(rhs));
        }
        Ok(node)
    }

    // -- equality / comparison ----------------------------------------

    fn parse_equality(&mut self) -> Result<ExprNode, ExprError> {
        let mut node = self.parse_comparison()?;
        loop {
            let op = match self.peek.tok {
                Token::EqEq => BinOp::Eq,
                Token::NotEq => BinOp::Ne,
                _ => break,
            };
            self.bump()?;
            let rhs = self.parse_comparison()?;
            node = ExprNode::Binary(op, Box::new(node), Box::new(rhs));
        }
        Ok(node)
    }

    fn parse_comparison(&mut self) -> Result<ExprNode, ExprError> {
        let mut node = self.parse_sum()?;
        loop {
            let op = match self.peek.tok {
                Token::Lt => BinOp::Lt,
                Token::Le => BinOp::Le,
                Token::Gt => BinOp::Gt,
                Token::Ge => BinOp::Ge,
                _ => break,
            };
            self.bump()?;
            let rhs = self.parse_sum()?;
            node = ExprNode::Binary(op, Box::new(node), Box::new(rhs));
        }
        Ok(node)
    }

    // -- sum / product / power ----------------------------------------

    fn parse_sum(&mut self) -> Result<ExprNode, ExprError> {
        let mut node = self.parse_product()?;
        loop {
            let op = match self.peek.tok {
                Token::Plus => BinOp::Add,
                Token::Minus => BinOp::Sub,
                _ => break,
            };
            self.bump()?;
            let rhs = self.parse_product()?;
            node = ExprNode::Binary(op, Box::new(node), Box::new(rhs));
        }
        Ok(node)
    }

    fn parse_product(&mut self) -> Result<ExprNode, ExprError> {
        let mut node = self.parse_power()?;
        loop {
            let op = match self.peek.tok {
                Token::Star => BinOp::Mul,
                Token::Slash => BinOp::Div,
                Token::Percent => BinOp::Mod,
                _ => break,
            };
            self.bump()?;
            let rhs = self.parse_power()?;
            node = ExprNode::Binary(op, Box::new(node), Box::new(rhs));
        }
        Ok(node)
    }

    /// Right-associative: `2^3^2` = `2^(3^2)` = 512.
    fn parse_power(&mut self) -> Result<ExprNode, ExprError> {
        let lhs = self.parse_unary()?;
        if matches!(self.peek.tok, Token::Caret) {
            self.bump()?;
            // Recurse into parse_power (not parse_unary) so the
            // right-hand side also folds further `^` operators
            // right-to-left.
            let rhs = self.parse_power()?;
            Ok(ExprNode::Binary(BinOp::Pow, Box::new(lhs), Box::new(rhs)))
        } else {
            Ok(lhs)
        }
    }

    // -- unary ---------------------------------------------------------

    fn parse_unary(&mut self) -> Result<ExprNode, ExprError> {
        match self.peek.tok {
            Token::Minus => {
                self.bump()?;
                let inner = self.parse_unary()?;
                Ok(ExprNode::Unary(UnaryOp::Neg, Box::new(inner)))
            }
            Token::Bang => {
                self.bump()?;
                let inner = self.parse_unary()?;
                Ok(ExprNode::Unary(UnaryOp::Not, Box::new(inner)))
            }
            _ => self.parse_primary(),
        }
    }

    // -- primary -------------------------------------------------------

    fn parse_primary(&mut self) -> Result<ExprNode, ExprError> {
        let pos = self.peek.pos;
        match self.peek.tok.clone() {
            Token::Quantity(text) => {
                self.bump()?;
                let q = parse_quantity(&text)?;
                Ok(ExprNode::Literal(q))
            }
            Token::Ident(name) => {
                self.bump()?;
                // `lookup(...)` is a reserved call form.
                if name == "lookup" && matches!(self.peek.tok, Token::LParen) {
                    return self.parse_lookup_call(pos);
                }
                // Bare `i` / `j` are array-index variables. Anything
                // longer (`ix`, `jj`, `ij`, ...) is a normal Ref.
                if name == "i" {
                    return Ok(ExprNode::ArrayIndex(ArrayIndex::I));
                }
                if name == "j" {
                    return Ok(ExprNode::ArrayIndex(ArrayIndex::J));
                }
                Ok(ExprNode::Ref(name))
            }
            Token::LParen => {
                self.bump()?;
                let inner = self.parse_expr()?;
                self.expect(&Token::RParen, "')' to close parenthesised expression")?;
                Ok(inner)
            }
            Token::Eof => Err(ExprError::Parse {
                pos,
                msg: "unexpected end of expression — expected a value".into(),
            }),
            other => Err(ExprError::Parse {
                pos,
                msg: format!("expected a value, found {other:?}"),
            }),
        }
    }

    /// `lookup` has already been consumed and `(` is the look-ahead.
    fn parse_lookup_call(&mut self, call_pos: usize) -> Result<ExprNode, ExprError> {
        self.expect(&Token::LParen, "'(' after lookup")?;
        let key = self.parse_expr()?;
        self.expect(&Token::Comma, "',' after lookup key")?;

        self.expect(&Token::LBrack, "'[' to open lookup keys list")?;
        let keys = self.parse_list()?;
        self.expect(&Token::RBrack, "']' to close lookup keys list")?;

        self.expect(&Token::Comma, "',' between lookup keys and values")?;

        self.expect(&Token::LBrack, "'[' to open lookup values list")?;
        let values = self.parse_list()?;
        self.expect(&Token::RBrack, "']' to close lookup values list")?;

        self.expect(&Token::RParen, "')' to close lookup call")?;

        if keys.len() != values.len() {
            return Err(ExprError::Parse {
                pos: call_pos,
                msg: "lookup keys/values length mismatch".into(),
            });
        }

        Ok(ExprNode::Lookup {
            key: Box::new(key),
            keys,
            values,
        })
    }

    /// `list ::= expr (',' expr)*` — parses until the next `]`.
    /// The list is at least one element; an empty `[]` is a parse
    /// error.
    fn parse_list(&mut self) -> Result<Vec<ExprNode>, ExprError> {
        let mut out = Vec::new();
        out.push(self.parse_expr()?);
        while matches!(self.peek.tok, Token::Comma) {
            self.bump()?;
            out.push(self.parse_expr()?);
        }
        Ok(out)
    }
}

// =========================================================================
// Public entry point
// =========================================================================

/// Parse a source string into an [`ExprNode`] tree.
///
/// On success the returned tree has been fully consumed — any trailing
/// junk produces [`ExprError::Parse`].
pub fn parse(src: &str) -> Result<ExprNode, ExprError> {
    let mut p = Parser::new(src)?;
    let node = p.parse_expr()?;
    if !matches!(p.peek.tok, Token::Eof) {
        return Err(ExprError::Parse {
            pos: p.peek.pos,
            msg: format!("unexpected trailing token {:?}", p.peek.tok),
        });
    }
    Ok(node)
}

// =========================================================================
// Smoke tests — keep these light; the heavy suite lives in
// `tests/expr_parser.rs`.
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke_literal() {
        let e = parse("0.5mm").unwrap();
        assert!(matches!(e, ExprNode::Literal(_)));
    }

    #[test]
    fn smoke_addition() {
        let e = parse("1 + 2").unwrap();
        match e {
            ExprNode::Binary(BinOp::Add, _, _) => {}
            other => panic!("expected Add, got {other:?}"),
        }
    }

    #[test]
    fn smoke_eof_after_expr() {
        // Trailing tokens beyond a valid expression must error.
        assert!(parse("1 + 2 3").is_err());
    }
}
