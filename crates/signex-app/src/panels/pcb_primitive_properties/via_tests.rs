//! The Via panel's Not stored yet group: a microvia type adds the greyed
//! Microvia Stacked / Staggered row (Caner, 2026-11-20); a through type
//! has no such row.

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

/// Mockup row 96 (#2808): Name, Span and Drill Pair are the type's, read
/// only: each value sits in a sunken box, takes no typing, and is tagged
/// "from type" on its row.
#[test]
fn name_span_and_drill_pair_are_sunken_read_only_and_from_type() {
    use super::super::LayerChoice;
    let ctx = context();
    let mut f = via(ViaType::Through, false);
    f.span = Some("Thru 1:2".to_string());
    f.span_ends = (
        Some(LayerChoice {
            layer: CopperLayer::Top,
            label: "Top Layer".to_string(),
        }),
        Some(LayerChoice {
            layer: CopperLayer::Bottom,
            label: "Bottom Layer".to_string(),
        }),
    );
    f.drill_pair = Some("Top Layer - Bottom Layer".to_string());
    let span = plain(
        &t!(ctx.i18n, ids::PCB_PRIM_SPAN_ENDS, "from" => "Top Layer".to_string(), "to" => "Bottom Layer".to_string())
            .into_string(),
    );
    let section = || definition(Look::of(&ctx), &f).expect("the definition");
    let texts: Vec<LaidOutText> = lay_out_texts(section(), WINDOW)
        .into_iter()
        .map(|t| LaidOutText {
            text: plain(&t.text),
            ..t
        })
        .collect();
    let inputs = crate::layout_probe::lay_out_inputs(section(), WINDOW);
    let pixels = crate::layout_probe::draw_pixels(section(), WINDOW);
    let sunken = crate::panels::form_look::FormLook::of(&iced::Theme::Dark).sunken;
    let from_type = said(&ctx, ids::PCB_PRIM_FROM_TYPE);
    let on_row = |at: &iced::Rectangle, label: &iced::Rectangle| {
        (at.center_y() - label.center_y()).abs() < label.height
    };
    for (id, value) in [
        (ids::PCB_PRIM_VIA_NAME, "Thru 1:2"),
        (ids::PCB_PRIM_SPAN, span.as_str()),
        (ids::PCB_PRIM_DRILL_PAIR, "Top Layer - Bottom Layer"),
    ] {
        let name = said(&ctx, id);
        let label = crate::layout_probe::find(&texts, &name).bounds;
        let shown = crate::layout_probe::find(&texts, value).bounds;
        assert!(on_row(&shown, &label), "{name}: {shown:?} vs {label:?}");
        let tags = texts
            .iter()
            .filter(|t| t.text == from_type && on_row(&t.bounds, &label))
            .count();
        assert_eq!(tags, 1, "{name} is tagged {from_type}");
        assert!(
            !inputs.iter().any(|i| on_row(&i.bounds, &label)),
            "{name} takes no typing: {inputs:?}"
        );
        let at =
            ((shown.center_y() as usize) * WINDOW.width as usize + (shown.x - 2.0) as usize) * 4;
        for (got, want) in pixels[at..at + 3]
            .iter()
            .zip([sunken.r, sunken.g, sunken.b])
        {
            let want = (want * 255.0).round() as i32;
            assert!(
                (i32::from(*got) - want).abs() <= 3,
                "{name}: {got} is not {want}"
            );
        }
    }
    let tags = texts.iter().filter(|t| t.text == from_type).count();
    assert_eq!(tags, 3, "only the type's rows are tagged");
}

fn plain(text: &str) -> String {
    text.replace(['\u{2068}', '\u{2069}'], "")
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
