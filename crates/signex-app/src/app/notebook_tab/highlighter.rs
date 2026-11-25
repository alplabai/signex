//! The editor's highlighter over `signex_notebook::Highlights`: the spans
//! of the whole text come from one parse in `update`, and the editor reads
//! them line by line, so a raw block or an equation over several lines is
//! coloured on each.

use std::ops::Range;
use std::sync::Arc;

use iced::advanced::text as core_text;
use signex_notebook::{Highlights, Token};

/// The parser the editor runs: the spans of line `line`, by the line
/// counter the editor drives.
pub(super) struct NotebookHighlighter {
    line: usize,
    highlights: Arc<Highlights>,
}

impl core_text::Parser for NotebookHighlighter {
    type Settings = Arc<Highlights>;
    type Output = Token;
    type Iterator<'a> = std::vec::IntoIter<(Range<usize>, Token)>;

    fn new(settings: &Self::Settings) -> Self {
        NotebookHighlighter {
            line: 0,
            highlights: settings.clone(),
        }
    }

    fn update(&mut self, new_settings: &Self::Settings) {
        self.highlights = new_settings.clone();
        self.line = 0;
    }

    fn change_line(&mut self, line: usize) {
        self.line = self.line.min(line);
    }

    fn parse_line(&mut self, _line: &str) -> Self::Iterator<'_> {
        let at = self.line;
        self.line += 1;
        self.highlights.line(at).to_vec().into_iter()
    }

    fn current_line(&self) -> usize {
        self.line
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::advanced::text::Parser as _;

    #[test]
    fn lines_are_read_in_order_and_a_changed_line_rewinds() {
        let highlights = Arc::new(Highlights::of("= A\nplain\n$ x $\n"));
        let mut parser = NotebookHighlighter::new(&highlights);
        let first: Vec<Token> = parser.parse_line("= A").map(|(_, t)| t).collect();
        assert_eq!(first, [Token::Heading]);
        assert!(parser.parse_line("plain").next().is_none());
        let third: Vec<Token> = parser.parse_line("$ x $").map(|(_, t)| t).collect();
        assert_eq!(third, [Token::Math]);
        assert_eq!(parser.current_line(), 3);
        parser.change_line(1);
        assert_eq!(parser.current_line(), 1);
        parser.update(&Arc::new(Highlights::of("plain\n")));
        assert_eq!(parser.current_line(), 0);
    }
}
