//! Live rule drawings (#2530): the selected rule, class pair or object
//! pair drawn on the board ground with its values as editable dimension
//! labels. The Rules grid's detail strip (#89) and the clearance matrix
//! (#90) show it. A label commits a value as a [`PreviewCommit`]; the
//! caller writes it to its rule, so the grid cell and the label stay one
//! value. This module never touches the rule text.
//!
//! One builder per rule kind turns the values into a [`scene::Scene`]
//! (parts, labels, notes in a fixed view box); [`view`] draws it with the
//! property illustration and stacks the labels over it. The board ground
//! and the label ink are the board palette's in both chrome themes.

mod clearance;
mod courtyard;
mod edge;
mod header;
mod hole;
mod labels;
mod mask;
mod pair;
mod scene;
mod silk;
mod via;
mod width;

#[cfg(test)]
mod tests;

use iced::keyboard::{Key, Modifiers};
use iced::widget::{column, container, stack};
use iced::{Background, Element, Length, Task, Widget as _};
use signex_drc_dsl::table::{LimitField, RuleKind};
use signex_i18n::I18n;
use signex_types::theme::{ThemeId, ThemeTokens, theme_tokens};
use signex_widgets::dimension_label::{DimensionLabelMessage, DimensionLabels};
use signex_widgets::property_illustration::{
    Illustration, IllustrationHover, property_illustration,
};

use labels::Layout;
use scene::{BoardInk, Scene, VIEW_H, VIEW_W, Values};

use crate::styles::{Spacing, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// What the preview draws.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PreviewTarget {
    /// A rule of the grid: its kind, the values it sets (mm) and the
    /// header caption.
    Rule {
        kind: RuleKind,
        values: Vec<(LimitField, f64)>,
        caption: String,
    },
    /// A class-to-class matrix cell, drawn as two tracks.
    ClassPair { a: String, b: String, gap_mm: f64 },
    /// An object pair of a matrix cell (`track`, `pad`, `via`, `pour`).
    ObjectPair { objects: [String; 2], gap_mm: f64 },
}

/// The preview's interaction state; lives in the Rules tab model.
#[derive(Debug, Clone, Default)]
pub(crate) struct RulePreview {
    labels: DimensionLabels,
    expanded: bool,
}

#[derive(Debug, Clone)]
pub enum PreviewMsg {
    Labels(DimensionLabelMessage),
    ExpandToggled,
}

/// A value set on the drawing, typed into a label or dragged by a handle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PreviewCommit {
    /// The grid column it writes; `None` for a pair gap.
    pub field: Option<LimitField>,
    pub value_mm: f64,
}

/// The scene of `target`, from its kind's builder.
fn scene_of(target: &PreviewTarget, ink: BoardInk) -> Scene {
    match target {
        PreviewTarget::Rule { kind, values, .. } => {
            let values = Values(values);
            match kind {
                RuleKind::Clearance => clearance::scene(&values, ink),
                RuleKind::Track => width::scene(&values, ink),
                RuleKind::Via => via::scene(&values, ink),
                RuleKind::Hole => hole::scene(&values, ink),
                RuleKind::Edge => edge::scene(&values, ink),
                RuleKind::Silk => silk::scene(&values, ink),
                RuleKind::Mask => mask::scene(&values, ink),
                RuleKind::Paste => mask::paste_scene(&values, ink),
                RuleKind::Courtyard => courtyard::scene(&values, ink),
            }
        }
        PreviewTarget::ClassPair { a, b, gap_mm } => pair::class_pair(a, b, *gap_mm, ink),
        PreviewTarget::ObjectPair { objects, gap_mm } => pair::object_pair(objects, *gap_mm, ink),
    }
}

/// Routes a preview message. A committed label comes back as the value to
/// write to `target`'s rule; the task focuses a label's field when one opens.
pub(crate) fn update(
    state: &mut RulePreview,
    target: &PreviewTarget,
    msg: PreviewMsg,
) -> (Option<PreviewCommit>, Task<PreviewMsg>) {
    match msg {
        PreviewMsg::ExpandToggled => {
            state.expanded = !state.expanded;
            (None, Task::none())
        }
        PreviewMsg::Labels(message) => {
            let scene = scene_of(target, BoardInk::new());
            // Lengths read and show `.` in every locale (`LengthFormat`), so
            // the shared fallback bundle parses exactly what the view shows.
            let shown =
                labels::dimension_labels(&scene, Layout::new(state.expanded), I18n::en_us_ref());
            let outcome = state.labels.update(message, &shown);
            let commit = outcome.committed.and_then(|c| {
                scene.labels.get(c.index).map(|label| PreviewCommit {
                    field: label.field,
                    value_mm: c.value,
                })
            });
            let task = if outcome.focus_input {
                state.labels.focus_input()
            } else {
                Task::none()
            };
            (commit, task)
        }
    }
}

/// Tab, Shift+Tab and Esc while a label is open or a handle is held; the
/// caller routes its key presses here first.
pub(crate) fn key_message(
    state: &RulePreview,
    key: &Key,
    modifiers: Modifiers,
) -> Option<PreviewMsg> {
    state
        .labels
        .key_message(key, modifiers)
        .map(PreviewMsg::Labels)
}

/// The header, the drawing on the board ground with its labels, and the
/// hint. Full width; the drawing is centred at its type-scale size.
pub(crate) fn view<'a>(
    state: &'a RulePreview,
    target: &'a PreviewTarget,
    i18n: &'a I18n,
    tokens: &'a ThemeTokens,
) -> Element<'a, PreviewMsg> {
    let ink = BoardInk::new();
    let scene = scene_of(target, ink);
    let layout = Layout::new(state.expanded);
    let shown = labels::dimension_labels(&scene, layout, i18n);
    let illustration = Illustration::new(VIEW_W, VIEW_H)
        .height_em(layout.height_em())
        .parts(scene.parts_with_notes(i18n));
    // The parts link no field, so the drawing publishes no hover message;
    // the map only satisfies the type.
    let drawing = property_illustration(
        illustration,
        &IllustrationHover::default(),
        &theme_tokens(ThemeId::GraphiteDark),
        TS.caption,
    )
    .map(|_| PreviewMsg::Labels(DimensionLabelMessage::PointerReleased));
    let overlay = state
        .labels
        .view(&shown, labels::style())
        .map(PreviewMsg::Labels);
    let board = container(stack![drawing, overlay])
        .width(layout.width())
        .height(layout.height());
    let ground = ink.ground;
    let framed =
        container(board)
            .center_x(Length::Fill)
            .padding(SP.m)
            .style(move |_: &iced::Theme| container::Style {
                background: Some(Background::Color(ground)),
                border: iced::Border {
                    radius: crate::styles::Radii::GRAPHITE.card.into(),
                    ..iced::Border::default()
                },
                ..container::Style::default()
            });
    column![
        header::header(state.expanded, target, i18n, tokens),
        framed,
        header::hint(i18n, tokens),
    ]
    .spacing(SP.s)
    .width(Length::Fill)
    .boxed()
}
