//! #2059 part C: the Properties panel reads its text from the catalog, in
//! the English the code wrote before, and its pick lists hand back values.

use super::*;
use crate::layout_probe::{LaidOutText, lay_out_texts, messages_for_events, messages_on_click};
use signex_i18n::{Arg, I18n, MessageId, ids};
use signex_types::schematic::WireWidthPreset;

const WINDOW: iced::Size = iced::Size::new(360.0, 6000.0);

/// Every message the panel shows with no argument, and the literal it
/// showed before #2059.
const PLAIN: &[(MessageId, &str)] = &[
    (ids::PANELS_PLACED_LOCATION, "Location"),
    (ids::PANELS_PLACED_PROPERTIES, "Properties"),
    (ids::PROPERTIES_SECTION_GENERAL, "General"),
    (ids::PROPERTIES_SECTION_GRAPHICAL, "Graphical"),
    (ids::PROPERTIES_SECTION_STYLE, "Style"),
    (ids::PROPERTIES_SECTION_STATE, "State"),
    (ids::PROPERTIES_SECTION_BASIC, "Basic Properties"),
    (ids::PROPERTIES_SECTION_TEXT, "Text Properties"),
    (ids::PROPERTIES_SECTION_FOOTPRINT, "Footprint"),
    (ids::PROPERTIES_SECTION_PAGE_OPTIONS, "Page Options"),
    (
        ids::PROPERTIES_SECTION_SELECTION_FILTERS,
        "Custom Selection Filters",
    ),
    (ids::PANELS_PLACED_ROTATION, "Rotation"),
    (ids::PROPERTIES_POSITION, "Position"),
    (ids::PROPERTIES_SIZE, "Size"),
    (ids::PROPERTIES_NAME, "Name"),
    (ids::PROPERTIES_NET_NAME, "Net Name"),
    (ids::PROPERTIES_PORT_NAME, "Port Name"),
    (ids::PROPERTIES_SHEET_NAME, "Sheet Name"),
    (ids::PROPERTIES_TEXT, "Text"),
    (ids::PROPERTIES_VALUE, "Value"),
    (ids::PROPERTIES_DESIGNATOR, "Designator"),
    (ids::PROPERTIES_DESCRIPTION, "Description"),
    (ids::PROPERTIES_DATASHEET, "Datasheet"),
    (ids::PROPERTIES_FONT, "Font"),
    (ids::PROPERTIES_FONT_DEFAULT, "Default"),
    (ids::PROPERTIES_FONT_SIZE, "Font Size"),
    (ids::PROPERTIES_WIDTH_MM, "Width (mm)"),
    (ids::PROPERTIES_NET_CLASS, "Net Class"),
    (ids::PROPERTIES_STYLE, "Style"),
    (ids::PROPERTIES_LOCKED, "Locked"),
    (ids::PROPERTIES_MIRROR_X, "Mirror X"),
    (ids::PROPERTIES_MIRROR_Y, "Mirror Y"),
    (ids::PROPERTIES_DNP, "DNP"),
    (ids::PROPERTIES_VERTICES, "Vertices"),
    (ids::PROPERTIES_FIELD, "Field"),
    (ids::PROPERTIES_FIELD_REFERENCE, "Reference"),
    (ids::PROPERTIES_FIELD_VALUE, "Value"),
    (ids::PROPERTIES_VISIBLE, "Visible"),
    (ids::PROPERTIES_FIELDS_AUTOPLACED, "Fields Autoplaced"),
    (ids::PROPERTIES_JUSTIFY_H, "Justify H"),
    (ids::PROPERTIES_JUSTIFY_V, "Justify V"),
    (ids::PROPERTIES_TEXT_SIZE, "Text Size"),
    (ids::PROPERTIES_YES, "Yes"),
    (ids::PROPERTIES_NO, "No"),
    (ids::PROPERTIES_NET, "Net"),
    (ids::PROPERTIES_HIDDEN, "Hidden"),
    (ids::PROPERTIES_PINS_COLUMN_PIN, "Pin"),
    (ids::PROPERTIES_PINS_COLUMN_NAME, "Name"),
    (ids::PROPERTIES_PINS_COLUMN_TYPE, "Type"),
    (ids::PROPERTIES_PINS_COLUMN_NET, "Net"),
    (ids::PROPERTIES_SYMBOL, "Symbol"),
    (ids::PROPERTIES_FOOTPRINT, "Footprint"),
    (ids::PROPERTIES_LIBRARY, "Library"),
    (ids::PROPERTIES_ACTION_OPEN_DATASHEET, "Open Datasheet"),
    (ids::PROPERTIES_ACTION_EDIT_SYMBOL, "Edit Symbol"),
    (
        ids::PROPERTIES_ACTION_CHANGE_COMPONENT,
        "Change Component\u{2026}",
    ),
    (ids::PROPERTIES_BODY, "Body"),
    (ids::PROPERTIES_BODY_NORMAL, "Normal"),
    (ids::PROPERTIES_BODY_ALTERNATE, "Alternate (De Morgan)"),
    (
        ids::PROPERTIES_BODY_ALTERNATE_MISSING,
        "Alternate (not in symbol; drawn as Normal)",
    ),
    (
        ids::PROPERTIES_BODY_NO_ALTERNATE_TIP,
        "This library symbol has no alternate (De Morgan) body",
    ),
    (ids::PROPERTIES_FOOTPRINT_3D_BODY, "3D body attached"),
    (
        ids::PROPERTIES_FOOTPRINT_READING,
        "Reading footprint\u{2026}",
    ),
    (
        ids::PROPERTIES_FOOTPRINT_UNLINKED,
        "Not linked to a library footprint",
    ),
    (ids::PROPERTIES_POWER_STYLE_BAR, "Bar"),
    (ids::PROPERTIES_POWER_STYLE_ARROW, "Arrow"),
    (ids::PROPERTIES_POWER_STYLE_WAVE, "Wave"),
    (ids::PROPERTIES_POWER_STYLE_CIRCLE, "Circle"),
    (ids::PROPERTIES_POWER_STYLE_POWER_GROUND, "Power Ground"),
    (ids::PROPERTIES_POWER_STYLE_SIGNAL_GROUND, "Signal Ground"),
    (ids::PROPERTIES_POWER_STYLE_EARTH, "Earth"),
    (ids::PROPERTIES_POWER_STYLE_CHASSIS, "Chassis"),
    (ids::PROPERTIES_PHYSICAL_NAME, "Physical Name"),
    (ids::PROPERTIES_POWER_NET, "Power Net"),
    (ids::PROPERTIES_HIGH_SPEED, "High Speed"),
    (ids::PROPERTIES_DIFF_PAIR, "Differential Pair"),
    (ids::PROPERTIES_DIFF_PAIR_NONE, "None"),
    (ids::PROPERTIES_TAB_ALL, "All"),
    (ids::PROPERTIES_TAB_PARAMETERS, "Parameters"),
    (ids::PROPERTIES_TAB_RULES, "Rules"),
    (ids::PROPERTIES_TAB_CLASSES, "Classes"),
    (ids::PROPERTIES_NET_PARAMS_ADD, "Add \u{25BE}"),
    (ids::PROPERTIES_NET_NO_PARAMETERS, "No Parameters"),
    (ids::PROPERTIES_WIRE_WIDTH, "Width"),
    (ids::PROPERTIES_WIRE_WIDTH_SMALLEST, "Smallest"),
    (ids::PROPERTIES_WIRE_WIDTH_SMALL, "Small"),
    (ids::PROPERTIES_WIRE_WIDTH_MEDIUM, "Medium"),
    (ids::PROPERTIES_WIRE_WIDTH_LARGE, "Large"),
    (ids::PROPERTIES_WIRE_COLOR, "Color"),
    (ids::PROPERTIES_COLOR_DEFAULT, "Default"),
    (ids::PROPERTIES_COLOR_PRESETS, "Preset Colours"),
    (ids::PROPERTIES_COLOR_CUSTOM, "Custom\u{2026}"),
    (ids::PROPERTIES_COLOR_RESET, "Reset to Default"),
    (ids::PROPERTIES_UNIT_MM, "mm"),
    (ids::PROPERTIES_UNIT_MILS, "mils"),
    (ids::PROPERTIES_VISIBLE_GRID, "Visible Grid"),
    (ids::PROPERTIES_SNAP_GRID, "Snap Grid"),
    (ids::PROPERTIES_SNAP_TO_HOTSPOTS, "Snap to Hotspots"),
    (ids::PROPERTIES_CANVAS_FONT, "Canvas Font"),
    (ids::PROPERTIES_SHEET_COLOR, "Sheet Color"),
    (ids::PROPERTIES_SHEET_COLOR_DEFAULT, "Default"),
    (ids::PROPERTIES_SHEET_COLOR_BLACK, "Black"),
    (ids::PROPERTIES_SHEET_COLOR_WHITE, "White"),
    (ids::PROPERTIES_SHEET_COLOR_DARK_GRAY, "Dark Gray"),
    (ids::PROPERTIES_SHEET_COLOR_LIGHT_GRAY, "Light Gray"),
    (ids::PROPERTIES_SHEET_COLOR_CREAM, "Cream"),
    (ids::PROPERTIES_FORMAT_TEMPLATE, "Template"),
    (ids::PROPERTIES_FORMAT_STANDARD, "Standard"),
    (ids::PROPERTIES_FORMAT_CUSTOM, "Custom"),
    (ids::PROPERTIES_PAPER, "Paper"),
    (ids::PROPERTIES_ORIENTATION, "Orientation"),
    (ids::PROPERTIES_ORIENTATION_LANDSCAPE, "Landscape"),
    (ids::PROPERTIES_ORIENTATION_PORTRAIT, "Portrait"),
    (ids::PROPERTIES_SHOW_BORDER, "Show Border"),
    (ids::PROPERTIES_SHOW_TITLE_BLOCK, "Show Title Block"),
    (ids::PROPERTIES_ORIGIN, "Origin"),
    (ids::PROPERTIES_ORIGIN_UPPER_LEFT, "Upper Left"),
    (ids::PROPERTIES_ORIGIN_LOWER_LEFT, "Lower Left"),
    (ids::PROPERTIES_DEFAULT_FONT, "Default Font"),
    (ids::PROPERTIES_TITLE_BLOCK_TITLE, "Title"),
    (ids::PROPERTIES_TITLE_BLOCK_DOCUMENT_NUMBER, "Document No."),
    (ids::PROPERTIES_TITLE_BLOCK_REVISION, "Revision"),
    (ids::PROPERTIES_TITLE_BLOCK_DATE, "Date"),
    (ids::PROPERTIES_TITLE_BLOCK_COMPANY, "Company"),
    (ids::PROPERTIES_TITLE_BLOCK_DRAWN_BY, "Drawn By"),
    (ids::PROPERTIES_TITLE_BLOCK_CHECKED_BY, "Checked By"),
    (ids::PROPERTIES_TITLE_BLOCK_APPROVED_BY, "Approved By"),
    (
        ids::PROPERTIES_TITLE_BLOCK_HINT,
        "Press Enter to apply a field.",
    ),
    (ids::PANELS_NET_CLASSES_SECTION, "Net Classes"),
    (ids::PANELS_NET_CLASSES_NAME, "Name"),
    (ids::PANELS_NET_CLASSES_COLOR, "Colour"),
    (ids::PANELS_NET_CLASSES_WIDTH, "Width (mm)"),
    (ids::PANELS_NET_CLASSES_LINE, "Line"),
    (ids::PANELS_NET_CLASSES_LINE_DEFAULT, "Default"),
    (ids::PANELS_NET_CLASSES_LINE_SOLID, "Solid"),
    (ids::PANELS_NET_CLASSES_LINE_DASHED, "Dashed"),
    (ids::PANELS_NET_CLASSES_LINE_DOTTED, "Dotted"),
    (ids::PANELS_NET_CLASSES_NETS, "Nets"),
    (ids::PANELS_NET_CLASSES_PATTERNS, "Patterns"),
    (ids::PANELS_COMPONENT_SHEET_SECTION_COMPONENT, "Component"),
    (ids::PANELS_COMPONENT_SHEET_SECTION_STORAGE, "Storage"),
];

