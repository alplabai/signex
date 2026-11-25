//! Mockup rows 95 and 98: Start and End each keep X and Y on one row at the
//! default 300 px dock, with room for the value, and stack at a narrow
//! dock. The track panel has no Position row, track and arc lengths read
//! to three decimals (the track's mask Expansion too) and arc angles to
//! one, Square and Flat ends are faded
//! as disabled, No Class sits where the class link would, and Show More
//! carries the net's counts on its line.

use iced::{Size, Theme, Widget as _};
use pcb_geom::ItemRef;
use signex_i18n::{UserText, ids};
use signex_widgets::property_illustration::Cap;
use signex_widgets::text;
use uuid::Uuid;

use super::drawing::fixture;
use super::linked::{prefixed, segment_style, two_fields, unit};
use super::rows::{Look, fixed_degrees, fixed_mm, labelled};
use super::{ArcFacts, NetInfo, PrimitiveField as F, TS, TrackFacts, end_choices};
use crate::app::Signex;
use crate::layout_probe::{LaidOutText, lay_out_inputs, lay_out_texts, natural};
use crate::panels::{PanelContext, PanelMsg};

const VALUE: &str = "12.704";
/// The row width inside a 300 px dock, its scrollbar taken off.
const DEFAULT_ROW: f32 = 288.0;
const NARROW_ROW: f32 = 220.0;

fn inputs(ctx: &PanelContext, width: f32) -> Vec<iced::Rectangle> {
    let look = Look::of(ctx);
    let common = fixture::common(vec![ItemRef::Via(Uuid::from_u128(1))]);
    let row = labelled(
        look,
        ids::PCB_PRIM_POINT_START,
        two_fields(
            prefixed(look, &common, F::StartX, Some(12.704), ids::PCB_PRIM_X),
            prefixed(look, &common, F::StartY, Some(12.704), ids::PCB_PRIM_Y),
            unit(look, ids::PCB_PRIM_UNIT_MM),
        ),
    );
    lay_out_inputs(row, Size::new(width, 200.0))
        .into_iter()
        .map(|input| input.bounds)
        .collect()
}

#[test]
fn x_and_y_share_a_row_at_the_default_dock_and_stack_when_narrow() {
    let (app, _boot) = Signex::new();
    let ctx = app.document_state.panel_ctx.clone();
    let value = natural::<PanelMsg>(text(UserText::new(VALUE)).size(TS.body).boxed(), VALUE);

    let wide = inputs(&ctx, DEFAULT_ROW);
    assert_eq!(wide.len(), 2, "{wide:?}");
    assert_eq!(wide[0].y, wide[1].y, "one row: {wide:?}");
    for field in &wide {
        assert!(field.width >= value.width, "{field:?} for {value:?}");
    }

    let narrow = inputs(&ctx, NARROW_ROW);
    assert!(narrow[1].y > narrow[0].y, "stacked: {narrow:?}");
}

/// A dock tall enough for a whole panel.
const PANEL: Size = Size::new(300.0, 4000.0);

fn context() -> PanelContext {
    let (app, _boot) = Signex::new();
    app.document_state.panel_ctx.clone()
}

/// `laid` with the isolation marks round message arguments stripped.
fn plain(laid: Vec<LaidOutText>) -> Vec<LaidOutText> {
    laid.into_iter()
        .map(|t| LaidOutText {
            text: t.text.replace(['\u{2068}', '\u{2069}'], ""),
            ..t
        })
        .collect()
}

fn shown(laid: &[LaidOutText]) -> Vec<&str> {
    laid.iter().map(|t| t.text.as_str()).collect()
}

/// One placed track on net GND with no class.
fn track() -> TrackFacts {
    let mut common = fixture::common(vec![ItemRef::Track(Uuid::from_u128(1))]);
    common.net_info = NetInfo {
        net: Some(3),
        name: Some("GND".into()),
        class: None,
        selected_length_mm: Some(10.0),
        total_length_mm: Some(38.9049),
        tracks: 8,
        vias: 2,
        pads: 4,
    };
    TrackFacts {
        common,
        location: Some((1.0, 2.0)),
        layer: None,
        layers: Vec::new(),
        width: Some(0.2),
        start: (Some(1.0), Some(2.0)),
        end: (Some(13.704), Some(2.0)),
        length_mm: Some(12.704),
        locked: Some(false),
        paste_mask: None,
        solder_mask: None,
    }
}

#[test]
fn the_track_panel_has_start_and_end_but_no_position_row() {
    let ctx = context();
    let texts = plain(lay_out_texts(super::track::view(&ctx, &track()), PANEL));
    let labels = shown(&texts);
    assert!(!labels.contains(&"Position"), "{labels:?}");
    assert!(labels.contains(&"Start"), "{labels:?}");
    assert!(labels.contains(&"End"), "{labels:?}");
}

