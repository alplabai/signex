//! The hover link between fields and callouts, as model state.
//!
//! The caller keeps one [`IllustrationHover`] in its panel state, routes
//! [`IllustrationMsg`] to [`IllustrationHover::update`], and reads it back in
//! `view`: the illustration highlights the parts linked to the hovered field,
//! and the panel highlights the field row a hovered callout reports.

use super::field::FieldId;
use super::illustration::Part;

/// A hover event on either side of the link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IllustrationMsg {
    /// The pointer moved onto a drawing part; carries the part's field.
    CalloutHovered(FieldId),
    /// The pointer left the drawing parts.
    CalloutLeft,
    /// The pointer moved onto a field row.
    FieldHovered(FieldId),
    /// The pointer left a field row.
    FieldLeft(FieldId),
}

/// Which side the current hover came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HoverSource {
    Field,
    Callout,
}

/// The field under the pointer, on the field side or the drawing side.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IllustrationHover {
    current: Option<(FieldId, HoverSource)>,
}

impl IllustrationHover {
    pub fn update(&mut self, message: IllustrationMsg) {
        match message {
            IllustrationMsg::CalloutHovered(field) => {
                self.current = Some((field, HoverSource::Callout));
            }
            IllustrationMsg::CalloutLeft => {
                if self.source() == Some(HoverSource::Callout) {
                    self.current = None;
                }
            }
            IllustrationMsg::FieldHovered(field) => {
                self.current = Some((field, HoverSource::Field));
            }
            // A late leave of row A must not clear a hover that already
            // moved on to row B.
            IllustrationMsg::FieldLeft(field) => {
                if self.current == Some((field, HoverSource::Field)) {
                    self.current = None;
                }
            }
        }
    }

    /// The hovered field, from either side.
    pub fn hovered(&self) -> Option<&FieldId> {
        self.current.as_ref().map(|(field, _)| field)
    }

    pub fn source(&self) -> Option<HoverSource> {
        self.current.as_ref().map(|(_, source)| *source)
    }

    /// The field a drawing part reported, if the hover came from the drawing.
    pub fn hovered_callout(&self) -> Option<&FieldId> {
        match &self.current {
            Some((field, HoverSource::Callout)) => Some(field),
            _ => None,
        }
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

/// The message a pointer move over the drawing should publish, given the
/// callout hover the model holds, the field now under the pointer, and
/// whether this drawing set that callout (`is_owner`). `None` when nothing
/// changed, so a still pointer publishes nothing.
///
/// Only the owner clears a callout: with two drawings on one hover, the
/// drawing away from the pointer would otherwise clear the other's callout
/// on every move, and the row it lights would flash.
pub fn hover_transition(
    current_callout: Option<&FieldId>,
    under_pointer: Option<FieldId>,
    is_owner: bool,
) -> Option<IllustrationMsg> {
    match (current_callout, under_pointer) {
        (Some(current), Some(hit)) if *current == hit => None,
        (_, Some(hit)) => Some(IllustrationMsg::CalloutHovered(hit)),
        (Some(_), None) if is_owner => Some(IllustrationMsg::CalloutLeft),
        (_, None) => None,
    }
}
