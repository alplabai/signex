/// VRML97 (ISO/IEC 14772-1:1997) source format tokens.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Word(String),
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
}

impl Token {
    /// The closing token matching an opener (`{` -> `}`, `[` -> `]`).
    /// Only meaningful for `LBrace` / `LBracket`.
    pub(crate) fn closer(&self) -> Token {
        if *self == Token::LBrace {
            Token::RBrace
        } else {
            Token::RBracket
        }
    }
}

/// Tokenize a VRML97 file into a flat token stream.
/// Comments (`# ... \n`) are stripped. Returns (tokens, line_offsets)
/// where `line_offsets[i]` is the 1-based source line for token `i`.
pub fn tokenize(source: &str) -> (Vec<Token>, Vec<usize>) {
    let mut tokens = Vec::new();
    let mut lines = Vec::new();
    let mut line = 1usize;
    let mut chars = source.char_indices().peekable();

    while let Some((_, ch)) = chars.peek().copied() {
        match ch {
            '\n' => {
                chars.next();
                line += 1;
            }
            '#' => {
                // strip comment to end of line
                for (_, c) in chars.by_ref() {
                    if c == '\n' {
                        line += 1;
                        break;
                    }
                }
            }
            // Every Unicode whitespace (form feed, vertical tab, NBSP, NEL, ...)
            // must be consumed here: the word arm below stops on whitespace
            // without advancing, which would otherwise spin forever (#689).
            _ if ch.is_whitespace() => {
                chars.next();
            }
            '{' => {
                chars.next();
                tokens.push(Token::LBrace);
                lines.push(line);
            }
            '}' => {
                chars.next();
                tokens.push(Token::RBrace);
                lines.push(line);
            }
            '[' => {
                chars.next();
                tokens.push(Token::LBracket);
                lines.push(line);
            }
            ']' => {
                chars.next();
                tokens.push(Token::RBracket);
                lines.push(line);
            }
            ',' => {
                chars.next();
                tokens.push(Token::Comma);
                lines.push(line);
            }
            _ => {
                // read a word token (anything non-whitespace, non-delimiter)
                let mut word = String::new();
                loop {
                    match chars.peek() {
                        Some((_, c))
                            if !c.is_whitespace()
                                && !matches!(*c, '{' | '}' | '[' | ']' | ',' | '#') =>
                        {
                            word.push(*c);
                            chars.next();
                        }
                        _ => break,
                    }
                }
                if word.is_empty() {
                    // Unreachable today (the arms above own every delimiter and
                    // whitespace); guarantees progress if an arm is ever removed.
                    chars.next();
                } else {
                    tokens.push(Token::Word(word));
                    lines.push(line);
                }
            }
        }
    }

    (tokens, lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_empty_is_empty() {
        let (tokens, _) = tokenize("");
        assert!(tokens.is_empty());
    }

    #[test]
    fn tokenize_strips_comment() {
        let (tokens, _) = tokenize("# this is a comment\nShape");
        assert_eq!(tokens, vec![Token::Word("Shape".into())]);
    }

    #[test]
    fn tokenize_braces_and_brackets() {
        let (tokens, _) = tokenize("{ [ ] }");
        assert_eq!(
            tokens,
            vec![
                Token::LBrace,
                Token::LBracket,
                Token::RBracket,
                Token::RBrace
            ]
        );
    }

    #[test]
    fn tokenize_words_across_lines() {
        let (tokens, lines) = tokenize("foo\nbar");
        assert_eq!(
            tokens,
            vec![Token::Word("foo".into()), Token::Word("bar".into())]
        );
        assert_eq!(lines, vec![1, 2]);
    }

    #[test]
    fn tokenize_treats_unicode_whitespace_as_separator() {
        use std::sync::mpsc;
        use std::time::Duration;
        for ws in ['\u{0c}', '\u{0b}', '\u{a0}', '\u{85}', '\u{2003}'] {
            let src = format!("Shape{ws}{{{ws}}}");
            let (tx, rx) = mpsc::channel();
            // Detached worker: a regression hangs the worker, not the test run.
            std::thread::spawn(move || {
                let _ = tx.send(tokenize(&src).0);
            });
            let tokens = rx
                .recv_timeout(Duration::from_secs(5))
                .unwrap_or_else(|_| panic!("tokenize hung on U+{:04X}", ws as u32));
            assert_eq!(
                tokens,
                vec![Token::Word("Shape".into()), Token::LBrace, Token::RBrace],
                "U+{:04X}",
                ws as u32
            );
        }
    }
}