#[test]
fn every_new_message_reads_as_the_literal_it_replaced() {
    let en = I18n::en_us();
    for (id, literal) in PLAIN {
        assert_eq!(en.message_plain(*id, &[]), *literal, "{}", id.as_str());
    }
    let with = |id: MessageId, args: &[(&str, Arg)]| en.message_plain(id, args);
    assert_eq!(
        with(ids::PROPERTIES_PIN_TITLE, &[("pin", "A1".into())]),
        "Pin A1"
    );
    let pads = |n: usize| {
        with(
            ids::PROPERTIES_FOOTPRINT_PADS,
            &[("n", n.into()), ("count", n.to_string().into())],
        )
    };
    assert_eq!(pads(1), "1 pad");
    assert_eq!(pads(0), "0 pads");
    assert_eq!(pads(2), "2 pads");
    assert_eq!(pads(1156), "1156 pads");
    assert_eq!(
        with(
            ids::PROPERTIES_FOOTPRINT_PITCH_PADS,
            &[("pitch", "0.95".into()), ("pads", "5 pads".into())]
        ),
        "0.95 mm pitch, 5 pads"
    );
    assert_eq!(
        with(
            ids::PROPERTIES_FOOTPRINT_UNREADABLE,
            &[("reason", "gone".into())]
        ),
        "Footprint could not be read: gone"
    );
    assert_eq!(
        with(
            ids::PROPERTIES_SHEET_COLOR_CUSTOM,
            &[("hex", "#123456".into())]
        ),
        "Custom (#123456)"
    );
    assert_eq!(
        with(
            ids::NOTICE_MENU_PATH,
            &[
                ("menu", "Project".into()),
                ("row", "Electrical Rules Check".into())
            ]
        ),
        "Project \u{25B8} Electrical Rules Check"
    );
}

