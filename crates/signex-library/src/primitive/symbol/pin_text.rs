//! Pin text the `.snxsym` pin table cannot store (#3129), for an editor to
//! refuse when the text is entered rather than at save.

use super::{Symbol, SymbolPin};

/// A control character in one of a pin's text cells.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnstorablePinText {
    pub pin_index: usize,
    /// The pin table column, as the file names it.
    pub column: &'static str,
    pub character: char,
}

impl Symbol {
    /// The first pin text a save would refuse for a control character: the
    /// same rule the writer applies to every cell
    /// ([`crate::toml_literal::first_unstorable_in_cell`]).
    pub fn first_unstorable_pin_text(&self) -> Option<UnstorablePinText> {
        self.pins.iter().enumerate().find_map(|(pin_index, pin)| {
            text_cells(pin).find_map(|(column, text)| {
                crate::toml_literal::first_unstorable_in_cell(text).map(|character| {
                    UnstorablePinText {
                        pin_index,
                        column,
                        character,
                    }
                })
            })
        })
    }
}

/// The cells of a pin that hold free text; the others are numbers, flags
/// and tokens the writer formats itself.
fn text_cells(pin: &SymbolPin) -> impl Iterator<Item = (&'static str, &str)> {
    [
        ("number", pin.number.as_str()),
        ("name", pin.name.as_str()),
        ("description", pin.description.as_str()),
    ]
    .into_iter()
    .chain(pin.function.iter().map(|f| ("function", f.as_str())))
}
