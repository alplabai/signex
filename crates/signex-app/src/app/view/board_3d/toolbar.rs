//! The 3D view's floating toolbar (mockup rows 109 and 175), at the top centre of the
//! view: the "3D View" title; 2D | 3D; Top, Iso, Front, Left, Right, Back,
//! Reset and Fit; Flip and Explode; the lighting preset; the screenshot
//! button and its menu chevron. A tinted button is the active viewpoint, a
//! flipped board, an explode in use or an open popover. The Explode popover
//! and the screenshot menu hang under their own buttons.

use iced::alignment::Vertical;
use iced::font::Weight;
use iced::widget::button::{Status, Style as ButtonStyle};
use iced::widget::text::Wrapping;
use iced::widget::{Space, container, mouse_area, row};
use iced::{
    Alignment, Background, Border, Color, Element, Font, Length, Padding, Theme, Widget as _,
};
use signex_gfx::camera3d::{ViewPreset, ViewSide};
use signex_i18n::{I18n, Localized, ids, t};
use signex_widgets::active_bar_dropdown::tooltip_style;
use signex_widgets::{Label, SegmentedStyle, button, segmented, text, theme_ext, tooltip};

use iced_aw::DropDown;
use iced_aw::drop_down::Alignment as DropAlignment;

use super::icons::{Icon, icon};
use super::overlays::{explode_popover, shot_menu};
use super::{Look, M, TS, backdrop_fill, card_style, event, lighting_name, preset_name, shadow};
use crate::app::state::Board3dSession;
use crate::app::{Board3dMsg, Message};
use crate::styles::{Radii, Spacing};

const SP: Spacing = Spacing::DEFAULT;

/// The toolbar of `session`'s view.
pub(super) fn toolbar<'a>(
    i18n: &I18n,
    session: &Board3dSession,
    look: &Look,
) -> Element<'a, Message> {
    let active = session.active_preset;
    let preset = |p: ViewPreset| {
        let on = active == Some(p);
        tool_button(
            label(t!(i18n, preset_name(p)), on, look),
            on,
            Board3dMsg::PresetPicked(p),
            look,
        )
    };
    let reset = tool_button(
        icon(Icon::Reset, M.icon, look.ink2).boxed(),
        false,
        Board3dMsg::ResetRequested,
        look,
    );
    let fit = tool_button(
        icon(Icon::Fit, M.icon, look.ink2).boxed(),
        false,
        Board3dMsg::FitRequested,
        look,
    );
    // The title stays (Caner, 2026-11-20).
    let title = container(
        text(t!(i18n, ids::PCB_3D_TITLE))
            .size(TS.body)
            .color(look.text)
            .wrapping(Wrapping::None),
    )
    .padding(Padding::from([0.0, M.button_pad]));
    let bar = row![
        title,
        separator(look),
        mode_switch(i18n, look),
        separator(look),
        preset(ViewPreset::Top),
        preset(ViewPreset::Iso),
        preset(ViewPreset::Front),
        // Left and Right are separate buttons (Caner, 2026-11-20).
        preset(ViewPreset::Left),
        preset(ViewPreset::Right),
        preset(ViewPreset::Back),
        with_tip(reset, t!(i18n, ids::PCB_3D_RESET), look),
        with_tip(fit, t!(i18n, ids::PCB_3D_FIT), look),
        separator(look),
        flip_button(i18n, session, look),
        explode_button(i18n, session, look),
        separator(look),
        lighting_button(i18n, session, look),
        separator(look),
        screenshot_buttons(i18n, session, look),
    ]
    .spacing(M.bar_gap)
    .align_y(Alignment::Center);
    let card = container(bar).padding(M.bar_pad).style(card_style(
        look,
        Radii::GRAPHITE.button,
        shadow(look, M.bar_shadow),
    ));
    // Presses on the card's padding stay off the scene under it.
    mouse_area(card)
        .interaction(iced::mouse::Interaction::Idle)
        .boxed()
}

