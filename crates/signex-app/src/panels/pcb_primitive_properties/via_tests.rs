//! The Via panel's Not stored yet group: a microvia type adds the greyed
//! Microvia Stacked / Staggered row (Caner, 2026-11-20); a through type
//! has no such row.

use pcb_geom::ItemRef;
use signex_engine::pcb::via_types::ViaTypeKey;
use signex_i18n::{ids, t};
use signex_types::pcb::ViaType;
use signex_types::pcb_layer::CopperLayer;
use uuid::Uuid;

use super::not_stored;
use crate::app::Signex;
use crate::layout_probe::{LaidOutText, lay_out_texts, messages_on_click_at};
use crate::panels::PanelContext;
use crate::panels::pcb_primitive_properties::drawing::fixture;
use crate::panels::pcb_primitive_properties::rows::Look;
use crate::panels::pcb_primitive_properties::{ViaFacts, ViaTypeChoice};

const WINDOW: iced::Size = iced::Size::new(300.0, 2000.0);

fn context() -> PanelContext {
    let (app, _boot) = Signex::new();
    app.document_state.panel_ctx.clone()
}

/// One via of a `kind` type, with the Not stored yet group open or not.
fn via(kind: ViaType, unstored_open: bool) -> ViaFacts {
    let mut common = fixture::common(vec![ItemRef::Via(Uuid::from_u128(1))]);
    common.unstored_open = unstored_open;
    let choice = ViaTypeChoice {
        key: ViaTypeKey {
            kind,
            top: CopperLayer::Top,
            bottom: CopperLayer::Bottom,
            tented_top: false,
            tented_bottom: false,
        },
        name: String::new(),
        spec: String::new(),
        kind,
        label: String::new(),
    };
    ViaFacts {
        common,
        x: None,
        y: None,
        diameter: Some(0.6),
        hole: Some(0.3),
        span: None,
        span_ends: (None, None),
        layers: Vec::new(),
        locked: Some(false),
        mode: None,
        layer_sizes: Vec::new(),
        top: Default::default(),
        bottom: Default::default(),
        from_hole_edge: None,
        types: vec![choice.clone()],
        via_type: Some(choice),
        type_size: Some((0.6, 0.3)),
        overridden: Some(false),
        override_open: false,
        protection: None,
        drill_pair: None,
        finished_hole: None,
        min_drill: None,
        reaches: (true, true),
        view_layer: None,
    }
}

/// The group's laid-out texts, isolation marks stripped. A greyed value is
/// veiled by `Inert`, which hides its texts from the probe: only labels and
/// the header are listed.
fn laid_out(ctx: &PanelContext, f: &ViaFacts) -> Vec<LaidOutText> {
    let group = not_stored(Look::of(ctx), f).expect("the via has rows not stored yet");
    lay_out_texts(group, WINDOW)
        .into_iter()
        .map(|t| LaidOutText {
            text: t.text.replace(['\u{2068}', '\u{2069}'], ""),
            ..t
        })
        .collect()
}

fn texts(ctx: &PanelContext, f: &ViaFacts) -> Vec<String> {
    laid_out(ctx, f).into_iter().map(|t| t.text).collect()
}

fn said(ctx: &PanelContext, id: signex_i18n::MessageId) -> String {
    t!(ctx.i18n, id)
        .into_string()
        .replace(['\u{2068}', '\u{2069}'], "")
}

fn header(ctx: &PanelContext, count: usize) -> String {
    t!(ctx.i18n, ids::PCB_PRIM_UNSTORED, "count" => count)
        .into_string()
        .replace(['\u{2068}', '\u{2069}'], "")
}

#[test]
fn a_microvia_type_adds_the_greyed_microvia_row_to_the_not_stored_yet_group() {
    let ctx = context();
    let micro = via(ViaType::Micro, true);
    let shown = laid_out(&ctx, &micro);
    let names: Vec<&str> = shown.iter().map(|t| t.text.as_str()).collect();
    assert_eq!(names.first(), Some(&header(&ctx, 5).as_str()), "{names:?}");
    let label = said(&ctx, ids::PCB_PRIM_MICROVIA);
    let row = shown
        .iter()
        .find(|t| t.text == label)
        .unwrap_or_else(|| panic!("{label} in {names:?}"));
    // The Stacked and Staggered cells share the value column (the right
    // six tenths of the row): a click on either sends nothing.
    for x in [0.55, 0.85] {
        let at = iced::Point::new(WINDOW.width * x, row.bounds.center_y());
        let group = not_stored(Look::of(&ctx), &micro).expect("the group");
        let sent = messages_on_click_at(group, WINDOW, None, at);
        assert!(sent.is_empty(), "a click at {at:?} sends nothing: {sent:?}");
    }

    let closed = texts(&ctx, &via(ViaType::Micro, false));
    assert_eq!(
        closed,
        vec![header(&ctx, 5)],
        "a closed group shows its header"
    );
}

#[test]
fn a_through_type_has_no_microvia_row() {
    let ctx = context();
    let texts = texts(&ctx, &via(ViaType::Through, true));
    assert_eq!(texts.first(), Some(&header(&ctx, 4)), "{texts:?}");
    assert!(
        !texts.contains(&said(&ctx, ids::PCB_PRIM_MICROVIA)),
        "{texts:?}"
    );
}