/// A schematic panel context with the Tab form of a net label open at
/// `rotation`.
fn net_label_form(rotation: f64) -> PanelContext {
    let (app, _boot) = crate::app::Signex::new();
    let mut ctx = app.document_state.panel_ctx.clone();
    ctx.has_schematic = true;
    let mut pp = PrePlacementData::new(
        "Net Label".to_owned(),
        PrePlacementKind::NetLabel,
        "N1".to_owned(),
        0.0,
        0.0,
    );
    pp.rotation = rotation;
    ctx.pre_placement = Some(pp);
    ctx
}

fn texts(ctx: &PanelContext) -> Vec<LaidOutText> {
    lay_out_texts(view_panel(PanelKind::Properties, ctx), WINDOW)
        .into_iter()
        .map(|t| LaidOutText {
            text: t.text.replace(['\u{2068}', '\u{2069}'], ""),
            bounds: t.bounds,
        })
        .collect()
}

/// A panel context with one object of `kind` selected and `info` its rows.
fn one_selected(kind: SelectedKindForTest, info: &[(&str, &str)]) -> PanelContext {
    let (app, _boot) = crate::app::Signex::new();
    let mut ctx = app.document_state.panel_ctx.clone();
    ctx.has_schematic = true;
    ctx.selection_count = 1;
    ctx.selected_uuid = Some(uuid::Uuid::from_u128(7));
    ctx.selected_kind = Some(kind);
    ctx.selection_info = info
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
    ctx
}

type SelectedKindForTest = signex_types::schematic::SelectedKind;

