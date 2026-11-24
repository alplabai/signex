//! What floats over the 3D view besides its toolbar (mockup rows 109 and 175): the
//! Explode popover and the screenshot menu (hung under their buttons by
//! the toolbar), the lighting
//! toast, the model-unread warn toast and the status pill at the bottom
//! left, and the interaction hint at the bottom right.

use std::collections::HashMap;

use iced::alignment::Vertical;
use iced::font::Weight;
use iced::widget::button::{Status, Style as ButtonStyle};
use iced::widget::{column, container, mouse_area, row, slider};
use iced::{Alignment, Background, Border, Element, Font, Length, Padding, Theme, Widget as _};
use signex_gfx::camera3d::{LightingPreset, ViewSide};
use signex_i18n::{I18n, MessageId, Translatable, UserText, ids, t};
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
                ..signex_widgets::ui_font::regular()
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

/// The screenshot menu's rows, in order (mockup row 175): Copy to
/// clipboard, Save PNG… and Save with transparent backdrop….
fn shot_menu_rows() -> [(Icon, MessageId, Board3dMsg); 3] {
    [
        (
            Icon::Camera,
            ids::PCB_3D_SHOT_COPY,
            Board3dMsg::ScreenshotCopyRequested,
        ),
        (
            Icon::Download,
            ids::PCB_3D_SHOT_SAVE,
            Board3dMsg::ScreenshotSaveRequested,
        ),
        (
            Icon::Download,
            ids::PCB_3D_SHOT_SAVE_TRANSPARENT,
            Board3dMsg::ScreenshotTransparentSaveRequested,
        ),
    ]
}

/// The screenshot menu, one row per [`shot_menu_rows`]. `copy_key` is the
/// copy command's shortcut label (empty when unbound), shown on its row.
pub(super) fn shot_menu<'a>(i18n: &I18n, copy_key: &str, look: &Look) -> Element<'a, Message> {
    let mut items = column![];
    for (glyph, name, press) in shot_menu_rows() {
        let key = menu_key(&press, copy_key);
        items = items.push(menu_item(glyph, t!(i18n, name), key, press, look));
    }
    popover(items.boxed(), M.menu_pad, look)
}

/// The key a menu row shows (mockup row 175, "Copy to clipboard
/// Ctrl+Shift+C"): the copy row's bound key; the save rows have none.
fn menu_key(press: &Board3dMsg, copy_key: &str) -> Option<String> {
    (matches!(press, Board3dMsg::ScreenshotCopyRequested) && !copy_key.is_empty())
        .then(|| copy_key.to_owned())
}

fn menu_item<'a>(
    glyph: Icon,
    name: signex_i18n::Localized,
    key: Option<String>,
    press: Board3dMsg,
    look: &Look,
) -> Element<'a, Message> {
    let hover = look.hover;
    let ink = look.text;
    let mut content = row![
        icon(glyph, M.icon, look.r.ink3),
        text(name).size(TS.body).color(ink).width(Length::Fill),
    ];
    if let Some(key) = key {
        content = content.push(key_cap(key, look));
    }
    let content = content.spacing(M.button_pad).align_y(Alignment::Center);
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

/// A popover card under its button: raised, card radius, the floating
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

/// A key as a cap (mockup row 175's key chips): the mono caption on the
/// shared key-cap style.
fn key_cap<'a>(key: String, look: &Look) -> Element<'a, Message> {
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    container(text(UserText::new(key)).size(TS.caption).font(mono))
        .padding(Padding::from([0.0, M.bar_gap * 2.0]))
        .style(crate::styles::key_cap_of(look.r))
        .boxed()
}

/// The lighting toast's key line around the cap (mockup row 175, "· press
/// H for the next"): the text before and after `{ $key }`, or `None` when
/// the next-lighting command has no key.
fn next_lighting_words(i18n: &I18n, next_key: &str) -> Option<(String, String)> {
    if next_key.is_empty() {
        return None;
    }
    // A marker no key label holds, split out so the key draws as a cap.
    const MARK: &str = "\u{1}";
    let line = i18n.message_plain(ids::PCB_3D_LIGHTING_NEXT_KEY, &[("key", MARK.into())]);
    let (before, after) = line.split_once(MARK)?;
    Some((before.trim_end().to_owned(), after.trim_start().to_owned()))
}

/// The lighting toast: the four swatches, the current one ringed, and its
/// name with its place in the cycle, then the next-lighting key when it has
/// one (`next_key`, empty when unbound).
pub(super) fn lighting_toast<'a>(
    i18n: &I18n,
    current: LightingPreset,
    next_key: &str,
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
            ..signex_widgets::ui_font::regular()
        });
    let place = text(t!(
        i18n,
        ids::PCB_3D_LIGHTING_POSITION,
        "index" => current.ordinal(),
        "count" => LightingPreset::ALL.len()
    ))
    .size(TS.body)
    .color(look.text);
    let mut words = row![name, place]
        .spacing(M.bar_gap * 2.0)
        .align_y(Alignment::Center);
    if let Some((before, after)) = next_lighting_words(i18n, next_key) {
        let word = |part: String| {
            text(UserText::new(part))
                .size(TS.body)
                .color(look.text)
                .boxed()
        };
        words = words
            .push(word(before))
            .push(key_cap(next_key.to_owned(), look))
            .push(word(after));
    }
    let content = row![swatches, words]
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
        // As wide as its text, up to the mockup's widest; longer text wraps.
        .width(Length::Fit.max(M.toast_max))
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

    /// Mockup row 175: the screenshot menu lists Copy to clipboard, Save
    /// PNG… and Save with transparent backdrop…, each sending its own
    /// request.
    #[test]
    fn the_screenshot_menu_offers_a_transparent_save() {
        let i18n = I18n::en_us_ref();
        let rows: Vec<(String, Board3dMsg)> = shot_menu_rows()
            .into_iter()
            .map(|(_, name, press)| (t!(i18n, name).into_string(), press))
            .collect();
        assert_eq!(
            rows,
            [
                (
                    "Copy to clipboard".to_owned(),
                    Board3dMsg::ScreenshotCopyRequested
                ),
                ("Save PNG…".to_owned(), Board3dMsg::ScreenshotSaveRequested),
                (
                    "Save with transparent backdrop…".to_owned(),
                    Board3dMsg::ScreenshotTransparentSaveRequested
                ),
            ]
        );
    }

    /// Mockup row 175: "Copy to clipboard Ctrl+Shift+C"; the save rows show
    /// no key, nor does Copy when its command is unbound.
    #[test]
    fn only_the_copy_row_shows_its_key() {
        let keys: Vec<Option<String>> = shot_menu_rows()
            .iter()
            .map(|(_, _, press)| menu_key(press, "Ctrl+Shift+C"))
            .collect();
        assert_eq!(keys, [Some("Ctrl+Shift+C".to_owned()), None, None]);
        assert_eq!(menu_key(&Board3dMsg::ScreenshotCopyRequested, ""), None);
    }

    /// Mockup row 175: "Workshop · 2 of 4 · press H for the next", the key
    /// drawn as a cap between the words; no words when lighting is unbound.
    #[test]
    fn the_lighting_toast_names_the_next_key() {
        let i18n = I18n::en_us_ref();
        assert_eq!(
            next_lighting_words(i18n, "H"),
            Some(("· press".to_owned(), "for the next".to_owned()))
        );
        assert_eq!(next_lighting_words(i18n, ""), None);
    }
}
