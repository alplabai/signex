//! What floats over the 3D view besides its toolbar (m109, m175): the
//! Explode popover and the screenshot menu under the toolbar, the lighting
//! toast, the model-unread warn toast and the status pill at the bottom
//! left, and the interaction hint at the bottom right.

use std::collections::HashMap;

use iced::alignment::Vertical;
use iced::font::Weight;
use iced::widget::button::{Status, Style as ButtonStyle};
use iced::widget::{column, container, mouse_area, row, slider};
use iced::{Alignment, Background, Border, Element, Font, Length, Padding, Theme, Widget as _};
use signex_gfx::camera3d::{LightingPreset, ViewSide};
use signex_i18n::{I18n, Translatable, ids, t};
use signex_renderer::board3d::{BoardScene3d, Face};
use signex_types::coord::Unit;
use signex_types::pcb::{PadType, PcbBoard};
use signex_widgets::{CheckboxLabel as _, LengthFormat, button, checkbox, text};

use super::icons::{Icon, icon};
use super::toolbar::{explode_percent, swatch};
use super::{Look, M, TS, card_style, event, lighting_name, preset_name, shadow};
use crate::app::state::Board3dSession;
use crate::app::{Board3dMsg, Message};
use crate::styles::Radii;
use crate::styles::board_palette::BOARD_3D_CAPTION;

/// Designators a status pill lists before it says how many more.
const LISTED: usize = 4;

/// The Explode popover: its amount, the slider from Assembled to Layers
/// apart, and what moves apart.
pub(super) fn explode_popover<'a>(
    i18n: &I18n,
    session: &Board3dSession,
    look: &Look,
) -> Element<'a, Message> {
    let percent = explode_percent(session);
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let title = row![
        text(t!(i18n, ids::PCB_3D_EXPLODE))
            .size(TS.body)
            .color(look.text)
            .font(Font {
                weight: Weight::Bold,
                ..Font::DEFAULT
            })
            .width(Length::Fill),
        text(t!(i18n, ids::PCB_3D_EXPLODE_READOUT, "percent" => percent))
            .size(TS.body)
            .color(look.text)
            .font(mono),
    ]
    .align_y(Alignment::Center);
    let value = u8::try_from(percent.min(100)).unwrap_or(100);
    let amount = slider(0..=100u8, value, |p| {
        event(Board3dMsg::ExplodeAmountChanged(p))
    })
    .width(Length::Fill);
    let captions = row![
        text(t!(i18n, ids::PCB_3D_EXPLODE_ASSEMBLED))
            .size(TS.caption)
            .color(look.r.ink4)
            .width(Length::Fill),
        text(t!(i18n, ids::PCB_3D_EXPLODE_APART))
            .size(TS.caption)
            .color(look.r.ink4),
    ];
    let explode = session.explode;
    let layers = tick(
        explode.separate_copper_and_mask,
        t!(i18n, ids::PCB_3D_EXPLODE_LAYERS),
        Board3dMsg::ExplodeLayersToggled,
    );
    let parts = tick(
        explode.lift_components,
        t!(i18n, ids::PCB_3D_EXPLODE_PARTS),
        Board3dMsg::ExplodePartsToggled,
    );
    let body = column![title, amount, captions, layers, parts].spacing(M.popover_gap * 1.5);
    popover(body.boxed(), M.popover_pad, look)
}

/// One tick box of the Explode popover.
fn tick<'a>(
    on: bool,
    label: signex_i18n::Localized,
    toggled: fn(bool) -> Board3dMsg,
) -> Element<'a, Message> {
    checkbox(on)
        .style(crate::panels::form_look::checkbox_style)
        .size(TS.title)
        .labelled(label)
        .text_size(TS.body)
        .on_toggle(move |on| event(toggled(on)))
        .boxed()
}

/// The screenshot menu: Copy to clipboard and Save PNG…. The key column
/// stays empty until the view has its own keys.
pub(super) fn shot_menu<'a>(i18n: &I18n, look: &Look) -> Element<'a, Message> {
    let items = column![
        menu_item(
            Icon::Camera,
            t!(i18n, ids::PCB_3D_SHOT_COPY),
            Board3dMsg::ScreenshotCopyRequested,
            look,
        ),
        menu_item(
            Icon::Download,
            t!(i18n, ids::PCB_3D_SHOT_SAVE),
            Board3dMsg::ScreenshotSaveRequested,
            look,
        ),
    ];
    popover(items.boxed(), M.menu_pad, look)
}

fn menu_item<'a>(
    glyph: Icon,
    name: signex_i18n::Localized,
    press: Board3dMsg,
    look: &Look,
) -> Element<'a, Message> {
    let hover = look.hover;
    let ink = look.text;
    let content = row![
        icon(glyph, M.icon, look.r.ink3),
        text(name).size(TS.body).color(ink),
    ]
    .spacing(M.button_pad)
    .align_y(Alignment::Center);
    button(
        container(content)
            .height(Length::Fill)
            .align_y(Vertical::Center),
    )
    .width(Length::Fill)
    .height(M.menu_item)
    .padding(Padding::from([0.0, M.button_pad]))
    .on_press(event(press))
    .style(move |_: &Theme, status| ButtonStyle {
        background: matches!(status, Status::Hovered | Status::Pressed)
            .then_some(Background::Color(hover)),
        text_color: ink,
        border: Border {
            radius: Radii::GRAPHITE.small.into(),
            ..Border::default()
        },
        ..ButtonStyle::default()
    })
    .boxed()
}

