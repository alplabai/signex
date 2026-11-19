//! Editable dimension labels over a rule drawing, idle and with a field open.
//! The samples are fixed fixtures, so interacting with them changes nothing.

use std::sync::LazyLock;

use iced::widget::{Row, Space, container, stack};
use iced::{Background, Border, Element, Point, Theme, Vector, Widget};
use signex_i18n::I18n;
use signex_types::coord::Unit;
use signex_types::theme::ThemeTokens;
use signex_widgets::LengthFormat;
use signex_widgets::dimension_label::{
    DimensionLabel, DimensionLabelMessage, DimensionLabelStyle, DimensionLabels, DragHandle,
    LabelAnchor, ValueLimits,
};
use signex_widgets::theme_ext::TypeScale;

use crate::catalog::Message;
use crate::theme;

/// The samples show millimetres in the en-US locale.
fn format() -> LengthFormat<'static> {
    LengthFormat::new(I18n::en_us_ref(), Unit::Mm)
}

/// Label text size, the caption step of the type scale; the drawing and
/// every label metric derive from it.
const TEXT_SIZE: f32 = TypeScale::DEFAULT.caption;
/// Drawing size, in ems.
const DRAWING_EM: (f32, f32) = (34.0, 14.0);

fn em(n: f32) -> f32 {
    n * TEXT_SIZE
}

static LABELS: LazyLock<Vec<DimensionLabel<'static>>> = LazyLock::new(|| {
    let limits = ValueLimits {
        min: Some(0.005),
        max: None,
        step: 0.005,
    };
    vec![
        DimensionLabel::new(0.15, format(), Point::new(em(17.0), em(2.5)))
            .handle(DragHandle {
                at: Point::new(em(21.0), em(7.0)),
                axis: Vector::new(1.0, 0.0),
                value_per_px: 0.005,
            })
            .limits(limits),
        DimensionLabel::new(0.2, format(), Point::new(em(3.0), em(11.5)))
            .anchor(LabelAnchor::Start)
            .handle(DragHandle {
                at: Point::new(em(8.0), em(5.0)),
                axis: Vector::new(0.0, -1.0),
                value_per_px: 0.005,
            })
            .limits(limits),
        DimensionLabel::new(1.0, format(), Point::new(em(31.0), em(11.5))).anchor(LabelAnchor::End),
    ]
});

static IDLE: LazyLock<DimensionLabels> = LazyLock::new(DimensionLabels::default);

static EDITING: LazyLock<DimensionLabels> = LazyLock::new(|| {
    let mut labels = DimensionLabels::default();
    let _ = labels.update(DimensionLabelMessage::LabelPressed(0), &LABELS);
    labels
});

pub(crate) fn view(tokens: &ThemeTokens) -> Element<'static, Message> {
    let style = DimensionLabelStyle {
        ink: theme::color(tokens.accent),
        fill: theme::color(tokens.bg),
        field: theme::color(tokens.panel_bg),
        selection: theme::color(tokens.selection),
        handle: theme::color(tokens.text),
        text_size: TEXT_SIZE,
    };
    Row::new()
        .spacing(em(2.0))
        .push(drawing(&IDLE, tokens, style))
        .push(drawing(&EDITING, tokens, style))
        .boxed()
}

fn drawing(
    labels: &'static DimensionLabels,
    tokens: &ThemeTokens,
    style: DimensionLabelStyle,
) -> Element<'static, Message> {
    let paper = theme::color(tokens.bg);
    let edge = theme::color(tokens.border);
    let board = container(Space::new())
        .width(em(DRAWING_EM.0))
        .height(em(DRAWING_EM.1))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(paper)),
            border: Border {
                color: edge,
                width: 1.0,
                ..Border::default()
            },
            ..container::Style::default()
        });
    let overlay = labels
        .view(&LABELS, style)
        .map(|_| Message::TreeInteracted)
        .boxed();
    container(stack![board.boxed(), overlay])
        .width(em(DRAWING_EM.0))
        .height(em(DRAWING_EM.1))
        .boxed()
}