/// 2D | 3D: the 3D segment is the one shown; 2D leaves the view.
fn mode_switch<'a>(i18n: &I18n, look: &Look) -> Element<'a, Message> {
    let style = SegmentedStyle::new(
        Color::TRANSPARENT,
        look.r.accent_soft,
        Color::TRANSPARENT,
        look.r.ink3,
        look.r.accent_soft_ink,
        TS.body,
    );
    let options: [(bool, Label<'a>); 2] = [
        (false, t!(i18n, ids::PCB_3D_FLAT).into()),
        (true, t!(i18n, ids::PCB_3D_SOLID).into()),
    ];
    segmented(options, true, style, |solid| {
        if solid {
            Message::noop()
        } else {
            event(Board3dMsg::LeaveRequested)
        }
    })
}

/// Flip: reads Bottom and is tinted while the bottom shows.
fn flip_button<'a>(i18n: &I18n, session: &Board3dSession, look: &Look) -> Element<'a, Message> {
    let flipped = session.side == ViewSide::Bottom;
    let (name, tip) = if flipped {
        (ids::PCB_3D_FLIPPED, ids::PCB_3D_FLIP_BACK_TOOLTIP)
    } else {
        (ids::PCB_3D_FLIP, ids::PCB_3D_FLIP_TOOLTIP)
    };
    let content = icon_label(Icon::Flip, t!(i18n, name), flipped, look);
    let button = tool_button(content, flipped, Board3dMsg::FlipRequested, look);
    with_tip(button, t!(i18n, tip), look)
}

/// Explode: reads its amount and is tinted while its popover is open or
/// the board stands apart.
fn explode_button<'a>(i18n: &I18n, session: &Board3dSession, look: &Look) -> Element<'a, Message> {
    let on = session.explode_open || !session.explode.is_assembled();
    let name = if on {
        t!(i18n, ids::PCB_3D_EXPLODE_AMOUNT, "percent" => explode_percent(session))
    } else {
        t!(i18n, ids::PCB_3D_EXPLODE)
    };
    let tip = if session.explode_open {
        ids::PCB_3D_EXPLODE_CLOSE
    } else {
        ids::PCB_3D_EXPLODE
    };
    let content = icon_label(Icon::Explode, name, on, look);
    let button = tool_button(content, on, Board3dMsg::ExplodePopoverToggled, look);
    let button = with_tip(button, t!(i18n, tip), look);
    let popover = explode_popover(i18n, session, look);
    anchored(button, popover, session.explode_open)
}

/// `popover` hanging under `button` while `open`, centred on it. Esc
/// closes it through the app's Esc rung (`board_3d_popover_escape`).
fn anchored<'a>(
    button: Element<'a, Message>,
    popover: Element<'a, Message>,
    open: bool,
) -> Element<'a, Message> {
    DropDown::new(button, popover, open)
        .width(Length::Shrink)
        .alignment(DropAlignment::Bottom)
        .offset(M.popover_gap + M.bar_pad)
        .boxed()
}

/// The explode amount in whole percent.
pub(super) fn explode_percent(session: &Board3dSession) -> u32 {
    (session.explode.amount.clamp(0.0, 1.0) * 100.0).round() as u32
}

/// The lighting preset: its swatch, its name and a chevron; a click moves
/// to the next one. Tinted while its toast shows.
fn lighting_button<'a>(i18n: &I18n, session: &Board3dSession, look: &Look) -> Element<'a, Message> {
    let on = session.lighting_toast.is_some();
    let ink = if on {
        look.r.accent_soft_ink
    } else {
        look.ink2
    };
    let content = row![
        swatch(session.lighting, M.swatch, 1.0, look),
        label(t!(i18n, lighting_name(session.lighting)), on, look),
        signex_widgets::chevron::chevron(false, chevron_font(), ink),
    ]
    .spacing(M.button_gap)
    .align_y(Alignment::Center)
    .boxed();
    let button = tool_button(content, on, Board3dMsg::LightingCycled, look);
    with_tip(button, t!(i18n, ids::PCB_3D_LIGHTING_TOOLTIP), look)
}

