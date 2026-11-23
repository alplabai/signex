//! Further states of mockup groups `sch-a`, `sch-b` and `new-sch`, each
//! reached through `Signex::update` on a fixture that holds what the row
//! draws. `open` fails when the state was not reached.
//!
//! Filter preset messages drop their task: it writes the user's preferences.

use super::canvas_pick::pick_in;
use super::helpers_c::{self, click_pick, drive, drive_in, expect_selected};
use super::helpers_d::{self, reached};
use super::helpers_f as h;
use super::states::{self, child_sheet, edit_sample_file, root_sheet, write_sheet};
use crate::fixture::{self, Fixture};
use crate::surface::{OpenFn, Surface, WindowSel};
use signex_app::active_bar::SelectionFilter;
use signex_app::app::{
    CompiledDesignMsg, EditMsg, Message, PasteIntent, PasteRequest, Signex, UiMsg,
};
use signex_app::canvas::BoxDirection;
use signex_app::dock::DockMessage;
use signex_app::library::LibraryMessage;
use signex_app::panels::compiled_design::NamingTarget;
use signex_app::panels::net_inspector::{NetInspectorTab, NetListStatus};
use signex_app::panels::{NetInspectorMsg, PanelKind, PanelMsg, SchFilterMsg, SheetInfo};
use signex_app::paste_array::PasteArrayMsg;
use signex_app::task_driver::TaskDriver;
use signex_library::AlternateSymbol;
use signex_types::net_class::NetClass;
use signex_types::schematic::{
    Bus, FillType, Junction, LabelType, NoConnect, NoErcDirective, Point, SchDrawing,
    SchematicSheet, SelectedKind, SheetPin,
};
use uuid::Uuid;

/// The stock library, its `STOCK-R` row, that row's default symbol and
/// default footprint.
const STOCK_LIBRARY: Uuid = Uuid::from_u128(0x019df45d_af5e_7a00_8c3b_dd3bf89b40ae);
const STOCK_RESISTOR: Uuid = Uuid::from_u128(0x01a18461_b555_75c3_995a_0a8a51e590a4);
const STOCK_R_SYMBOL: Uuid = Uuid::from_u128(0xbf88a609_74df_4453_a656_d68ef86e421d);
const STOCK_R_FOOTPRINT: &str = "019df45d-af63-7823-b5c3-01594028ccef.snxfpt";
/// A format minor no build has written, so the sheet opens read-only.
const NEWER_FORMAT: &str = "format = \"snxsch/1.99\"";
/// The centre of a sheet symbol placed by `states::child_sheet` at x 101.6.
const CHILD_CENTRE: (f64, f64) = (127.0, 95.25);
/// The Properties sections above a sheet symbol's Entries.
const ABOVE_ENTRIES: [&str; 3] = [
    "sel_location",
    "sel_child_sheet_props",
    "sel_child_sheet_style",
];

fn row(id: &'static str, title: &'static str, open: OpenFn) -> Surface {
    Surface {
        id,
        title,
        open,
        window: WindowSel::Auto,
        checks: &[],
    }
}

pub fn rows() -> Vec<Surface> {
    let mut rows = properties_rows();
    rows.extend(panel_rows());
    rows.extend(sch_b_rows());
    rows.extend(new_sch_rows());
    rows
}

fn properties_rows() -> Vec<Surface> {
    vec![
        row("m42-library", "Part linked to a library row", open_part),
        row("m43-bus-drawn", "Bus read from the file", open_bus),
        row("m44-port", "Global port", open_global_port),
        row("m44-net-class", "Net label with a class", open_label_class),
        row("m46-entries", "Sheet symbol: Entries", open_sheet_entries),
        row(
            "m46-missing",
            "Sheet symbol: file missing",
            open_sheet_missing,
        ),
        row("m48-arc", "Drawing shape: arc", open_arc),
        row("m49-no-erc", "No-ERC directive", open_no_erc),
        row("m51-nothing", "Nothing shared", open_nothing_shared),
    ]
}

