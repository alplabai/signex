//! A Gerber file as blocks: word commands ending in `*`, and extended
//! commands between `%` signs, each holding `*`-ended statements. Line
//! breaks are dropped inside a statement; every block keeps the line it
//! starts on.

/// One block of the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Block {
    /// A word command without its `*`: `X100Y200D01`, `G04 note`.
    Word { text: String, line: u32 },
    /// An extended command: its statements without their `*`, each with
    /// its line. `closed` is false when the file ends inside it.
    Extended {
        statements: Vec<(String, u32)>,
        line: u32,
        closed: bool,
    },
}

#[derive(Default)]
struct Lexer {
    blocks: Vec<Block>,
    line: u32,
    current: String,
    current_line: u32,
    extended: Option<(Vec<(String, u32)>, u32)>,
}

impl Lexer {
    /// End the statement being read: a word, or a statement of the open
    /// extended command. Blank text ends nothing.
    fn end_statement(&mut self) {
        let text = self.current.trim().to_string();
        self.current.clear();
        if text.is_empty() {
            return;
        }
        let line = self.current_line;
        match &mut self.extended {
            Some((statements, _)) => statements.push((text, line)),
            None => self.blocks.push(Block::Word { text, line }),
        }
    }

    fn percent(&mut self) {
        self.end_statement();
        match self.extended.take() {
            Some((statements, line)) => self.blocks.push(Block::Extended {
                statements,
                line,
                closed: true,
            }),
            None => self.extended = Some((Vec::new(), self.line)),
        }
    }

    fn push(&mut self, c: char) {
        if self.current.trim().is_empty() {
            if c.is_whitespace() {
                return;
            }
            self.current.clear();
            self.current_line = self.line;
        }
        self.current.push(c);
    }
}

pub(crate) fn blocks(text: &str) -> Vec<Block> {
    let mut lexer = Lexer {
        line: 1,
        ..Lexer::default()
    };
    for c in text.chars() {
        match c {
            '\n' => lexer.line += 1,
            '\r' => {}
            '%' => lexer.percent(),
            '*' => lexer.end_statement(),
            _ => lexer.push(c),
        }
    }
    lexer.end_statement();
    if let Some((statements, line)) = lexer.extended.take() {
        lexer.blocks.push(Block::Extended {
            statements,
            line,
            closed: false,
        });
    }
    lexer.blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(text: &str, line: u32) -> Block {
        Block::Word {
            text: text.into(),
            line,
        }
    }

    #[test]
    fn words_and_extended_commands_keep_their_lines() {
        let text =
            "%FSLAX46Y46*%\n%MOMM*%\nG04 a note*\nX1Y2D0\n1*\n%AMBOX*\n21,1,1,1,\n0,0,0*\n%\nM02*";
        let got = blocks(text);
        assert_eq!(
            got,
            vec![
                Block::Extended {
                    statements: vec![("FSLAX46Y46".into(), 1)],
                    line: 1,
                    closed: true
                },
                Block::Extended {
                    statements: vec![("MOMM".into(), 2)],
                    line: 2,
                    closed: true
                },
                word("G04 a note", 3),
                word("X1Y2D01", 4),
                Block::Extended {
                    statements: vec![("AMBOX".into(), 6), ("21,1,1,1,0,0,0".into(), 7)],
                    line: 6,
                    closed: true
                },
                word("M02", 10),
            ]
        );
    }

    #[test]
    fn a_file_that_ends_inside_a_command_says_so() {
        let got = blocks("%MOMM*%\n%ADD10C,1*\n");
        assert_eq!(
            got[1],
            Block::Extended {
                statements: vec![("ADD10C,1".into(), 2)],
                line: 2,
                closed: false
            }
        );
        assert_eq!(blocks("X1Y1D03"), vec![word("X1Y1D03", 1)]);
    }
}
