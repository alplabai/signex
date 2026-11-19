//! Recursive-descent parser for the expression language.
//! Cleanroom — see the crate-level doc comment in `lib.rs`.
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
//! primary     ::= QUANTITY | 'pi' | IDENT | '(' expr ')' | lookup | call
//! lookup      ::= 'lookup' '(' expr ',' '[' list ']' ',' '[' list ']' ')'
//! call        ::= FUNC '(' list ')'        // min max abs sqrt round floor ceil
//! list        ::= expr (',' expr)*
//! ```
//!
//! # Notes
//!
//! - Lexing is in `lex.rs`.
//! - The reserved word `lookup` and the [`Func`] names become calls when
//!   followed by `(`; `pi` is the constant π; every other identifier is
//!   an [`ExprNode::Ref`] the evaluator resolves (a sketch array's `i`
//!   and `j` included).
//! - There are no string or boolean literals — comparison and
//!   logical operators yield dimensionless 0/1 at evaluation time.
//! - There are no comments inside expressions; the source is a
//!   single line.

use crate::ExprError;
use crate::ast::{BinOp, ExprNode, Func, UnaryOp};
use crate::lex::{Lexer, Spanned, Token};
use crate::symbols::LiteralContext;
use crate::unit::{Quantity, parse_quantity_with};
use crate::{MAX_EXPR_CHAIN, MAX_EXPR_DEPTH, MAX_EXPR_LEN};

/// The name of the constant π.
const PI_NAME: &str = "pi";

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
    /// Current recursion depth, bounded by [`MAX_EXPR_DEPTH`].
    depth: usize,
    /// Left-associative operators consumed so far; bounded by
    /// [`MAX_EXPR_CHAIN`] because each one adds a tree level.
    links: usize,
    /// How a literal's bare `m` reads.
    context: LiteralContext,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str, context: LiteralContext) -> Result<Self, ExprError> {
        let mut lexer = Lexer::new(src);
        let peek = lexer.next_token()?;
        Ok(Self {
            lexer,
            peek,
            depth: 0,
            links: 0,
            context,
        })
    }

    /// Run `body` one nesting level deeper, failing with
    /// [`ExprError::TooDeep`] instead of recursing past the cap.
    fn nested<T>(
        &mut self,
        body: impl FnOnce(&mut Self) -> Result<T, ExprError>,
    ) -> Result<T, ExprError> {
        if self.depth >= MAX_EXPR_DEPTH {
            return Err(ExprError::TooDeep {
                max: MAX_EXPR_DEPTH,
            });
        }
        self.depth += 1;
        let result = body(self);
        self.depth -= 1;
        result
    }

    /// Consume a left-associative binary operator, failing with
    /// [`ExprError::ChainTooLong`] once the expression holds more than
    /// [`MAX_EXPR_CHAIN`] of them. A flat `1+1+1+...` builds a left-nested
    /// tree in a loop, so only this count keeps the evaluator and the
    /// recursive `Drop` of the AST bounded.
    fn chain_link(&mut self) -> Result<(), ExprError> {
        if self.links >= MAX_EXPR_CHAIN {
            return Err(ExprError::ChainTooLong {
                max: MAX_EXPR_CHAIN,
            });
        }
        self.links += 1;
        self.bump()?;
        Ok(())
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
        self.nested(Self::parse_ternary)
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
            self.chain_link()?;
            let rhs = self.parse_and()?;
            node = ExprNode::Binary(BinOp::Or, Box::new(node), Box::new(rhs));
        }
        Ok(node)
    }

    fn parse_and(&mut self) -> Result<ExprNode, ExprError> {
        let mut node = self.parse_equality()?;
        while matches!(self.peek.tok, Token::AndAnd) {
            self.chain_link()?;
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
            self.chain_link()?;
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
            self.chain_link()?;
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
            self.chain_link()?;
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
            self.chain_link()?;
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
            // right-to-left. The recursion goes through `nested` so a
            // `1^1^1^...` chain hits the depth cap instead of the stack.
            let rhs = self.nested(Self::parse_power)?;
            Ok(ExprNode::Binary(BinOp::Pow, Box::new(lhs), Box::new(rhs)))
        } else {
            Ok(lhs)
        }
    }

    // -- unary ---------------------------------------------------------

    fn parse_unary(&mut self) -> Result<ExprNode, ExprError> {
        self.nested(Self::parse_unary_inner)
    }

    fn parse_unary_inner(&mut self) -> Result<ExprNode, ExprError> {
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
                let q = parse_quantity_with(&text, self.context)?;
                Ok(ExprNode::Literal(q))
            }
            Token::Ident(name) => {
                self.bump()?;
                if matches!(self.peek.tok, Token::LParen) {
                    // `lookup(...)` is a reserved call form.
                    if name == "lookup" {
                        return self.parse_lookup_call(pos);
                    }
                    return match Func::from_name(&name) {
                        Some(func) => self.parse_call(func, pos),
                        None => Err(ExprError::Parse {
                            pos,
                            msg: format!("unknown function '{name}'"),
                        }),
                    };
                }
                if name == PI_NAME {
                    return Ok(ExprNode::Literal(Quantity::count(std::f64::consts::PI)));
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

    /// The function name has been consumed and `(` is the look-ahead.
    fn parse_call(&mut self, func: Func, call_pos: usize) -> Result<ExprNode, ExprError> {
        self.expect(&Token::LParen, "'(' after function name")?;
        let args = if matches!(self.peek.tok, Token::RParen) {
            Vec::new()
        } else {
            self.parse_list()?
        };
        self.expect(&Token::RParen, "')' to close function call")?;
        if !func.accepts(args.len()) {
            return Err(ExprError::Parse {
                pos: call_pos,
                msg: format!("{}() does not take {} arguments", func.name(), args.len()),
            });
        }
        Ok(ExprNode::Call(func, args))
    }

    /// `list ::= expr (',' expr)*` — parses until the next `]` or `)`.
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

/// Parse a source string into an [`ExprNode`] tree, reading literals in
/// the [`LiteralContext::General`] context.
///
/// On success the returned tree has been fully consumed — any trailing
/// junk produces [`ExprError::Parse`].
pub fn parse(src: &str) -> Result<ExprNode, ExprError> {
    parse_with(src, LiteralContext::General)
}

/// [`parse`] with an explicit literal context. Use
/// [`LiteralContext::Length`] where the whole value must be a length, so a
/// bare `10m` reads as metres (see [`crate::symbols::bare_m_meaning`]).
pub fn parse_with(src: &str, context: LiteralContext) -> Result<ExprNode, ExprError> {
    // A long flat chain (`1+1+1+...`) builds an equally deep left-nested
    // tree that the evaluator and `Drop` walk recursively, so bound the
    // source length as well as the parser's own recursion.
    if src.len() > MAX_EXPR_LEN {
        return Err(ExprError::TooLong {
            max: MAX_EXPR_LEN,
            len: src.len(),
        });
    }
    let mut p = Parser::new(src, context)?;
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
// `tests/integration/expr_parser.rs`.
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