fn panel_rows() -> Vec<Surface> {
    vec![
        Surface {
            window: WindowSel::Main,
            ..row("m53-drag", "Components row dragged", open_row_drag)
        },
        row("m54-hierarchy", "Navigator: hierarchy", open_navigator_tree),
        row("m54-no-project", "Navigator: no project", |app, _, _| {
            helpers_c::show_panel(app, PanelKind::Navigator)?;
            reached(
                app.document_state.panel_ctx.navigator.tree.is_empty(),
                "the Navigator lists a project",
            )
        }),
        row(
            "m56-presets",
            "SCH Filter: saved presets",
            open_filter_presets,
        ),
    ]
}

fn sch_b_rows() -> Vec<Surface> {
    vec![
        row(
            "m58-component",
            "Net Inspector: By Component",
            open_by_component,
        ),
        row("m58-no-match", "Net Inspector: no match", |app, r, fx| {
            fixture::open_sample(app, r, fx);
            helpers_c::show_panel(app, PanelKind::NetInspector)?;
            inspector(app, NetInspectorMsg::FilterChanged("NO_SUCH_NET_42".into()))?;
            let view = &app.document_state.panel_ctx.net_inspector;
            reached(
                view.nets.is_empty() && view.total > 0,
                "the filter did not empty the list",
            )
        }),
        row(
            "m58-no-project",
            "Net Inspector: no project",
            |app, _, _| {
                helpers_c::show_panel(app, PanelKind::NetInspector)?;
                reached(
                    app.document_state.panel_ctx.net_inspector.status == NetListStatus::NoProject,
                    "the Net Inspector does not say No project",
                )
            },
        ),
        row(
            "m69-spaced",
            "Paste Array at its default spacing",
            open_paste_spaced,
        ),
        row("m74-read-only", "Sheet from a newer Signex", open_read_only),
    ]
}

fn new_sch_rows() -> Vec<Surface> {
    vec![
        row(
            "m151-details",
            "Repeat, naming and entries",
            |app, r, fx| {
                open_compiled_repeat(app, r, fx)?;
                fold(app, &ABOVE_ENTRIES);
                Ok(())
            },
        ),
        row("m151-naming", "Channel naming menu", |app, r, fx| {
            open_compiled_repeat(app, r, fx)?;
            fold(app, &ABOVE_ENTRIES);
            h::settle(
                app,
                CompiledDesignMsg::NamingMenuToggled(NamingTarget::Designators).into(),
            )?;
            reached(
                app.ui_state.compiled_design.naming_menu.is_some(),
                "the naming menu did not open",
            )
        }),
        row(
            "m151-channel",
            "Channel tab of a repeated sheet",
            open_channel,
        ),
        row(
            "m156-compiled",
            "Contents table after a compile",
            open_contents,
        ),
    ]
}

// ── Shared steps ──

fn fold(app: &mut Signex, keys: &[&str]) {
    for key in keys {
        let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::ToggleSection(
            (*key).to_string(),
        ))));
    }
}

fn inspector(app: &mut Signex, msg: NetInspectorMsg) -> Result<(), String> {
    drive(
        app,
        Message::Dock(DockMessage::Panel(PanelMsg::NetInspector(msg))),
    )
}

/// Whether the project tree marks a child sheet as missing.
fn tree_marks_missing(rows: &[SheetInfo]) -> bool {
    rows.iter()
        .any(|s| (s.is_child_reference && s.missing) || tree_marks_missing(&s.children))
}

fn project_marks_missing(app: &Signex) -> bool {
    app.document_state
        .panel_ctx
        .projects
        .iter()
        .any(|p| tree_marks_missing(&p.sheets))
}