/// A popover card under the toolbar: raised, card radius, the floating
/// shadow, [`Metrics::popover`] wide.
fn popover<'a>(body: Element<'a, Message>, pad: f32, look: &Look) -> Element<'a, Message> {
    let card = container(body)
        .width(M.popover)
        .padding(pad)
        .style(card_style(
            look,
            Radii::GRAPHITE.card,
            shadow(look, M.popover_shadow),
        ));
    mouse_area(card)
        .interaction(iced::mouse::Interaction::Idle)
        .boxed()
}

/// The lighting toast: the four swatches, the current one ringed, and its
/// name with its place in the cycle.
pub(super) fn lighting_toast<'a>(
    i18n: &I18n,
    current: LightingPreset,
    look: &Look,
) -> Element<'a, Message> {
    let mut swatches = row![].spacing(M.button_pad).align_y(Alignment::Center);
    for preset in LightingPreset::ALL {
        let shown = if preset == current {
            ringed(swatch(preset, M.toast_swatch, 1.0, look), look)
        } else {
            swatch(preset, M.toast_swatch, 0.5, look)
        };
        swatches = swatches.push(shown);
    }
    let name = text(t!(i18n, lighting_name(current)))
        .size(TS.body)
        .color(look.text)
        .font(Font {
            weight: Weight::Bold,
            ..Font::DEFAULT
        });
    let place = text(t!(
        i18n,
        ids::PCB_3D_LIGHTING_POSITION,
        "index" => current.ordinal(),
        "count" => LightingPreset::ALL.len()
    ))
    .size(TS.body)
    .color(look.text);
    let content = row![swatches, row![name, place].spacing(M.bar_gap * 2.0)]
        .spacing(M.button_pad)
        .align_y(Alignment::Center);
    container(content)
        .padding(Padding::from([M.toast_pad_v, M.toast_pad_h]))
        .style(card_style(
            look,
            Radii::GRAPHITE.button,
            shadow(look, M.toast_shadow),
        ))
        .boxed()
}

/// `swatch` with a 2 px accent ring around it.
fn ringed<'a>(swatch: Element<'a, Message>, look: &Look) -> Element<'a, Message> {
    let accent = look.accent;
    // The mockup's 2 px ring, on the type scale.
    let ring = M.bar_gap;
    container(swatch)
        .padding(ring)
        .style(move |_| container::Style {
            border: Border {
                width: ring,
                color: accent,
                radius: Radii::GRAPHITE.pill.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// The model-unread warn toast: the first part whose model could not be
/// read, how many more, and a dismiss button. The Messages lines stay.
pub(super) fn unread_toast<'a>(
    i18n: &I18n,
    session: &Board3dSession,
    look: &Look,
) -> Option<Element<'a, Message>> {
    let (source, why) = session.unread.first()?;
    let warn = look.warn;
    let mut lines = column![
        text(t!(
            i18n,
            ids::PCB_3D_MODEL_UNREAD,
            "source" => source.as_str(),
            "why" => why.as_str()
        ))
        .size(TS.caption)
        .color(warn)
    ];
    let more = session.unread.len() - 1;
    if more > 0 {
        lines = lines.push(
            text(t!(i18n, ids::PCB_3D_MODEL_UNREAD_MORE, "count" => more))
                .size(TS.caption)
                .color(warn)
                .boxed(),
        );
    }
    let close = button(icon(Icon::Close, M.icon, warn))
        .padding(0)
        .on_press(event(Board3dMsg::UnreadNoticeDismissed))
        .style(move |_: &Theme, _| ButtonStyle {
            text_color: warn,
            ..ButtonStyle::default()
        });
    let close = signex_widgets::tooltip(
        close,
        container(
            text(t!(i18n, ids::PCB_3D_DISMISS))
                .size(TS.caption)
                .color(look.text),
        )
        .padding(Padding::from([M.bar_gap, M.button_pad]))
        .style(signex_widgets::active_bar_dropdown::tooltip_style(
            look.r.raised,
            look.r.line_strong,
        )),
        signex_widgets::tooltip::Position::Top,
    );
    let content = row![icon(Icon::Warn, M.icon, warn), lines, close]
        .spacing(M.button_gap)
        .align_y(Alignment::Start);
    let fill = look.r.warn_soft;
    let toast_shadow = shadow(look, M.bar_shadow);
    let card = container(content)
        .max_width(M.toast_max)
        .padding(Padding::from([M.warn_pad_v, M.warn_pad_h]))
        .style(move |_| container::Style {
            background: Some(Background::Color(fill)),
            text_color: Some(warn),
            border: Border {
                radius: Radii::GRAPHITE.button.into(),
                ..Border::default()
            },
            shadow: toast_shadow,
            ..container::Style::default()
        });
    Some(
        mouse_area(card)
            .interaction(iced::mouse::Interaction::Idle)
            .boxed(),
    )
}

/// The status pill: while exploding, what stands apart; while flipped,
/// the bottom side's parts and the parts with through-hole pins; else the
/// viewpoint, the board's size and how many bodies it carries.
pub(super) fn status_pill<'a>(
    i18n: &I18n,
    session: &Board3dSession,
    scene: &BoardScene3d,
    board: Option<&PcbBoard>,
    look: &Look,
) -> Element<'a, Message> {
    let line = if !session.explode.is_assembled() {
        let explode = session.explode;
        let apart = match (explode.separate_copper_and_mask, explode.lift_components) {
            (true, true) => "all",
            (true, false) => "layers",
            (false, true) => "parts",
            (false, false) => "none",
        };
        t!(
            i18n,
            ids::PCB_3D_STATUS_EXPLODE,
            "percent" => explode_percent(session),
            "apart" => apart
        )
    } else if session.side == ViewSide::Bottom {
        bottom_line(i18n, scene, board)
    } else {
        size_line(i18n, session, scene)
    };
    container(text(line).size(TS.caption).color(look.r.ink3))
        .padding(Padding::from([M.pill_pad_v, M.pill_pad_h]))
        .style(card_style(
            look,
            Radii::GRAPHITE.small,
            iced::Shadow::default(),
        ))
        .boxed()
}

