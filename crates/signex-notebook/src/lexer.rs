//! A walk over Typst source that knows just enough syntax to find the
//! embedded calls (`#name(args)[body].field`) and match their brackets:
//! markup, code groups, content blocks, strings, escapes, raw text, math and
//! comments. While it walks it writes the plain text of the markup (what a
//! reader sees, references as their names), so a block's title and snippet
//! come from the same walk that found it. The rules are listed in
//! `docs/NOTEBOOK_ANNOTATIONS.md`, "How a notebook is read".

use std::ops::Range;

use crate::reference::Reference;

/// One embedded call, in the order the calls start.
#[derive(Debug, Clone)]
pub(crate) struct Call {
    pub name: Range<usize>,
    /// The `#`.
    pub start: usize,
    /// One past the last byte of the call, its field chain included.
    pub end: usize,
    pub line: u32,
    /// Inside the first `( )`.
    pub args: Option<Range<usize>>,
    /// Inside the first `[ ]` after the arguments.
    pub body: Option<Range<usize>>,
    /// The plain text of the body, a range of [`Walk::text`].
    pub body_text: Option<Range<usize>>,
    /// The call whose arguments or body this one sits in.
    pub parent: Option<usize>,
    /// The walk reached the end of the call.
    pub complete: bool,
}

/// What was left open when the source ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Unclosed {
    /// The opening character: `[`, `(`, `{`, `"`, `` ` ``, `$` or `/` for a
    /// block comment.
    pub opener: char,
    pub line: u32,
}

/// The calls of a source, its plain text and what was left open, if
/// anything.
#[derive(Debug)]
pub(crate) struct Walk {
    pub calls: Vec<Call>,
    pub text: String,
    pub unclosed: Option<Unclosed>,
    /// The calls open when the source ended, outermost first.
    pub open_calls: Vec<usize>,
}

/// Walk `src` from byte `start`, which begins line `line`.
pub(crate) fn walk(src: &str, start: usize, line: u32) -> Walk {
    let mut lexer = Lexer {
        src,
        b: src.as_bytes(),
        pos: start,
        line,
        calls: Vec::new(),
        stack: Vec::new(),
        out: Vec::new(),
        quiet: 0,
    };
    let unclosed = lexer.markup(None).err();
    let open_calls = if unclosed.is_some() {
        lexer.stack.clone()
    } else {
        Vec::new()
    };
    Walk {
        calls: lexer.calls,
        text: String::from_utf8_lossy(&lexer.out).into_owned(),
        unclosed,
        open_calls,
    }
}

struct Lexer<'s> {
    src: &'s str,
    b: &'s [u8],
    pos: usize,
    line: u32,
    calls: Vec<Call>,
    /// The calls being walked, outermost first.
    stack: Vec<usize>,
    /// The plain text written so far.
    out: Vec<u8>,
    /// Above zero inside code (arguments, code blocks): nothing is written.
    quiet: u32,
}

type Step = Result<(), Unclosed>;

