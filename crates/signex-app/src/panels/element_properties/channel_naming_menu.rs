//! The Channel naming section of a repeated sheet symbol and its format
//! menus (#1279): Designators and Local nets each open a menu right under
//! the field, right-aligned to it, with the preset formats, a Custom
//! format… field checked by `check_format`, and the token help line. The
//! choice is a project setting (`app/compiled_design/naming.rs`).

use iced::widget::{Column, column, container, row};
use iced::{Alignment, Background, Border, Element, Length, Shadow, Theme, Widget as _};
use signex_i18n::{UserText, ids, t};
use signex_types::schematic::{ChannelFormatError, ChannelOccurrence};
use signex_widgets::{text, text_input};

use super::super::compiled_design::{NamingTarget, SheetSymbolRepeatView, msg};
use super::super::{PROPERTY_LABEL_WIDTH, PanelContext, PanelMsg};
use crate::app::CompiledDesignMsg;
use crate::app::compiled_design::{DESIGNATOR_PRESETS, NET_PRESETS};
use crate::styles::{Radii, Spacing, TypeScale, ti, tokens::resolve};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Width of the open menu: 340 px at the 12 px body.
const MENU_W: f32 = TS.body * 340.0 / 12.0;

/// The example the preview line and the menus spell out.
const EXAMPLE_DESIGNATOR: &str = "R5";
const EXAMPLE_NET: &str = "VOUT_3V3";

/// The first channel of a repeat, for the previews.
fn first_channel(view: &SheetSymbolRepeatView) -> ChannelOccurrence {
    view.repeat
        .as_ref()
        .and_then(|r| r.occurrences().into_iter().next())
        .unwrap_or(ChannelOccurrence {
            name: "CH1".to_string(),
            index: 1,
            number: 1,
            of: 1,
        })
}

/// Designators and Local nets fields, the open menu and the live preview.
pub(super) fn naming_section<'a>(
    ctx: &'a PanelContext,
    view: &'a SheetSymbolRepeatView,
) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let ch = first_channel(view);
    let preview = t!(
        i18n,
        ids::PROPERTIES_SHEET_SYMBOL_NAMING_PREVIEW,
        "designator" => EXAMPLE_DESIGNATOR,
        "channel-designator" => view.naming.designator(EXAMPLE_DESIGNATOR, &ch),
        "net" => EXAMPLE_NET,
        "channel-net" => view.naming.net(EXAMPLE_NET, &ch)
    );
    let mut body: Column<Element<'a, PanelMsg>> = column![].width(Length::Fill);
    for target in [NamingTarget::Designators, NamingTarget::Nets] {
        body = body.push(format_field(ctx, view, target));
        if view.menu.target == Some(target) {
            body = body.push(menu(ctx, view, target, &ch));
        }
    }
    body = body.push(
        container(
            text(preview)
                .size(TS.caption)
                .color(resolve(&ctx.tokens).ink3),
        )
        .padding([SP.xs, SP.m]),
    );
    ctx.collapsible(
        "sel_child_sheet_channel_naming",
        ids::PROPERTIES_SHEET_SYMBOL_CHANNEL_NAMING,
        &ctx.collapsed_sections,
        move || body,
    )
    .boxed()
}

/// A dropdown field showing the format in mono; it opens its menu.
fn format_field<'a>(
    ctx: &'a PanelContext,
    view: &'a SheetSymbolRepeatView,
    target: NamingTarget,
) -> Element<'a, PanelMsg> {
    let tokens = &ctx.tokens;
    let res = resolve(tokens);
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let format = match target {
        NamingTarget::Designators => view.naming.designator_format.as_str(),
        NamingTarget::Nets => view.naming.net_format.as_str(),
    };
    let is_open = view.menu.target == Some(target);
    let edge = if is_open {
        ti(tokens.accent)
    } else {
        res.line_soft
    };
    let field = signex_widgets::button(
        row![
            text(UserText::new(format))
                .size(TS.body)
                .font(mono)
                .width(Length::Fill)
                .wrapping(iced::widget::text::Wrapping::None),
            signex_widgets::chevron::chevron(is_open, TS.body, res.ink3),
        ]
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([SP.xs, SP.s])
    .on_press(msg(CompiledDesignMsg::NamingMenuToggled(target)))
    .style(move |_: &Theme, _| iced::widget::button::Style {
        background: Some(Background::Color(res.field)),
        border: Border {
            color: edge,
            width: 1.0,
            radius: Radii::GRAPHITE.control.into(),
            ..Border::default()
        },
        ..iced::widget::button::Style::default()
    });
    container(
        row![
            text(t!(
                ctx.i18n,
                super::child_sheet_repeat::target_label(target)
            ))
            .size(TS.body)
            .color(res.ink3)
            .width(PROPERTY_LABEL_WIDTH),
            field,
        ]
        .spacing(SP.s)
        .align_y(Alignment::Center),
    )
    .padding([SP.s, SP.m])
    .width(Length::Fill)
    .boxed()
}

