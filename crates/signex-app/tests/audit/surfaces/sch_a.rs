//! Mockup group `sch-a` (rows m40-m57).
//!
//! The sample sheet has parts, wires, net labels, power ports and text
//! notes. A sheet symbol, a rectangle and a blanket are added in memory
//! (never saved) in the empty area right of the circuit, x 160-190 mm.

use super::checks;
use super::helpers_c::{self, click_pick, expect_selected};
use crate::fixture;
use crate::render;
use crate::surface::{Check, Surface, WindowSel};
use signex_app::active_bar::SelectionFilter;
use signex_app::app::{Message, Signex, Tool};
use signex_app::canvas::{BoxDirection, CanvasEvent};
use signex_app::dock::DockMessage;
use signex_app::library::LibraryMessage;
use signex_app::panels::{
    MultiKind, ObjectListStatus, PanelKind, PanelMsg, SchFilterMsg, SelectionFormMsg,
};
use signex_app::toolbar::ToolMessage;
use signex_types::schematic::{
    BlanketPour, ChildSheet, FillType, Point, SchDirective, SchDrawing, SchematicSheet,
    SelectedKind, SheetPin,
};
use uuid::Uuid;

/// The added sheet symbol: top-left corner and size, mm.
const SHEET_SYMBOL_AT: (f64, f64) = (160.02, 30.48);
const SHEET_SYMBOL_SIZE: (f64, f64) = (30.48, 20.32);
/// The added rectangle's corners, mm.
const RECT: ((f64, f64), (f64, f64)) = ((160.02, 81.28), (190.5, 101.6));
/// The added blanket's corners, mm.
const BLANKET: ((f64, f64), (f64, f64)) = ((160.02, 111.76), (190.5, 132.08));