fn pin(name: &str, direction: &str, at: (f64, f64)) -> SheetPin {
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

/// Entries on the left and right edge of a `states::child_sheet` symbol.
fn add_entries(sheet: &mut SchematicSheet) {
    if let Some(symbol) = sheet.child_sheets.first_mut() {
        let (x, y) = (symbol.position.x, symbol.position.y);
        let right = x + symbol.size.0;
        symbol.pins = vec![
            pin("VIN", "input", (x, y + 7.62)),
            pin("EN", "input", (x, y + 15.24)),
            pin("VOUT", "output", (right, y + 7.62)),
            pin("PGOOD", "output", (right, y + 15.24)),
        ];
    }
}

// ── sch-a ──

/// R1 bound to `STOCK-R`, whose row offers its footprints and, added here to
/// the open row, a second symbol; the file carries a datasheet link.
fn open_part(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    let footprint = stock_footprint_name(fx)?;
    edit_sample_file(fx, |sheet| {
        if let Some(part) = sheet.symbols.iter_mut().find(|s| s.reference == "R1") {
            part.library_id = Some(STOCK_LIBRARY);
            part.row_id = Some(STOCK_RESISTOR);
            part.lib_id = format!("{STOCK_LIBRARY}:{STOCK_R_SYMBOL}");
            part.footprint = footprint;
            part.datasheet = "https://example.invalid/datasheets/chip-resistor.pdf".to_string();
        }
    })?;
    fixture::open_sample(app, r, fx);
    fixture::mount_stock(app, fx)?;
    add_alternate_symbol(app)?;
    helpers_c::show_panel(app, PanelKind::Properties)?;
    let (id, x, y) = {
        let doc = app
            .document_state
            .active_engine()
            .ok_or("no active sheet")?
            .document();
        let part = doc
            .symbols
            .iter()
            .find(|s| s.reference == "R1")
            .ok_or("no R1 on the sheet")?;
        (part.uuid, part.position.x, part.position.y)
    };
    pick_in(
        app,
        (x - 3.81, y - 3.81),
        (x + 3.81, y + 3.81),
        id,
        SelectedKind::Symbol,
    )?;
    let ctx = &app.document_state.panel_ctx;
    reached(
        ctx.footprint_choices.len() >= 2 && ctx.symbol_choices.len() >= 2,
        "Properties lists no footprint and symbol choices for R1",
    )
}

/// The display name of `STOCK-R`'s default footprint, so R1 holds it unedited.
fn stock_footprint_name(fx: &Fixture) -> Result<String, String> {
    let path = fx
        .stock
        .parent()
        .ok_or("the stock library has no folder")?
        .join("footprints")
        .join(STOCK_R_FOOTPRINT);
    let text = std::fs::read_to_string(&path).map_err(|e| format!("read footprint: {e}"))?;
    text.lines()
        .find_map(|line| line.strip_prefix("name = \""))
        .and_then(|rest| rest.strip_suffix('"'))
        .map(str::to_string)
        .ok_or_else(|| format!("{} names no footprint", path.display()))
}

/// Offer another stock row's symbol as an alternate of `STOCK-R`.
fn add_alternate_symbol(app: &mut Signex) -> Result<(), String> {
    let libraries = &mut app.library.open_libraries;
    let other = libraries
        .iter()
        .flat_map(|l| l.tables.values())
        .flatten()
        .find(|row| row.row_id != STOCK_RESISTOR && row.symbol_ref.uuid != STOCK_R_SYMBOL)
        .map(|row| row.symbol_ref)
        .ok_or("the stock library has no second symbol")?;
    let resistor = libraries
        .iter_mut()
        .flat_map(|l| l.tables.values_mut())
        .flatten()
        .find(|row| row.row_id == STOCK_RESISTOR)
        .ok_or("the stock library has no STOCK-R row")?;
    resistor.alternate_symbols.push(AlternateSymbol {
        symbol_ref: other,
        symbol_version: String::new(),
        label: "IEC".to_string(),
    });
    Ok(())
}

/// A bus in the file, so the first draw of the sheet holds its stroke.
fn open_bus(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    edit_sample_file(fx, |sheet| {
        sheet.buses.push(Bus {
            uuid: Uuid::from_u128(0x4302),
            start: Point::new(160.02, 40.64),
            end: Point::new(190.5, 40.64),
            stroke_width: 0.0,
            color: None,
        });
    })?;
    fixture::open_sample(app, r, fx);
    helpers_c::show_panel(app, PanelKind::Properties)?;
    click_pick(app, (175.26, 40.64), SelectedKind::Bus)
}

/// A global port made from the sample's first label.
fn open_global_port(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    let id = Uuid::from_u128(0x4401);
    edit_sample_file(fx, |sheet| {
        let Some(mut port) = sheet.labels.first().cloned() else {
            return;
        };
        port.uuid = id;
        port.text = "VBUS".to_string();
        port.position = Point::new(165.1, 60.96);
        port.label_type = LabelType::Global;
        port.shape = "input".to_string();
        sheet.labels.push(port);
    })?;
    fixture::open_sample(app, r, fx);
    helpers_c::show_panel(app, PanelKind::Properties)?;
    pick_in(app, (160.02, 58.42), (177.8, 63.5), id, SelectedKind::Label).map(|_| ())
}

/// The +5V label in a project with a class table, once the project netlist
/// has named the label's net.
fn open_label_class(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    fixture::open_sample(app, r, fx);
    let project = helpers_d::sample_project(app, fx);
    if let Some(loaded) = helpers_d::project_mut(app, project) {
        loaded.data.net_classes = vec![NetClass {
            nets: vec!["+5V".to_string()],
            ..NetClass::named("Power")
        }];
    }
    helpers_c::show_panel(app, PanelKind::Properties)?;
    helpers_c::box_select(app, (65.0, 96.0), (70.5, 99.8), BoxDirection::LeftToRight)?;
    expect_selected(app, SelectedKind::Label)?;
    // The first pick settled the class job; the picker follows on the next.
    helpers_c::box_select(app, (65.0, 96.0), (70.5, 99.8), BoxDirection::LeftToRight)?;
    reached(
        app.document_state.panel_ctx.net_classes.label.is_some(),
        "the label has no Net Class row",
    )
}

/// A project whose root places `file` once; `entries` adds its entries.
fn open_one_child(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
    folder: &str,
    file: &str,
    entries: bool,
) -> Result<(), String> {
    let dir = h::new_dir(fx, folder)?;
    let mut sheets = vec!["top.snxsch"];
    if entries {
        h::copy_sample_sheet(fx, &dir.join(file))?;
        sheets.push(file);
    }
    let mut root = root_sheet(vec![child_sheet("Power", file, 101.6)]);
    if entries {
        add_entries(&mut root);
    }
    let top = dir.join("top.snxsch");
    write_sheet(&top, root)?;
    let prj = h::write_project(&dir, "dcdc-buck", &sheets, None)?;
    h::open(app, &prj)?;
    h::open(app, &top)?;
    h::show_sheet(app, r);
    helpers_c::show_panel(app, PanelKind::Properties)?;
    click_pick(app, CHILD_CENTRE, SelectedKind::ChildSheet)
}

fn open_sheet_entries(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
) -> Result<(), String> {
    open_one_child(app, r, fx, "sheet-entries", "power.snxsch", true)?;
    fold(app, &ABOVE_ENTRIES);
    Ok(())
}

fn open_sheet_missing(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
) -> Result<(), String> {
    open_one_child(app, r, fx, "sheet-missing", "debug.snxsch", false)?;
    reached(
        project_marks_missing(app),
        "the project tree does not mark debug.snxsch missing",
    )
}

fn open_arc(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    let id = Uuid::from_u128(0x4801);
    edit_sample_file(fx, |sheet| {
        sheet.drawings.push(SchDrawing::Arc {
            uuid: id,
            start: Point::new(165.1, 88.9),
            mid: Point::new(175.26, 78.74),
            end: Point::new(185.42, 88.9),
            width: 0.0,
            fill: FillType::None,
            stroke_color: None,
            locked: false,
        });
    })?;
    fixture::open_sample(app, r, fx);
    helpers_c::show_panel(app, PanelKind::Properties)?;
    pick_in(
        app,
        (173.99, 77.47),
        (176.53, 80.01),
        id,
        SelectedKind::Drawing,
    )
    .map(|_| ())
}

/// A No-ERC directive naming no rule, so All rules reads as on.
fn open_no_erc(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    let id = Uuid::from_u128(0x4901);
    edit_sample_file(fx, |sheet| {
        sheet.no_erc_directives.push(NoErcDirective {
            uuid: id,
            position: Point::new(170.18, 60.96),
            rules: Vec::new(),
        });
    })?;
    fixture::open_sample(app, r, fx);
    helpers_c::show_panel(app, PanelKind::Properties)?;
    pick_in(
        app,
        (167.64, 58.42),
        (172.72, 63.5),
        id,
        SelectedKind::NoErcDirective,
    )
    .map(|_| ())
}

fn junction_at(n: u128, x: f64) -> Junction {
    Junction {
        uuid: Uuid::from_u128(0x5100 + n),
        position: Point::new(x, 139.7),
        diameter: 0.0,
        minted: false,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

/// Two junctions and a no-connect: a selection with no shared field.
fn open_nothing_shared(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
) -> Result<(), String> {
    edit_sample_file(fx, |sheet| {
        sheet.junctions.push(junction_at(1, 165.1));
        sheet.junctions.push(junction_at(2, 172.72));
        sheet.no_connects.push(NoConnect {
            uuid: Uuid::from_u128(0x5103),
            position: Point::new(180.34, 139.7),
        });
    })?;
    fixture::open_sample(app, r, fx);
    helpers_c::show_panel(app, PanelKind::Properties)?;
    helpers_c::box_select(
        app,
        (162.56, 137.16),
        (182.88, 142.24),
        BoxDirection::LeftToRight,
    )?;
    let kinds = helpers_c::selected_kinds(app);
    let only_marks = kinds
        .iter()
        .all(|k| matches!(k, SelectedKind::Junction | SelectedKind::NoConnect));
    reached(
        kinds.len() == 3 && only_marks,
        "the box did not take just the junctions and the no-connect",
    )
}

/// A row pressed and moved past the drag threshold, still over the dock, so
/// the label with the part's thumbnail follows the pointer.
fn open_row_drag(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    fixture::open_sample(app, r, fx);
    fixture::mount_stock(app, fx)?;
    fixture::select_panel(app, PanelKind::Components)?;
    let key = helpers_c::first_stock_row(app, fx)?;
    let _ = app.update(Message::Ui(UiMsg::DragMove(180.0, 320.0)));
    drive(
        app,
        Message::Library(LibraryMessage::ComponentsPanelRowPressed(key)),
    )?;
    drive(app, Message::Ui(UiMsg::DragMove(236.0, 372.0)))?;
    reached(
        app.library
            .components_panel
            .drag
            .as_ref()
            .is_some_and(|drag| drag.started),
        "the press did not become a drag",
    )
}

/// A root placing `power` twice (it holds a child sheet), `mcu` and a file
/// that is gone, with one project page no sheet places.
fn open_navigator_tree(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
) -> Result<(), String> {
    let dir = h::new_dir(fx, "navigator")?;
    for file in ["ldo.snxsch", "mcu.snxsch", "notes.snxsch"] {
        h::copy_sample_sheet(fx, &dir.join(file))?;
    }
    write_sheet(
        &dir.join("power.snxsch"),
        root_sheet(vec![child_sheet("LDO", "ldo.snxsch", 101.6)]),
    )?;
    let top = dir.join("top.snxsch");
    write_sheet(
        &top,
        root_sheet(vec![
            child_sheet("Power A", "power.snxsch", 12.7),
            child_sheet("Power B", "power.snxsch", 68.58),
            child_sheet("MCU", "mcu.snxsch", 124.46),
            child_sheet("Debug", "debug.snxsch", 180.34),
        ]),
    )?;
    let prj = h::write_project(
        &dir,
        "dcdc-buck",
        &[
            "top.snxsch",
            "power.snxsch",
            "ldo.snxsch",
            "mcu.snxsch",
            "notes.snxsch",
        ],
        None,
    )?;
    h::open(app, &prj)?;
    h::open(app, &top)?;
    h::show_sheet(app, r);
    helpers_c::show_panel(app, PanelKind::Navigator)?;
    reached(
        project_marks_missing(app),
        "the project tree does not mark debug.snxsch missing",
    )
}

/// Two saved presets, the second named Wiring. Each task is dropped, so the
/// presets stay out of the user's preferences file.
fn open_filter_presets(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
) -> Result<(), String> {
    fixture::open_sample(app, r, fx);
    helpers_c::show_panel(app, PanelKind::SchFilter)?;
    let before = app.interaction_state.custom_filter_presets.len();
    let wiring = before + 1;
    for msg in [
        SchFilterMsg::PresetSaved,
        SchFilterMsg::KindToggled(SelectionFilter::Wires),
        SchFilterMsg::PresetSaved,
        SchFilterMsg::PresetRenamed(wiring, "Wiring".into()),
        SchFilterMsg::PresetApplied(wiring),
    ] {
        let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::SchFilter(msg))));
    }
    reached(
        app.interaction_state.custom_filter_presets.len() == before + 2,
        "the two presets were not saved",
    )
}

