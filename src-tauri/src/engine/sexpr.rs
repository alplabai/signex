/// S-expression tokenizer and tree parser for KiCad files.
/// KiCad uses a Lisp-like format: (keyword arg1 "string arg" (nested ...))

#[derive(Debug, Clone, PartialEq)]
pub enum SExpr {
    Atom(String),
    List(Vec<SExpr>),
}

impl SExpr {
    /// Get the keyword (first atom) of a list node
    pub fn keyword(&self) -> Option<&str> {
        match self {
            SExpr::List(items) if !items.is_empty() => {
                if let SExpr::Atom(s) = &items[0] {
                    Some(s)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Get children of a list (everything after the keyword)
    pub fn children(&self) -> &[SExpr] {
        match self {
            SExpr::List(items) if items.len() > 1 => &items[1..],
            _ => &[],
        }
    }

    /// Find the first child list with the given keyword
    pub fn find(&self, keyword: &str) -> Option<&SExpr> {
        self.children().iter().find(|c| c.keyword() == Some(keyword))
    }

    /// Find all child lists with the given keyword
    pub fn find_all(&self, keyword: &str) -> Vec<&SExpr> {
        self.children()
            .iter()
            .filter(|c| c.keyword() == Some(keyword))
            .collect()
    }

    /// Get the first atom argument (second item if first is keyword)
    pub fn first_arg(&self) -> Option<&str> {
        match self {
            SExpr::List(items) if items.len() > 1 => {
                if let SExpr::Atom(s) = &items[1] {
                    Some(s)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Get nth argument as string
    pub fn arg(&self, n: usize) -> Option<&str> {
        match self {
            SExpr::List(items) if items.len() > n + 1 => {
                if let SExpr::Atom(s) = &items[n + 1] {
                    Some(s)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Get nth argument as f64
    pub fn arg_f64(&self, n: usize) -> Option<f64> {
        self.arg(n).and_then(|s| s.parse::<f64>().ok())
    }

    /// Find a (property "Key" "Value" ...) by key name
    pub fn property(&self, key: &str) -> Option<&str> {
        for child in self.children() {
            if child.keyword() == Some("property") {
                if child.first_arg() == Some(key) {
                    return child.arg(1);
                }
            }
        }
        None
    }
}

/// Parse an S-expression string into a tree
pub fn parse(input: &str) -> Result<SExpr, String> {
    let tokens = tokenize(input)?;
    let (expr, _) = parse_tokens(&tokens, 0)?;
    Ok(expr)
}

/// Parse multiple top-level expressions (KiCad files have one root list)
fn parse_tokens(tokens: &[Token], pos: usize) -> Result<(SExpr, usize), String> {
    if pos >= tokens.len() {
        return Err("Unexpected end of input".to_string());
    }

    match &tokens[pos] {
        Token::Open => {
            let mut items = Vec::new();
            let mut i = pos + 1;
            loop {
                if i >= tokens.len() {
                    return Err("Unclosed parenthesis".to_string());
                }
                if tokens[i] == Token::Close {
                    return Ok((SExpr::List(items), i + 1));
                }
                let (expr, next) = parse_tokens(tokens, i)?;
                items.push(expr);
                i = next;
            }
        }
        Token::Close => Err("Unexpected ')'".to_string()),
        Token::Str(s) => Ok((SExpr::Atom(s.clone()), pos + 1)),
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Open,
    Close,
    Str(String),
}

fn tokenize(input: &str) -> Result<Vec<Token>, String> {
    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut tokens = Vec::with_capacity(len / 4);
    let mut i = 0;

    while i < len {
        match bytes[i] {
            b'(' => {
                tokens.push(Token::Open);
                i += 1;
            }
            b')' => {
                tokens.push(Token::Close);
                i += 1;
            }
            b'"' => {
                // Quoted string
                i += 1;
                let start = i;
                let mut s = String::new();
                while i < len && bytes[i] != b'"' {
                    if bytes[i] == b'\\' && i + 1 < len {
                        match bytes[i + 1] {
                            b'n' => s.push('\n'),
                            b'\\' => s.push('\\'),
                            b'"' => s.push('"'),
                            other => {
                                s.push('\\');
                                s.push(other as char);
                            }
                        }
                        i += 2;
                    } else {
                        s.push(bytes[i] as char);
                        i += 1;
                    }
                }
                if i >= len {
                    return Err(format!("Unclosed string starting at byte {}", start - 1));
                }
                i += 1; // skip closing "
                tokens.push(Token::Str(s));
            }
            b' ' | b'\t' | b'\n' | b'\r' => {
                i += 1;
            }
            _ => {
                // Unquoted atom (keyword, number, etc.)
                let start = i;
                while i < len && !matches!(bytes[i], b'(' | b')' | b' ' | b'\t' | b'\n' | b'\r') {
                    i += 1;
                }
                let s = String::from_utf8_lossy(&bytes[start..i]).to_string();
                tokens.push(Token::Str(s));
            }
        }
    }

    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_real_kicad_file() {
        let path = r"C:\Users\caner\Documents\GitHub\smart-home-hub-pcb\smart-home-hub.kicad_sch";
        if !std::path::Path::new(path).exists() {
            return; // skip if file not present
        }
        let content = std::fs::read_to_string(path).unwrap();
        let start = std::time::Instant::now();
        let expr = parse(&content).unwrap();
        let elapsed = start.elapsed();
        println!("Parsed {} bytes in {:?}", content.len(), elapsed);
        assert_eq!(expr.keyword(), Some("kicad_sch"));
    }

    #[test]
    fn test_simple() {
        let expr = parse("(hello world)").unwrap();
        assert_eq!(expr.keyword(), Some("hello"));
        assert_eq!(expr.first_arg(), Some("world"));
    }

    #[test]
    fn test_nested() {
        let expr = parse("(root (at 1.5 2.5) (name \"test\"))").unwrap();
        assert_eq!(expr.keyword(), Some("root"));
        let at = expr.find("at").unwrap();
        assert_eq!(at.arg_f64(0), Some(1.5));
        assert_eq!(at.arg_f64(1), Some(2.5));
        let name = expr.find("name").unwrap();
        assert_eq!(name.first_arg(), Some("test"));
    }

    #[test]
    fn test_property() {
        let expr = parse(r#"(symbol (property "Reference" "R1") (property "Value" "10k"))"#).unwrap();
        assert_eq!(expr.property("Reference"), Some("R1"));
        assert_eq!(expr.property("Value"), Some("10k"));
    }
}
