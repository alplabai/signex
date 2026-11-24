//! The Via panel: the Not stored yet group (a microvia type adds the greyed
//! Microvia Stacked / Staggered row, Caner 2026-11-20; a through type has
//! none), the Position row and mm units, the Via Type face and span glyph,
//! the via stack and the Protection header (mockup row 96).

use pcb_geom::ItemRef;
use signex_engine::pcb::via_types::ViaTypeKey;
use signex_i18n::{ids, t};
use signex_types::pcb::ViaType;
use signex_types::pcb_layer::CopperLayer;
use uuid::Uuid;

use super::{definition, not_stored, via_stack};
use crate::app::Signex;
use crate::layout_probe::{LaidOutText, lay_out_texts, messages_on_click_at};
use crate::panels::PanelContext;
use crate::panels::pcb_primitive_properties::drawing::fixture;
use crate::panels::pcb_primitive_properties::rows::Look;
use crate::panels::pcb_primitive_properties::{ViaFacts, ViaTypeChoice};

const WINDOW: iced::Size = iced::Size::new(300.0, 2000.0);
/// The fixture type's name and spec, as the board's via types name them.
const TYPE_NAME: &str = "Thru 1:4";
const TYPE_SPEC: &str = "Through 1:4 · 0.6/0.3 · open";

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
        name: TYPE_NAME.to_string(),
        spec: TYPE_SPEC.to_string(),
        kind,
        label: format!("{TYPE_NAME} · {TYPE_SPEC}"),
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

/// The Via Stack section's laid-out texts, isolation marks stripped.
fn stack_texts(ctx: &PanelContext, f: &ViaFacts) -> Vec<String> {
    let section = via_stack(Look::of(ctx), f).expect("the via stack");
    lay_out_texts(section, WINDOW)
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
}

/// Mockup row 96: the type's Name is a sunken read-only value, not a field.
#[test]
fn the_type_name_is_a_sunken_value() {
    let ctx = context();
    let mut f = via(ViaType::Through, false);
    f.span = Some("Thru 1:2".to_string());
    let section = || definition(Look::of(&ctx), &f).expect("the definition");
    let texts: Vec<LaidOutText> = lay_out_texts(section(), WINDOW)
        .into_iter()
        .map(|t| LaidOutText {
            text: t.text.replace(['\u{2068}', '\u{2069}'], ""),
            ..t
        })
        .collect();
    let name = crate::layout_probe::find(&texts, "Thru 1:2").bounds;
    let pixels = crate::layout_probe::draw_pixels(section(), WINDOW);
    let at = ((name.center_y() as usize) * WINDOW.width as usize + (name.x - 2.0) as usize) * 4;
    let sunken = crate::panels::form_look::FormLook::of(&iced::Theme::Dark).sunken;
    for (got, want) in pixels[at..at + 3]
        .iter()
        .zip([sunken.r, sunken.g, sunken.b])
    {
        let want = (want * 255.0).round() as i32;
        assert!((i32::from(*got) - want).abs() <= 3, "{got} is not {want}");
    }
}

/// Caner, 2026-11-21: no Override switch; the row's note follows the value.
#[test]
fn the_via_stack_has_no_override_switch_and_a_changed_size_loses_from_type() {
    let ctx = context();
    let from_type = said(&ctx, ids::PCB_PRIM_FROM_TYPE);
    let count = |texts: &[String]| texts.iter().filter(|t| **t == from_type).count();

    let plain = via(ViaType::Through, false);
    let shown = stack_texts(&ctx, &plain);
    for gone in ["Override", "for this via"] {
        assert!(!shown.iter().any(|t| t == gone), "{gone} in {shown:?}");
    }
    assert_eq!(count(&shown), 2, "Diameter and Drill: {shown:?}");

    let mut own = via(ViaType::Through, false);
    own.diameter = Some(0.8);
    own.overridden = Some(true);
    let shown = stack_texts(&ctx, &own);
    assert_eq!(count(&shown), 1, "only Drill is the type's: {shown:?}");
}

