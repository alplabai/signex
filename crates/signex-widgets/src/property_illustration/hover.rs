//! The hover link from a field row to its drawing parts, as model state.
//!
//! The caller keeps one [`IllustrationHover`] in its panel state, routes
//! [`IllustrationMsg`] to [`IllustrationHover::update`], and reads it back in
//! `view`: the illustration highlights the parts linked to the hovered field.
//! The drawing itself does not follow the pointer.

use super::field::FieldId;
use super::illustration::Part;

/// A hover event on a field row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IllustrationMsg {
    /// The pointer moved onto a field row.
    FieldHovered(FieldId),
    /// The pointer left a field row.
    FieldLeft(FieldId),
}

/// The field row under the pointer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IllustrationHover {
    current: Option<FieldId>,
}

impl IllustrationHover {
    pub fn update(&mut self, message: IllustrationMsg) {
        match message {
            IllustrationMsg::FieldHovered(field) => self.current = Some(field),
            // A late leave of row A must not clear a hover that already
            // moved on to row B.
            IllustrationMsg::FieldLeft(field) => {
                if self.current.as_ref() == Some(&field) {
                    self.current = None;
                }
            }
        }
    }

    /// The hovered field.
    pub fn hovered(&self) -> Option<&FieldId> {
        self.current.as_ref()
    }

    /// Whether the panel should highlight the row of `field`.
    pub fn is_field_highlighted(&self, field: &FieldId) -> bool {
        self.hovered() == Some(field)
    }

    /// Whether the illustration should highlight `part`.
    pub fn is_part_highlighted(&self, part: &Part) -> bool {
        self.hovered().is_some_and(|field| part.is_linked_to(field))
    }
}