pub fn rows() -> Vec<Surface> {
    vec![
        Surface {
            id: "m40",
            title: "Properties: Document Options (General / Parameters)",
            // Nothing selected: the General tab. The Parameters tab is the
            // check below.
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                helpers_c::show_panel(app, PanelKind::Properties)
            },
            window: WindowSel::Auto,
            checks: DOCUMENT_OPTIONS,
        },
        Surface {
            id: "m41",
            title: "Properties: pre-placement",
            // The Net label tool paused with Tab; the Wire and Junction
            // states are the same flow with Tool::Wire / Tool::Junction.
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                helpers_c::show_panel(app, PanelKind::Properties)?;
                let _ = app.update(Message::Tool(ToolMessage::SelectTool(Tool::Label)));
                let _ = app.update(Message::Tool(ToolMessage::PrePlacementTab));
                Ok(())
            },
            window: WindowSel::Auto,
            checks: PRE_PLACEMENT,
        },
        Surface {
            id: "m42",
            title: "Properties: Part",
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                // Click U1 (88.9, 101.6 mm) so Properties shows the component.
                let _ = app.update(Message::CanvasEvent(CanvasEvent::Clicked {
                    world_x: 88.9,
                    world_y: 101.6,
                }));
                let _ = render::draw_main(r, app);
                Ok(())
            },
            window: WindowSel::Auto,
            checks: SCHEMATIC,
        },
        Surface {
            id: "m43",
            title: "Properties: Wire / bus",
            // A plain wire. The mock's open colour menu and the Bus state
            // are not driven.
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                helpers_c::show_panel(app, PanelKind::Properties)?;
                // The +5V wire from 66.04 to 78.74 mm at y 99.06.
                click_pick(app, (72.39, 99.06), SelectedKind::Wire)
            },
            window: WindowSel::Auto,
            checks: WIRE,
        },
        Surface {
            id: "m44",
            title: "Properties: Net label / port",
            // The net label state; the sample has no global port.
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                helpers_c::show_panel(app, PanelKind::Properties)?;
                // The +5V label sits at (66.04, 99.06), its text above the
                // wire. A left-to-right box takes only what is wholly inside
                // it, so not the wires that run past.
                helpers_c::box_select(app, (65.0, 96.0), (70.5, 99.8), BoxDirection::LeftToRight)?;
                expect_selected(app, SelectedKind::Label)
            },
            window: WindowSel::Auto,
            checks: LABEL,
        },
        Surface {
            id: "m45",
            title: "Properties: Power port",
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                helpers_c::show_panel(app, PanelKind::Properties)?;
                // The stem of #PWR02 (+3V3) above its pin at (104.14, 99.06).
                click_pick(app, (104.14, 98.0), SelectedKind::Symbol)
            },
            window: WindowSel::Auto,
            checks: SYMBOL,
        },
        Surface {
            id: "m46",
            title: "Properties: Sheet symbol",
            // The sheet symbol with its file present. The "File missing"
            // state is the same symbol without power.snxsch on disk.
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                write_child_sheet(fx)?;
                helpers_c::add_to_sheet(app, add_sheet_symbol)?;
                helpers_c::show_panel(app, PanelKind::Properties)?;
                let centre = (
                    SHEET_SYMBOL_AT.0 + SHEET_SYMBOL_SIZE.0 / 2.0,
                    SHEET_SYMBOL_AT.1 + SHEET_SYMBOL_SIZE.1 / 2.0,
                );
                click_pick(app, centre, SelectedKind::ChildSheet)
            },
            window: WindowSel::Auto,
            checks: SHEET_SYMBOL,
        },
        Surface {
            id: "m47",
            title: "Properties: Text note",
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                helpers_c::show_panel(app, PanelKind::Properties)?;
                // The title note, left-justified at (40, 60).
                click_pick(app, (45.0, 60.0), SelectedKind::TextNote)
            },
            window: WindowSel::Auto,
            checks: TEXT_NOTE,
        },
        Surface {
            id: "m48",
            title: "Properties: Drawing shape",
            // The rectangle state; the Arc state is not driven.
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                helpers_c::add_to_sheet(app, add_rectangle)?;
                helpers_c::show_panel(app, PanelKind::Properties)?;
                let ((x1, y1), (x2, _)) = RECT;
                click_pick(app, ((x1 + x2) / 2.0, y1), SelectedKind::Drawing)
            },
            window: WindowSel::Auto,
            checks: DRAWING,
        },
        Surface {
            id: "m49",
            title: "Properties: Directive / No-ERC",
            // The blanket state; a No-ERC directive is the other one.
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                helpers_c::add_to_sheet(app, add_blanket)?;
                helpers_c::show_panel(app, PanelKind::Properties)?;
                // A blanket is picked at its edge; a click inside reaches
                // what it covers.
                let ((x1, y1), (x2, _)) = BLANKET;
                click_pick(app, ((x1 + x2) / 2.0, y1), SelectedKind::Directive)
            },
            window: WindowSel::Auto,
            checks: DIRECTIVE,
        },
        Surface {
            id: "m50",
            title: "Properties: Pin (placed)",
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                helpers_c::show_panel(app, PanelKind::Properties)?;
                // U1's input pin runs right from (78.74, 99.06); the click
                // lands on the pin, past the wire end.
                click_pick(app, (80.01, 99.06), SelectedKind::Symbol)
            },
            window: WindowSel::Auto,
            checks: PIN,
        },
        Surface {
            id: "m51",
            title: "Properties: Multi-selection",
            // The mixed selection; "Nothing shared" (only junctions and
            // no-connects) is not driven.
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                helpers_c::show_panel(app, PanelKind::Properties)?;
                // A touching box over the whole circuit: parts, labels, wires.
                helpers_c::box_select(app, (145.0, 90.0), (40.0, 125.0), BoxDirection::RightToLeft)
            },
            window: WindowSel::Auto,
            checks: MULTI,
        },
        Surface {
            id: "m52",
            title: "Components panel",
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                fixture::mount_stock(app, fx)?;
                fixture::select_panel(app, PanelKind::Components)
            },
            window: WindowSel::Auto,
            checks: SCHEMATIC,
        },
        Surface {
            id: "m53",
            title: "Components row context menu + drag label",
            // The row menu; the drag label needs a pointer drag and is not
            // driven.
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                fixture::mount_stock(app, fx)?;
                fixture::select_panel(app, PanelKind::Components)?;
                let key = helpers_c::first_stock_row(app, fx)?;
                helpers_c::drive(
                    app,
                    Message::Library(LibraryMessage::ComponentsPanelRowRightClicked(key)),
                )
            },
            window: WindowSel::Main,
            checks: ROW_MENU,
        },
        Surface {
            id: "m54",
            title: "Navigator",
            // Project open; the "No project" state is a fresh app.
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                helpers_c::show_panel(app, PanelKind::Navigator)
            },
            window: WindowSel::Auto,
            checks: SCHEMATIC,
        },
        Surface {
            id: "m55",
            title: "Filter (selection filter)",
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                helpers_c::show_panel(app, PanelKind::Filter)
            },
            window: WindowSel::Auto,
            checks: FILTER,
        },
        Surface {
            id: "m56",
            title: "SCH Filter",
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                helpers_c::show_panel(app, PanelKind::SchFilter)
            },
            window: WindowSel::Auto,
            checks: FILTER,
        },
        Surface {
            id: "m57",
            title: "Object List",
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                helpers_c::show_panel(app, PanelKind::ObjectList)
            },
            window: WindowSel::Auto,
            checks: OBJECT_LIST,
        },
    ]
}

