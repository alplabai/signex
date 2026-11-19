//! The board's Grid popover (m164, REL-05): the pitch presets with the
//! current one checked, a Custom… row whose field takes any pitch in mm on
//! Enter, and the footer's Snap to grid switch and Grids…. There is no
//! drawing: decisions REL-05 keeps editable drawings to document tabs,
//! dialogs and the Rules tab.

use iced::widget::{Space, Widget, row};
use iced::{Background, Border, Color, Element, Length, Theme};
use signex_i18n::{I18n, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::text;

use super::super::graphite::{TS, mono_font};
use super::kit::{
    PX, caption, card, card_width, check, cycles, footer_row, header, list_row, small_button,
};
use super::{BOARD_GRID_PRESETS_MM, BoardPopoverEvent, BoardPopoverKind, GridChoice};
use crate::styles::tokens::{input_bg, input_border, resolve};
use crate::styles::{Radii, ti};

/// Width of the Custom… field: room for "12.700" in the mono face.
const FIELD_W: f32 = 64.0 * PX;
/// How close a pitch must be to a preset to be that preset, in mm.
const SAME_PITCH_MM: f32 = 1e-4;
const MM_PER_MIL: f32 = 0.0254;

/// The pitch in whole mil, if it is one (1.270 mm is 50 mil).
pub(super) fn whole_mil(mm: f32) -> Option<u32> {
    let mil = mm / MM_PER_MIL;
    ((mil - mil.round()).abs() < 1e-3).then_some(mil.round() as u32)
}

/// The pitch a Custom… entry asks for: a number of mm above zero.
pub(crate) fn custom_pitch(entry: &str) -> Option<f32> {
    entry
        .trim()
        .parse::<f32>()
        .ok()
        .filter(|mm| mm.is_finite() && *mm > 0.0)
}

pub(super) fn view<'a>(
    grid: &GridChoice,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, BoardPopoverEvent> {
    let ink = ti(tokens.text);
    let keys = cycles(&grid.cycle_key, "", i18n).map(|keys| caption(keys, tokens));
    let mut parts = vec![header(t!(i18n, ids::BOARD_STATUS_GRID_TITLE), keys, tokens)];
    for mm in BOARD_GRID_PRESETS_MM {
        let value = t!(i18n, ids::BOARD_STATUS_GRID_PRESET, "value" => format!("{mm:.3}"));
        let mut body = row![
            text(value)
                .size(TS.body)
                .font(mono_font())
                .color(ink)
                .wrapping(iced::widget::text::Wrapping::None),
        ]
        .spacing(4.0 * PX)
        .align_y(iced::Alignment::Center);
        if let Some(mil) = whole_mil(mm) {
            body = body.push(caption(
                t!(i18n, ids::BOARD_STATUS_GRID_MIL, "mil" => mil),
                tokens,
            ));
        }
        let current = (grid.current_mm - mm).abs() < SAME_PITCH_MM;
        parts.push(list_row(
            None,
            body.boxed(),
            Some(check(current, tokens)),
            Some(BoardPopoverEvent::GridPresetPicked(mm)),
            tokens,
        ));
    }
    parts.push(custom_row(grid, tokens, i18n));
    let snap = row![
        signex_widgets::toggle_switch(
            grid.snap,
            switch_style(tokens),
            Some(BoardPopoverEvent::SnapToggled)
        ),
        text(t!(i18n, ids::BOARD_STATUS_SNAP_TO_GRID))
            .size(TS.body)
            .color(ink),
    ]
    .spacing(8.0 * PX)
    .align_y(iced::Alignment::Center)
    .width(Length::Fill);
    parts.push(footer_row(
        vec![
            snap.boxed(),
            small_button(
                t!(i18n, ids::BOARD_STATUS_GRIDS),
                None,
                false,
                Some(BoardPopoverEvent::GridSettingsOpened),
                tokens,
            ),
        ],
        tokens,
    ));
    card(card_width(BoardPopoverKind::Grid), parts, tokens)
}

/// The Custom… row: the label, then the field and its unit at the right. A
/// refused entry shows in the error ink until it is edited.
fn custom_row<'a>(
    grid: &GridChoice,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, BoardPopoverEvent> {
    let ink = ti(tokens.text);
    let refused = grid.refused;
    let (field_bg, line, accent, err, muted) = (
        input_bg(tokens),
        input_border(tokens),
        ti(tokens.accent),
        ti(tokens.error),
        resolve(tokens).ink3,
    );
    let field = signex_widgets::text_input(
        t!(i18n, ids::BOARD_STATUS_GRID_CUSTOM_PLACEHOLDER),
        grid.custom.clone(),
    )
    .on_input(BoardPopoverEvent::CustomGridEdited)
    .on_submit(BoardPopoverEvent::CustomGridSubmitted)
    .font(mono_font())
    .size(TS.body)
    .padding([1.0 * PX, 4.0 * PX])
    .width(FIELD_W)
    .style(move |_: &Theme, status| signex_widgets::text_input::Style {
        background: Background::Color(field_bg),
        border: Border {
            width: 1.0,
            color: match (refused, status) {
                (true, _) => err,
                (false, signex_widgets::text_input::Status::Focused { .. }) => accent,
                _ => line,
            },
            radius: Radii::GRAPHITE.small.into(),
            ..Border::default()
        },
        placeholder: muted,
        value: if refused { err } else { ink },
        selection: Color { a: 0.4, ..accent },
    });
    let body = row![
        text(t!(i18n, ids::BOARD_STATUS_GRID_CUSTOM))
            .size(TS.body)
            .color(ink),
        Space::new().width(Length::Fill),
        field,
        text(t!(i18n, ids::BOARD_STATUS_GRID_UNIT))
            .size(TS.body)
            .font(mono_font())
            .color(muted),
    ]
    .spacing(4.0 * PX)
    .align_y(iced::Alignment::Center);
    list_row(None, body.boxed(), None, None, tokens)
}

/// The switch: the accent track when on, the strong line when off.
fn switch_style(tokens: &ThemeTokens) -> signex_widgets::ToggleSwitchStyle {
    let r = resolve(tokens);
    signex_widgets::ToggleSwitchStyle::new(ti(tokens.accent), r.line_strong, r.raised)
}