/// #2751, mockup row 96: the stack tiles, the widest labels of any segmented
/// row, keep every label whole and apart at 220 and 300 px.
#[test]
fn the_stack_tile_labels_never_overlap() {
    use super::super::linked::{Segment, segments};
    let ctx = context();
    let tiles = [
        ids::PCB_PRIM_STACK_SIMPLE,
        ids::PCB_PRIM_STACK_TOP_MIDDLE_BOTTOM,
        ids::PCB_PRIM_STACK_FULL,
    ];
    let names = tiles.map(|id| said(&ctx, id));
    for dock in [220.0, 300.0] {
        let cells = tiles
            .iter()
            .map(|&id| Segment {
                label: t!(ctx.i18n, id),
                is_on: false,
                msg: None,
            })
            .collect();
        let laid: Vec<LaidOutText> = lay_out_texts(segments(cells), iced::Size::new(dock, 400.0))
            .into_iter()
            .map(|t| LaidOutText {
                text: t.text.replace(['\u{2068}', '\u{2069}'], ""),
                ..t
            })
            .collect();
        let at: Vec<iced::Rectangle> = names
            .iter()
            .map(|name| crate::layout_probe::find(&laid, name).bounds)
            .collect();
        for (i, a) in at.iter().enumerate() {
            let whole = crate::layout_probe::natural::<crate::panels::PanelMsg>(
                iced::Widget::boxed(
                    signex_widgets::text(signex_i18n::UserText::new(names[i].clone()))
                        .size(crate::styles::TypeScale::DEFAULT.caption),
                ),
                &names[i],
            );
            assert!(a.width + 1.0 >= whole.width, "{} cut: {a:?}", names[i]);
            for b in &at[i + 1..] {
                assert!(!a.intersects(b), "overlap at {dock} px: {a:?} {b:?}");
            }
        }
    }
}

/// #2754, mockup row 96: Via Stack folds under its chevron; a press on the
/// header reports its key, and folded it shows the header alone.
#[test]
fn the_via_stack_folds_under_its_header() {
    let mut ctx = context();
    let f = via(ViaType::Through, false);
    let title = ctx
        .i18n
        .upper_for_display(&said(&ctx, ids::PCB_PRIM_VIA_STACK));
    let open = stack_texts(&ctx, &f);
    assert!(open.len() > 1, "{open:?}");
    let at = lay_out_texts(via_stack(Look::of(&ctx), &f).expect("the stack"), WINDOW)
        .into_iter()
        .find(|t| t.text.replace(['\u{2068}', '\u{2069}'], "") == title)
        .unwrap_or_else(|| panic!("{title} in {open:?}"))
        .bounds
        .center();
    let section = via_stack(Look::of(&ctx), &f).expect("the stack");
    let sent = messages_on_click_at(section, WINDOW, None, at);
    assert!(
        sent.iter().any(|m| matches!(
            m,
            crate::panels::PanelMsg::ToggleSection(key) if key == "pcb_prim.via.stack"
        )),
        "{sent:?}"
    );
    ctx.collapsed_sections.insert("pcb_prim.via.stack".into());
    assert_eq!(stack_texts(&ctx, &f), [title]);
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

/// `section` laid out, isolation marks stripped.
fn plain_texts(section: iced::Element<'_, crate::panels::PanelMsg>) -> Vec<LaidOutText> {
    lay_out_texts(section, WINDOW)
        .into_iter()
        .map(|t| LaidOutText {
            text: t.text.replace(['\u{2068}', '\u{2069}'], ""),
            ..t
        })
        .collect()
}

/// The texts on the line of the label `label`, the label left out. The last
/// text that says it is the row's label (a layer chip above says it too).
fn on_line_of<'t>(texts: &'t [LaidOutText], label: &str) -> Vec<&'t str> {
    let row = texts
        .iter()
        .rev()
        .find(|t| t.text == label)
        .unwrap_or_else(|| panic!("no {label:?} in {texts:#?}"))
        .bounds;
    texts
        .iter()
        .filter(|t| t.text != label && (t.bounds.center_y() - row.center_y()).abs() < row.height)
        .map(|t| t.text.as_str())
        .collect()
}

/// Mockup row 96: Position marks its fields X and Y and ends in mm.
#[test]
fn the_position_row_marks_x_and_y_and_ends_in_mm() {
    let ctx = context();
    let mut f = via(ViaType::Through, false);
    f.x = Some(18.4);
    f.y = Some(27.15);
    let texts = plain_texts(definition(Look::of(&ctx), &f).expect("the definition"));
    let line = on_line_of(&texts, &said(&ctx, ids::PCB_PRIM_POSITION));
    for want in [ids::PCB_PRIM_X, ids::PCB_PRIM_Y, ids::PCB_PRIM_UNIT_MM] {
        let want = said(&ctx, want);
        assert!(line.contains(&want.as_str()), "{want} in {line:?}");
    }
}

/// Mockup row 96: Diameter, Drill and Finished Hole end in mm.
#[test]
fn diameter_drill_and_finished_hole_end_in_mm() {
    let ctx = context();
    let mut f = via(ViaType::Through, false);
    f.finished_hole = Some(0.25);
    let texts = plain_texts(via_stack(Look::of(&ctx), &f).expect("the via stack"));
    let mm = said(&ctx, ids::PCB_PRIM_UNIT_MM);
    for label in [
        ids::PCB_PRIM_DIAMETER,
        ids::PCB_PRIM_DRILL,
        ids::PCB_PRIM_FINISHED_HOLE,
    ] {
        let line = on_line_of(&texts, &said(&ctx, label));
        assert!(line.contains(&mm.as_str()), "{label:?}: {line:?}");
    }
}

