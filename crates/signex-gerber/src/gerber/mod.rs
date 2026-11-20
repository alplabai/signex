//! Gerber RS-274X with X2 attributes, per the public Gerber format
//! specification: the file read into an [`Image`] of objects in integer
//! nm. Every command not understood, and every object that cannot be
//! built, becomes a `ReadProblem` with its line; what can be drawn still
//! is. The supported and unsupported commands: docs/GERBER_VIEWER.md.

mod apertures;
mod arcs;
mod attributes;
mod commands;
mod expr;
mod extended;
mod lexer;
mod macros;
mod state;

#[cfg(test)]
mod tests;

pub(crate) use attributes::{file_attribute, split_attribute};

use crate::image::Image;
use lexer::Block;
use state::Reader;

/// Read the Gerber file `file` from its text.
pub fn read_gerber(file: &str, text: &str) -> Image {
    let mut reader = Reader::new(file, text);
    for block in lexer::blocks(text) {
        match block {
            Block::Word { text, line } => reader.word(&text, line),
            Block::Extended {
                statements,
                line,
                closed,
            } => reader.extended(&statements, line, closed),
        }
    }
    reader.finish()
}
