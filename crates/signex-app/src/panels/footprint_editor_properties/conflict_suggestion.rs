//! The suggestion under the conflict rows of an over-constrained sketch
//! (mockup rows 179 and 187): which constraint to remove, with Remove
//! conflicting constraint and, for a dimension, Convert to reference. The
//! actions are the conflict rows' own (Caner 2026-11-20).

use iced::widget::{Row, column, container};
use iced::{Background, Border, Element, Length, Widget as _};
use signex_i18n::{I18n, MessageId, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, text};

use super::super::{ConflictSuggestion, PanelMsg, SketchFixMsg};
use crate::styles::{PillTone, Radii, Spacing, TypeScale, pill_colors};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The suggestion's buttons, primary first: Remove, then Convert to
/// reference on a dimension.
fn actions(suggestion: &ConflictSuggestion) -> Vec<(MessageId, SketchFixMsg)> {
    let constraint = suggestion.constraint;
    let mut actions = vec![(
        ids::SKETCH_DOF_SUGGEST_REMOVE,
        SketchFixMsg::ConflictRemoveClicked { constraint },
    )];
    if suggestion.dimension {
        actions.push((
            ids::SKETCH_DOF_SUGGEST_CONVERT,
            SketchFixMsg::ConflictConvertClicked(constraint),
        ));
    }
    actions
}

/// The suggestion box, on the danger tone of the DOF card.
pub(super) fn view<'a>(
    suggestion: &ConflictSuggestion,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, PanelMsg> {
    let (fill, ink) = pill_colors(tokens, PillTone::Err);
    let kind = t!(i18n, suggestion.kind_name).into_string();
    let line = text(t!(i18n, ids::SKETCH_DOF_SUGGEST, "kind" => kind))
        .size(TS.caption)
        .color(ink);
    let mut buttons = Row::new().spacing(SP.s);
    for (index, (label, msg)) in actions(suggestion).into_iter().enumerate() {
        let pressable = button(text(t!(i18n, label)).size(TS.caption))
            .padding([SP.xs, SP.m])
            .on_press(PanelMsg::SketchFix(msg));
        let pressable = if index == 0 {
            pressable.style(crate::styles::primary_button(tokens))
        } else {
            pressable.style(crate::styles::ghost_button(tokens))
        };
        buttons = buttons.push(pressable.boxed());
    }
    let card = container(column![line, buttons].spacing(SP.s))
        .padding([SP.s, SP.m])
        .width(Length::Fill)
        .style(move |_: &iced::Theme| iced::widget::container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                radius: Radii::GRAPHITE.card.into(),
                width: 1.0,
                color: ink,
                ..Border::default()
            },
            ..Default::default()
        });
    container(card)
        .padding([SP.xs, SP.m])
        .width(Length::Fill)
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_sketch::id::ConstraintId;

    fn suggestion(dimension: bool) -> ConflictSuggestion {
        ConflictSuggestion {
            constraint: ConstraintId::new(),
            kind_name: ids::SKETCH_CANVAS_KIND_DISTANCE,
            dimension,
        }
    }

    #[test]
    fn a_dimension_offers_remove_then_convert_to_reference() {
        let s = suggestion(true);
        let c = s.constraint;
        assert_eq!(
            actions(&s),
            vec![
                (
                    ids::SKETCH_DOF_SUGGEST_REMOVE,
                    SketchFixMsg::ConflictRemoveClicked { constraint: c }
                ),
                (
                    ids::SKETCH_DOF_SUGGEST_CONVERT,
                    SketchFixMsg::ConflictConvertClicked(c)
                ),
            ]
        );
    }

    #[test]
    fn another_kind_offers_remove_only() {
        let s = suggestion(false);
        let only: Vec<_> = actions(&s).into_iter().map(|(id, _)| id).collect();
        assert_eq!(only, vec![ids::SKETCH_DOF_SUGGEST_REMOVE]);
    }

    #[test]
    fn the_line_names_the_constraint_kind() {
        let i18n = I18n::en_us();
        let kind = t!(i18n, ids::SKETCH_CANVAS_KIND_DISTANCE).into_string();
        let line = t!(i18n, ids::SKETCH_DOF_SUGGEST, "kind" => kind)
            .into_string()
            .replace(['\u{2068}', '\u{2069}'], "");
        assert_eq!(line, "Suggested: remove the conflicting Distance.");
    }
}
