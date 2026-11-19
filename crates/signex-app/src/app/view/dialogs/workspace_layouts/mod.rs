//! Save Layout and Manage Layouts (#166), from the Graphite mock m166. The
//! state and the rules are `crate::workspace_layouts`; the update half is
//! `app/handlers/workspace_layouts.rs`. Design: docs/WORKSPACE_LAYOUTS.md.

mod manage;
mod save;
mod thumb;

use iced::widget::{Row, Space, container, svg};
use iced::{Background, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, MessageId, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::text;

use crate::app::{Message, Signex};
use crate::styles::{Radii, Spacing, TypeScale, ti, tokens};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Design size (width, centring height) of Save Layout.
const SAVE_LAYOUT: (f32, f32) = (460.0, 380.0);
/// Design size of Manage Layouts: the mock's 820 × 520.
const MANAGE_LAYOUTS: (f32, f32) = (820.0, 520.0);

/// The icons are `currentColor` line art; any theme's handle draws the same
/// strokes and the colour comes from the style.
const GLYPH_THEME: signex_types::theme::ThemeId = signex_types::theme::ThemeId::GraphiteDark;
/// Side of an inline icon, the mock's 14 px at the default scale.
const ICON: f32 = TS.label;

impl Signex {
    pub(in crate::app::view) fn save_layout_overlay(&self) -> Option<Element<'_, Message>> {
        let dialog = self.ui_state.workspace_layouts.save.as_ref()?;
        Some(self.wrap_centred_dialog(self.view_save_layout_card(dialog), SAVE_LAYOUT))
    }

    pub(in crate::app::view) fn manage_layouts_overlay(&self) -> Option<Element<'_, Message>> {
        let dialog = self.ui_state.workspace_layouts.manage.as_ref()?;
        Some(self.wrap_centred_dialog(self.view_manage_layouts_card(dialog), MANAGE_LAYOUTS))
    }
}

/// An inline icon in `color`.
fn icon<'a>(handle: svg::Handle, color: iced::Color, side: f32) -> Element<'a, Message> {
    svg(handle)
        .width(Length::Fixed(side))
        .height(Length::Fixed(side))
        .style(move |_: &Theme, _| svg::Style {
            color: Some(color),
            ..svg::Style::default()
        })
        .boxed()
}

/// A catalog caption: caption size, the faintest ink.
fn caption<'a>(i18n: &I18n, id: MessageId, tokens: &ThemeTokens) -> Element<'a, Message> {
    text(t!(i18n, id))
        .size(TS.caption)
        .color(tokens::resolve(tokens).ink4)
        .boxed()
}

/// One form-grid row: the key in a `key_width` column, the value beside it.
fn form_row<'a>(
    key: Option<Element<'a, Message>>,
    value: Element<'a, Message>,
    key_width: f32,
) -> Element<'a, Message> {
    let key = key.unwrap_or_else(|| Space::new().boxed());
    Row::<Element<'a, Message>>::new()
        .push(container(key).width(Length::Fixed(key_width)).boxed())
        .push(container(value).width(Length::Fill).boxed())
        .spacing(SP.m + SP.xs)
        .align_y(iced::Alignment::Center)
        .boxed()
}

/// A form-grid key: body size, the third ink.
fn form_key<'a>(i18n: &I18n, id: MessageId, tokens: &ThemeTokens) -> Element<'a, Message> {
    text(t!(i18n, id))
        .size(TS.body)
        .color(tokens::resolve(tokens).ink3)
        .wrapping(iced::widget::text::Wrapping::None)
        .boxed()
}

/// The Shortcut dropdown, greyed: shortcuts for layouts are not built yet.
fn shortcut_field<'a>(i18n: &I18n, tokens: &ThemeTokens, width: Length) -> Element<'a, Message> {
    let r = tokens::resolve(tokens);
    let (fill, line, ink) = (r.sunken, ti(tokens.border), r.ink4);
    let line_row = Row::<Element<'a, Message>>::new()
        .push(
            text(t!(i18n, signex_i18n::ids::WORKSPACE_LAYOUTS_SHORTCUT_NONE))
                .size(TS.body)
                .color(ink)
                .width(Length::Fill)
                .boxed(),
        )
        .push(icon(
            crate::icons::icon_chrome_chevron_down(GLYPH_THEME),
            ink,
            ICON,
        ))
        .spacing(SP.s)
        .align_y(iced::Alignment::Center);
    container(line_row)
        .padding([SP.s, SP.m])
        .width(width)
        .height(Length::Fixed(crate::styles::ChromeMetrics::DEFAULT.button))
        .align_y(iced::alignment::Vertical::Center)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                width: 1.0,
                color: line,
                radius: Radii::GRAPHITE.control.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}
