//! Aperture macro arithmetic: numbers, `$n` variables, `+ - x /` with the
//! usual precedence, unary signs and parentheses. Evaluated in `f64`; a
//! result becomes nm once, in `number::mm_f64_to_nm`.

use std::collections::BTreeMap;

use thiserror::Error;

/// The deepest parenthesis nesting read, so hostile input cannot overflow
/// the stack.
const MAX_DEPTH: usize = 64;
/// The longest expression read: a chain of operators nests one level per
/// operator when evaluated.
const MAX_LEN: usize = 1024;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub(crate) enum ExprError {
    #[error("the expression is empty")]
    Empty,
    #[error("unexpected text at {0}")]
    Unexpected(usize),
    #[error("the expression nests too deep")]
    TooDeep,
    #[error("the expression is too long")]
    TooLong,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Op {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Expr {
    Number(f64),
    Var(u32),
    Neg(Box<Expr>),
    Binary(Box<Expr>, Op, Box<Expr>),
}

impl Expr {
    /// The value with `vars`; an unset variable is 0, as the format says.
    pub(crate) fn eval(&self, vars: &BTreeMap<u32, f64>) -> f64 {
        match self {
            Expr::Number(v) => *v,
            Expr::Var(n) => vars.get(n).copied().unwrap_or(0.0),
            Expr::Neg(e) => -e.eval(vars),
            Expr::Binary(a, op, b) => {
                let (a, b) = (a.eval(vars), b.eval(vars));
                match op {
                    Op::Add => a + b,
                    Op::Sub => a - b,
                    Op::Mul => a * b,
                    Op::Div => a / b,
                }
            }
        }
    }
}

pub(crate) fn parse(text: &str) -> Result<Expr, ExprError> {
    let chars: Vec<char> = text.chars().filter(|c| !c.is_whitespace()).collect();
    if chars.is_empty() {
        return Err(ExprError::Empty);
    }
    if chars.len() > MAX_LEN {
        return Err(ExprError::TooLong);
    }
    let mut parser = Parser { chars, at: 0 };
    let expr = parser.sum(0)?;
    if parser.at < parser.chars.len() {
        return Err(ExprError::Unexpected(parser.at));
    }
    Ok(expr)
}

struct Parser {
    chars: Vec<char>,
    at: usize,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.at).copied()
    }

    fn sum(&mut self, depth: usize) -> Result<Expr, ExprError> {
        let mut left = self.product(depth)?;
        while let Some(op) = self.peek().and_then(|c| match c {
            '+' => Some(Op::Add),
            '-' => Some(Op::Sub),
            _ => None,
        }) {
            self.at += 1;
            let right = self.product(depth)?;
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    fn product(&mut self, depth: usize) -> Result<Expr, ExprError> {
        let mut left = self.factor(depth)?;
        while let Some(op) = self.peek().and_then(|c| match c {
            'x' | 'X' => Some(Op::Mul),
            '/' => Some(Op::Div),
            _ => None,
        }) {
            self.at += 1;
            let right = self.factor(depth)?;
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    fn factor(&mut self, depth: usize) -> Result<Expr, ExprError> {
        if depth > MAX_DEPTH {
            return Err(ExprError::TooDeep);
        }
        match self.peek() {
            Some('+') => {
                self.at += 1;
                self.factor(depth + 1)
            }
            Some('-') => {
                self.at += 1;
                Ok(Expr::Neg(Box::new(self.factor(depth + 1)?)))
            }
            Some('(') => {
                self.at += 1;
                let inner = self.sum(depth + 1)?;
                if self.peek() != Some(')') {
                    return Err(ExprError::Unexpected(self.at));
                }
                self.at += 1;
                Ok(inner)
            }
            Some('$') => {
                self.at += 1;
                let digits = self.take(|c| c.is_ascii_digit());
                digits
                    .parse()
                    .map(Expr::Var)
                    .map_err(|_| ExprError::Unexpected(self.at))
            }
            Some(c) if c.is_ascii_digit() || c == '.' => {
                let digits = self.take(|c| c.is_ascii_digit() || c == '.');
                digits
                    .parse()
                    .map(Expr::Number)
                    .map_err(|_| ExprError::Unexpected(self.at))
            }
            _ => Err(ExprError::Unexpected(self.at)),
        }
    }

    fn take(&mut self, keep: impl Fn(char) -> bool) -> String {
        let start = self.at;
        while self.peek().is_some_and(&keep) {
            self.at += 1;
        }
        self.chars[start..self.at].iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(text: &str, vars: &[(u32, f64)]) -> f64 {
        let vars: BTreeMap<u32, f64> = vars.iter().copied().collect();
        parse(text).expect("parses").eval(&vars)
    }

    #[test]
    fn precedence_signs_and_parentheses() {
        assert_eq!(value("1+2x3", &[]), 7.0);
        assert_eq!(value("(1+2)x3", &[]), 9.0);
        assert_eq!(value("10-4-3", &[]), 3.0);
        assert_eq!(value("8/2/2", &[]), 2.0);
        assert_eq!(value("-2x-3", &[]), 6.0);
        assert_eq!(value("+.5", &[]), 0.5);
        assert_eq!(value("2X3", &[]), 6.0);
    }

    #[test]
    fn variables_read_their_value_or_zero() {
        assert_eq!(value("$1x0.75", &[(1, 2.0)]), 1.5);
        assert_eq!(value("$2+$1", &[(1, 1.0)]), 1.0);
        assert_eq!(value("($1-$2)/2", &[(1, 2.3), (2, 1.5)]), (2.3 - 1.5) / 2.0);
    }

    #[test]
    fn bad_text_is_an_error_not_a_panic() {
        assert_eq!(parse(""), Err(ExprError::Empty));
        assert!(parse("1+").is_err());
        assert!(parse("(1").is_err());
        assert!(parse("1..2").is_err());
        assert!(parse("$").is_err());
        assert!(parse("2a").is_err());
        assert_eq!(parse(&"(".repeat(500)), Err(ExprError::TooDeep));
        assert_eq!(parse(&"1+".repeat(600)), Err(ExprError::TooLong));
    }
}