/// `<Preset> · W × H × T mm · n bodies`; the preset is left out once the
/// view was turned by hand.
fn size_line(
    i18n: &I18n,
    session: &Board3dSession,
    scene: &BoardScene3d,
) -> signex_i18n::Localized {
    let mm = LengthFormat::new(i18n, Unit::Mm);
    let [w, h, thick] = scene.board_size_mm().unwrap_or([0.0; 3]);
    let shown = |v: f32| mm.number_rounded(f64::from(v), 1);
    let bodies = scene.parts.len();
    match session.active_preset {
        Some(preset) => t!(
            i18n,
            ids::PCB_3D_STATUS_VIEW,
            "preset" => Translatable::from(preset_name(preset)),
            "width" => shown(w),
            "height" => shown(h),
            "thickness" => shown(thick),
            "bodies" => bodies
        ),
        None => t!(
            i18n,
            ids::PCB_3D_STATUS_SIZE,
            "width" => shown(w),
            "height" => shown(h),
            "thickness" => shown(thick),
            "bodies" => bodies
        ),
    }
}

/// `Bottom side · <bottom parts> · through-hole pins of <THT parts>`.
fn bottom_line(
    i18n: &I18n,
    scene: &BoardScene3d,
    board: Option<&PcbBoard>,
) -> signex_i18n::Localized {
    let names: HashMap<_, _> = board
        .map(|b| {
            b.footprints
                .iter()
                .map(|f| (f.uuid, f.reference.as_str()))
                .collect()
        })
        .unwrap_or_default();
    let bottom: Vec<&str> = scene
        .parts
        .iter()
        .filter(|part| part.face == Face::Bottom)
        .filter_map(|part| names.get(&part.footprint).copied())
        .collect();
    let through: Vec<&str> = board
        .map(|b| {
            b.footprints
                .iter()
                .filter(|f| f.pads.iter().any(|p| p.pad_type == PadType::Thru))
                .map(|f| f.reference.as_str())
                .collect()
        })
        .unwrap_or_default();
    let parts = if bottom.is_empty() {
        t!(i18n, ids::PCB_3D_STATUS_NO_PARTS).into_string()
    } else {
        designators(i18n, &bottom)
    };
    if through.is_empty() {
        t!(i18n, ids::PCB_3D_STATUS_BOTTOM, "parts" => parts)
    } else {
        t!(
            i18n,
            ids::PCB_3D_STATUS_BOTTOM_THT,
            "parts" => parts,
            "tht" => designators(i18n, &through)
        )
    }
}

/// The first [`LISTED`] designators, and how many more.
fn designators(i18n: &I18n, names: &[&str]) -> String {
    let listed = names[..names.len().min(LISTED)].join(", ");
    if names.len() <= LISTED {
        return listed;
    }
    t!(
        i18n,
        ids::PCB_3D_STATUS_LIST_MORE,
        "list" => listed,
        "count" => names.len() - LISTED
    )
    .into_string()
}

/// The interaction hint: a quiet caption at the view's bottom right.
pub(super) fn hint<'a>(i18n: &I18n) -> Element<'a, Message> {
    text(t!(i18n, ids::PCB_3D_HINT))
        .size(TS.caption)
        .color(BOARD_3D_CAPTION)
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_designator_list_says_how_many_more() {
        let i18n = I18n::en_us_ref();
        assert_eq!(designators(i18n, &["J1", "J2"]), "J1, J2");
        let many = designators(i18n, &["C1", "C2", "C3", "C4", "C5", "C6"]);
        assert!(many.contains("C1, C2, C3, C4"), "{many}");
        assert!(many.contains("more"), "{many}");
        assert!(!many.contains("C5"), "{many}");
    }
}
