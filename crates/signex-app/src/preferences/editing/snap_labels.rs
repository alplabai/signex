//! The snap preview block on Editing ▸ Grid (#147) and its two pitch labels.
//! Click a label to type the pitch: Enter commits, Tab moves to the other
//! pitch, Esc cancels; the ● handle drags it. Each label shows its field's
//! text, and a pitch set here goes back through the field's draft message.
//!
//! The interaction is the BB5 `DimensionLabels` state machine, kept in
//! `PagesViewState`. The labels are drawn here with BB5's metrics
//! (`label_box`) rather than by its overlay: that overlay borrows its label
//! list for the whole frame, and these labels are built from the drafts in
//! the view, and a refused pitch draws in the error colour.

use super::snap_preview::{SnapDrawing, SnapGeometry, SnapNotes};
use crate::canvas::grid::{parse_fast_grid_field, pitch_mm_exact};
use crate::preferences::pages_view::controls::{
    GLYPH_PENCIL, PreviewBlock, PreviewColours, glyph, preview_block, view_msg,
};
use crate::preferences::pages_view::key_catcher::catch_keys;
use crate::preferences::pages_view::{PagesViewMsg, PagesViewState, PreviewKind, snap_field_id};
use crate::preferences::{PrefMsg, PrefsView};
use crate::styles;
use iced::alignment::{Horizontal, Vertical};
use iced::widget::{Space, Stack, canvas, container, mouse_area, responsive, row};
use iced::{
    Alignment, Background, Border, Element, Font, Length, Point, Rectangle, Size, Theme,
    Widget as _, mouse,
};
use signex_i18n::{Arg, I18n, Localized, MessageId, UserText, ids, t};
use signex_types::coord::Unit;
use signex_types::theme::ThemeTokens;
use signex_widgets::dimension_label::{DimensionLabelMessage, LabelAnchor, label_box};
use signex_widgets::{LengthFormat, Translate, button, text, text_input};

const TS: styles::TypeScale = styles::TypeScale::DEFAULT;

/// Text size of a pitch label; its other sizes are ems of it, as in BB5.
const LABEL_SIZE: f32 = TS.caption;
/// Room for the pencil glyph after the text, in glyphs.
const PENCIL_GLYPHS: usize = 2;
const GAP_EM: f32 = 0.35;
const RADIUS_EM: f32 = 0.35;
const PAD_V_EM: f32 = 0.2;
const PAD_H_EM: f32 = 0.5;
/// Narrowest open field, in ems, so a short pitch still has room to type.
const FIELD_MIN_EM: f32 = 7.5;
const LABEL_EDGE_W: f32 = 1.2;
const FIELD_EDGE_W: f32 = 1.5;
/// Drag handle: the dot, its ring and the area that takes the press.
const HANDLE_DOT: f32 = 10.0;
const HANDLE_RING: f32 = 2.0;
const HANDLE_HIT: f32 = 22.0;
/// Decimals a pitch shows.
const PITCH_DECIMALS: usize = 6;

fn label_msg(message: DimensionLabelMessage) -> PrefMsg {
    view_msg(PagesViewMsg::SnapLabel(message))
}

/// The preview block: label, caption, expand button, the drawing with its
/// pitch labels, and the how-to line under it.
pub(super) fn snap_preview<'a>(v: PrefsView<'a>, expanded: bool) -> Element<'a, PrefMsg> {
    let (t, i18n) = (v.panel_tokens, v.i18n);
    let (drafts, state) = (v.draft_fast_grids, v.pages_view);
    // The drawing is laid out per size: the labels sit at the arrow ends,
    // which move with the drawing's width.
    let picture = responsive(move |size| drawing(size, drafts, state, t, i18n))
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed();
    preview_block(
        PreviewBlock {
            kind: PreviewKind::Snap,
            expanded,
            title: t!(i18n, ids::PREFERENCES_EDITING_SNAP_PREVIEW),
            caption: caption(i18n, pitches(drafts)),
            drawing: picture,
            footer: Some(t!(i18n, ids::PREFERENCES_EDITING_SNAP_HELP)),
        },
        t,
        i18n,
    )
}

/// Each field's grid in mm, or `None` when Save would refuse it.
fn pitches(drafts: &[String; 2]) -> [Option<f32>; 2] {
    [0, 1].map(|index| parse_fast_grid_field(&drafts[index]).ok())
}

/// "1.27 mm"; lengths keep `.` in every locale.
fn millimetres(i18n: &I18n, mm: f32) -> String {
    let value =
        LengthFormat::new(i18n, Unit::Mm).number_rounded(pitch_mm_exact(mm), PITCH_DECIMALS);
    i18n.message_plain(
        ids::PREFERENCES_PAGES_MILLIMETRES,
        &[("value", Arg::from(value))],
    )
}

