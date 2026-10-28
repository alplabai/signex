//! What the symbol editor currently shows: one unit (part) of one body.

use signex_types::schematic::{BodyStyle, item_shown};

/// The unit and body the editor draws, hit-tests and stamps new items with.
/// Items tagged part 0 or body 0 are common to every unit / both bodies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PartView {
    pub part: u8,
    pub body: BodyStyle,
}

impl PartView {
    pub fn new(part: u8, body: BodyStyle) -> Self {
        Self { part, body }
    }

    /// Unit `part` on the normal body.
    pub fn normal(part: u8) -> Self {
        Self::new(part, BodyStyle::Normal)
    }

    /// Is an item tagged `(part_number, body_style)` drawn and editable here?
    /// The same predicate the schematic uses, so the editor shows what a
    /// placed symbol will.
    pub fn shows(self, part_number: u8, body_style: u8) -> bool {
        item_shown(
            u32::from(part_number),
            u32::from(body_style),
            u32::from(self.part),
            self.body,
        )
    }

    /// The body-style tag new items are stamped with (1 normal, 2 alternate).
    pub fn body_tag(self) -> u8 {
        match self.body {
            BodyStyle::Normal => signex_library::BODY_NORMAL,
            BodyStyle::Alternate => signex_library::BODY_ALTERNATE,
        }
    }
}
