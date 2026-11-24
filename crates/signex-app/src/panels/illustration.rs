//! The Properties side of the property illustration (epic #2523, BB2): the
//! widget is [`signex_widgets::property_illustration`]; this module draws it
//! with the panel's tokens and hover, wraps field rows so a hover on a row
//! lights the callouts linked to it, and keeps the hover as panel state.
//!
//! The hover belongs to one subject: the selected object or the open Tab
//! form. A hover left over from another subject reads as no hover and is
//! dropped by the next hover message.
#![allow(
    dead_code,
    reason = "a building block of epic #2523; the Properties panels adopt it in later units"
)]

use iced::widget::{container, mouse_area};
use iced::{Background, Element, Length, Theme, Widget};
use signex_types::schematic::SelectedKind;
use signex_widgets::property_illustration::{
    self as pi, DEFAULT_HEIGHT_EM, FieldId, Illustration, IllustrationHover, IllustrationMsg, Paint,
};

use super::{PanelContext, PanelMsg, PrePlacementKind};
use crate::styles::{Spacing, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// What a hover belongs to: the one selected object, or the Tab form of a
/// placement tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HoverSubject {
    selected: Option<(uuid::Uuid, SelectedKind)>,
    pre_placement: Option<PrePlacementKind>,
}

impl HoverSubject {
    /// The subject the panel shows now. The Tab form wins over a selection,
    /// as it does in the view.
    pub fn of(ctx: &PanelContext) -> Self {
        if let Some(pp) = ctx.pre_placement.as_ref() {
            return Self {
                selected: None,
                pre_placement: Some(pp.kind),
            };
        }
        let selected = match (ctx.selection_count, ctx.selected_uuid, ctx.selected_kind) {
            (1, Some(uuid), Some(kind)) => Some((uuid, kind)),
            _ => None,
        };
        Self {
            selected,
            pre_placement: None,
        }
    }
}

/// The illustration hover of the Properties panel, tied to its subject.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PanelIllustrationHover {
    subject: HoverSubject,
    hover: IllustrationHover,
}

impl PanelIllustrationHover {
    /// Applies `message` for `subject`; a hover of another subject is
    /// dropped first, so a selection change never inherits a stale hover.
    pub fn update(&mut self, subject: HoverSubject, message: IllustrationMsg) {
        if self.subject != subject {
            self.subject = subject;
            self.hover = IllustrationHover::default();
        }
        self.hover.update(message);
    }

    /// The hover, when it belongs to `subject`.
    pub fn for_subject(&self, subject: HoverSubject) -> Option<&IllustrationHover> {
        (self.subject == subject).then_some(&self.hover)
    }
}

/// The illustration hover the panel shows now: empty when the stored hover
/// belongs to another subject.
pub(crate) fn current_hover(ctx: &PanelContext) -> IllustrationHover {
    ctx.illustration_hover
        .for_subject(HoverSubject::of(ctx))
        .cloned()
        .unwrap_or_default()
}

/// The ink palette of the panel's theme.
pub(crate) fn palette(ctx: &PanelContext) -> pi::Palette {
    pi::Palette::from_tokens(&ctx.tokens)
}

/// The power-port colour as a document paint, for a power port's strokes
/// and its net-name label (review.md: power ports use the power colour).
pub(crate) fn power_port_paint(ctx: &PanelContext) -> Paint {
    Paint::Document(signex_widgets::theme_ext::power_port(&ctx.canvas_colors))
}

/// The illustration of the selected object at the default height: no
/// handles, no expand button, no rotation (the widget draws none of them).
pub(crate) fn illustration_view<'a>(
    ctx: &PanelContext,
    illustration: Illustration,
) -> Element<'a, PanelMsg> {
    let hover = current_hover(ctx);
    let drawing = pi::property_illustration(
        illustration.height_em(DEFAULT_HEIGHT_EM),
        &hover,
        &ctx.tokens,
        TS.caption,
    )
    .map(PanelMsg::Illustration);
    container(drawing)
        .padding([SP.s, SP.m])
        .width(Length::Fill)
        .boxed()
}

/// Whether the row of `field` is lit by a hover on its callout.
pub(crate) fn is_field_highlighted(ctx: &PanelContext, field: &FieldId) -> bool {
    current_hover(ctx).is_field_highlighted(field)
}

/// A field row linked to `field`: entering it lights the callouts of the
/// field, leaving it clears them, and a hover on a callout lights the row
/// in the accent-soft wash. The row sits in a plain container, which never
/// captures the pointer, so the mouse area sees every move.
pub(crate) fn field_row<'a>(
    ctx: &PanelContext,
    field: FieldId,
    content: Element<'a, PanelMsg>,
) -> Element<'a, PanelMsg> {
    let is_lit = is_field_highlighted(ctx, &field);
    let row = container(content)
        .width(Length::Fill)
        .style(move |theme: &Theme| container::Style {
            background: is_lit
                .then(|| Background::Color(super::form_look::FormLook::of(theme).accent_soft)),
            ..container::Style::default()
        });
    mouse_area(row)
        .on_enter(PanelMsg::Illustration(IllustrationMsg::FieldHovered(
            field.clone(),
        )))
        .on_exit(PanelMsg::Illustration(IllustrationMsg::FieldLeft(field)))
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    const X: FieldId = FieldId::new("x");

    fn selected(n: u128) -> HoverSubject {
        HoverSubject {
            selected: Some((uuid::Uuid::from_u128(n), SelectedKind::Wire)),
            pre_placement: None,
        }
    }

    #[test]
    fn a_field_hover_lights_its_row_for_the_same_subject_only() {
        let mut state = PanelIllustrationHover::default();
        state.update(selected(1), IllustrationMsg::FieldHovered(X));
        let hover = state.for_subject(selected(1)).expect("same subject");
        assert!(hover.is_field_highlighted(&X));
        assert!(state.for_subject(selected(2)).is_none());
    }

    #[test]
    fn a_new_subject_drops_the_old_hover() {
        let mut state = PanelIllustrationHover::default();
        state.update(selected(1), IllustrationMsg::FieldHovered(X));
        // The late leave of the old row arrives after the selection moved.
        state.update(selected(2), IllustrationMsg::FieldLeft(X));
        let hover = state.for_subject(selected(2)).expect("new subject");
        assert_eq!(hover.hovered(), None);
    }

    #[test]
    fn the_tab_form_is_its_own_subject() {
        let form = HoverSubject {
            selected: None,
            pre_placement: Some(PrePlacementKind::Wire),
        };
        let mut state = PanelIllustrationHover::default();
        state.update(form, IllustrationMsg::FieldHovered(X));
        assert!(state.for_subject(form).is_some());
        assert!(state.for_subject(HoverSubject::default()).is_none());
    }
}