/// The text laid out on the same line as `label`, right of it.
fn beside<'t>(shown: &'t [LaidOutText], label: &str) -> &'t str {
    let at = shown
        .iter()
        .find(|t| t.text == label)
        .unwrap_or_else(|| panic!("{label:?} missing from {shown:#?}"));
    let middle = |t: &LaidOutText| t.bounds.y + t.bounds.height / 2.0;
    shown
        .iter()
        .filter(|t| (middle(t) - middle(at)).abs() < 4.0 && t.bounds.x > at.bounds.x)
        .min_by(|a, b| a.bounds.x.total_cmp(&b.bounds.x))
        .map_or("", |t| t.text.as_str())
}

/// The colour a segment of `row`'s segmented switch is filled with: the
/// pixel just inside the top-left of its `segment` label, above the glyphs.
/// The segment drawn on has the active fill, the rest the track.
fn segment_fill(ctx: &PanelContext, row: &str, segment: &str) -> [u8; 4] {
    let shown = texts(ctx);
    let middle = |t: &LaidOutText| t.bounds.y + t.bounds.height / 2.0;
    // The row label with the segment on its own line (an illustration
    // callout can carry the same word).
    let cell = (shown.iter().filter(|t| t.text == row))
        .find_map(|label| {
            shown.iter().find(|t| {
                t.text == segment
                    && t.bounds.x > label.bounds.x
                    && (middle(t) - middle(label)).abs() < 8.0
            })
        })
        .unwrap_or_else(|| panic!("{segment:?} of {row:?} missing from {shown:#?}"));
    let pixels = crate::layout_probe::draw_pixels(view_panel(PanelKind::Properties, ctx), WINDOW);
    let (x, y) = (
        (cell.bounds.x + 1.0) as usize,
        (cell.bounds.y + 1.0) as usize,
    );
    let at = (y * WINDOW.width as usize + x) * 4;
    [pixels[at], pixels[at + 1], pixels[at + 2], pixels[at + 3]]
}

/// The clicked pin's rows pair each label with its own value; a hidden pin
/// reads Yes.
#[test]
fn the_focused_pin_rows_pair_labels_with_their_values() {
    let ctx = one_selected(
        SelectedKindForTest::Symbol,
        &[
            ("Type", "Symbol"),
            ("Pin: 1", "ANODE\tPassive\tYes"),
            ("Focused Pin", "1"),
        ],
    );
    let shown = texts(&ctx);
    assert!(shown.iter().any(|t| t.text == "PIN 1"), "{shown:#?}");
    assert_eq!(beside(&shown, "Designator"), "1");
    assert_eq!(beside(&shown, "Name"), "ANODE");
    assert_eq!(beside(&shown, "Type"), "Passive");
    assert_eq!(beside(&shown, "Hidden"), "Yes");
}

/// A selected Reference field says so on its Field row.
#[test]
fn the_field_row_names_the_selected_field() {
    let info = [("Type", "Symbol Field")];
    let reference = one_selected(SelectedKindForTest::SymbolRefField, &info);
    assert_eq!(beside(&texts(&reference), "Field"), "Reference");
    let value = one_selected(SelectedKindForTest::SymbolValField, &info);
    assert_eq!(beside(&texts(&value), "Field"), "Value");
}

/// The engine's Yes / No values of a field read in the panel's language;
/// other engine words pass through.
#[test]
fn a_fields_yes_and_no_values_follow_the_language() {
    let info = [
        ("Type", "Symbol Field"),
        ("Visible", "No"),
        ("Fields Autoplaced", "Yes"),
        (signex_engine::info_keys::JUSTIFY_H, "Left"),
    ];
    let mut ctx = one_selected(SelectedKindForTest::SymbolRefField, &info);
    let shown = texts(&ctx);
    assert_eq!(beside(&shown, "Visible"), "No");
    assert_eq!(beside(&shown, "Fields Autoplaced"), "Yes");
    ctx.i18n = std::sync::Arc::new(I18n::new(&signex_i18n::LanguagePref::Locale(
        "en-XA".into(),
    )));
    let xa = &ctx.i18n;
    let shown = texts(&ctx);
    let label = |id| xa.message_plain(id, &[]);
    assert_eq!(
        beside(&shown, &label(ids::PROPERTIES_VISIBLE)),
        label(ids::PROPERTIES_NO)
    );
    assert_eq!(
        beside(&shown, &label(ids::PROPERTIES_FIELDS_AUTOPLACED)),
        label(ids::PROPERTIES_YES)
    );
    assert_eq!(beside(&shown, &label(ids::PROPERTIES_JUSTIFY_H)), "Left");
}

/// The Border row opens the border picker and the Fill row the fill picker
/// (mockup row 46 shortened the labels).
#[test]
fn the_sheet_symbol_colour_rows_open_their_own_pickers() {
    let id = uuid::Uuid::from_u128(9);
    let mut ctx = one_selected(SelectedKindForTest::ChildSheet, &[("Type", "Sheet Symbol")]);
    ctx.selected_child_sheet = Some(signex_types::schematic::ChildSheet {
        uuid: id,
        name: "Power".into(),
        filename: "power.snxsch".into(),
        position: signex_types::schematic::Point::new(10.0, 10.0),
        size: (40.0, 30.0),
        stroke_width: 0.0,
        fill: signex_types::schematic::FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: Vec::new(),
        instances: Vec::new(),
    });
    let shown = texts(&ctx);
    let click = |label: &str| {
        let at = shown
            .iter()
            .find(|t| t.text == label)
            .unwrap_or_else(|| panic!("{label:?} missing from {shown:#?}"));
        let swatch = iced::Point::new(
            at.bounds.x + 96.0 + 30.0,
            at.bounds.y + at.bounds.height / 2.0,
        );
        crate::layout_probe::messages_on_click_at(
            view_panel(PanelKind::Properties, &ctx),
            WINDOW,
            None,
            swatch,
        )
    };
    assert!(
        click("Border")
            .iter()
            .any(|m| matches!(m, PanelMsg::ToggleChildSheetBorderPicker(i) if *i == id)),
        "Border opens the border picker"
    );
    assert!(
        click("Fill")
            .iter()
            .any(|m| matches!(m, PanelMsg::ToggleChildSheetFillPicker(i) if *i == id)),
        "Fill opens the fill picker"
    );
}

