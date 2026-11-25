//! #1281, #1489: the Net Parameters and Net Blanket Properties snapshot and
//! drawings.

use super::directive::{DirectivePanel, DraftField, NetRule, format_value};
use signex_types::schematic::{
    BlanketPour, Label, LabelType, LibPin, LibSymbol, NetRules, Pin, PinDirection, PinShapeStyle,
    Point, SchDirective, SchematicSheet, Symbol, Wire,
};
use uuid::Uuid;

fn wire(n: u128, a: (f64, f64), b: (f64, f64)) -> Wire {
    Wire {
        uuid: Uuid::from_u128(n),
        start: Point::new(a.0, a.1),
        end: Point::new(b.0, b.1),
        ..Default::default()
    }
}

/// A one-pin part: the netlist forms a net only where a pin lands.
fn one_pin_part() -> LibSymbol {
    LibSymbol {
        id: "R".to_string(),
        reference: String::new(),
        value: String::new(),
        footprint: String::new(),
        datasheet: String::new(),
        description: String::new(),
        keywords: String::new(),
        fp_filters: String::new(),
        in_bom: true,
        on_board: true,
        in_pos_files: true,
        duplicate_pin_numbers_are_jumpers: false,
        graphics: Vec::new(),
        pins: vec![LibPin {
            unit: 0,
            body_style: 1,
            pin: Pin {
                direction: PinDirection::Passive,
                shape_style: PinShapeStyle::Plain,
                position: Point::new(0.0, 0.0),
                rotation: 0.0,
                length: 0.0,
                name: String::new(),
                number: "1".to_string(),
                visible: true,
                name_visible: true,
                number_visible: true,
            },
        }],
        show_pin_numbers: true,
        show_pin_names: true,
        pin_name_offset: 0.0,
    }
}

