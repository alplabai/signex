//! Mockup row 124 (#3008): the header notes of Courtyard & Silkscreen sit
//! beside their labels, never over them, and the Snap Distance label of the
//! snap illustration stands clear of the circle and the cursor.

use iced::{Point, Rectangle};
use signex_widgets::property_illustration::{Illustration, Leader, Primitive};

use super::fields;
use super::tests::{app_with_eight_pads, panel_ctx, texts};
use crate::app::{Message, Signex};
use crate::layout_probe::LaidOutText;
use crate::library::editor::footprint::SnappingMode;
use crate::panels::{FootprintEditorPanelContext, FootprintFeatureEdit, PanelMsg};

const NO_BODY: &str = "needs a 3D body or fab outline";

/// One Properties panel message, sent through `Signex::update`.
fn panel(app: &mut Signex, msg: PanelMsg) {
    drop(app.update(Message::Dock(crate::dock::DockMessage::Panel(msg))));
}

fn bounds_of(texts: &[LaidOutText], label: &str) -> Rectangle {
    crate::layout_probe::find(texts, label).bounds
}

/// Every note `note` lies right of `label`, shares its line and does not
/// cover it.
fn assert_beside(texts: &[LaidOutText], label: &str, note: &str) {
    let label_at = bounds_of(texts, label);
    let notes: Vec<Rectangle> = texts
        .iter()
        .filter(|t| t.text == note)
        .map(|t| t.bounds)
        .collect();
    assert!(!notes.is_empty(), "{note:?} missing: {texts:#?}");
    let beside = notes
        .iter()
        .find(|n| n.y < label_at.y + label_at.height && label_at.y < n.y + n.height);
    let beside = beside.unwrap_or_else(|| panic!("no {note:?} on the {label:?} line"));
    assert!(
        beside.x >= label_at.x + label_at.width,
        "{note:?} {beside:?} starts inside {label:?} {label_at:?}"
    );
    for n in &notes {
        assert!(
            label_at.intersection(n).is_none(),
            "{note:?} {n:?} covers {label:?} {label_at:?}"
        );
    }
}

/// GUI path: both outlines switched on through `Signex::update`, with no
/// body, say what they need beside their labels at the dock width.
#[test]
fn the_outline_reasons_sit_beside_their_labels() {
    let mut app = app_with_eight_pads();
    panel(
        &mut app,
        PanelMsg::FpEditorGeneratedFeature(FootprintFeatureEdit::SilkOutlineToggled),
    );
    panel(
        &mut app,
        PanelMsg::FpEditorGeneratedFeature(FootprintFeatureEdit::AssemblyOutlineToggled),
    );
    let fp = panel_ctx(&app);
    assert!(fp.generated.silk_outline && fp.generated.assembly_outline);
    assert_eq!(fp.body_extent_mm, None);
    let shown = texts(&app, &fp);
    assert_beside(&shown, "Silkscreen outline", NO_BODY);
    assert_beside(&shown, "Assembly outline", NO_BODY);
    // A wrapped reason pushes the fields down rather than covering them.
    let silk = bounds_of(&shown, "Silkscreen outline");
    let first_field = shown
        .iter()
        .filter(|t| t.text == "Line width" && t.bounds.y > silk.y)
        .map(|t| t.bounds.y)
        .fold(f32::INFINITY, f32::min);
    for note in shown.iter().filter(|t| t.text == NO_BODY) {
        if note.bounds.y < first_field {
            assert!(
                note.bounds.y + note.bounds.height <= first_field,
                "{note:?}"
            );
        }
    }
}

/// Headless: the courtyard size and the body size sit beside their labels.
#[test]
fn the_courtyard_and_body_sizes_sit_beside_their_labels() {
    let app = app_with_eight_pads();
    let mut fp: FootprintEditorPanelContext = panel_ctx(&app);
    fp.auto_fit_courtyard = true;
    fp.courtyard_extent_mm = Some([-3.5, -2.7, 3.5, 2.7]);
    fp.generated.silk_outline = true;
    fp.generated.assembly_outline = true;
    fp.body_extent_mm = Some([-1.95, -2.45, 1.95, 2.45]);
    let shown = texts(&app, &fp);
    assert_beside(&shown, "Courtyard (generated)", "7.00 × 5.40 mm");
    assert_beside(&shown, "Silkscreen outline", "clipped at pads");
    assert_beside(&shown, "Assembly outline", "body 3.90 × 4.90");
}

/// The Snap Distance leader and the circle it names.
fn snap_leader(drawing: &Illustration) -> Option<(Leader, Point, f32)> {
    let named = |p: &&signex_widgets::property_illustration::Part| {
        p.fields.contains(&fields::SNAP_DISTANCE)
    };
    let leader = drawing
        .visible_parts()
        .filter(named)
        .find_map(|p| match &p.primitive {
            Primitive::Leader(leader) => Some(leader.clone()),
            _ => None,
        })?;
    let (center, radius) =
        drawing
            .visible_parts()
            .filter(named)
            .find_map(|p| match p.primitive {
                Primitive::Circle { center, radius } => Some((center, radius)),
                _ => None,
            })?;
    Some((leader, center, radius))
}

fn assert_label_clears_the_circle(fp: &FootprintEditorPanelContext) {
    let i18n = signex_i18n::I18n::en_us();
    let drawing = super::snap_illustration::illustration(fp, &i18n);
    let (leader, center, radius) = snap_leader(&drawing).expect("the snap leader");
    assert!(
        (leader.at.distance(center) - radius).abs() < 1e-3,
        "the leader starts on the circle: {leader:?}"
    );
    assert!(
        leader.label_at.x > center.x + radius && leader.label_at.y < center.y - radius,
        "the label clears the circle: {leader:?} {center:?} r {radius}"
    );
    assert!(leader.label_at.y > 0.0, "inside the drawing: {leader:?}");
}

/// Headless: at the default and at the largest drawn snap distance the
/// label is right of and above the circle.
#[test]
fn the_snap_distance_label_clears_the_circle() {
    let app = app_with_eight_pads();
    let mut fp = panel_ctx(&app);
    fp.snapping_mode = SnappingMode::AllLayers;
    assert_label_clears_the_circle(&fp);
    fp.snap_options.snap_distance_mm = 100.0;
    assert_label_clears_the_circle(&fp);
}

/// GUI path: a snap distance typed through `Signex::update` keeps the label
/// clear; with snapping Off the leader is gone with the circle.
#[test]
fn a_typed_snap_distance_keeps_the_label_clear() {
    let mut app = app_with_eight_pads();
    panel(
        &mut app,
        PanelMsg::FpEditorSetSnappingMode(SnappingMode::AllLayers),
    );
    panel(&mut app, PanelMsg::FpEditorSetSnapDistance("50".into()));
    let fp = panel_ctx(&app);
    assert_eq!(fp.snapping_mode, SnappingMode::AllLayers);
    assert_label_clears_the_circle(&fp);

    panel(
        &mut app,
        PanelMsg::FpEditorSetSnappingMode(SnappingMode::Off),
    );
    let fp = panel_ctx(&app);
    let i18n = signex_i18n::I18n::en_us();
    let drawing = super::snap_illustration::illustration(&fp, &i18n);
    assert!(snap_leader(&drawing).is_none());
}