// ── sch-b ──

fn open_by_component(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    fixture::open_sample(app, r, fx);
    helpers_c::show_panel(app, PanelKind::NetInspector)?;
    inspector(
        app,
        NetInspectorMsg::TabSelected(NetInspectorTab::ByComponent),
    )?;
    let key = app
        .document_state
        .panel_ctx
        .net_inspector
        .components
        .iter()
        .find(|part| part.designator == "U1")
        .map(|part| part.key.clone())
        .ok_or("By Component does not list U1")?;
    inspector(app, NetInspectorMsg::ComponentToggled(key))
}

/// R1 alone copied into Paste Array with four copies, keeping the spacing
/// the dialog worked out from the copy's width.
fn open_paste_spaced(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    fixture::open_sample(app, r, fx);
    h::select_parts(app, &["R1"])?;
    // One driver, so the copy is on its clipboard for the paste.
    let mut driver = TaskDriver::new();
    drive_in(&mut driver, app, Message::Edit(EditMsg::Copy))?;
    drive_in(
        &mut driver,
        app,
        Message::Edit(EditMsg::PasteArrayRequested),
    )?;
    if app.ui_state.paste_array.is_none() {
        // The clipboard read went unanswered: "nothing usable" pastes the
        // copy made in the window.
        let request = PasteRequest {
            intent: PasteIntent::Array,
            epoch: app.interaction_state.placement_epoch,
            path: app.document_state.active_path.clone(),
            dialog: 0,
            from_agent: false,
        };
        drive(
            app,
            Message::Edit(EditMsg::PasteTextRead {
                request,
                text: None,
            }),
        )?;
    }
    reached(
        app.ui_state.paste_array.is_some(),
        "Paste Array did not open",
    )?;
    let _ = app.update(Message::Edit(EditMsg::PasteArray(
        PasteArrayMsg::CountChanged("4".into()),
    )));
    Ok(())
}

