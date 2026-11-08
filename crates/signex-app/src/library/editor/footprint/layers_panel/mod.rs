//! Footprint Layers panel: the layers grouped by board side, an eye toggle
//! and a colour swatch per layer, and the "Show all" / "Top side only"
//! shortcuts. A pure view over the panel context; every press is a
//! `PanelMsg` that the dock handler forwards to the editor.

use iced::widget::{Column, button, container, row, text};
use iced::{Background, Border, Color, Element, Length, Theme, Widget as _};
use signex_types::theme::ThemeTokens;

use super::layers::{FpLayer, FpSide, LayerPalette};
use crate::panels::{PanelContext, PanelMsg};
use crate::styles::{Radii, Spacing, TypeScale, ti, tokens::resolve};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Side of a layer colour swatch.
const SWATCH: f32 = TS.body;
/// Width of the eye button.
const EYE_WIDTH: f32 = TS.title * 1.6;

/// Render the panel for the active footprint editor.
pub fn view<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let tokens = &ctx.tokens;
    let ink = ti(tokens.text);
    let muted = ti(tokens.text_secondary);
    let Some(fp) = ctx.footprint_editor.as_ref() else {
        return container(
            text("Open a `.snxfpt` to see its layers here.")
                .size(TS.caption)
                .color(muted),
        )
        .padding([6.0, SP.m])
        .width(Length::Fill)
        .boxed();
    };
    let palette = LayerPalette::for_tokens(tokens);
    let radius = Radii::GRAPHITE.control;

    let actions = row![
        action_button(
            "Show all",
            PanelMsg::FpEditorLayersShowAllClicked,
            ink,
            radius,
        ),
        action_button(
            "Top side only",
            PanelMsg::FpEditorLayersTopSideOnlyClicked,
            ink,
            radius,
        ),
    ]
    .spacing(SP.s);

    let mut col: Column<Element<'a, PanelMsg>> = Column::new().spacing(SP.s).width(Length::Fill);
    col = col.push(
        container(actions)
            .padding([SP.m, SP.m])
            .width(Length::Fill)
            .boxed(),
    );
    for side in FpSide::ORDER {
        col = col.push(
            container(
                text(side.label().to_uppercase())
                    .size(TS.section)
                    .color(muted),
            )
            .padding([SP.s, SP.m])
            .width(Length::Fill)
            .boxed(),
        );
        for layer in FpLayer::ORDER.iter().filter(|l| l.side() == *side) {
            let visible = fp.layer_visibility.get(*layer);
            col = col.push(layer_row(*layer, visible, palette, tokens));
        }
    }
    col.boxed()
}

/// One layer: eye, swatch, display name and the standard name.
fn layer_row<'a>(
    layer: FpLayer,
    visible: bool,
    palette: LayerPalette,
    tokens: &ThemeTokens,
) -> Element<'a, PanelMsg> {
    let ink = ti(tokens.text);
    let muted = resolve(tokens).ink3;
    let swatch_colour = layer.color_in(palette);
    let label_ink = if visible { ink } else { muted };
    let radius = Radii::GRAPHITE.control;

    let eye = button(
        text(if visible { "\u{25C9}" } else { "\u{25CB}" })
            .size(TS.label)
            .color(label_ink)
            .align_x(iced::alignment::Horizontal::Center),
    )
    .width(Length::Fixed(EYE_WIDTH))
    .padding([SP.xs, 0.0])
    .on_press(PanelMsg::FpEditorLayerEyeClicked(
        layer.standard_name().to_string(),
    ))
    .style(move |_: &Theme, status| button::Style {
        background: match status {
            button::Status::Hovered => Some(Background::Color(Color { a: 0.08, ..ink })),
            _ => None,
        },
        text_color: label_ink,
        border: Border {
            radius: radius.into(),
            ..Border::default()
        },
        ..button::Style::default()
    });

    let swatch = container(text("").size(TS.caption))
        .width(Length::Fixed(SWATCH))
        .height(Length::Fixed(SWATCH))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(if visible {
                swatch_colour
            } else {
                Color {
                    a: 0.35,
                    ..swatch_colour
                }
            })),
            border: Border {
                width: 1.0,
                color: Color {
                    a: 0.5,
                    ..swatch_colour
                },
                radius: 2.0.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });

    container(
        row![
            eye.boxed(),
            swatch.boxed(),
            text(layer.label()).size(TS.body).color(label_ink).boxed(),
            text(layer.standard_name())
                .size(TS.caption)
                .font(crate::fonts::iced_font_for_family(
                    crate::fonts::CHROME_MONO_FONT,
                ))
                .color(muted)
                .boxed(),
        ]
        .spacing(SP.m)
        .align_y(iced::Alignment::Center),
    )
    .padding([0.0, SP.m])
    .width(Length::Fill)
    .boxed()
}

/// A bordered text button of the panel's action row.
fn action_button<'a>(
    label: &'static str,
    msg: PanelMsg,
    ink: Color,
    radius: f32,
) -> Element<'a, PanelMsg> {
    button(text(label).size(TS.body).color(ink))
        .padding([SP.s, SP.l])
        .on_press(msg)
        .style(move |_: &Theme, status| button::Style {
            background: Some(Background::Color(Color {
                a: if status == button::Status::Hovered {
                    0.10
                } else {
                    0.03
                },
                ..ink
            })),
            text_color: ink,
            border: Border {
                width: 1.0,
                color: Color { a: 0.2, ..ink },
                radius: radius.into(),
                ..Border::default()
            },
            ..button::Style::default()
        })
        .boxed()
}