/// Document Options with nothing selected.
fn document_options() -> PanelContext {
    let (app, _boot) = crate::app::Signex::new();
    let mut ctx = app.document_state.panel_ctx.clone();
    ctx.has_schematic = true;
    ctx.selection_count = 0;
    ctx.pre_placement = None;
    ctx
}

/// Each typed pick list shows the entry of the stored value, not its first
/// entry: a non-first value per list, read from the closed list.
#[test]
fn each_pick_list_shows_the_stored_entry() {
    use signex_types::schematic::{PageOrigin, SheetOrientation, SheetSize};
    let mut ctx = document_options();
    ctx.sheet_color = SchematicSheetColor::Preset(SheetColor::Cream);
    ctx.paper_size = SheetSize::parse("A4").0.token(SheetOrientation::Portrait);
    ctx.page_origin = PageOrigin::LowerLeft;
    ctx.net_classes.available = true;
    ctx.net_classes.classes = vec![signex_types::net_class::NetClass {
        name: "Power".into(),
        style: signex_types::net_class::NetClassStyle {
            line: Some(signex_types::net_class::NetLineStyle::Dashed),
            ..Default::default()
        },
        ..Default::default()
    }];
    // Every row is checked before the test fails, so a report names each
    // list that shows the wrong entry.
    let mut wrong: Vec<String> = Vec::new();
    let mut check = |row: &str, got: &str, want: &str| {
        if got != want {
            wrong.push(format!("{row}: shows {got:?}, wants {want:?}"));
        }
    };
    let shown = texts(&ctx);
    check("Sheet Color", beside(&shown, "Sheet Color"), "Cream");
    // #2531: Paper names its size at the sheet's orientation.
    check("Paper", beside(&shown, "Paper"), "A4 · 210 × 297 mm");
    check("Line", beside(&shown, "Line"), "Dashed");

    // Orientation and Origin are segmented switches: the stored Portrait
    // and Lower Left segments draw as on. A sheet storing the other two
    // draws those as on instead, so each pair is compared both ways.
    let mut flipped = ctx.clone();
    flipped.paper_size = SheetSize::parse("A4").0.token(SheetOrientation::Landscape);
    flipped.page_origin = PageOrigin::UpperLeft;
    for (row, stored, other) in [
        ("Orientation", "Portrait", "Landscape"),
        ("Origin", "Lower Left", "Upper Left"),
    ] {
        let on_here = segment_fill(&ctx, row, stored);
        let on_there = segment_fill(&flipped, row, other);
        let off_here = segment_fill(&ctx, row, other);
        let drawn_on = if on_here == on_there && on_here != off_here {
            stored
        } else {
            "another segment"
        };
        check(row, drawn_on, stored);
    }

    // Width is no pick list since #2523: a 2 x 2 grid of the four presets
    // (`selected::tests` checks the cell it marks).
    let mut wire = one_selected(SelectedKindForTest::Wire, &[("Type", "Wire")]);
    wire.selected_wire_style = Some((WireWidthPreset::Medium, None));
    let wire_texts = texts(&wire);
    for preset in WireWidthPreset::ALL {
        assert!(
            wire_texts.iter().any(|t| t.text == preset.label()),
            "Width lacks {}",
            preset.label()
        );
    }

    // A power port's Style is a tile grid now (mockup row 45); `power_port::tests`
    // checks the tile it marks, Chassis (#1284) included.
    let body = one_selected(
        SelectedKindForTest::Symbol,
        &[("Type", "Symbol"), ("Body Style", "Alternate")],
    );
    check(
        "Body",
        beside(&texts(&body), "Body"),
        "Alternate (De Morgan)",
    );

    let note = one_selected(
        SelectedKindForTest::TextNote,
        &[("Type", "Text"), ("Font Family", "Signex Probe Sans")],
    );
    check("Font", beside(&texts(&note), "Font"), "Signex Probe Sans");
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// Pressing a power-port style tile asks for that style's library id (mockup row 45:
/// the Style list became a grid of toggle tiles).
#[test]
fn picking_a_power_port_style_asks_for_its_library_id() {
    let ctx = one_selected(
        SelectedKindForTest::Symbol,
        &[("Type", "Power Port"), ("Library ID", "power:bar")],
    );
    let shown = texts(&ctx);
    let tile = shown
        .iter()
        .find(|t| t.text == "Arrow")
        .expect("Arrow tile");
    let over_tile = iced::Point::new(tile.bounds.center_x(), tile.bounds.center_y());
    let events = [
        iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)),
        iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
            iced::mouse::Button::Left,
        )),
    ];
    let asked: Vec<String> = messages_for_events(
        view_panel(PanelKind::Properties, &ctx),
        WINDOW,
        over_tile,
        &events,
    )
    .into_iter()
    .filter_map(|m| match m {
        PanelMsg::EditPowerPortStyle { new_lib_id, .. } => Some(new_lib_id),
        _ => None,
    })
    .collect();
    assert_eq!(asked, ["power:arrow"]);
}

