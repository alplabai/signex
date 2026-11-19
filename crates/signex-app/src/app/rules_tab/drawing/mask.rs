//! Solder mask and paste: two pads of a fine-pitch part. The mask is drawn
//! over the pads only, as dashed opening edges, never as a flood (CX review
//! 2026-11-19); the sliver is the mask web left between the openings.
//! Paste draws its aperture against the pad.

use iced::{Point, Rectangle};
use signex_drc_dsl::table::LimitField;
use signex_i18n::{MessageId, ids};
use signex_widgets::dimension_label::LabelAnchor;
use signex_widgets::property_illustration::{Anchor, Tint};

use super::scene::{BoardInk, NoteText, Scene, Values, grown, rect};

/// The pads, in millimetres, and their pitch.
const PAD_MM: (f64, f64) = (0.6, 0.8);
const PITCH_MM: f64 = 0.85;
/// View units per millimetre.
const SCALE: f32 = 190.0;
const CENTRE: Point = Point::new(320.0, 150.0);
const NOMINAL_EXPANSION_MM: f64 = 0.05;
/// Arrows stand this far above and below the pad centre.
const ARROW_DY: f32 = 30.0;
const LABEL_GAP: f32 = 12.0;
const SLIVER_GAP: f32 = 24.0;

/// The two pads, left to right.
fn pads(scene: &mut Scene) -> [Rectangle; 2] {
    let (w, h) = (PAD_MM.0 as f32 * SCALE, PAD_MM.1 as f32 * SCALE);
    let half_pitch = PITCH_MM as f32 * SCALE / 2.0;
    let pads = [CENTRE.x - half_pitch, CENTRE.x + half_pitch]
        .map(|x| rect(x - w / 2.0, CENTRE.y - h / 2.0, w, h));
    let copper = scene.ink.copper;
    for pad in pads {
        scene.solid_rect(pad, 3.0, copper);
    }
    pads
}

/// An expansion arrow from the pad edge at `edge_x` outwards by `value`
/// (`direction` -1 left, +1 right), with its label beyond the arrow head.
fn expansion(
    scene: &mut Scene,
    values: &Values,
    field: LimitField,
    caption: MessageId,
    edge_x: f32,
    direction: f32,
    y: f32,
) {
    let Some(value) = values.get(field) else {
        return;
    };
    let tip = edge_x + direction * value as f32 * SCALE;
    scene.dimension(Point::new(edge_x, y), Point::new(tip, y));
    let anchor = if direction < 0.0 {
        LabelAnchor::End
    } else {
        LabelAnchor::Start
    };
    let outer = edge_x + direction * (value.abs() as f32 * SCALE + LABEL_GAP);
    scene.value(
        Some(field),
        caption,
        Some(value),
        Point::new(outer, y),
        anchor,
        None,
    );
}

pub(super) fn scene(values: &Values, ink: BoardInk) -> Scene {
    let mut scene = Scene::new(ink);
    let pads = pads(&mut scene);
    let min = values.get(LimitField::MinMaskExpansion);
    let max = values.get(LimitField::MaxMaskExpansion);
    let drawn = max.or(min).unwrap_or(NOMINAL_EXPANSION_MM);
    for value in [min, max].into_iter().flatten() {
        for pad in pads {
            let by = value as f32 * SCALE;
            scene.outline_rect(grown(pad, by), 3.0 + by.max(0.0), ink.mask, true, None);
        }
    }
    let left_edge = pads[0].x;
    for (field, caption, dy) in [
        (
            LimitField::MaxMaskExpansion,
            ids::RULES_PREVIEW_CAP_MAX,
            -ARROW_DY,
        ),
        (
            LimitField::MinMaskExpansion,
            ids::RULES_PREVIEW_CAP_MIN,
            ARROW_DY,
        ),
    ] {
        expansion(
            &mut scene,
            values,
            field,
            caption,
            left_edge,
            -1.0,
            CENTRE.y + dy,
        );
    }

    // The web between the openings at the drawn expansion.
    let by = drawn as f32 * SCALE;
    let (a, b) = (pads[0].x + pads[0].width + by, pads[1].x - by);
    let y = pads[0].y + pads[0].height + by.max(0.0) + SLIVER_GAP;
    if b > a {
        scene.extension(Point::new(a, CENTRE.y), Point::new(a, y + 4.0));
        scene.extension(Point::new(b, CENTRE.y), Point::new(b, y + 4.0));
        scene.dimension(Point::new(a, y), Point::new(b, y));
    } else {
        scene.note(
            Point::new(CENTRE.x, y - 10.0),
            NoteText::Id(ids::RULES_PREVIEW_NOTE_MASK_MERGED),
            Anchor::Middle,
            ink.dimension,
        );
    }
    scene.value(
        Some(LimitField::MinSliver),
        ids::RULES_PREVIEW_CAP_SLIVER,
        values.get(LimitField::MinSliver),
        Point::new(CENTRE.x, y + LABEL_GAP + 6.0),
        LabelAnchor::Middle,
        None,
    );
    scene.title(ids::RULES_PREVIEW_TITLE_MASK, ids::RULES_PREVIEW_NOTE_MASK);
    scene
}

/// Paste: the aperture over each pad, grown or shrunk by the expansion.
pub(super) fn paste_scene(values: &Values, ink: BoardInk) -> Scene {
    let mut scene = Scene::new(ink);
    let pads = pads(&mut scene);
    let min = values.get(LimitField::MinPasteExpansion);
    let max = values.get(LimitField::MaxPasteExpansion);
    for value in [min, max].into_iter().flatten() {
        for pad in pads {
            let by = value as f32 * SCALE;
            scene.outline_rect(grown(pad, by), 2.0, ink.paste, false, Some(Tint::Strong));
        }
    }
    let right_edge = pads[1].x + pads[1].width;
    for (field, caption, dy) in [
        (
            LimitField::MaxPasteExpansion,
            ids::RULES_PREVIEW_CAP_MAX,
            -ARROW_DY,
        ),
        (
            LimitField::MinPasteExpansion,
            ids::RULES_PREVIEW_CAP_MIN,
            ARROW_DY,
        ),
    ] {
        expansion(
            &mut scene,
            values,
            field,
            caption,
            right_edge,
            1.0,
            CENTRE.y + dy,
        );
    }
    scene.title(
        ids::RULES_PREVIEW_TITLE_PASTE,
        ids::RULES_PREVIEW_NOTE_PASTE,
    );
    scene
}