/// The caption beside the label: both pitches, or "not valid".
pub(super) fn caption(i18n: &I18n, pitches: [Option<f32>; 2]) -> Localized {
    let shown = |pitch: Option<f32>| match pitch {
        Some(mm) => millimetres(i18n, mm),
        None => i18n.message_plain(ids::PREFERENCES_EDITING_SNAP_NOT_VALID, &[]),
    };
    t!(
        i18n,
        ids::PREFERENCES_EDITING_SNAP_CAPTION,
        "first" => shown(pitches[0]),
        "second" => shown(pitches[1])
    )
}

fn drawing<'a>(
    size: Size,
    drafts: &'a [String; 2],
    state: &'a PagesViewState,
    t: &'a ThemeTokens,
    i18n: &'a I18n,
) -> Element<'a, PrefMsg> {
    let grids = pitches(drafts);
    let geometry = SnapGeometry::new(size, grids);
    let colours = PreviewColours::new(t);
    let notes = SnapNotes {
        every: (geometry.every > 1.0).then(|| {
            i18n.message_plain(
                ids::PREFERENCES_EDITING_SNAP_EVERY,
                &[("count", Arg::from(geometry.every as u64))],
            )
        }),
        snaps_here: i18n.message_plain(ids::PREFERENCES_EDITING_SNAP_HERE, &[]),
        no_rings: i18n.message_plain(ids::PREFERENCES_EDITING_SNAP_NO_RINGS, &[]),
    };
    let labels = state.snap_labels();
    // A fixed layer order (drawing, two handles, two labels), so a widget
    // keeps its state when a handle comes and goes.
    let mut layers: Vec<Element<'a, PrefMsg>> = vec![
        canvas::Canvas::new(SnapDrawing {
            geometry,
            colours,
            notes,
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed(),
    ];
    for index in 0..2 {
        layers.push(match geometry.handle_at(index) {
            Some(at) => handle(index, at, &colours),
            None => Space::new().boxed(),
        });
    }
    for (index, pitch) in grids.into_iter().enumerate() {
        let tag = label_tag(index, pitch, geometry.label_at(index), i18n);
        layers.push(match labels.draft() {
            Some(draft) if labels.editing_index() == Some(index) => field(tag, draft, &colours),
            _ => tag_view(tag, &colours),
        });
    }
    let mut area = mouse_area(
        Stack::with_children(layers)
            .width(Length::Fill)
            .height(Length::Fill),
    );
    if labels.dragging_index().is_some() {
        area = area
            .on_move(|at| label_msg(DimensionLabelMessage::PointerMoved(at)))
            .on_release(label_msg(DimensionLabelMessage::PointerReleased));
    }
    catch_keys(area.boxed(), move |key, modifiers| {
        labels.key_message(key, modifiers).map(label_msg)
    })
}

/// A pitch label: which fast grid, where its centre is, its text and
/// whether the field holds a grid Save would take.
struct LabelTag {
    index: usize,
    bounds: Rectangle,
    text: Localized,
    valid: bool,
}

fn label_tag(index: usize, pitch: Option<f32>, at: Point, i18n: &I18n) -> LabelTag {
    let number = Arg::from(index + 1);
    let (id, args): (MessageId, Vec<(&str, Arg)>) = match pitch {
        Some(mm) => (
            ids::PREFERENCES_EDITING_SNAP_LABEL,
            vec![
                ("number", number),
                ("pitch", Arg::from(millimetres(i18n, mm))),
            ],
        ),
        None => (
            ids::PREFERENCES_EDITING_SNAP_LABEL_INVALID,
            vec![("number", number)],
        ),
    };
    let glyphs = i18n.message_plain(id, &args).chars().count();
    LabelTag {
        index,
        bounds: label_box(
            glyphs + PENCIL_GLYPHS,
            0,
            LABEL_SIZE,
            at,
            LabelAnchor::Middle,
        ),
        text: i18n.message(id, &args),
        valid: pitch.is_some(),
    }
}