/// The sample with its format token raised to a minor this build cannot
/// write, which is what a sheet saved by a newer Signex holds.
fn open_read_only(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    let path = fx.sample.with_extension("snxsch");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("read sample: {e}"))?;
    let (_, body) = text.split_once('\n').ok_or("the sample has one line")?;
    std::fs::write(&path, format!("{NEWER_FORMAT}\n{body}"))
        .map_err(|e| format!("write sample: {e}"))?;
    fixture::open_sample(app, r, fx);
    let read_only = app
        .document_state
        .active_engine()
        .is_some_and(|engine| engine.document().is_read_only());
    reached(read_only, "the sheet did not open read-only")
}

// ── new-sch ──

/// The repeated sheet with entries, compiled and picked with a click.
fn open_compiled_repeat(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
) -> Result<(), String> {
    h::open_multichannel(app, r, fx)?;
    helpers_c::add_to_sheet(app, add_entries)?;
    h::settle(app, CompiledDesignMsg::CompileRequested.into())?;
    helpers_c::show_panel(app, PanelKind::Properties)?;
    click_pick(app, CHILD_CENTRE, SelectedKind::ChildSheet)
}

/// The repeated sheet itself, on the tab of its second channel.
fn open_channel(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    let top = h::open_multichannel(app, r, fx)?;
    h::settle(app, CompiledDesignMsg::CompileRequested.into())?;
    let sheet = top.with_file_name("channel.snxsch");
    h::open(app, &sheet)?;
    h::settle(
        app,
        CompiledDesignMsg::ChannelPicked {
            sheet: sheet.clone(),
            channel: Some(2),
        }
        .into(),
    )?;
    h::show_sheet(app, r);
    reached(
        app.ui_state.compiled_design.channel_tabs.get(&sheet) == Some(&2),
        "the sheet did not switch to channel 2",
    )
}

/// The contents sheet added after a compile, so its table lists the pages.
fn open_contents(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    states::open_hierarchy(app, r, fx)?;
    helpers_d::sample_project(app, fx);
    h::settle(app, CompiledDesignMsg::CompileRequested.into())?;
    h::command(app, "add_table_of_contents")?;
    let contents = h::new_dir(fx, "hierarchy")?.join("contents.snxsch");
    h::open(app, &contents)?;
    h::show_sheet(app, r);
    let filled = app
        .document_state
        .active_engine()
        .ok_or("no active sheet")?
        .document()
        .drawings
        .iter()
        .any(|d| matches!(d, SchDrawing::Table(t) if !t.rows.is_empty()));
    reached(filled, "the contents table has no rows after the compile")
}
