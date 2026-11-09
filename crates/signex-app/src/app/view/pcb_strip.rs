//! Layer strip under the PCB canvas: the board's own layers as chips with a
//! colour swatch. Read-only; layer visibility is not wired yet, so the chips
//! carry no press action.

use iced::widget::{Row, Space, container, row};
use iced::{Background, Border, Element, Length, Theme, Widget as _};
use signex_types::pcb::PcbBoard;
use signex_types::theme::ThemeTokens;
use signex_widgets::{raw, text};

use super::*;
use crate::library::editor::footprint::layers::FpLayer;
use crate::styles::{ti, tokens::resolve};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// Display name and swatch of a board layer: the two copper layers use the
/// editor's layer names and colours, any other layer keeps its own name.
fn describe(name: &str) -> (String, iced::Color) {
    match name {
        "TopCopper" | "F.Cu" => (FpLayer::FCu.label().to_string(), FpLayer::FCu.color()),
        "BottomCopper" | "B.Cu" => (FpLayer::BCu.label().to_string(), FpLayer::BCu.color()),
        other => (other.to_string(), iced::Color::from_rgb(0.6, 0.6, 0.62)),
    }
}

/// The strip for `board`; `None` when the board declares no layers.
pub(super) fn layer_strip<'a>(
    board: &PcbBoard,
    tokens: &ThemeTokens,
) -> Option<Element<'a, Message>> {
    if board.layers.is_empty() {
        return None;
    }
    let r = resolve(tokens);
    let (ink, muted, line) = (ti(tokens.text), r.ink3, r.line_soft);
    let mut chips: Row<Element<'a, Message>> = row![
        text(raw("Layers")).size(TS.caption).color(muted).boxed(),
        Space::new().width(SP.s).boxed(),
    ]
    .spacing(SP.m)
    .align_y(iced::Alignment::Center);
    for layer in &board.layers {
        let (label, swatch) = describe(&layer.name);
        chips = chips.push(
            row![
                container(Space::new())
                    .width(TS.caption)
                    .height(TS.caption)
                    .style(move |_: &Theme| container::Style {
                        background: Some(Background::Color(swatch)),
                        border: Border {
                            radius: 2.0.into(),
                            ..Border::default()
                        },
                        ..container::Style::default()
                    })
                    .boxed(),
                text(raw(label)).size(TS.body).color(ink).boxed(),
            ]
            .spacing(SP.s)
            .align_y(iced::Alignment::Center)
            .boxed(),
        );
    }
    Some(
        container(chips)
            .padding([SP.s, SP.l])
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                border: Border {
                    width: 1.0,
                    color: line,
                    ..Border::default()
                },
                ..container::Style::default()
            })
            .boxed(),
    )
}

/// `view` with `strip` (when there is one) stacked under it.
pub(super) fn with_strip<'a>(
    view: Element<'a, Message>,
    strip: Option<Element<'a, Message>>,
) -> Element<'a, Message> {
    match strip {
        Some(strip) => iced::widget::column![view, strip].boxed(),
        None => view,
    }
}