/// `USB_DP` and `USB_DN` on two wires, `VBUS` on a third, and a power net
/// `GND` on a fourth; a one-pin part ends each wire.
fn sheet() -> SchematicSheet {
    let mut sheet = SchematicSheet {
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    sheet.lib_symbols.insert("R".to_string(), one_pin_part());
    sheet.wires = vec![
        wire(1, (0.0, 0.0), (20.0, 0.0)),
        wire(2, (0.0, 10.0), (20.0, 10.0)),
        wire(3, (0.0, 30.0), (20.0, 30.0)),
        wire(4, (0.0, 50.0), (20.0, 50.0)),
    ];
    for (i, (text, y, ty)) in [
        ("USB_DP", 0.0, LabelType::Net),
        ("USB_DN", 10.0, LabelType::Net),
        ("VBUS", 30.0, LabelType::Net),
        ("GND", 50.0, LabelType::Power),
    ]
    .into_iter()
    .enumerate()
    {
        sheet.labels.push(Label::new(text, Point::new(0.0, y), ty));
        sheet.symbols.push(Symbol {
            uuid: Uuid::from_u128(100 + i as u128),
            lib_id: "R".to_string(),
            reference: format!("R{}", i + 1),
            position: Point::new(20.0, y),
            ..Default::default()
        });
    }
    sheet
}

fn net_parameters(at: Point, show: bool) -> SchDirective {
    SchDirective::NetParameters {
        uuid: Uuid::from_u128(90),
        position: at,
        net_class: Some("POWER_IN".to_string()),
        diff_pair_class: None,
        rules: NetRules {
            width_mm: Some(0.8),
            clearance_mm: Some(0.3),
            via_diameter_mm: Some(0.6),
            via_hole_mm: Some(0.3),
            current_a: Some(3.0),
        },
        show_on_flag: show,
    }
}

fn panel(sheet: &SchematicSheet) -> DirectivePanel {
    DirectivePanel::build(
        sheet,
        Uuid::from_u128(90),
        "power_in.snxsch",
        vec!["POWER_IN".to_string()],
        vec!["USB".to_string()],
    )
    .expect("a panel")
}

#[test]
fn net_parameters_name_their_net_and_whether_it_is_half_of_a_pair() {
    let mut on_vbus = sheet();
    on_vbus
        .directives
        .push(net_parameters(Point::new(10.0, 30.0), false));
    let p = panel(&on_vbus);
    assert_eq!(p.net.as_deref(), Some("VBUS"));
    assert!(!p.in_pair, "VBUS is a single net");
    assert_eq!(p.sheet, "power_in.snxsch");

    let mut on_pair = sheet();
    on_pair
        .directives
        .push(net_parameters(Point::new(10.0, 0.0), false));
    assert!(panel(&on_pair).in_pair, "USB_DP has USB_DN beside it");

    let mut off_wire = sheet();
    off_wire
        .directives
        .push(net_parameters(Point::new(80.0, 80.0), false));
    assert_eq!(panel(&off_wire).net, None);
}

#[test]
fn a_blanket_lists_caught_and_edge_nets_but_never_a_power_net() {
    let mut s = sheet();
    // Wholly over USB_DN, VBUS and GND; USB_DP lies outside it.
    s.directives.push(SchDirective::Blanket {
        uuid: Uuid::from_u128(90),
        start: Point::new(-5.0, 5.0),
        end: Point::new(25.0, 55.0),
        net_class: Some("SW_NODE".to_string()),
        erc_exclusions: Vec::new(),
        track_width: None,
        pour: BlanketPour::Normal,
        include_edge_nets: false,
    });
    let p = panel(&s);
    let names: Vec<&str> = p.nets.iter().map(|n| n.name.as_str()).collect();
    assert!(names.contains(&"VBUS"), "{names:?}");
    assert!(names.contains(&"USB_DN"), "{names:?}");
    assert!(
        !names.contains(&"GND"),
        "power nets are never caught: {names:?}"
    );
    assert_eq!(p.caught_count(), 2, "VBUS inside, USB_DN inside: {names:?}");
    let vbus = p.nets.iter().find(|n| n.name == "VBUS").unwrap();
    assert_eq!(vbus.elements, [Uuid::from_u128(3)]);
    assert!(
        p.wires.iter().any(|(_, _, caught)| *caught),
        "the drawing marks the caught wires"
    );
}

#[test]
fn a_typed_draft_shows_until_the_value_is_stored() {
    let mut s = sheet();
    s.directives
        .push(net_parameters(Point::new(10.0, 30.0), false));
    let mut p = panel(&s);
    let width = DraftField::Rule(NetRule::Width);
    assert_eq!(p.shown(width, Some(0.8)), "0.8");
    p.draft = Some((width, "0.".to_string()));
    assert_eq!(p.shown(width, Some(0.0)), "0.");
    assert_eq!(p.shown(DraftField::Rule(NetRule::Clearance), None), "");
}

#[test]
fn values_show_without_trailing_zeros() {
    assert_eq!(format_value(0.8), "0.8");
    assert_eq!(format_value(1.0), "1");
    assert_eq!(format_value(50.8), "50.8");
    assert_eq!(format_value(0.0), "0");
}

#[test]
fn the_flag_drawing_follows_show_parameters_on_the_flag() {
    let i18n = signex_i18n::I18n::en_us();
    let mut s = sheet();
    s.directives
        .push(net_parameters(Point::new(10.0, 30.0), false));
    let hidden = super::net_parameters::illustration(&panel(&s), &i18n);
    s.directives[0] = net_parameters(Point::new(10.0, 30.0), true);
    let shown = super::net_parameters::illustration(&panel(&s), &i18n);
    let linked = |ill: &signex_widgets::property_illustration::Illustration| {
        ill.visible_parts()
            .filter(|p| p.is_linked_to(&super::net_parameters::FIELD_WIDTH))
            .count()
    };
    assert_eq!(linked(&hidden), 0, "no rule values on a hidden flag");
    assert_eq!(linked(&shown), 2, "Width label and value on the flag");
    assert!(
        shown.visible_parts().count() > hidden.visible_parts().count(),
        "the flag lists the rules"
    );
}

/// Mockup row 153 lays out a selected Net Blanket: the sheet and size
/// under the header, then Outline, Directive and the nets under it; the
/// Suppressed rules section it shares with a No-ERC directive (mockup row
/// 49) comes last.
#[test]
fn a_blanket_panel_runs_outline_directive_nets_then_suppressed_rules() {
    use signex_i18n::ids;
    let mut s = sheet();
    s.directives.push(SchDirective::Blanket {
        uuid: Uuid::from_u128(90),
        start: Point::new(-5.0, 5.0),
        end: Point::new(25.0, 55.0),
        net_class: None,
        erc_exclusions: Vec::new(),
        track_width: None,
        pour: BlanketPour::Normal,
        include_edge_nets: false,
    });
    let (app, _boot) = crate::app::Signex::new();
    let mut ctx = app.document_state.panel_ctx.clone();
    ctx.selected_kind = Some(signex_types::schematic::SelectedKind::Directive);
    ctx.selected_uuid = Some(Uuid::from_u128(90));
    ctx.directive_panel = Some(Box::new(panel(&s)));
    let view = super::view_selected_element_properties(
        &ctx,
        iced::Color::WHITE,
        iced::Color::WHITE,
        iced::Color::BLACK,
    );
    let texts = crate::layout_probe::lay_out_texts(view, iced::Size::new(400.0, 4000.0));
    let top_of = |id: signex_i18n::MessageId| {
        let title = ctx.i18n.upper_for_display(&ctx.i18n.message_plain(id, &[]));
        crate::layout_probe::find(&texts, &title).bounds.y
    };
    let caption = texts
        .iter()
        .find(|t| t.text.contains("power_in.snxsch"))
        .unwrap_or_else(|| panic!("no sheet and size caption: {texts:#?}"))
        .bounds
        .y;
    let outline = top_of(ids::PANELS_BLANKET_OUTLINE);
    let directive = top_of(ids::PANELS_BLANKET_DIRECTIVE);
    let nets = texts
        .iter()
        .find(|t| t.text.contains(&nets_title(&ctx.i18n)))
        .unwrap_or_else(|| panic!("no nets section: {texts:#?}"))
        .bounds
        .y;
    let rules = top_of(ids::PROPERTIES_SUPPRESSED_RULES);
    assert!(
        caption < outline && outline < directive && directive < nets && nets < rules,
        "caption {caption}, outline {outline}, directive {directive}, nets {nets}, rules {rules}"
    );
}

/// The start of the nets section title, as displayed, without its count.
fn nets_title(i18n: &signex_i18n::I18n) -> String {
    let title = i18n.message_plain(
        signex_i18n::ids::PANELS_BLANKET_NETS,
        &[("count", signex_i18n::Arg::from(0u32))],
    );
    let shown = i18n.upper_for_display(&title);
    shown
        .split('0')
        .next()
        .unwrap_or_default()
        .trim()
        .to_string()
}
