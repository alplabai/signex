//! Layer strip under the PCB canvas: the board's own layers as chips with a
//! colour swatch. Read-only; layer visibility is not wired yet, so the chips
//! carry no press action.

use iced::widget::{Row, Space, container, row};
use iced::{Background, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, ids, t};
use signex_types::format::parse_pcb_layer;
use signex_types::pcb::{LayerDef, PcbBoard};
use signex_types::pcb_layer::PcbLayer;
use signex_types::theme::ThemeTokens;
use signex_widgets::{Label, text};

use super::*;
use crate::pcb_canvas::layer_style;
use crate::styles::{ti, tokens::resolve};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// Display name and swatch of a board layer: the same name and colour the
/// Layers panel gives it (a mechanical layer the user named keeps that
/// name), so the strip and the status-bar layer chip agree; a layer token
/// the format does not know keeps its own name.
fn describe(i18n: &I18n, def: &LayerDef, tokens: &ThemeTokens) -> (Label<'static>, iced::Color) {
    let Some(layer) = parse_pcb_layer(&def.name) else {
        return (
            signex_i18n::UserText::new(def.name.clone()).into(),
            resolve(tokens).ink4,
        );
    };
    let user_name = matches!(layer, PcbLayer::Mechanical(_)) && !def.display_name.is_empty();
    let label = if user_name {
        signex_i18n::UserText::new(def.display_name.clone()).into()
    } else {
        layer_style::layer_name(i18n, layer).into()
    };
    (label, layer_style::layer_colour(layer, tokens))
}

/// The strip for `board`; `None` when the board declares no layers.
pub(super) fn layer_strip<'a>(
    board: &PcbBoard,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Option<Element<'a, Message>> {
    if board.layers.is_empty() {
        return None;
    }
    let r = resolve(tokens);
    let (ink, muted, line) = (ti(tokens.text), r.ink3, r.line_soft);
    let mut chips: Row<Element<'a, Message>> = row![
        text(t!(i18n, ids::PCB_LAYERS_TITLE))
            .size(TS.caption)
            .color(muted)
            .boxed(),
        Space::new().width(SP.s).boxed(),
    ]
    .spacing(SP.m)
    .align_y(iced::Alignment::Center);
    for layer in &board.layers {
        let (label, swatch) = describe(i18n, layer, tokens);
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
                text(label).size(TS.body).color(ink).boxed(),
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