/// Mockup row 96: a Full Stack via's diameter on each layer ends in mm too.
#[test]
fn each_layer_diameter_ends_in_mm() {
    use crate::panels::pcb_primitive_properties::LayerSizeRow;
    let ctx = context();
    let mut f = via(ViaType::Through, false);
    f.mode = Some(signex_types::pcb_stack::StackMode::Full);
    f.layer_sizes = [
        (CopperLayer::Top, "Top Layer"),
        (CopperLayer::Bottom, "Bottom Layer"),
    ]
    .into_iter()
    .map(|(layer, label)| LayerSizeRow {
        layer,
        label: label.to_string(),
        x: Some(0.6),
        y: None,
    })
    .collect();
    let texts = plain_texts(via_stack(Look::of(&ctx), &f).expect("the via stack"));
    let mm = said(&ctx, ids::PCB_PRIM_UNIT_MM);
    for label in ["Top Layer", "Bottom Layer"] {
        let line = on_line_of(&texts, label);
        assert!(line.contains(&mm.as_str()), "{label}: {line:?}");
    }
}

/// Mockup row 96: the closed Via Type field shows the type's name and its
/// diameter / drill on its own line, never the whole list entry cut short.
#[test]
fn the_via_type_field_shows_the_name_and_sizes_not_the_list_entry() {
    let ctx = context();
    let f = via(ViaType::Through, false);
    let texts = plain_texts(definition(Look::of(&ctx), &f).expect("the definition"));
    let line = on_line_of(&texts, &said(&ctx, ids::PCB_PRIM_VIA_TYPE));
    assert!(line.contains(&TYPE_NAME), "{line:?}");
    assert!(line.contains(&"0.6/0.3"), "{line:?}");
    let entry = f.via_type.as_ref().expect("a type").label.clone();
    assert!(!texts.iter().any(|t| t.text == entry), "{texts:?}");
}

/// Mockup row 96: the list under the face draws no label of its own, so the
/// picked entry never shows through the face: with a type picked it draws
/// the same pixels as with none.
#[test]
fn the_list_under_the_type_face_draws_no_label() {
    use crate::layout_probe::draw_pixels;
    let picked = via(ViaType::Through, false);
    let mut unpicked = via(ViaType::Through, false);
    unpicked.via_type = None;
    let size = iced::Size::new(300.0, 40.0);
    let drawn = draw_pixels(super::via_type_list(&picked), size);
    assert_eq!(drawn, draw_pixels(super::via_type_list(&unpicked), size));
}

/// Mockup row 96: the span glyph draws each copper layer in its colour.
#[test]
fn the_span_glyph_draws_the_copper_layers_in_their_colours() {
    use crate::panels::pcb_primitive_properties::LayerChoice;
    let ctx = context();
    let mut f = via(ViaType::Through, false);
    f.layers = [CopperLayer::Top, CopperLayer::Bottom]
        .into_iter()
        .map(|layer| LayerChoice {
            layer,
            label: String::new(),
        })
        .collect();
    let key = f.via_type.as_ref().expect("a type").key;
    let size = iced::Size::new(40.0, 40.0);
    let pixels =
        crate::layout_probe::draw_pixels(super::span_glyph(Look::of(&ctx), &f.layers, key), size);
    for layer in [CopperLayer::Top, CopperLayer::Bottom] {
        let colour = crate::pcb_canvas::layer_style::layer_colour(layer.into(), &ctx.tokens);
        let want = [colour.r, colour.g, colour.b].map(|c| (c * 255.0).round() as i32);
        let found = pixels.chunks(4).any(|p| {
            p.iter()
                .zip(want)
                .all(|(got, want)| (i32::from(*got) - want).abs() <= 3)
        });
        assert!(found, "no pixel of {layer:?} in the glyph");
    }
}

/// Mockup row 96: the Protection header names the class on a chip, with
/// its "from type" tag, on the header's own line.
#[test]
fn the_protection_header_names_the_class_on_a_chip() {
    use signex_types::pcb_via_protection::Ipc4761;
    let ctx = context();
    let mut f = via(ViaType::Through, false);
    f.protection = Ipc4761::IB.preset();
    let texts = plain_texts(super::protection(Look::of(&ctx), &f).expect("the protection"));
    let title = ctx
        .i18n
        .upper_for_display(&said(&ctx, ids::PCB_PRIM_PROTECTION));
    let line = on_line_of(&texts, &title);
    let chip = t!(ctx.i18n, ids::PCB_PRIM_IPC_CODE, "code" => "I-b")
        .into_string()
        .replace(['\u{2068}', '\u{2069}'], "");
    assert!(line.contains(&chip.as_str()), "{chip} in {line:?}");
    assert!(
        line.contains(&said(&ctx, ids::PCB_PRIM_FROM_TYPE).as_str()),
        "{line:?}"
    );
    let chips = texts.iter().filter(|t| t.text == chip).count();
    assert_eq!(chips, 1, "the chip shows once: {texts:?}");
}