/// Document Options open on the Parameters tab.
fn document_parameters() -> PanelContext {
    let mut ctx = document_options();
    ctx.properties_tab = 1;
    ctx
}

/// #2531: a title-block row the sheet leaves empty, while the project has a
/// value, carries a "project" pill on its own line; once the sheet has its
/// own value the pill is gone. The row keeps its plain field name.
#[test]
fn a_row_inherited_from_the_project_carries_a_project_pill() {
    let mut ctx = document_parameters();
    ctx.project_parameters
        .insert("drawn_by".into(), "Ann".into());
    let shown = texts(&ctx);
    let middle = |t: &LaidOutText| t.bounds.y + t.bounds.height / 2.0;
    let name = shown
        .iter()
        .find(|t| t.text == "Drawn By")
        .unwrap_or_else(|| panic!("Drawn By missing from {shown:#?}"));
    let pill = shown
        .iter()
        .find(|t| t.text == "project")
        .unwrap_or_else(|| panic!("no project pill in {shown:#?}"));
    assert!(
        (middle(pill) - middle(name)).abs() < 4.0,
        "{pill:?} {name:?}"
    );
    assert!(pill.bounds.x > name.bounds.x);
    ctx.title_block.insert("drawn_by".into(), "Bob".into());
    let shown = texts(&ctx);
    assert!(!shown.iter().any(|t| t.text == "project"), "{shown:#?}");
    assert!(shown.iter().any(|t| t.text == "Drawn By"), "{shown:#?}");
}

/// Maintainer decision 2026-11-18: the General tab has no title-block
/// field; the Parameters tab is the one place they are edited.
#[test]
fn the_general_tab_has_no_title_block_field() {
    let mut ctx = document_options();
    ctx.title_block.insert("drawn_by".into(), "Bob".into());
    let shown = texts(&ctx);
    for name in [
        "Title",
        "Drawn By",
        "TITLE BLOCK",
        "Sheet Variables",
        "SHEET VARIABLES",
    ] {
        let found = shown.iter().any(|t| t.text == name);
        assert!(!found, "{name:?} in {shown:#?}");
    }
    let shown = texts(&document_parameters());
    for name in [
        "Title block",
        "Title",
        "Document No.",
        "Revision",
        "Sheet variables",
    ] {
        let found = shown.iter().any(|t| t.text == name);
        assert!(found, "{name:?} missing: {shown:#?}");
    }
}

/// An empty Date row shows today, greyed, and says so; a typed Date does
/// not (decision 2026-11-18).
#[test]
fn an_empty_date_row_says_it_shows_today() {
    let mut ctx = document_parameters();
    ctx.builtin_parameters = vec![(BuiltinParameter::CurrentDate, "2026-11-18".into())];
    let shown = texts(&ctx);
    assert!(shown.iter().any(|t| t.text == "Date (today)"), "{shown:#?}");
    ctx.title_block.insert("date".into(), "2026-01-02".into());
    let shown = texts(&ctx);
    let today = shown.iter().any(|t| t.text == "Date (today)");
    assert!(!today, "{shown:#?}");
}

fn shows(shown: &[LaidOutText], text: &str) -> bool {
    shown.iter().any(|t| t.text == text)
}

/// #2531: Format = Custom swaps the Paper and Orientation rows for one Size
/// row of typed W / H; Standard brings them back.
#[test]
fn a_custom_format_swaps_paper_and_orientation_for_typed_w_and_h() {
    let mut ctx = document_options();
    ctx.paper_size = "A4".into();
    ctx.page_format_mode = PageFormatMode::Standard;
    let shown = texts(&ctx);
    assert!(
        shows(&shown, "Paper") && shows(&shown, "Orientation"),
        "{shown:#?}"
    );
    assert!(!shows(&shown, "Size") && !shows(&shown, "W"), "{shown:#?}");
    ctx.page_format_mode = PageFormatMode::Custom;
    let shown = texts(&ctx);
    assert!(
        !shows(&shown, "Paper") && !shows(&shown, "Orientation"),
        "{shown:#?}"
    );
    assert!(shows(&shown, "Size") && shows(&shown, "W"), "{shown:#?}");
}

/// #2531: Format, Orientation and Origin are segmented switches; a press on
/// a segment reports its value.
#[test]
fn the_page_switches_report_the_pressed_segment() {
    let mut ctx = document_options();
    ctx.paper_size = "A4".into();
    let pressed = |label: &str| {
        messages_on_click(view_panel(PanelKind::Properties, &ctx), WINDOW, label)
            .into_iter()
            .filter(|m| !matches!(m, PanelMsg::DocumentOptions(_)))
            .map(|m| format!("{m:?}"))
            .collect::<Vec<_>>()
    };
    assert_eq!(pressed("Portrait"), ["SetSheetOrientation(Portrait)"]);
    assert_eq!(pressed("Lower Left"), ["SetPageOrigin(LowerLeft)"]);
    assert_eq!(pressed("Custom"), ["SetPageFormatMode(Custom)"]);
}