/// The open menu: title, a radio row per preset with its example, Custom
/// format… and the token help line. Right-aligned under its field.
fn menu<'a>(
    ctx: &'a PanelContext,
    view: &'a SheetSymbolRepeatView,
    target: NamingTarget,
    ch: &ChannelOccurrence,
) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let tokens = &ctx.tokens;
    let res = resolve(tokens);
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let (presets, current, title, base) = match target {
        NamingTarget::Designators => (
            DESIGNATOR_PRESETS,
            view.naming.designator_format.as_str(),
            ids::PROPERTIES_CHANNEL_NAMING_DESIGNATOR_FORMAT,
            "${Designator}",
        ),
        NamingTarget::Nets => (
            NET_PRESETS,
            view.naming.net_format.as_str(),
            ids::PROPERTIES_CHANNEL_NAMING_NET_FORMAT,
            "${Net}",
        ),
    };
    let mut rows: Column<Element<'a, PanelMsg>> =
        column![text(t!(i18n, title)).size(TS.caption).color(res.ink3)].spacing(SP.xs);
    for preset in presets {
        let example = match target {
            NamingTarget::Designators => view
                .naming_with(target, preset)
                .designator(EXAMPLE_DESIGNATOR, ch),
            NamingTarget::Nets => view.naming_with(target, preset).net(EXAMPLE_NET, ch),
        };
        let mark = if preset == current { "●" } else { "○" };
        rows = rows.push(
            signex_widgets::button(
                row![
                    text(UserText::new(mark))
                        .size(TS.caption)
                        .color(ti(tokens.accent)),
                    text(UserText::new(preset))
                        .size(TS.body)
                        .font(mono)
                        .width(Length::Fill),
                    text(UserText::new(example))
                        .size(TS.caption)
                        .font(mono)
                        .color(res.ink3),
                ]
                .spacing(SP.s)
                .align_y(Alignment::Center),
            )
            .width(Length::Fill)
            .padding([SP.xs, SP.s])
            .on_press(msg(CompiledDesignMsg::NamingPicked {
                target,
                format: preset.to_string(),
            }))
            .style(crate::styles::menu_item(tokens)),
        );
    }
    rows = rows.push(rule(res.line_soft));
    rows = rows.push(
        text_input(
            t!(i18n, ids::PROPERTIES_CHANNEL_NAMING_CUSTOM),
            &view.menu.custom,
        )
        .size(TS.body)
        .padding([SP.xs, SP.s])
        .on_input(|s| msg(CompiledDesignMsg::NamingCustomTyped(s)))
        .on_submit(msg(CompiledDesignMsg::NamingCustomSubmitted))
        .style(super::super::form_look::text_input_style),
    );
    if let Some(error) = &view.menu.error {
        rows = rows.push(
            text(format_error(ctx, error))
                .size(TS.caption)
                .color(ti(tokens.error)),
        );
    }
    rows = rows.push(
        text(t!(
            i18n,
            ids::PROPERTIES_CHANNEL_NAMING_TOKENS,
            "channel" => "${Channel}",
            "index" => "${Index}",
            "alpha" => "${ChannelAlpha}",
            "base" => base,
            "example" => match target {
                NamingTarget::Designators => EXAMPLE_DESIGNATOR,
                NamingTarget::Nets => EXAMPLE_NET,
            }
        ))
        .size(TS.caption)
        .color(res.ink3),
    );
    let (fill, edge, shadow) = (res.raised, res.line_soft, res.shadow);
    let card = container(rows)
        .padding(SP.s)
        .width(MENU_W)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                color: edge,
                width: 1.0,
                radius: Radii::GRAPHITE.card.into(),
                ..Border::default()
            },
            shadow: Shadow {
                color: shadow,
                blur_radius: SP.m,
                ..Shadow::default()
            },
            ..container::Style::default()
        });
    container(card)
        .padding([0.0, SP.m])
        .width(Length::Fill)
        .align_x(iced::alignment::Horizontal::Right)
        .boxed()
}

fn rule<'a>(color: iced::Color) -> Element<'a, PanelMsg> {
    container(iced::widget::Space::new())
        .width(Length::Fill)
        .height(1)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(color)),
            ..container::Style::default()
        })
        .boxed()
}

/// Why a custom format is refused, from the catalog.
fn format_error(ctx: &PanelContext, error: &ChannelFormatError) -> signex_i18n::Localized {
    let i18n = &*ctx.i18n;
    match error {
        ChannelFormatError::Empty => t!(i18n, ids::PROPERTIES_CHANNEL_NAMING_ERROR_EMPTY),
        ChannelFormatError::Whitespace => {
            t!(i18n, ids::PROPERTIES_CHANNEL_NAMING_ERROR_WHITESPACE)
        }
        ChannelFormatError::Unclosed => t!(i18n, ids::PROPERTIES_CHANNEL_NAMING_ERROR_UNCLOSED),
        ChannelFormatError::UnknownToken(token) => t!(
            i18n,
            ids::PROPERTIES_CHANNEL_NAMING_ERROR_UNKNOWN,
            "token" => token.as_str()
        ),
        ChannelFormatError::NoChannelToken => {
            t!(i18n, ids::PROPERTIES_CHANNEL_NAMING_ERROR_NO_CHANNEL)
        }
        ChannelFormatError::NoBaseToken => t!(i18n, ids::PROPERTIES_CHANNEL_NAMING_ERROR_NO_BASE),
    }
}
