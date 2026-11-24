//! The conflicting-constraints rows of the DOF section.
//!
//! One row per constraint, in the theme's danger colour. A click selects the
//! row's focus entity; hovering isolates the constraint's glyph on the canvas.
//! Each row has Remove; a dimension's row also has Convert to reference
//! (mockup row 187, item E4), which keeps it as a measurement that never drives.

use iced::widget::{container, mouse_area};
use iced::{Background, Element, Length, Theme, Widget};
use signex_i18n::{ids, t};
use signex_widgets::{button, raw, text};

use super::super::{OverConstraintSummary, PanelMsg, RejectedConflictSummary, SketchFixMsg};
use crate::styles::{Spacing, TypeScale};
use signex_sketch::id::{ConstraintId, SketchEntityId};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Rows shown before "... +N more".
pub(super) const MAX_ROWS: usize = 8;

/// One conflicting constraint's text and where a click on it goes.
#[derive(Debug, Clone, PartialEq)]
pub struct ConflictRowView {
    pub constraint: ConstraintId,
    pub label: String,
    pub focus: Option<SketchEntityId>,
    /// The constraint is a dimension, so it can become a reference.
    pub dimension: bool,
}

/// Whether `kind_label`, a [`ConstraintKind::label`], names a dimension
/// kind (one with [`ConstraintKind::dim_target`]).
///
/// [`ConstraintKind::label`]: signex_sketch::constraint::ConstraintKind::label
/// [`ConstraintKind::dim_target`]: signex_sketch::constraint::ConstraintKind::dim_target
fn is_dimension_label(kind_label: &str) -> bool {
    matches!(
        kind_label,
        "DistancePtPt" | "DistancePtLine" | "DistancePtCircle" | "Angle"
    )
}

/// The rows the panel shows: the last solve's over-constrained constraints,
/// then the ones the last rejected edit conflicted with.
pub fn conflict_rows(
    over_constraints: &[OverConstraintSummary],
    rejected_conflicts: &[RejectedConflictSummary],
) -> Vec<ConflictRowView> {
    let solved = over_constraints.iter().map(|oc| ConflictRowView {
        constraint: oc.constraint_id,
        dimension: is_dimension_label(oc.kind_label),
        // GH #599: an unevaluable residual says so, not "residual 0".
        label: match oc.residual_magnitude {
            Some(mag) => format!("{} — residual {:.3e}", oc.kind_label, mag),
            None => format!(
                "{} — residual unavailable (target expression did not evaluate)",
                oc.kind_label
            ),
        },
        focus: oc.focus_entity_id,
    });
    let rejected = rejected_conflicts.iter().map(|rc| ConflictRowView {
        constraint: rc.constraint_id,
        dimension: is_dimension_label(rc.kind_label),
        label: format!(
            "{} — rejected edit, kept at the last good solve",
            rc.kind_label
        ),
        focus: rc.focus_entity_id,
    });
    solved.chain(rejected).collect()
}

fn danger_text(label: String) -> iced::widget::Text<'static, Theme> {
    text(raw(label))
        .size(TS.caption)
        .style(|theme: &Theme| iced::widget::text::Style {
            color: Some(theme.palette().danger.base.color),
            selection: None,
        })
}

/// One clickable (when it has a focus entity) conflict row.
pub(super) fn conflict_row<'a>(
    row: &ConflictRowView,
    i18n: &signex_i18n::I18n,
) -> Element<'a, PanelMsg> {
    let inner: Element<'a, PanelMsg> = match row.focus {
        Some(id) => button(danger_text(row.label.clone()))
            .padding([SP.xs, SP.m])
            .width(Length::Fill)
            .on_press(PanelMsg::FpEditorSelectSketchEntity { id })
            .style(|theme: &Theme, _| iced::widget::button::Style {
                background: Some(Background::Color(
                    theme.palette().danger.base.color.scale_alpha(0.06),
                )),
                ..iced::widget::button::Style::default()
            })
            .boxed(),
        None => container(danger_text(row.label.clone()))
            .padding([SP.xs, SP.m])
            .width(Length::Fill)
            .boxed(),
    };
    let constraint = row.constraint;
    let mut line = iced::widget::row![inner]
        .align_y(iced::Alignment::Center)
        .width(Length::Fill);
    if row.dimension {
        line = line.push(convert_button(constraint, i18n));
    }
    line = line.push(remove_button(constraint, i18n));
    mouse_area(line)
        .on_enter(PanelMsg::FpEditorHoverOverConstraint {
            constraint: Some(constraint),
        })
        .on_exit(PanelMsg::FpEditorHoverOverConstraint { constraint: None })
        .boxed()
}

/// Remove (Caner 2026-11-20): deletes the row's constraint as one undo step.
/// A delete glyph in the danger colour, named by its tooltip.
fn remove_button<'a>(constraint: ConstraintId, i18n: &signex_i18n::I18n) -> Element<'a, PanelMsg> {
    glyph_button(
        crate::icons::icon_dd_delete(signex_types::theme::ThemeId::DEFAULT),
        |theme| theme.palette().danger.base.color,
        PanelMsg::SketchFix(SketchFixMsg::ConflictRemoveClicked { constraint }),
        (i18n, ids::SKETCH_CANVAS_CONFLICT_REMOVE),
    )
}