/// The closed label: its text and a pencil, pressed to type the pitch.
fn tag_view<'a>(tag: LabelTag, c: &PreviewColours) -> Element<'a, PrefMsg> {
    let (ink, fill, edge) = if tag.valid {
        (c.accent_text, c.raised, c.accent)
    } else {
        (c.err, c.err_soft, c.err)
    };
    let bounds = tag.bounds;
    let content = row![
        text(tag.text)
            .size(LABEL_SIZE)
            .font(Font::MONOSPACE)
            .color(ink),
        glyph(GLYPH_PENCIL, LABEL_SIZE, ink),
    ]
    .spacing(GAP_EM * LABEL_SIZE)
    .align_y(Alignment::Center);
    let face = container(content)
        .width(bounds.width)
        .height(bounds.height)
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                color: edge,
                width: LABEL_EDGE_W,
                radius: (RADIUS_EM * LABEL_SIZE).into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    let pressable = button(face)
        .padding(0.0)
        .style(|_: &Theme, _| button::Style::default())
        .on_press(label_msg(DimensionLabelMessage::LabelPressed(tag.index)));
    Element::new(Translate::new(pressable, (bounds.x, bounds.y)))
}

/// The open label: a field holding the typed pitch.
fn field<'a>(tag: LabelTag, draft: &'a str, c: &PreviewColours) -> Element<'a, PrefMsg> {
    let bounds = tag.bounds;
    let width = bounds.width.max(FIELD_MIN_EM * LABEL_SIZE);
    let x = bounds.center_x() - width / 2.0;
    let edge = if tag.valid { c.accent } else { c.err };
    let (fill, ink, selection) = (c.field, c.ink, c.accent_text);
    let index = tag.index;
    let input = text_input(UserText::new(String::new()), draft)
        .id(snap_field_id())
        .on_input(|typed| label_msg(DimensionLabelMessage::DraftEdited(typed)))
        .on_submit(label_msg(DimensionLabelMessage::DraftSubmitted))
        .on_blur(label_msg(DimensionLabelMessage::DraftBlurred(index)))
        .font(Font::MONOSPACE)
        .size(LABEL_SIZE)
        .padding([PAD_V_EM * LABEL_SIZE, PAD_H_EM * LABEL_SIZE])
        .width(width)
        .align_x(Horizontal::Center)
        .style(move |_: &Theme, _| text_input::Style {
            background: Background::Color(fill),
            border: Border {
                color: edge,
                width: FIELD_EDGE_W,
                radius: (RADIUS_EM * LABEL_SIZE).into(),
                ..Border::default()
            },
            placeholder: ink,
            value: ink,
            selection: iced::Color {
                a: 0.3,
                ..selection
            },
        });
    Element::new(Translate::new(input, (x, bounds.y)))
}

/// The ● handle that drags a pitch along the arrow.
fn handle<'a>(index: usize, at: Point, c: &PreviewColours) -> Element<'a, PrefMsg> {
    let (fill, ring) = (c.raised, c.accent);
    let dot =
        container(Space::new().width(HANDLE_DOT).height(HANDLE_DOT)).style(move |_: &Theme| {
            container::Style {
                background: Some(Background::Color(fill)),
                border: Border {
                    color: ring,
                    width: HANDLE_RING,
                    radius: (HANDLE_DOT / 2.0).into(),
                    ..Border::default()
                },
                ..container::Style::default()
            }
        });
    let target = container(dot)
        .width(HANDLE_HIT)
        .height(HANDLE_HIT)
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center);
    let grip = mouse_area(target)
        .on_press(label_msg(DimensionLabelMessage::HandlePressed(index)))
        .interaction(mouse::Interaction::ResizingHorizontally);
    Element::new(Translate::new(
        grip,
        (at.x - HANDLE_HIT / 2.0, at.y - HANDLE_HIT / 2.0),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(text: Localized) -> String {
        text.into_string().replace(['\u{2068}', '\u{2069}'], "")
    }

    #[test]
    fn the_caption_names_both_pitches_or_says_one_is_not_valid() {
        let en = I18n::en_us();
        assert_eq!(
            plain(caption(&en, [Some(1.27), Some(2.54)])),
            "Fast grid 1 1.27 mm (dots) · Fast grid 2 2.54 mm (rings)"
        );
        assert_eq!(
            plain(caption(&en, [Some(1.27), None])),
            "Fast grid 1 1.27 mm (dots) · Fast grid 2 not valid (rings)"
        );
    }

    #[test]
    fn a_label_reads_its_pitch_or_says_it_is_invalid() {
        let en = I18n::en_us();
        let good = label_tag(0, Some(1.27), Point::new(200.0, 30.0), &en);
        assert!(good.valid);
        assert_eq!(plain(good.text), "Fast grid 1 · 1.27 mm");
        let bad = label_tag(1, None, Point::new(200.0, 30.0), &en);
        assert!(!bad.valid);
        assert_eq!(plain(bad.text), "Fast grid 2 · invalid");
    }
}