#[test]
fn track_lengths_read_to_three_decimals() {
    let ctx = context();
    let f = track();
    let inputs = plain(lay_out_inputs(super::track::view(&ctx, &f), PANEL));
    let values = shown(&inputs);
    for value in ["0.200", "1.000", "2.000", "13.704"] {
        assert!(values.contains(&value), "{value} in {values:?}");
    }
    let texts = plain(lay_out_texts(super::track::view(&ctx, &f), PANEL));
    let labels = shown(&texts);
    for value in ["12.704", "10.000 mm", "38.905 mm"] {
        assert!(labels.contains(&value), "{value} in {labels:?}");
    }
}

#[test]
fn no_class_sits_where_the_class_link_would_and_not_in_a_field() {
    let ctx = context();
    let look = Look::of(&ctx);
    // The track view passes its layer dot; the Class row has none.
    let dot = Some(iced::Color::WHITE);
    let class_x = |f: &TrackFacts, shown: &str| {
        let section =
            super::net_info::open_section(look, &f.common, dot).expect("Net Information shows");
        let texts = plain(lay_out_texts(section, PANEL));
        crate::layout_probe::find(&texts, shown).bounds.x
    };
    let unclassed = track();
    let mut classed = track();
    classed.common.net_info.class = Some("HighSpeed".into());
    assert_eq!(
        class_x(&unclassed, "No Class"),
        class_x(&classed, "HighSpeed")
    );
}

#[test]
fn the_track_mask_expansion_reads_to_three_decimals() {
    use signex_types::pcb::MaskExpansion;
    let ctx = context();
    let mut f = track();
    f.solder_mask = Some(MaskExpansion::Manual { mm: 0.05 });
    let inputs = plain(lay_out_inputs(super::track::view(&ctx, &f), PANEL));
    assert!(shown(&inputs).contains(&"0.050"), "{inputs:?}");
}

#[test]
fn show_more_carries_the_nets_counts_before_it_is_pressed() {
    let ctx = context();
    let f = track();
    assert!(!f.common.show_more);
    let section = super::net_info::open_section(Look::of(&ctx), &f.common, None)
        .expect("Net Information shows");
    let texts = plain(lay_out_texts(section, PANEL));
    let toggle = crate::layout_probe::find(&texts, "Show More");
    let counts = crate::layout_probe::find(&texts, "· 8 tracks, 2 vias, 4 pads on net");
    assert!(counts.bounds.x > toggle.bounds.x, "{toggle:?} {counts:?}");
    assert!(
        (counts.bounds.center_y() - toggle.bounds.center_y()).abs() < 1.0,
        "one line: {toggle:?} {counts:?}"
    );
}

/// One placed copper arc, as mockup row 98 draws it.
fn arc() -> ArcFacts {
    ArcFacts {
        common: fixture::common(vec![ItemRef::Arc(Uuid::from_u128(1))]),
        is_copper: true,
        layer: None,
        layers: Vec::new(),
        center: (Some(8.0), Some(33.5)),
        radius: Some(2.0),
        start_angle: Some(0.0),
        end_angle: Some(90.0),
        width: Some(0.5),
        length_mm: Some(std::f64::consts::PI),
        mode: None,
    }
}

#[test]
fn arc_lengths_read_to_three_decimals_and_angles_to_one() {
    let ctx = context();
    let f = arc();
    let inputs = plain(lay_out_inputs(super::arc::view(&ctx, &f), PANEL));
    let values = shown(&inputs);
    for value in ["0.500", "8.000", "33.500", "2.000", "0.0", "90.0"] {
        assert!(values.contains(&value), "{value} in {values:?}");
    }
    let texts = plain(lay_out_texts(super::arc::view(&ctx, &f), PANEL));
    assert!(shown(&texts).contains(&"3.142"), "{texts:?}");
}

#[test]
fn a_value_that_rounds_to_zero_reads_without_a_minus_sign() {
    assert_eq!(fixed_mm(-0.0001), "0.000");
    assert_eq!(fixed_mm(-1.25), "-1.250");
    assert_eq!(fixed_degrees(-0.01), "0.0");
}

#[test]
fn square_and_flat_ends_are_faded_as_disabled_controls() {
    use iced::widget::button::Status;
    let disabled: Vec<Cap> = end_choices()
        .into_iter()
        .filter(|c| !c.is_enabled)
        .map(|c| c.cap)
        .collect();
    assert_eq!(disabled, [Cap::Square, Cap::Flat]);
    for theme in [Theme::Dark, Theme::Light] {
        let faded = segment_style(&theme, false, false, true, Status::Active);
        let inert = segment_style(&theme, false, false, false, Status::Active);
        let round = segment_style(&theme, true, false, false, Status::Active);
        assert!(
            faded.text_color.a <= crate::styles::DISABLED_OPACITY + f32::EPSILON,
            "{faded:?}"
        );
        assert!(
            faded.text_color.a < inert.text_color.a,
            "{faded:?} {inert:?}"
        );
        assert!(
            round.text_color.a > faded.text_color.a,
            "Round stays chosen: {round:?}"
        );
    }
}