impl Lexer<'_> {
    fn peek(&self, ahead: usize) -> Option<u8> {
        self.b.get(self.pos + ahead).copied()
    }

    fn bump(&mut self) {
        match self.peek(0) {
            Some(b'\n') => self.line += 1,
            Some(_) => {}
            None => return,
        }
        self.pos += 1;
    }

    /// Step over one byte, writing it to the plain text.
    fn copy(&mut self) {
        if let Some(byte) = self.peek(0)
            && self.quiet == 0
        {
            self.out.push(byte);
        }
        self.bump();
    }

    /// Step over a whole character, writing it.
    fn copy_char(&mut self) {
        let len = self
            .src
            .get(self.pos..)
            .and_then(|rest| rest.chars().next())
            .map_or(1, char::len_utf8);
        for _ in 0..len {
            self.copy();
        }
    }

    fn write(&mut self, text: &str) {
        if self.quiet == 0 {
            self.out.extend_from_slice(text.as_bytes());
        }
    }

    fn unclosed(&self, opener: char, line: u32) -> Unclosed {
        Unclosed { opener, line }
    }

    /// Markup up to the end, or up to the `]` that closes a content block
    /// opened on line `close`.
    fn markup(&mut self, close: Option<u32>) -> Step {
        loop {
            let Some(c) = self.peek(0) else {
                return match close {
                    Some(line) => Err(self.unclosed('[', line)),
                    None => Ok(()),
                };
            };
            match c {
                b'\\' => {
                    self.bump();
                    self.copy_char();
                }
                b'/' if self.peek(1) == Some(b'/') && !self.after_scheme() => self.line_comment(),
                b'/' if self.peek(1) == Some(b'*') => self.block_comment()?,
                b'`' => self.raw()?,
                b'$' => self.math()?,
                b'#' => self.hash()?,
                // Strong emphasis markers are not text.
                b'*' => self.bump(),
                b'[' => {
                    let line = self.line;
                    self.copy();
                    self.markup(Some(line))?;
                    self.write("]");
                }
                b']' if close.is_some() => {
                    self.bump();
                    return Ok(());
                }
                _ => self.copy(),
            }
        }
    }

    /// `//` right after `http:` or `https:` is part of a link, not a comment.
    fn after_scheme(&self) -> bool {
        self.pos > 0 && self.b[self.pos - 1] == b':'
    }

    fn line_comment(&mut self) {
        while let Some(c) = self.peek(0) {
            if c == b'\n' {
                return;
            }
            self.bump();
        }
    }

    /// `/* ... */`, nested.
    fn block_comment(&mut self) -> Step {
        let line = self.line;
        self.bump();
        self.bump();
        let mut depth = 1;
        while depth > 0 {
            match (self.peek(0), self.peek(1)) {
                (None, _) => return Err(self.unclosed('/', line)),
                (Some(b'/'), Some(b'*')) => {
                    depth += 1;
                    self.bump();
                    self.bump();
                }
                (Some(b'*'), Some(b'/')) => {
                    depth -= 1;
                    self.bump();
                    self.bump();
                }
                _ => self.bump(),
            }
        }
        Ok(())
    }

    /// Raw text between one backtick, or between runs of three or more;
    /// its content is written as it is. Two backticks are an empty raw.
    fn raw(&mut self) -> Step {
        let line = self.line;
        let mut fence = 0;
        while self.peek(0) == Some(b'`') {
            fence += 1;
            self.bump();
        }
        if fence == 2 {
            return Ok(());
        }
        loop {
            match self.peek(0) {
                None => return Err(self.unclosed('`', line)),
                Some(b'`') => {
                    let run = self.b[self.pos..]
                        .iter()
                        .take_while(|&&b| b == b'`')
                        .count();
                    if run >= fence {
                        for _ in 0..fence {
                            self.bump();
                        }
                        return Ok(());
                    }
                    for _ in 0..run {
                        self.copy();
                    }
                }
                Some(_) => self.copy(),
            }
        }
    }

    /// An equation, `$` to `$`. Its text is written without the dollars;
    /// strings in it lose their quotes.
    fn math(&mut self) -> Step {
        let line = self.line;
        self.bump();
        loop {
            match self.peek(0) {
                None => return Err(self.unclosed('$', line)),
                Some(b'$') => {
                    self.bump();
                    return Ok(());
                }
                Some(b'\\') => {
                    self.bump();
                    self.copy_char();
                }
                Some(b'"') => self.string(true)?,
                Some(b'#') => self.hash()?,
                Some(b'/') if self.peek(1) == Some(b'/') => self.line_comment(),
                Some(b'/') if self.peek(1) == Some(b'*') => self.block_comment()?,
                Some(_) => self.copy(),
            }
        }
    }

    /// A string literal; its text is written when `written`.
    fn string(&mut self, written: bool) -> Step {
        let line = self.line;
        self.bump();
        loop {
            match self.peek(0) {
                None => return Err(self.unclosed('"', line)),
                Some(b'"') => {
                    self.bump();
                    return Ok(());
                }
                Some(b'\\') => {
                    self.bump();
                    if written {
                        self.copy_char();
                    } else {
                        self.bump();
                    }
                }
                Some(_) if written => self.copy(),
                Some(_) => self.bump(),
            }
        }
    }

    /// Code inside a group opened on line `line`, up to its `close`.
    fn code_group(&mut self, close: u8, line: u32) -> Step {
        let opener = if close == b')' { '(' } else { '{' };
        self.quiet += 1;
        let result = self.code_until(close, opener, line);
        self.quiet -= 1;
        result
    }

    fn code_until(&mut self, close: u8, opener: char, line: u32) -> Step {
        loop {
            let Some(c) = self.peek(0) else {
                return Err(self.unclosed(opener, line));
            };
            match c {
                _ if c == close => {
                    self.bump();
                    return Ok(());
                }
                b'"' => self.string(false)?,
                b'/' if self.peek(1) == Some(b'/') => self.line_comment(),
                b'/' if self.peek(1) == Some(b'*') => self.block_comment()?,
                b'`' => self.raw()?,
                b'$' => self.math()?,
                b'[' => {
                    let at = self.line;
                    self.bump();
                    self.markup(Some(at))?;
                }
                b'(' | b'{' => {
                    let at = self.line;
                    self.bump();
                    self.code_until(if c == b'(' { b')' } else { b'}' }, c as char, at)?;
                }
                // A closer that does not match leaves this group open.
                b')' | b'}' | b']' => return Err(self.unclosed(opener, line)),
                _ => self.bump(),
            }
        }
    }

    /// After `#`: an embedded call or field chain, a code group, or nothing.
    fn hash(&mut self) -> Step {
        let start = self.pos;
        let line = self.line;
        self.bump();
        match self.peek(0) {
            Some(c) if is_ident_start(c) => self.call(start, line),
            Some(b'(') => {
                self.bump();
                self.code_group(b')', line)
            }
            Some(b'{') => {
                self.bump();
                self.code_group(b'}', line)
            }
            Some(b'"') => {
                self.quiet += 1;
                let result = self.string(false);
                self.quiet -= 1;
                result
            }
            _ => Ok(()),
        }
    }

    fn ident(&mut self) -> Range<usize> {
        let start = self.pos;
        while self.peek(0).is_some_and(is_ident_continue) {
            self.bump();
        }
        start..self.pos
    }

    /// `name`, then any `( )`, `[ ]` and `.field` that follow without a gap.
    fn call(&mut self, start: usize, line: u32) -> Step {
        let name = self.ident();
        let index = self.calls.len();
        self.calls.push(Call {
            name,
            start,
            end: start,
            line,
            args: None,
            body: None,
            body_text: None,
            parent: self.stack.last().copied(),
            complete: false,
        });
        self.stack.push(index);
        loop {
            match self.peek(0) {
                Some(b'(') => {
                    let at = self.line;
                    self.bump();
                    let from = self.pos;
                    self.code_group(b')', at)?;
                    let args = from..self.pos - 1;
                    let call = &mut self.calls[index];
                    if call.args.is_none() {
                        call.args = Some(args);
                    }
                }
                Some(b'[') => {
                    let at = self.line;
                    self.bump();
                    let (from, text_from) = (self.pos, self.out.len());
                    self.markup(Some(at))?;
                    let (body, text) = (from..self.pos - 1, text_from..self.out.len());
                    let call = &mut self.calls[index];
                    if call.body.is_none() {
                        call.body = Some(body);
                        call.body_text = Some(text);
                    }
                }
                Some(b'.') if self.peek(1).is_some_and(is_ident_start) => {
                    self.bump();
                    self.ident();
                }
                _ => break,
            }
        }
        self.stack.pop();
        let call = &mut self.calls[index];
        call.end = self.pos;
        call.complete = true;
        let (name, args) = (call.name.clone(), call.args.clone());
        // A reference reads as its name.
        if let Some(args) = args
            && let Some(Ok(reference)) = Reference::from_call(&self.src[name], &self.src[args])
        {
            self.write(&reference.name());
        }
        Ok(())
    }
}

fn is_ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_'
}

fn is_ident_continue(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'-'
}
