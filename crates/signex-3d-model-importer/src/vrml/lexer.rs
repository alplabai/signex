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
/// Comments (`# ... \n`) are stripped. A quoted `SFString` / `MFString` value
/// is one `Word` that keeps its quotes and escapes verbatim (`"Part #1"`), so
/// brackets and `#` inside it are inert and a string never reads as a keyword
/// such as `USE` or `Shape` (#781). Returns (tokens, line_offsets)
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
            '"' => {
                // A string may span lines; it is attributed to its opening line.
                let start = line;
                let mut word = String::new();
                word.push(ch);
                chars.next();
                let mut escaped = false;
                for (_, c) in chars.by_ref() {
                    word.push(c);
                    if c == '\n' {
                        line += 1;
                    }
                    if escaped {
                        escaped = false;
                    } else if c == '\\' {
                        escaped = true;
                    } else if c == '"' {
                        break;
                    }
                }
                // An unterminated string runs to end of input; the parser then
                // reports the missing closers as it would for any truncated file.
                tokens.push(Token::Word(word));
                lines.push(start);
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

    /// #781: `#` inside a string is not a comment start.
    #[test]
    fn tokenize_hash_inside_string_is_inert() {
        let (tokens, _) = tokenize(r#"WorldInfo { title "Part #1" }"#);
        assert_eq!(
            tokens,
            vec![
                Token::Word("WorldInfo".into()),
                Token::LBrace,
                Token::Word("title".into()),
                Token::Word(r#""Part #1""#.into()),
                Token::RBrace,
            ]
        );
    }

    /// #781: a bracket inside a string does not open a list.
    #[test]
    fn tokenize_bracket_inside_string_is_inert() {
        let (tokens, _) = tokenize(r#"WorldInfo { info [ "a [v1" ] }"#);
        assert_eq!(
            tokens,
            vec![
                Token::Word("WorldInfo".into()),
                Token::LBrace,
                Token::Word("info".into()),
                Token::LBracket,
                Token::Word(r#""a [v1""#.into()),
                Token::RBracket,
                Token::RBrace,
            ]
        );
    }

    /// `\"` and `\\` escapes do not end the string; adjacent strings split.
    #[test]
    fn tokenize_string_escapes_and_adjacent_strings() {
        let (tokens, _) = tokenize(r#"[ "say \"}\"" "c:\\" "x" ]"#);
        assert_eq!(
            tokens,
            vec![
                Token::LBracket,
                Token::Word(r#""say \"}\"""#.into()),
                Token::Word(r#""c:\\""#.into()),
                Token::Word(r#""x""#.into()),
                Token::RBracket,
            ]
        );
    }

    /// A multi-line string keeps its opening line and still advances the count.
    #[test]
    fn tokenize_multiline_string_tracks_lines() {
        let (tokens, lines) = tokenize("\"a\nb\" c");
        assert_eq!(
            tokens,
            vec![Token::Word("\"a\nb\"".into()), Token::Word("c".into())]
        );
        assert_eq!(lines, vec![1, 2]);
    }

    /// An unterminated string swallows the rest of the input without panicking.
    #[test]
    fn tokenize_unterminated_string_runs_to_eof() {
        let (tokens, _) = tokenize("{ \"open } ]");
        assert_eq!(
            tokens,
            vec![Token::LBrace, Token::Word("\"open } ]".into())]
        );
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