/// #2531: the Rules chip shows the empty card, since a sheet has no rules;
/// the chips carry their counts.
#[test]
fn the_rules_chip_shows_the_empty_card() {
    let mut ctx = document_parameters();
    let shown = texts(&ctx);
    assert!(shows(&shown, "Title block"), "{shown:#?}");
    assert!(!shows(&shown, "No rules"), "{shown:#?}");
    let picked: Vec<PanelMsg> =
        messages_on_click(view_panel(PanelKind::Properties, &ctx), WINDOW, "Rules");
    assert!(
        picked.iter().any(|m| matches!(
            m,
            PanelMsg::DocumentOptions(crate::panels::DocumentOptionsMsg::ParametersFilterPicked(
                crate::panels::properties_parameters::ParametersFilter::Rules
            ))
        )),
        "{picked:?}"
    );
    ctx.document_options.parameters_filter =
        crate::panels::properties_parameters::ParametersFilter::Rules;
    let shown = texts(&ctx);
    assert!(shows(&shown, "No rules"), "{shown:#?}");
    assert!(!shows(&shown, "Title block"), "{shown:#?}");
}

/// Maintainer decision 2026-11-18: a Parameters row shows its real value,
/// never a `*` placeholder, and a long name wraps inside its column instead
/// of running into the value.
#[test]
fn the_parameters_tab_shows_real_values_and_names_clear_of_them() {
    let mut ctx = document_parameters();
    ctx.builtin_parameters = vec![
        (BuiltinParameter::DocumentName, "power.snxsch".into()),
        (BuiltinParameter::ProjectName, String::new()),
    ];
    ctx.project_parameters
        .insert("approved_by".into(), "Ann".into());
    ctx.title_block.insert("approved_by".into(), "Bob".into());
    ctx.sheet_variables
        .insert("DocumentFullPathAndName".into(), "x".into());
    let shown = texts(&ctx);
    assert!(!shown.iter().any(|t| t.text == "*"), "{shown:#?}");
    assert_eq!(beside(&shown, "Document Name"), "power.snxsch");
    let value = shown
        .iter()
        .find(|t| t.text == "power.snxsch")
        .expect("the document name");
    for name in ["Approved By", "DocumentFullPathAndName", "Project Name"] {
        let at = shown
            .iter()
            .find(|t| t.text == name)
            .unwrap_or_else(|| panic!("{name:?} missing from {shown:#?}"));
        assert!(
            at.bounds.x + at.bounds.width <= value.bounds.x,
            "{name:?} reaches the value column: {at:?} {value:?}"
        );
    }
}

/// The Tab form of a global port names its text Port Name.
#[test]
fn the_global_port_form_asks_for_a_port_name() {
    let mut ctx = net_label_form(0.0);
    if let Some(pp) = ctx.pre_placement.as_mut() {
        pp.kind = PrePlacementKind::GlobalPort;
    }
    let shown = texts(&ctx);
    assert!(shown.iter().any(|t| t.text == "Port Name"), "{shown:#?}");
    assert!(!shown.iter().any(|t| t.text == "Sheet Name"), "{shown:#?}");
}

/// A selected part shows its Graphical section.
#[test]
fn a_selected_part_shows_its_graphical_section() {
    let ctx = one_selected(SelectedKindForTest::Symbol, &[("Type", "Symbol")]);
    let shown = texts(&ctx);
    assert!(shown.iter().any(|t| t.text == "GRAPHICAL"), "{shown:#?}");
    assert!(!shown.iter().any(|t| t.text == "STATE"), "{shown:#?}");
}

/// The visible text of `shown`, in drawing order.
fn plain(shown: &[LaidOutText]) -> Vec<&str> {
    shown.iter().map(|t| t.text.as_str()).collect()
}

/// The net-class editor names each field row as before, in order.
#[test]
fn the_net_class_field_rows_keep_their_names_and_order() {
    let mut ctx = document_options();
    ctx.net_classes.available = true;
    ctx.net_classes.classes = vec![signex_types::net_class::NetClass {
        name: "Power".into(),
        ..Default::default()
    }];
    let shown = texts(&ctx);
    let fields = ["Name", "Colour", "Width (mm)", "Line", "Nets", "Patterns"];
    let rows: Vec<&str> = plain(&shown)
        .into_iter()
        .filter(|t| fields.contains(t))
        .collect();
    assert!(rows.ends_with(&fields), "{rows:?}");
}

/// A sheet entry's facts sit under a Properties section, its own title. (A
/// wire's End and Length moved into Location, mockup row 43; `placed_rows_tests`
/// covers them.)
#[test]
fn a_sheet_entrys_facts_sit_under_properties() {
    let ctx = one_selected(
        SelectedKindForTest::SheetPin,
        &[
            ("Type", "Sheet Entry"),
            ("Name", "VIN"),
            ("Direction", "input"),
        ],
    );
    let shown = texts(&ctx);
    let all = plain(&shown);
    let title = all.iter().position(|t| *t == "PROPERTIES");
    let name = all.iter().position(|t| *t == "Name");
    assert!(title.is_some() && title < name, "{all:?}");
    assert_eq!(beside(&shown, "Name"), "VIN");
}

