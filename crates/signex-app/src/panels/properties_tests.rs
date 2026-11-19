//! #2059 part C: the Properties panel reads its text from the catalog, in
//! the English the code wrote before, and its pick lists hand back values.

use super::*;
use crate::layout_probe::{LaidOutText, lay_out_texts, messages_for_events};
use signex_i18n::{Arg, I18n, MessageId, ids};

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
    (ids::PROPERTIES_SECTION_FONT, "Font"),
    (ids::PROPERTIES_SECTION_BASIC, "Basic Properties"),
    (ids::PROPERTIES_SECTION_TEXT, "Text Properties"),
    (ids::PROPERTIES_SECTION_FOOTPRINT, "Footprint"),
    (ids::PROPERTIES_SECTION_NET_GENERAL, "General (Net)"),
    (ids::PROPERTIES_SECTION_NET_PARAMETERS, "Parameters (Net)"),
    (ids::PROPERTIES_SECTION_PARAMETERS, "Parameters"),
    (ids::PROPERTIES_SECTION_PAGE_OPTIONS, "Page Options"),
    (ids::PROPERTIES_SECTION_TITLE_BLOCK, "Title Block"),
    (
        ids::PROPERTIES_SECTION_SELECTION_FILTERS,
        "Custom Selection Filters",
    ),
    (ids::PROPERTIES_XY, "(X/Y)"),
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
    (ids::PROPERTIES_ELECTRICAL_TYPE, "Electrical Type"),
    (ids::PROPERTIES_NET, "Net"),
    (ids::PROPERTIES_HIDDEN, "Hidden"),
    (ids::PROPERTIES_PINS_COLUMN_PIN, "Pin"),
    (ids::PROPERTIES_PINS_COLUMN_NAME, "Name"),
    (ids::PROPERTIES_PINS_COLUMN_TYPE, "Type"),
    (ids::PROPERTIES_PINS_COLUMN_NET, "Net"),
    (ids::PROPERTIES_PINS_COLUMN_HIDDEN, "Hidden"),
    (ids::PROPERTIES_SYMBOL, "Symbol"),
    (ids::PROPERTIES_FOOTPRINT, "Footprint"),
    (ids::PROPERTIES_FOOTPRINT_OVERRIDE, "Footprint override"),
    (ids::PROPERTIES_LIBRARY, "Library"),
    (ids::PROPERTIES_LIBRARY_ID, "Library ID"),
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
    (ids::PROPERTIES_PHYSICAL_NAME, "Physical Name"),
    (ids::PROPERTIES_POWER_NET, "Power Net"),
    (ids::PROPERTIES_HIGH_SPEED, "High Speed"),
    (ids::PROPERTIES_DIFF_PAIR, "Differential Pair"),
    (ids::PROPERTIES_DIFF_PAIR_NONE, "None"),
    (ids::PROPERTIES_TAB_ALL, "All"),
    (ids::PROPERTIES_TAB_PARAMETERS, "Parameters"),
    (ids::PROPERTIES_TAB_RULES, "Rules"),
    (ids::PROPERTIES_TAB_CLASSES, "Classes"),
    (ids::PROPERTIES_NET_PARAMS_NAME, "Name"),
    (ids::PROPERTIES_NET_PARAMS_VALUE, "Value"),
    (ids::PROPERTIES_NET_PARAMS_ADD, "Add \u{25BE}"),
    (ids::PROPERTIES_NET_NO_PARAMETERS, "No Parameters"),
    (ids::PROPERTIES_NET_NO_RULES, "No Rules"),
    (ids::PROPERTIES_NET_NO_CLASSES, "No Classes"),
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
    (ids::PROPERTIES_BORDER_COLOUR, "Border Colour"),
    (ids::PROPERTIES_FILL_COLOUR, "Fill Colour"),
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
    (ids::PROPERTIES_PAPER_WIDTH, "Width"),
    (ids::PROPERTIES_PAPER_HEIGHT, "Height"),
    (ids::PROPERTIES_ORIENTATION, "Orientation"),
    (ids::PROPERTIES_ORIENTATION_LANDSCAPE, "Landscape"),
    (ids::PROPERTIES_ORIENTATION_PORTRAIT, "Portrait"),
    (ids::PROPERTIES_SHOW_BORDER, "Show Border"),
    (ids::PROPERTIES_SHOW_TITLE_BLOCK, "Show Title Block"),
    (ids::PROPERTIES_ZONES_VERTICAL, "Vertical"),
    (ids::PROPERTIES_ZONES_HORIZONTAL, "Horizontal"),
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
    // Numbers go in as `format!` wrote them, so 1000 is not grouped.
    assert_eq!(
        with(ids::PANELS_PLACED_DEGREES, &[("degrees", "270".into())]),
        "270 Degrees"
    );
    assert_eq!(
        with(ids::PROPERTIES_PIN_TITLE, &[("pin", "A1".into())]),
        "Pin A1"
    );
    assert_eq!(
        with(ids::PROPERTIES_PINS_TITLE, &[("count", "1200".into())]),
        "Pins (1200)"
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
            ids::PROPERTIES_PAPER_DIMENSIONS,
            &[("width", "1189".into()), ("height", "841".into())]
        ),
        "Width: 1189mm  Height: 841mm"
    );
    assert_eq!(
        with(
            ids::PROPERTIES_TITLE_BLOCK_FROM_PROJECT,
            &[("field", "Drawn By".into())]
        ),
        "Drawn By (project)"
    );
    assert_eq!(
        with(
            ids::PROPERTIES_TITLE_BLOCK_OVERRIDES_PROJECT,
            &[("field", "Title".into())]
        ),
        "Title (overrides project)"
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

/// The panel built from a real context shows its section titles and its
/// rows from the catalog: upper-cased titles, a label beside its value.
#[test]
fn the_tab_form_shows_its_sections_and_rows_from_the_catalog() {
    let shown = texts(&net_label_form(0.0));
    let has = |label: &str| shown.iter().any(|t| t.text == label);
    for label in ["LOCATION", "PROPERTIES", "(X/Y)", "Rotation", "Net Name"] {
        assert!(has(label), "{label:?} missing from {shown:#?}");
    }
    let xy = shown.iter().find(|t| t.text == "(X/Y)").expect("label");
    let expected = pre_placement_readout(
        net_label_form(0.0).pre_placement.as_ref().expect("form"),
        &net_label_form(0.0),
    );
    let value = shown
        .iter()
        .find(|t| t.text == expected)
        .unwrap_or_else(|| panic!("the X/Y reading {expected:?} in {shown:#?}"));
    assert!(
        value.bounds.x > xy.bounds.x && (value.bounds.y - xy.bounds.y).abs() < 4.0,
        "the reading sits beside its label: {xy:?} {value:?}"
    );
}

/// The Rotation pick list names its entries as `format!("{:.0} Degrees")`
/// did and hands back the rotation itself: Command + wheel over the closed
/// list picks the next entry, 90 after 0.
#[test]
fn the_rotation_pick_list_is_typed() {
    let en = I18n::en_us();
    let names: Vec<String> = [0.0, 90.0, 180.0, 270.0, 45.0]
        .into_iter()
        .map(|deg| properties_parameters_rotation_label(&en, deg))
        .collect();
    assert_eq!(
        names,
        [
            "0 Degrees",
            "90 Degrees",
            "180 Degrees",
            "270 Degrees",
            "45 Degrees"
        ]
    );

    let ctx = net_label_form(0.0);
    let shown = texts(&ctx);
    let label = shown
        .iter()
        .find(|t| t.text == "Rotation")
        .expect("the Rotation row");
    let over_list = iced::Point::new(
        WINDOW.width - 60.0,
        label.bounds.y + label.bounds.height / 2.0,
    );
    let events = [
        iced::Event::Keyboard(iced::keyboard::Event::ModifiersChanged(
            iced::keyboard::Modifiers::COMMAND,
        )),
        iced::Event::Mouse(iced::mouse::Event::WheelScrolled {
            delta: iced::mouse::ScrollDelta::Lines { x: 0.0, y: -1.0 },
        }),
    ];
    let messages = messages_for_events(
        view_panel(PanelKind::Properties, &ctx),
        WINDOW,
        over_list,
        &events,
    );
    let picked: Vec<f64> = messages
        .into_iter()
        .filter_map(|m| match m {
            PanelMsg::SetPrePlacementRotation(deg) => Some(deg),
            _ => None,
        })
        .collect();
    assert_eq!(picked, [90.0], "the list hands back the next rotation");
}

/// A stored 89.9999 highlights the 90 entry, as the string-matched list
/// did (`{:.0}` reads 90): Command + wheel picks the entry after it, 180.
#[test]
fn a_rotation_that_rounds_to_a_step_selects_that_step() {
    let ctx = net_label_form(89.9999);
    let shown = texts(&ctx);
    assert_eq!(beside(&shown, "Rotation"), "90 Degrees");
    let label = shown
        .iter()
        .find(|t| t.text == "Rotation")
        .expect("the Rotation row");
    let over_list = iced::Point::new(
        WINDOW.width - 60.0,
        label.bounds.y + label.bounds.height / 2.0,
    );
    let events = [
        iced::Event::Keyboard(iced::keyboard::Event::ModifiersChanged(
            iced::keyboard::Modifiers::COMMAND,
        )),
        iced::Event::Mouse(iced::mouse::Event::WheelScrolled {
            delta: iced::mouse::ScrollDelta::Lines { x: 0.0, y: -1.0 },
        }),
    ];
    let picked: Vec<f64> = messages_for_events(
        view_panel(PanelKind::Properties, &ctx),
        WINDOW,
        over_list,
        &events,
    )
    .into_iter()
    .filter_map(|m| match m {
        PanelMsg::SetPrePlacementRotation(deg) => Some(deg),
        _ => None,
    })
    .collect();
    assert_eq!(picked, [180.0], "the 90 entry is the selected one");
}

/// The name a rotation entry shows.
fn properties_parameters_rotation_label(i18n: &I18n, degrees: f64) -> String {
    crate::panels::properties_parameters::rotation_choice(i18n, degrees).label
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
    assert_eq!(beside(&shown, "Electrical Type"), "Passive");
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

/// The Border Colour row opens the border picker and the Fill Colour row
/// the fill picker.
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
        click("Border Colour")
            .iter()
            .any(|m| matches!(m, PanelMsg::ToggleChildSheetBorderPicker(i) if *i == id)),
        "Border Colour opens the border picker"
    );
    assert!(
        click("Fill Colour")
            .iter()
            .any(|m| matches!(m, PanelMsg::ToggleChildSheetFillPicker(i) if *i == id)),
        "Fill Colour opens the fill picker"
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
    check("Orientation", beside(&shown, "Orientation"), "Portrait");
    check("Origin", beside(&shown, "Origin"), "Lower Left");
    check("Line", beside(&shown, "Line"), "Dashed");

    // A wire's Width and a power port's Style are segmented controls and
    // tile grids now (#43, #45); `selected::tests` and `power_port::tests`
    // check the entry they mark.
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

    let turned = net_label_form(90.0);
    check(
        "Rotation",
        beside(&texts(&turned), "Rotation"),
        "90 Degrees",
    );
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// Pressing a power-port style tile asks for that style's library id (#45:
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

/// A title-block field the sheet leaves empty names the project's value; one
/// the sheet fills says it overrides it.
#[test]
fn a_title_block_label_says_where_its_value_comes_from() {
    let mut ctx = document_options();
    ctx.project_parameters
        .insert("drawn_by".into(), "Ann".into());
    let shown = texts(&ctx);
    assert!(
        shown.iter().any(|t| t.text == "Drawn By (project)"),
        "{shown:#?}"
    );
    ctx.title_block.insert("drawn_by".into(), "Bob".into());
    let shown = texts(&ctx);
    assert!(
        shown
            .iter()
            .any(|t| t.text == "Drawn By (overrides project)"),
        "{shown:#?}"
    );
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
/// wire's End and Length moved into Location, #43; `placed_rows_tests`
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

/// A Blanket's two sections keep their titles above their rows.
#[test]
fn a_blankets_sections_keep_their_titles() {
    let ctx = one_selected(
        SelectedKindForTest::Directive,
        &[("Type", "Blanket"), ("Net Class", "PWR")],
    );
    let shown = texts(&ctx);
    let all = plain(&shown);
    let at = |text: &str| all.iter().position(|t| *t == text);
    assert!(
        at("BLANKET").is_some() && at("BLANKET") < at("Net Class"),
        "{all:?}"
    );
    assert!(
        at("ERC EXCLUSIONS").is_some() && at("ERC EXCLUSIONS") < at("All Rules"),
        "{all:?}"
    );
}

/// A text note's Rotation and Justify rows are segmented controls (#47):
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
/// all-letter text is compared; digits and punctuation are skipped.
fn same_in_en_xa(mut ctx: PanelContext) -> Vec<String> {
    let english: Vec<String> = texts(&ctx).into_iter().map(|t| t.text).collect();
    ctx.i18n = std::sync::Arc::new(I18n::new(&signex_i18n::LanguagePref::Locale(
        "en-XA".into(),
    )));
    let xa: Vec<String> = texts(&ctx).into_iter().map(|t| t.text).collect();
    let is_words = |t: &str| {
        t.chars().any(char::is_alphabetic)
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