/// The screenshot button (a click copies) and its menu chevron.
fn screenshot_buttons<'a>(
    i18n: &I18n,
    session: &Board3dSession,
    look: &Look,
) -> Element<'a, Message> {
    let copy = tool_button(
        icon(Icon::Camera, M.icon, look.ink2).boxed(),
        false,
        Board3dMsg::ScreenshotCopyRequested,
        look,
    );
    let open = session.shot_menu_open;
    let ink = if open {
        look.r.accent_soft_ink
    } else {
        look.ink2
    };
    let menu = tool_button(
        signex_widgets::chevron::chevron(true, chevron_font(), ink).boxed(),
        open,
        Board3dMsg::ScreenshotMenuToggled,
        look,
    );
    let menu = anchored(menu, shot_menu(i18n, look), open);
    row![
        with_tip(copy, t!(i18n, ids::PCB_3D_SHOT_TOOLTIP), look),
        menu
    ]
    .align_y(Alignment::Center)
    .boxed()
}

/// The font size whose chevron box is the mockup's 12 px glyph.
fn chevron_font() -> f32 {
    M.chevron / signex_widgets::chevron::SIDE_EM
}

/// A lighting swatch: the preset's backdrop in a circle of `size` px, at
/// `alpha`.
pub(super) fn swatch<'a>(
    preset: signex_gfx::camera3d::LightingPreset,
    size: f32,
    alpha: f32,
    look: &Look,
) -> Element<'a, Message> {
    let fill = backdrop_fill(preset, alpha);
    let line = look.r.line_strong.scale_alpha(alpha);
    container(Space::new())
        .width(size)
        .height(size)
        .style(move |_| container::Style {
            background: Some(fill),
            border: Border {
                width: theme_ext::HAIRLINE,
                color: line,
                radius: (size / 2.0).into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// A button label: the button's ink, medium weight while it is on.
fn label<'a>(text_: Localized, on: bool, look: &Look) -> Element<'a, Message> {
    let ink = if on {
        look.r.accent_soft_ink
    } else {
        look.ink2
    };
    let mut label = text(text_)
        .size(TS.body)
        .color(ink)
        .wrapping(Wrapping::None);
    if on {
        label = label.font(Font {
            weight: Weight::Medium,
            ..Font::DEFAULT
        });
    }
    label.boxed()
}

/// An icon and a label, in the button's ink.
fn icon_label<'a>(glyph: Icon, name: Localized, on: bool, look: &Look) -> Element<'a, Message> {
    let ink = if on {
        look.r.accent_soft_ink
    } else {
        look.ink2
    };
    row![icon(glyph, M.icon, ink), label(name, on, look)]
        .spacing(M.button_gap)
        .align_y(Alignment::Center)
        .boxed()
}

/// One toolbar button around `content`: no fill at rest, the hover wash,
/// the soft accent while `on`.
fn tool_button<'a>(
    content: Element<'a, Message>,
    on: bool,
    press: Board3dMsg,
    look: &Look,
) -> Element<'a, Message> {
    let (soft, hover, ink) = (look.r.accent_soft, look.hover, look.ink2);
    let content = container(content)
        .height(Length::Fill)
        .align_y(Vertical::Center);
    button(content)
        .height(M.button)
        .padding(Padding::from([0.0, M.button_pad]))
        .on_press(event(press))
        .style(move |_: &Theme, status| ButtonStyle {
            background: if on {
                Some(Background::Color(soft))
            } else {
                matches!(status, Status::Hovered | Status::Pressed)
                    .then_some(Background::Color(hover))
            },
            text_color: ink,
            border: Border {
                radius: Radii::GRAPHITE.small.into(),
                ..Border::default()
            },
            ..ButtonStyle::default()
        })
        .boxed()
}

/// `button` with a hover tip under it.
fn with_tip<'a>(button: Element<'a, Message>, tip: Localized, look: &Look) -> Element<'a, Message> {
    let hint = container(text(tip).size(TS.caption).color(look.text))
        .padding([SP.xs, SP.m])
        .style(tooltip_style(look.r.raised, look.r.line_strong));
    tooltip(button, hint, tooltip::Position::Bottom)
        .gap(SP.xs)
        .boxed()
}

/// A short vertical rule between the toolbar's groups.
fn separator<'a>(look: &Look) -> Element<'a, Message> {
    let line = look.r.line_soft;
    let rule = container(Space::new())
        .width(theme_ext::HAIRLINE)
        .height(M.sep)
        .style(move |_| container::Style {
            background: Some(Background::Color(line)),
            ..container::Style::default()
        });
    container(rule)
        .padding(Padding::from([0.0, M.sep_margin]))
        .boxed()
}
