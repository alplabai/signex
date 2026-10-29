//! The conflicting-constraints rows of the DOF section.
//!
//! One row per constraint, in the theme's danger colour. A click selects the
//! row's focus entity; hovering isolates the constraint's glyph on the canvas.

use iced::widget::{button, container, mouse_area, text};
use iced::{Background, Element, Length, Theme};

use super::super::{OverConstraintSummary, PanelMsg, RejectedConflictSummary};
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
}

/// The rows the panel shows: the last solve's over-constrained constraints,
/// then the ones the last rejected edit conflicted with.
pub fn conflict_rows(
    over_constraints: &[OverConstraintSummary],
    rejected_conflicts: &[RejectedConflictSummary],
) -> Vec<ConflictRowView> {
    let solved = over_constraints.iter().map(|oc| ConflictRowView {
        constraint: oc.constraint_id,
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
        label: format!(
            "{} — rejected edit, kept at the last good solve",
            rc.kind_label
        ),
        focus: rc.focus_entity_id,
    });
    solved.chain(rejected).collect()
}

fn danger_text(label: String) -> iced::widget::Text<'static, Theme> {
    text(label)
        .size(TS.caption)
        .style(|theme: &Theme| iced::widget::text::Style {
            color: Some(theme.palette().danger),
        })
}

/// One clickable (when it has a focus entity) conflict row.
pub(super) fn conflict_row<'a>(row: &ConflictRowView) -> Element<'a, PanelMsg> {
    let inner: Element<'a, PanelMsg> = match row.focus {
        Some(id) => button(danger_text(row.label.clone()))
            .padding([SP.xs, SP.m])
            .width(Length::Fill)
            .on_press(PanelMsg::FpEditorSelectSketchEntity { id })
            .style(|theme: &Theme, _| iced::widget::button::Style {
                background: Some(Background::Color(theme.palette().danger.scale_alpha(0.06))),
                ..iced::widget::button::Style::default()
            })
            .into(),
        None => container(danger_text(row.label.clone()))
            .padding([SP.xs, SP.m])
            .width(Length::Fill)
            .into(),
    };
    let constraint = row.constraint;
    mouse_area(inner)
        .on_enter(PanelMsg::FpEditorHoverOverConstraint {
            constraint: Some(constraint),
        })
        .on_exit(PanelMsg::FpEditorHoverOverConstraint { constraint: None })
        .into()
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
}
