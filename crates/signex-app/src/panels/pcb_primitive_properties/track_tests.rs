//! Mockup row 95 (#2808): the Track panel has no Position row (the location
//! is the header's lock), its Start and End each keep their X and Y on one
//! row behind "X" / "Y" prefixes, and a value the selected tracks do not
//! share reads "Mixed" in italics.

use pcb_geom::ItemRef;
use signex_i18n::{MessageId, ids, t};
use uuid::Uuid;

use super::properties;
use crate::app::Signex;
use crate::layout_probe::{LaidOutText, lay_out_inputs, lay_out_texts};
use crate::panels::PanelContext;
use crate::panels::pcb_primitive_properties::TrackFacts;
use crate::panels::pcb_primitive_properties::drawing::fixture;
use crate::panels::pcb_primitive_properties::rows::{Look, input_font};

/// The default dock width.
const DOCK: iced::Size = iced::Size::new(300.0, 2000.0);

fn context() -> PanelContext {
    let (app, _boot) = Signex::new();
    app.document_view.panel_ctx.clone()
}

/// `count` tracks; one track has its own ends, several share none.
fn tracks(count: u128) -> TrackFacts {
    let items = (1..=count)
        .map(|i| ItemRef::Track(Uuid::from_u128(i)))
        .collect();
    let alone = count == 1;
    let at = |x: f64, y: f64| {
        if alone {
            (Some(x), Some(y))
        } else {
            (None, None)
        }
    };
    TrackFacts {
        common: fixture::common(items),
        location: alone.then_some((1.25, 2.5)),
        layer: None,
        layers: Vec::new(),
        width: Some(0.25),
        start: at(1.25, 2.5),
        end: at(7.75, 2.5),
        length_mm: alone.then_some(6.5),
        locked: Some(false),
        paste_mask: None,
        solder_mask: None,
    }
}

fn plain(text: &str) -> String {
    text.replace(['\u{2068}', '\u{2069}'], "")
}

fn said(ctx: &PanelContext, id: MessageId) -> String {
    plain(&t!(ctx.i18n, id).into_string())
}

fn strip(laid: Vec<LaidOutText>) -> Vec<LaidOutText> {
    laid.into_iter()
        .map(|t| LaidOutText {
            text: plain(&t.text),
            ..t
        })
        .collect()
}

/// Whether `at` sits on the row of `label`.
fn on_row(at: &iced::Rectangle, label: &iced::Rectangle) -> bool {
    (at.center_y() - label.center_y()).abs() < label.height
}

/// No Position row; each of Start and End: its label once, then on its row
/// two fields, one "X" and one "Y" prefix.
#[test]
fn position_start_and_end_each_keep_x_and_y_on_one_row() {
    let ctx = context();
    let f = tracks(1);
    let texts = strip(lay_out_texts(
        properties(Look::of(&ctx), &f).expect("the properties"),
        DOCK,
    ));
    let inputs = lay_out_inputs(
        properties(Look::of(&ctx), &f).expect("the properties"),
        DOCK,
    );
    let (x, y) = (said(&ctx, ids::PCB_PRIM_X), said(&ctx, ids::PCB_PRIM_Y));
    let position = said(&ctx, ids::PCB_PRIM_POSITION);
    assert!(
        texts.iter().all(|t| t.text != position),
        "no {position} row: {texts:#?}"
    );
    let mut tops = Vec::new();
    for id in [ids::PCB_PRIM_POINT_START, ids::PCB_PRIM_POINT_END] {
        let name = said(&ctx, id);
        let labels: Vec<&LaidOutText> = texts.iter().filter(|t| t.text == name).collect();
        assert_eq!(labels.len(), 1, "one {name} row: {texts:#?}");
        let label = labels[0].bounds;
        let fields: Vec<_> = inputs
            .iter()
            .filter(|i| on_row(&i.bounds, &label))
            .collect();
        assert_eq!(fields.len(), 2, "{name}: X and Y on its row: {fields:?}");
        assert!(
            fields[0].bounds.x < fields[1].bounds.x,
            "{name}: {fields:?}"
        );
        for prefix in [&x, &y] {
            let shown = texts
                .iter()
                .filter(|t| t.text == *prefix && on_row(&t.bounds, &label))
                .count();
            assert_eq!(shown, 1, "{name}: one {prefix} prefix");
        }
        tops.push(label.y);
    }
    assert!(
        tops.windows(2).all(|w| w[0] < w[1]),
        "Start, End in order: {tops:?}"
    );
}

/// Two tracks with different ends: every Start and End half reads "Mixed",
/// and a Mixed value is set in italics; one track shows its own numbers.
#[test]
fn ends_the_tracks_do_not_share_read_mixed_in_italics() {
    let ctx = context();
    let mixed = said(&ctx, ids::PCB_PRIM_MIXED);
    let shown = |f: &TrackFacts| -> Vec<String> {
        lay_out_inputs(properties(Look::of(&ctx), f).expect("the properties"), DOCK)
            .into_iter()
            .map(|i| plain(&i.text))
            .collect()
    };

    let two = shown(&tracks(2));
    // Start and End: four halves, all Mixed (no Position row, mockup row 95).
    assert_eq!(two.iter().filter(|t| **t == mixed).count(), 4, "{two:?}");
    let one = shown(&tracks(1));
    assert!(!one.contains(&mixed), "{one:?}");

    let font = input_font(true).expect("a Mixed face");
    assert_eq!(font.style, iced::font::Style::Italic);
    assert_eq!(
        input_font(false),
        None,
        "a shared value keeps the default face"
    );
}
