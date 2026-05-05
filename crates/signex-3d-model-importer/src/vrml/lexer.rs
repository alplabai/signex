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
            ' ' | '\t' | '\r' => {
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
                if !word.is_empty() {
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
            vec![Token::LBrace, Token::LBracket, Token::RBracket, Token::RBrace]
        );
    }

    #[test]
    fn tokenize_words_across_lines() {
        let (tokens, lines) = tokenize("foo\nbar");
        assert_eq!(tokens, vec![Token::Word("foo".into()), Token::Word("bar".into())]);
        assert_eq!(lines, vec![1, 2]);
    }
}
