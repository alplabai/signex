//! The Pad panel (mockup row 97): the read-only Layer, sunken with a round
//! layer dot before the name, and the Paste and Solder mask rows, live
//! while their switches allow editing.

use iced::Size;
use pcb_geom::ItemRef;
use signex_i18n::{MessageId, ids, t};
use signex_types::pcb_layer::{CopperLayer, PcbLayer};
use uuid::Uuid;

use super::{layer_value, paste, solder};
use crate::app::Signex;
use crate::layout_probe::{draw_pixels, find, lay_out_inputs, lay_out_texts, messages_on_click};
use crate::panels::PanelContext;
use crate::panels::form_look::FormLook;
use crate::panels::pcb_primitive_properties::PadFacts;
use crate::panels::pcb_primitive_properties::drawing::fixture;
use crate::panels::pcb_primitive_properties::rows::Look;
use crate::pcb_canvas::layer_style;

const WINDOW: Size = Size::new(160.0, 40.0);
const NAME: &str = "Top Layer";

/// The RGB of the pixel at (`x`, `y`) of an RGBA image `WINDOW` wide.
fn pixel(pixels: &[u8], x: f32, y: f32) -> [i32; 3] {
    let at = (y as usize * WINDOW.width as usize + x as usize) * 4;
    [
        pixels[at].into(),
        pixels[at + 1].into(),
        pixels[at + 2].into(),
    ]
}

fn rgb(c: iced::Color) -> [i32; 3] {
    [c.r, c.g, c.b].map(|v| (v * 255.0).round() as i32)
}

fn assert_close(got: [i32; 3], want: [i32; 3], what: &str) {
    let is_close = got.iter().zip(want).all(|(g, w)| (g - w).abs() <= 3);
    assert!(is_close, "{what}: {got:?} is not {want:?}");
}

#[test]
fn the_layer_is_sunken_with_a_round_dot_and_takes_no_typing() {
    let (app, _boot) = Signex::new();
    let ctx = app.document_state.panel_ctx.clone();
    let look = Look::of(&ctx);
    let layer = PcbLayer::Copper(CopperLayer::Top);
    let shown = (NAME.to_string(), Some(layer));

    assert!(lay_out_inputs(layer_value(look, Some(&shown)), WINDOW).is_empty());
    let texts: Vec<_> = lay_out_texts(layer_value(look, Some(&shown)), WINDOW)
        .into_iter()
        .map(|t| crate::layout_probe::LaidOutText {
            text: t.text.replace(['\u{2068}', '\u{2069}'], ""),
            ..t
        })
        .collect();
    let name = find(&texts, NAME).bounds;
    let pixels = draw_pixels(layer_value(look, Some(&shown)), WINDOW);

    // The fill right of the name is the sunken one the probe's theme gives.
    let sunken = FormLook::of(&iced::Theme::Dark).sunken;
    assert_close(
        pixel(&pixels, name.x + name.width + 6.0, name.center_y()),
        rgb(sunken),
        "the value's fill",
    );
    // The dot sits between the box's edge and the name, in the layer colour.
    let dot = layer_style::layer_colour(layer, look.tokens());
    assert_close(
        pixel(&pixels, name.x / 2.0 + 1.0, name.center_y()),
        rgb(dot),
        "the layer dot",
    );
}

const MASK_WINDOW: Size = Size::new(300.0, 400.0);

fn context() -> PanelContext {
    let (app, _boot) = Signex::new();
    app.document_state.panel_ctx.clone()
}

/// One pad, surface or through-hole.
fn pad(has_hole: bool) -> PadFacts {
    PadFacts {
        common: fixture::common(vec![ItemRef::Pad {
            footprint: Uuid::from_u128(1),
            pad: Uuid::from_u128(2),
        }]),
        designator: Some("49".into()),
        layer: None,
        x: None,
        y: None,
        rotation: None,
        size_x: Some(5.6),
        size_y: Some(5.6),
        hole: has_hole.then_some(1.0),
        has_hole,
        shape: None,
        shapes: Vec::new(),
        mode: None,
        layer_sizes: Vec::new(),
        component_designator: Some("U3".into()),
        plated: has_hole.then_some(true),
    }
}

fn said(ctx: &PanelContext, id: MessageId) -> String {
    t!(ctx.i18n, id)
        .into_string()
        .replace(['\u{2068}', '\u{2069}'], "")
}

/// The laid-out texts of a subsection, isolation marks stripped. A greyed
/// row is veiled by `Inert`, which hides its texts from the probe.
fn texts(element: iced::Element<'_, crate::panels::PanelMsg>) -> Vec<String> {
    lay_out_texts(element, MASK_WINDOW)
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
}

fn count(shown: &[String], label: &str) -> usize {
    shown.iter().filter(|t| *t == label).count()
}

#[test]
fn a_surface_pads_paste_source_is_live_with_rule_chosen_and_manual_inert() {
    let ctx = context();
    let look = Look::of(&ctx);
    let f = pad(false);
    let shown = texts(paste(look, &f));
    let rule = said(&ctx, ids::PCB_PRIM_RULE);
    let manual = said(&ctx, ids::PCB_PRIM_MANUAL);
    // Source shows both segments; the Expansion value ("Rule") stays greyed
    // while the rule decides, so only the segment's "Rule" is laid out.
    assert_eq!(count(&shown, &rule), 1, "{shown:?}");
    assert_eq!(count(&shown, &manual), 1, "{shown:?}");
    assert_eq!(
        count(&shown, &said(&ctx, ids::PCB_PRIM_EXPANSION)),
        1,
        "{shown:?}"
    );
    // The board stores no expansion per pad: Manual sends no edit. The
    // pointer entering the row lights its callout, which is not an edit.
    let sent = messages_on_click(paste(look, &f), MASK_WINDOW, &manual);
    let edits: Vec<_> = sent
        .iter()
        .filter(|m| {
            !matches!(
                m,
                crate::panels::PanelMsg::PcbPrimitive(
                    crate::panels::pcb_primitive_properties::PcbPrimitiveMsg::IllustrationHovered(
                        _
                    )
                )
            )
        })
        .collect();
    assert!(edits.is_empty(), "Manual sends no edit: {sent:?}");
}

#[test]
fn a_through_hole_pads_paste_rows_stay_greyed_under_its_unticked_switch() {
    let ctx = context();
    let look = Look::of(&ctx);
    let f = pad(true);
    let shown = texts(paste(look, &f));
    assert_eq!(
        count(&shown, &said(&ctx, ids::PCB_PRIM_RULE)),
        0,
        "{shown:?}"
    );
    assert_eq!(
        count(&shown, &said(&ctx, ids::PCB_PRIM_MANUAL)),
        0,
        "{shown:?}"
    );
}

#[test]
fn the_solder_source_is_live_while_the_pad_is_not_tented() {
    let ctx = context();
    let look = Look::of(&ctx);
    for has_hole in [false, true] {
        let f = pad(has_hole);
        let shown = texts(solder(look, &f));
        assert_eq!(
            count(&shown, &said(&ctx, ids::PCB_PRIM_RULE)),
            1,
            "{shown:?}"
        );
        assert_eq!(
            count(&shown, &said(&ctx, ids::PCB_PRIM_MANUAL)),
            1,
            "{shown:?}"
        );
    }
}