// ── Added objects ──

/// A file for the sheet symbol, so Properties shows it as present.
fn write_child_sheet(fx: &fixture::Fixture) -> Result<(), String> {
    let dir = fx.sample.parent().ok_or("the sample has no folder")?;
    let sheet = SchematicSheet {
        uuid: Uuid::now_v7(),
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    let text = signex_types::format::SnxSchematic::new(sheet)
        .write_string()
        .map_err(|e| format!("encode power.snxsch: {e:?}"))?;
    std::fs::write(dir.join("power.snxsch"), text).map_err(|e| format!("write power.snxsch: {e}"))
}

fn sheet_pin(name: &str, direction: &str, at: (f64, f64)) -> SheetPin {
    SheetPin {
        uuid: Uuid::now_v7(),
        name: name.to_string(),
        direction: direction.to_string(),
        position: Point::new(at.0, at.1),
        rotation: 0.0,
        auto_generated: false,
        user_moved: false,
    }
}

fn add_sheet_symbol(sheet: &mut SchematicSheet) {
    let (x, y) = SHEET_SYMBOL_AT;
    let (w, _) = SHEET_SYMBOL_SIZE;
    sheet.child_sheets.push(ChildSheet {
        uuid: Uuid::now_v7(),
        name: "Power".to_string(),
        filename: "power.snxsch".to_string(),
        position: Point::new(x, y),
        size: SHEET_SYMBOL_SIZE,
        stroke_width: 0.0,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: vec![
            sheet_pin("VIN", "input", (x, y + 5.08)),
            sheet_pin("EN", "input", (x, y + 10.16)),
            sheet_pin("+3V3", "output", (x + w, y + 5.08)),
        ],
        instances: Vec::new(),
    });
}

fn add_rectangle(sheet: &mut SchematicSheet) {
    let ((x1, y1), (x2, y2)) = RECT;
    sheet.drawings.push(SchDrawing::Rect {
        uuid: Uuid::now_v7(),
        start: Point::new(x1, y1),
        end: Point::new(x2, y2),
        width: 0.0,
        fill: FillType::None,
        stroke_color: None,
        locked: false,
    });
}

fn add_blanket(sheet: &mut SchematicSheet) {
    let ((x1, y1), (x2, y2)) = BLANKET;
    sheet.directives.push(SchDirective::Blanket {
        uuid: Uuid::now_v7(),
        start: Point::new(x1, y1),
        end: Point::new(x2, y2),
        net_class: None,
        erc_exclusions: vec![
            "builtin::unused_pin".to_string(),
            "builtin::dangling_wire".to_string(),
        ],
        track_width: None,
        pour: BlanketPour::Normal,
        include_edge_nets: false,
    });
}

// ── Checks ──

const SCHEMATIC: &[Check] = &[("a schematic tab is active", checks::schematic_tab_active)];

const DOCUMENT_OPTIONS: &[Check] = &[
    ("nothing is selected", nothing_selected),
    ("the Parameters tab opens", parameters_tab_opens),
];

const PRE_PLACEMENT: &[Check] = &[
    ("Tab paused the placement", placement_paused),
    ("the net label tool is armed", label_tool_armed),
];

const WIRE: &[Check] = &[("a wire is selected", wire_selected)];
const LABEL: &[Check] = &[("a net label is selected", label_selected)];
const SYMBOL: &[Check] = &[("a symbol is selected", symbol_selected)];
const SHEET_SYMBOL: &[Check] = &[("a sheet symbol is selected", sheet_symbol_selected)];
const TEXT_NOTE: &[Check] = &[("a text note is selected", text_note_selected)];
const DRAWING: &[Check] = &[("a drawing is selected", drawing_selected)];
const DIRECTIVE: &[Check] = &[("a directive is selected", directive_selected)];

const PIN: &[Check] = &[
    ("the part is selected", symbol_selected),
    ("the click picked a pin", pin_clicked),
];

const MULTI: &[Check] = &[
    ("more than one object is selected", several_selected),
    (
        "a kind chip narrows the sections, not the selection",
        kind_chip_filters_sections,
    ),
];

const ROW_MENU: &[Check] = &[("the row menu is open", row_menu_open)];

const FILTER: &[Check] = &[
    ("a schematic tab is active", checks::schematic_tab_active),
    (
        "toggling Wires takes wires out of the filter",
        wires_toggle_off,
    ),
];

const OBJECT_LIST: &[Check] = &[("the list is ready", object_list_ready)];

fn nothing_selected(app: &mut Signex) -> Result<(), String> {
    match helpers_c::selected_kinds(app).as_slice() {
        [] => Ok(()),
        kinds => Err(format!("selected: {kinds:?}")),
    }
}

fn parameters_tab_opens(app: &mut Signex) -> Result<(), String> {
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::PropertiesTab(
        1,
    ))));
    match app.document_state.panel_ctx.properties_tab {
        1 => Ok(()),
        tab => Err(format!("properties_tab is {tab}")),
    }
}

