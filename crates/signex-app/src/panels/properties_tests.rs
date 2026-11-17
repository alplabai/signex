//! #2059 part C: the Properties panel reads its text from the catalog, in
//! the English the code wrote before, and its pick lists hand back values.

use super::*;
use crate::layout_probe::{LaidOutText, lay_out_texts, messages_for_events};
use signex_i18n::{Arg, I18n, MessageId, ids};

const WINDOW: iced::Size = iced::Size::new(360.0, 6000.0);

/// Every message this part added with no argument, and the literal the
/// panel showed before it.
const PLAIN: &[(MessageId, &str)] = &[
    (ids::PROPERTIES_SECTION_LOCATION, "Location"),
    (ids::PROPERTIES_SECTION_PROPERTIES, "Properties"),
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
    (ids::PROPERTIES_SECTION_PARAMETERS_NONE, "Parameters (none)"),
    (ids::PROPERTIES_SECTION_PAGE_OPTIONS, "Page Options"),
    (ids::PROPERTIES_SECTION_TITLE_BLOCK, "Title Block"),
    (
        ids::PROPERTIES_SECTION_SELECTION_FILTERS,
        "Custom Selection Filters",
    ),
    (ids::PROPERTIES_XY, "(X/Y)"),
    (ids::PROPERTIES_ROTATION, "Rotation"),
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
    (ids::PROPERTIES_NO_CUSTOM_PARAMETERS, "No custom parameters"),
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
    (ids::PROPERTIES_FOOTPRINT_ONE_PAD, "1 pad"),
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
        with(
            ids::PROPERTIES_ROTATION_DEGREES,
            &[("degrees", "270".into())]
        ),
        "270 Degrees"
    );
    assert_eq!(
        with(
            ids::PROPERTIES_SECTION_PARAMETERS_COUNT,
            &[("count", "1000".into())]
        ),
        "Parameters (1000)"
    );
    assert_eq!(
        with(ids::PROPERTIES_PIN_TITLE, &[("pin", "A1".into())]),
        "Pin A1"
    );
    assert_eq!(
        with(ids::PROPERTIES_PINS_TITLE, &[("count", "1200".into())]),
        "Pins (1200)"
    );
    assert_eq!(
        with(ids::PROPERTIES_FOOTPRINT_PADS, &[("count", "1000".into())]),
        "1000 pads"
    );
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

/// The name a rotation entry shows.
fn properties_parameters_rotation_label(i18n: &I18n, degrees: f64) -> String {
    crate::panels::properties_parameters::rotation_choice(i18n, degrees).label
}