/// Convert to reference (mockup row 187, item E4): the row's dimension measures from now
/// on, one undo step. A reference glyph in the text colour, named by its
/// tooltip.
fn convert_button<'a>(constraint: ConstraintId, i18n: &signex_i18n::I18n) -> Element<'a, PanelMsg> {
    glyph_button(
        crate::icons::icon_dd_reference(signex_types::theme::ThemeId::DEFAULT),
        |theme| theme.palette().background.base.text,
        PanelMsg::SketchFix(SketchFixMsg::ConflictConvertClicked(constraint)),
        (i18n, ids::SKETCH_CANVAS_CONFLICT_CONVERT),
    )
}

/// A small glyph button on a conflict row, tinted by `tint`, with the
/// tooltip `tip` names.
fn glyph_button<'a>(
    icon: iced::widget::svg::Handle,
    tint: fn(&Theme) -> iced::Color,
    msg: PanelMsg,
    (i18n, tip): (&signex_i18n::I18n, signex_i18n::MessageId),
) -> Element<'a, PanelMsg> {
    use iced::widget::svg;
    let glyph = svg(icon)
        .width(Length::Fixed(TS.body))
        .height(Length::Fixed(TS.body))
        .style(move |theme: &Theme, _| svg::Style {
            color: Some(tint(theme)),
            ..Default::default()
        });
    let pressable = button(glyph)
        .padding([SP.xs, SP.s])
        .on_press(msg)
        .style(|_: &Theme, _| iced::widget::button::Style::default());
    let tip = container(text(t!(i18n, tip)).size(TS.caption))
        .padding([SP.xs, SP.s])
        .style(crate::panels::form_look::value_tip);
    signex_widgets::tooltip(pressable, tip, iced::widget::tooltip::Position::Left)
        .gap(SP.xs)
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rejected_edit_is_a_clickable_row_after_the_solve_rows() {
        let (a, b, focus) = (
            ConstraintId::new(),
            ConstraintId::new(),
            SketchEntityId::new(),
        );
        let solved = [OverConstraintSummary {
            constraint_id: a,
            kind_label: "Horizontal",
            residual_magnitude: Some(0.5),
            focus_entity_id: None,
        }];
        let rejected = [RejectedConflictSummary {
            constraint_id: b,
            kind_label: "DistancePtPt",
            focus_entity_id: Some(focus),
        }];

        let rows = conflict_rows(&solved, &rejected);

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].constraint, a);
        assert_eq!(rows[1].constraint, b);
        assert!(
            rows[1].label.starts_with("DistancePtPt"),
            "{}",
            rows[1].label
        );
        assert_eq!(rows[1].focus, Some(focus), "click selects the point");
        assert!(conflict_rows(&[], &[]).is_empty());
    }

    #[test]
    fn only_a_dimension_row_offers_convert_to_reference() {
        let summary = |kind_label| OverConstraintSummary {
            constraint_id: ConstraintId::new(),
            kind_label,
            residual_magnitude: Some(0.5),
            focus_entity_id: None,
        };
        let rows = conflict_rows(&[summary("Horizontal"), summary("Angle")], &[]);
        assert!(!rows[0].dimension);
        assert!(rows[1].dimension);
    }

    #[test]
    fn the_dimension_labels_are_the_kinds_with_a_target() {
        use signex_sketch::constraint::{ConstraintKind, DimTarget};
        let (a, b, c) = (
            SketchEntityId::new(),
            SketchEntityId::new(),
            SketchEntityId::new(),
        );
        let t = || DimTarget::Literal(1.0);
        let kinds = [
            ConstraintKind::Coincident { p1: a, p2: b },
            ConstraintKind::PointOnLine { point: a, line: b },
            ConstraintKind::PointOnArc { point: a, arc: b },
            ConstraintKind::Horizontal { line: a },
            ConstraintKind::Vertical { line: a },
            ConstraintKind::Parallel { l1: a, l2: b },
            ConstraintKind::Perpendicular { l1: a, l2: b },
            ConstraintKind::DistancePtPt {
                p1: a,
                p2: b,
                target: t(),
            },
            ConstraintKind::DistancePtLine {
                point: a,
                line: b,
                target: t(),
            },
            ConstraintKind::DistancePtCircle {
                point: a,
                circle: b,
                target: t(),
            },
            ConstraintKind::Angle {
                l1: a,
                l2: b,
                target: t(),
            },
            ConstraintKind::EqualLength { l1: a, l2: b },
            ConstraintKind::EqualRadius { e1: a, e2: b },
            ConstraintKind::TangentLineArc { line: a, arc: b },
            ConstraintKind::TangentArcArc {
                a1: a,
                a2: b,
                internal: false,
            },
            ConstraintKind::SymmetricAboutLine {
                p1: a,
                p2: b,
                line: c,
            },
            ConstraintKind::SymmetricAboutPoint {
                p1: a,
                p2: b,
                center: c,
            },
            ConstraintKind::Midpoint { point: a, line: b },
            ConstraintKind::Fixed { point: a },
        ];
        for kind in &kinds {
            assert_eq!(
                is_dimension_label(kind.label()),
                kind.dim_target().is_some(),
                "{}",
                kind.label()
            );
        }
    }
}
