//! Lexer for the expression language. Cleanroom — see the crate-level doc
//! comment in `lib.rs`.
//!
//! - Whitespace between tokens is silently skipped.
//! - A `QUANTITY` lexes greedily as `[0-9.]+` followed by unit letters so
//!   suffixes like `mm`, `mil`, `deg`, `nF`, `kΩ` ride along with the
//!   digit run; the resulting slice is fed to [`crate::unit::parse_quantity`].
//!   One SI prefix letter between two digit runs is RKM notation and
//!   belongs to the literal too (`4k7`, `2n2F`).
//! - Identifiers are `[a-zA-Z_][a-zA-Z0-9_]*`, optionally joined by dots
//!   into a path (`R1.Value`).

use crate::ExprError;
use crate::symbols::{is_unit_char, prefix_factor};

/// One lexical token.
///
/// `Quantity` carries the raw source slice (digits + unit suffix) so
/// the parser can hand it to [`crate::unit::parse_quantity`] without
/// re-lexing.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Token {
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
pub(crate) struct Spanned {
    pub(crate) tok: Token,
    pub(crate) pos: usize,
}

/// Scans the source string one token at a time on demand.
pub(crate) struct Lexer<'a> {
    src: &'a str,
    /// Byte offset of the next character to inspect. Always on a char
    /// boundary: ASCII advances one byte, unit characters their UTF-8 width.
    cursor: usize,
}

impl<'a> Lexer<'a> {
    pub(crate) fn new(src: &'a str) -> Self {
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

    /// Look at the byte `offset` past the current cursor.
    fn peek_byte_at(&self, offset: usize) -> Option<u8> {
        self.src.as_bytes().get(self.cursor + offset).copied()
    }

    /// Look at the character at the current cursor.
    fn peek_char(&self) -> Option<char> {
        self.src[self.cursor..].chars().next()
    }

    /// Consume characters while `pred` holds.
    fn eat_while(&mut self, pred: impl Fn(char) -> bool) {
        while let Some(c) = self.peek_char() {
            if !pred(c) {
                break;
            }
            self.cursor += c.len_utf8();
        }
    }

    /// Produce the next token, advancing the cursor.
    pub(crate) fn next_token(&mut self) -> Result<Spanned, ExprError> {
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

        // Quantity. We start the greedy run on a digit. A leading '.'
        // (e.g. ".5mm") is rejected — quantities are required to start
        // with a digit, matching the unit parser's accepted forms.
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
            b'(' => self.single(Token::LParen),
            b')' => self.single(Token::RParen),
            b'[' => self.single(Token::LBrack),
            b']' => self.single(Token::RBrack),
            b',' => self.single(Token::Comma),
            b'+' => self.single(Token::Plus),
            b'-' => self.single(Token::Minus),
            b'*' => self.single(Token::Star),
            b'/' => self.single(Token::Slash),
            b'%' => self.single(Token::Percent),
            b'^' => self.single(Token::Caret),
            b'?' => self.single(Token::Question),
            b':' => self.single(Token::Colon),
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
            b'!' => self.maybe_eq(Token::NotEq, Token::Bang),
            b'<' => self.maybe_eq(Token::Le, Token::Lt),
            b'>' => self.maybe_eq(Token::Ge, Token::Gt),
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
            _ => {
                let shown = self.peek_char().unwrap_or(char::REPLACEMENT_CHARACTER);
                return Err(ExprError::Parse {
                    pos,
                    msg: format!("unexpected character '{shown}'"),
                });
            }
        };

        Ok(Spanned { tok, pos })
    }

    /// Consume one ASCII byte and return `tok`.
    fn single(&mut self, tok: Token) -> Token {
        self.cursor += 1;
        tok
    }

    /// `with_eq` when the next byte is `=` (both consumed), else `alone`.
    fn maybe_eq(&mut self, with_eq: Token, alone: Token) -> Token {
        if self.peek_byte_at(1) == Some(b'=') {
            self.cursor += 2;
            with_eq
        } else {
            self.cursor += 1;
            alone
        }
    }

    /// Eat a quantity literal: digits/dot followed by an optional unit
    /// suffix. The cursor is positioned at a digit when this is called.
    fn lex_quantity(&mut self) -> Token {
        let start = self.cursor;
        self.eat_while(|c| c.is_ascii_digit() || c == '.');
        // Unit letters (no underscores or digits — those would be a
        // separate identifier or token).
        let suffix_start = self.cursor;
        self.eat_while(is_unit_char);
        // RKM: a lone prefix letter followed by digits is the decimal point.
        let marker = &self.src[suffix_start..self.cursor];
        let is_rkm = marker.chars().count() == 1
            && prefix_factor(marker).is_some()
            && self.peek_char().is_some_and(|c| c.is_ascii_digit());
        if is_rkm {
            self.eat_while(|c| c.is_ascii_digit());
            self.eat_while(is_unit_char);
        }
        Token::Quantity(self.src[start..self.cursor].to_string())
    }

    /// Eat an identifier: `[a-zA-Z_][a-zA-Z0-9_]*`, then any `.name`
    /// segments of a path. The cursor is on an alpha or underscore when
    /// this is called.
    fn lex_ident(&mut self) -> Token {
        let start = self.cursor;
        let ident_char = |c: char| c.is_ascii_alphanumeric() || c == '_';
        self.eat_while(ident_char);
        while self.peek_byte() == Some(b'.')
            && self
                .peek_byte_at(1)
                .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
        {
            self.cursor += 1;
            self.eat_while(ident_char);
        }
        Token::Ident(self.src[start..self.cursor].to_string())
    }
}