fn placement_paused(app: &mut Signex) -> Result<(), String> {
    if app.interaction_state.active_canvas().placement_paused {
        Ok(())
    } else {
        Err("placement_paused is false".into())
    }
}

fn label_tool_armed(app: &mut Signex) -> Result<(), String> {
    match app.interaction_state.current_tool {
        Tool::Label => Ok(()),
        tool => Err(format!("current tool is {tool:?}")),
    }
}

fn wire_selected(app: &mut Signex) -> Result<(), String> {
    expect_selected(app, SelectedKind::Wire)
}

fn label_selected(app: &mut Signex) -> Result<(), String> {
    expect_selected(app, SelectedKind::Label)
}

fn symbol_selected(app: &mut Signex) -> Result<(), String> {
    expect_selected(app, SelectedKind::Symbol)
}

fn sheet_symbol_selected(app: &mut Signex) -> Result<(), String> {
    expect_selected(app, SelectedKind::ChildSheet)
}

fn text_note_selected(app: &mut Signex) -> Result<(), String> {
    expect_selected(app, SelectedKind::TextNote)
}

fn drawing_selected(app: &mut Signex) -> Result<(), String> {
    expect_selected(app, SelectedKind::Drawing)
}

fn directive_selected(app: &mut Signex) -> Result<(), String> {
    expect_selected(app, SelectedKind::Directive)
}

fn pin_clicked(app: &mut Signex) -> Result<(), String> {
    if app.interaction_state.clicked_pin.is_some() {
        Ok(())
    } else {
        Err("clicked_pin is None".into())
    }
}

fn several_selected(app: &mut Signex) -> Result<(), String> {
    match helpers_c::selected_kinds(app).len() {
        n if n > 1 => Ok(()),
        n => Err(format!("{n} objects selected")),
    }
}

fn kind_chip_filters_sections(app: &mut Signex) -> Result<(), String> {
    let before = helpers_c::selected_kinds(app);
    let picked = SelectionFormMsg::KindChipPicked(Some(MultiKind::Wires));
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::SelectionForm(
        picked,
    ))));
    let filter = app.document_state.panel_ctx.selection_form.kind_filter;
    if filter != Some(MultiKind::Wires) {
        return Err(format!("kind_filter is {filter:?}"));
    }
    if helpers_c::selected_kinds(app) != before {
        return Err("the chip changed the selection".into());
    }
    Ok(())
}

fn row_menu_open(app: &mut Signex) -> Result<(), String> {
    if app.library.components_panel.row_menu.is_some() {
        Ok(())
    } else {
        Err("components_panel.row_menu is None".into())
    }
}

fn wires_toggle_off(app: &mut Signex) -> Result<(), String> {
    let toggled = PanelMsg::SchFilter(SchFilterMsg::KindToggled(SelectionFilter::Wires));
    let _ = app.update(Message::Dock(DockMessage::Panel(toggled)));
    if app
        .interaction_state
        .selection_filters
        .contains(&SelectionFilter::Wires)
    {
        Err("Wires is still in the selection filter".into())
    } else {
        Ok(())
    }
}

fn object_list_ready(app: &mut Signex) -> Result<(), String> {
    let status = app.document_state.panel_ctx.object_list.status;
    if status == ObjectListStatus::Ready {
        Ok(())
    } else {
        Err(format!("object list status is {status:?}"))
    }
}