/// A Net Blanket's sections keep their titles above their rows (#1281:
/// Outline, Directive with the net class, the nets under it, then the
/// Suppressed rules section shared with the No-ERC directive).
#[test]
fn a_blankets_sections_keep_their_titles() {
    let mut ctx = one_selected(
        SelectedKindForTest::Directive,
        &[("Type", "Net Blanket"), ("Net Class", "PWR")],
    );
    let mut sheet = signex_types::schematic::SchematicSheet {
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    sheet
        .directives
        .push(signex_types::schematic::SchDirective::Blanket {
            uuid: uuid::Uuid::from_u128(7),
            start: signex_types::schematic::Point::new(10.0, 10.0),
            end: signex_types::schematic::Point::new(60.8, 40.48),
            net_class: Some("PWR".to_string()),
            erc_exclusions: Vec::new(),
            track_width: None,
            pour: signex_types::schematic::BlanketPour::Normal,
            include_edge_nets: false,
        });
    ctx.directive_panel = crate::panels::DirectivePanel::build(
        &sheet,
        uuid::Uuid::from_u128(7),
        "power_in.snxsch",
        vec!["PWR".to_string()],
        Vec::new(),
    )
    .map(Box::new);
    let shown = texts(&ctx);
    let all = plain(&shown);
    let at = |text: &str| all.iter().position(|t| *t == text);
    assert!(
        at("OUTLINE").is_some() && at("OUTLINE") < at("Width / Height"),
        "{all:?}"
    );
    assert!(
        at("DIRECTIVE").is_some() && at("DIRECTIVE") < at("Net class"),
        "{all:?}"
    );
    assert!(at("Net class") < at("Track width"), "{all:?}");
    assert!(
        at("SUPPRESSED RULES").is_some() && at("Track width") < at("SUPPRESSED RULES"),
        "{all:?}"
    );
}

/// A text note's Rotation and Justify rows are segmented controls (mockup row 47):
/// each row lays its own steps out on its line, in order. Which step is on
/// is read from the engine's keys (`text_note::tests`).
#[test]
fn a_text_notes_rows_lay_out_their_own_steps() {
    let ctx = one_selected(
        SelectedKindForTest::TextNote,
        &[
            ("Type", "Text"),
            ("Rotation", "90"),
            ("Horizontal Justification", "Left"),
            ("Vertical Justification", "Top"),
        ],
    );
    let shown = texts(&ctx);
    let line = |label: &str| {
        let at = shown.iter().find(|t| t.text == label).expect(label);
        let mut cells: Vec<&LaidOutText> = (shown.iter())
            .filter(|t| (t.bounds.center_y() - at.bounds.center_y()).abs() < 4.0)
            .filter(|t| t.bounds.x > at.bounds.x)
            .collect();
        cells.sort_by(|a, b| a.bounds.x.total_cmp(&b.bounds.x));
        cells
            .into_iter()
            .map(|t| t.text.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(line("Rotation"), ["0°", "90°", "180°", "270°"]);
    assert_eq!(line("Justify H"), ["Left", "Center", "Right"]);
    assert_eq!(line("Justify V"), ["Top", "Middle", "Bottom"]);
}

/// A selected net label titles its section "Properties" (trunk's id).
#[test]
fn a_selected_label_titles_its_properties_section() {
    let ctx = one_selected(SelectedKindForTest::Label, &[("Type", "Net Label")]);
    let shown = texts(&ctx);
    assert!(
        shown.iter().any(|t| t.text == "PROPERTIES"),
        "PROPERTIES missing from {shown:#?}"
    );
}

/// The words `ctx` draws the same in en-US and en-XA: text a call site
/// rendered in `I18n::en_us()` instead of the panel's language. Only
/// all-letter text is compared; digits and punctuation are skipped, and so
/// are the unit symbols, which are the same in every language.
fn same_in_en_xa(mut ctx: PanelContext) -> Vec<String> {
    let english: Vec<String> = texts(&ctx).into_iter().map(|t| t.text).collect();
    ctx.i18n = std::sync::Arc::new(I18n::new(&signex_i18n::LanguagePref::Locale(
        "en-XA".into(),
    )));
    let xa: Vec<String> = texts(&ctx).into_iter().map(|t| t.text).collect();
    let is_words = |t: &str| {
        !matches!(t, "mm" | "mil")
            && t.chars().any(char::is_alphabetic)
            && t.chars().all(|c| c.is_alphabetic() || c == ' ' || c == '…')
    };
    english
        .into_iter()
        .filter(|t| is_words(t) && xa.contains(t))
        .collect()
}

/// #2059: Document Options and a selected part with its footprint card
/// draw no catalog text in English under en-XA; only the design's own
/// words (the part's footprint name) stay.
#[test]
fn the_properties_panel_draws_no_english_under_en_xa() {
    assert_eq!(same_in_en_xa(document_options()), Vec::<String>::new());
    assert_eq!(same_in_en_xa(document_parameters()), Vec::<String>::new());
    let mut part = one_selected(
        SelectedKindForTest::Symbol,
        &[("Type", "Symbol"), ("Footprint", "Probe")],
    );
    part.footprint_card = Some(crate::panels::FootprintCardView {
        name: "Probe".into(),
        key: None,
        version: 0,
        status: crate::panels::FootprintCardStatus::Loading,
    });
    assert_eq!(same_in_en_xa(part), ["Probe"]);
}
