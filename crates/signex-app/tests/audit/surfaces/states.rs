//! Further states of mockup rows, captured as `<row>-<state>-<theme>.png`.
//!
//! A row's own entry keeps its checks; a state that would contradict them
//! (m117 opens on the BOM) is an entry here. `open` fails when the state was
//! not reached, so a wrong render is never saved under a state's name.

use super::helpers_c::{self, click_pick, drive};
use super::helpers_d::{self, command, reached};
use super::helpers_f as h;
use super::{annot_valid, project};
use crate::fixture::{self, Fixture};
use crate::render::{self, Crop};
use crate::settle_support;
use crate::surface::{OpenFn, Surface, WindowSel};
use iced::widget::text_editor::{Action, Edit};
use signex_app::app::{
    AnnotateMsg, BoardOutputMsg, CommandPaletteMsg, EditMsg, ErcMsg, Message, OverlayMsg,
    ParameterManagerMsg, PreferencesMsg, ProjectMsg, ProjectSection, ProjectTreeAction,
    RulesTabMsg, SheetNumberingMsg, Signex, Tool,
};
use signex_app::canvas::CanvasEvent;
use signex_app::dock::DockMessage;
use signex_app::menu_bar::MenuMessage;
use signex_app::panels::{ErcWaiverMsg, LayersPanelMsg, LayersTab, PanelKind, PanelMsg};
use signex_app::preferences::PrefNav;
use signex_app::toolbar::ToolMessage;
use signex_output::BoardReportKind;
use signex_types::format::{SnxPcb, SnxSchematic};
use signex_types::project::{LibraryEntry, LibraryEntryKind};
use signex_types::schematic::{Bus, ChildSheet, FillType, Point, SchematicSheet, SelectedKind};
use signex_widgets::passive_calculator::CalculatorMessage;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

/// The right dock, where the History panel sits.
const RIGHT_DOCK: Crop = Crop {
    x: 880,
    y: 0,
    w: 560,
    h: 900,
};

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
    let mut rows = schematic_rows();
    rows.extend(project_rows());
    rows.extend(validation_rows());
    rows
}

fn schematic_rows() -> Vec<Surface> {
    vec![
        row("m9-query", "Command palette with a query", |app, r, fx| {
            fixture::open_sample(app, r, fx);
            helpers_d::sample_project(app, fx);
            let _ = app.update(Message::CommandPalette(CommandPaletteMsg::Open));
            settle_support::settle(
                app,
                Message::CommandPalette(CommandPaletteMsg::QueryChanged("GND".into())),
            );
            reached(app.ui_state.command_palette.open, "the palette closed")
        }),
        row("m17-result", "Passive calculator result", |app, _, _| {
            settle_support::settle(app, Message::Menu(MenuMessage::OpenPassiveCalculator));
            for msg in [
                CalculatorMessage::TargetChanged("4.7".into()),
                CalculatorMessage::Calculate,
            ] {
                settle_support::settle(app, Message::PassiveCalculator(msg));
            }
            reached(
                app.ui_state.passive_calculator_open,
                "the calculator closed",
            )
        }),
        row(
            "m40-parameters",
            "Document Options: Parameters",
            |app, r, fx| {
                fixture::open_sample(app, r, fx);
                helpers_c::show_panel(app, PanelKind::Properties)?;
                let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::PropertiesTab(
                    1,
                ))));
                reached(
                    app.document_state.panel_ctx.properties_tab == 1,
                    "the Parameters tab did not open",
                )
            },
        ),
        row("m41-wire", "Pre-placement: wire", |app, r, fx| {
            pre_placement(app, r, fx, Tool::Wire)
        }),
        row("m41-junction", "Pre-placement: junction", |app, r, fx| {
            pre_placement(app, r, fx, Tool::Junction)
        }),
        row("m43-bus", "Properties: bus", |app, r, fx| {
            fixture::open_sample(app, r, fx);
            helpers_c::show_panel(app, PanelKind::Properties)?;
            helpers_c::add_to_sheet(app, |sheet| {
                sheet.buses.push(Bus {
                    uuid: Uuid::from_u128(0x4301),
                    start: Point::new(160.02, 40.64),
                    end: Point::new(190.5, 40.64),
                });
            })?;
            click_pick(app, (175.26, 40.64), SelectedKind::Bus)
        }),
        row(
            "m43-colour",
            "Properties: wire colour popover",
            |app, r, fx| {
                fixture::open_sample(app, r, fx);
                helpers_c::show_panel(app, PanelKind::Properties)?;
                click_pick(app, (72.39, 99.06), SelectedKind::Wire)?;
                let _ = app.update(Message::Dock(DockMessage::Panel(
                    PanelMsg::ToggleWireColorPicker,
                )));
                Ok(())
            },
        ),
        row("m60-snippets", "Snippets panel", |app, r, fx| {
            fixture::open_sample(app, r, fx);
            helpers_c::show_panel(app, PanelKind::Snippets)
        }),
        row("m60-todo", "To-Do panel", |app, r, fx| {
            fixture::open_sample(app, r, fx);
            helpers_c::show_panel(app, PanelKind::Todo)
        }),
        row(
            "m70-edited",
            "Parameter Manager with edits",
            open_parameter_edits,
        ),
        row(
            "m151-selected",
            "Repeated sheet symbol selected",
            |app, r, fx| {
                h::open_multichannel(app, r, fx)?;
                // A real click, so the selection reaches Properties.
                click_pick(app, (127.0, 95.25), SelectedKind::ChildSheet)
            },
        ),
    ]
}

fn project_rows() -> Vec<Surface> {
    vec![
        Surface {
            id: "m24-child",
            title: "Remove a placed child sheet",
            open: open_remove_child,
            window: WindowSel::Auto,
            checks: project::REMOVE,
        },
        row(
            "m33-libraries",
            "Project tab: mounted libraries",
            open_libraries,
        ),
        row("m35-repository", "Project tab: repository", |app, r, fx| {
            commit_sample(fx)?;
            fixture::open_sample(app, r, fx);
            let project = app
                .document_state
                .projects
                .first_mut()
                .ok_or("the sample project is not loaded")?;
            project.data.enable_git = true;
            project::show_section(app, r, ProjectSection::VersionControl)
        }),
        Surface {
            id: "m39-commits",
            title: "History panel with a commit",
            open: |app, r, fx| {
                commit_sample(fx)?;
                fixture::open_sample(app, r, fx);
                settle_support::settle(
                    app,
                    Message::Overlay(OverlayMsg::OpenPanel(PanelKind::History)),
                );
                fixture::select_panel(app, PanelKind::History)?;
                let _ = render::draw_main(r, app);
                Ok(())
            },
            window: WindowSel::MainCrop(RIGHT_DOCK),
            checks: project::HISTORY,
        },
        row("m75-project", "Annotate on a hierarchy", |app, r, fx| {
            open_hierarchy(app, r, fx)?;
            settle_support::settle(app, Message::Annotate(AnnotateMsg::OpenDialog));
            reached(app.ui_state.annotate_dialog_open, "Annotate did not open")
        }),
        row("m78-clash", "Number Sheets with a clash", |app, r, fx| {
            open_hierarchy(app, r, fx)?;
            command(app, "number_schematic_sheets")?;
            reached(
                app.ui_state.sheet_numbering.is_some(),
                "number_schematic_sheets opened no dialog",
            )?;
            settle_support::settle(
                app,
                Message::from(SheetNumberingMsg::NumberEdited {
                    row: 1,
                    text: "1".into(),
                }),
            );
            Ok(())
        }),
        row(
            "m117-net-lengths",
            "Board Reports: Net Lengths",
            |app, r, fx| {
                fixture::open_board(app, r, fx);
                settle_support::settle(
                    app,
                    Message::from(BoardOutputMsg::ReportRequested(BoardReportKind::NetLength)),
                );
                Ok(())
            },
        ),
        row("m143-annotation", "Preferences: Annotation", |app, _, _| {
            settle_support::settle(app, Message::Preferences(PreferencesMsg::Open));
            settle_support::settle(
                app,
                Message::Preferences(PreferencesMsg::Nav(PrefNav::Annotation)),
            );
            reached(app.ui_state.preferences_open, "Preferences did not open")
        }),
    ]
}

fn validation_rows() -> Vec<Surface> {
    vec![
        row("m85-findings", "ERC panel with findings", |app, r, fx| {
            open_erc_findings(app, r, fx)
        }),
        row("m85-waive", "ERC: stale, waive form open", open_erc_waive),
        row("m88-findings", "DRC panel with findings", open_drc_findings),
        row("m91-error", "Rules text with an error", |app, r, fx| {
            let project = annot_valid::open_rules(app, r, fx)?;
            settle_support::settle(
                app,
                Message::Project(ProjectMsg::Rules(RulesTabMsg::TextViewToggled {
                    project_id: project,
                    on: true,
                })),
            );
            let text = app
                .document_state
                .project_tabs
                .get_mut(&project)
                .and_then(|tab| tab.rules.text.as_mut())
                .ok_or("the Rules tab shows no text view")?;
            let broken = Arc::new("rule \"clearance\" {\n    min_clearance = \n".to_string());
            text.perform(Action::Edit(Edit::Paste(broken)));
            reached(text.edited, "the rule text was not typed in")
        }),
        row(
            "m103-view-options",
            "Layers panel: View Options",
            |app, r, fx| {
                fixture::open_board(app, r, fx);
                settle_support::settle(app, Message::Menu(MenuMessage::OpenLayersPanel));
                fixture::select_panel(app, PanelKind::Layers)?;
                let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::LayersPanel(
                    LayersPanelMsg::TabPicked(LayersTab::ViewOptions),
                ))));
                let _ = render::draw_main(r, app);
                Ok(())
            },
        ),
    ]
}

/// Rewrite the sample sheet copy before it is opened, so the open, the
/// project's sheet reads and a run all see the same document.
pub(super) fn edit_sample_file(
    fx: &Fixture,
    edit: impl FnOnce(&mut SchematicSheet),
) -> Result<(), String> {
    let path = fx.sample.with_extension("snxsch");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("read sample: {e}"))?;
    let mut sheet = SnxSchematic::parse(&text)
        .map_err(|e| format!("parse sample: {e:?}"))?
        .sheet;
    edit(&mut sheet);
    write_sheet(&path, sheet)
}

fn write_sheet(path: &Path, sheet: SchematicSheet) -> Result<(), String> {
    let text = SnxSchematic::new(sheet)
        .write_string()
        .map_err(|e| format!("encode sheet: {e:?}"))?;
    std::fs::write(path, text).map_err(|e| format!("write sheet: {e}"))
}

fn child_sheet(name: &str, filename: &str, x: f64) -> ChildSheet {
    ChildSheet {
        uuid: Uuid::now_v7(),
        name: name.to_string(),
        filename: filename.to_string(),
        position: Point::new(x, 76.2),
        size: (50.8, 38.1),
        stroke_width: 0.0,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: Vec::new(),
        instances: Vec::new(),
    }
}

fn root_sheet(children: Vec<ChildSheet>) -> SchematicSheet {
    SchematicSheet {
        uuid: Uuid::now_v7(),
        paper_size: "A4".to_string(),
        child_sheets: children,
        ..Default::default()
    }
}

fn pre_placement(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
    tool: Tool,
) -> Result<(), String> {
    fixture::open_sample(app, r, fx);
    helpers_c::show_panel(app, PanelKind::Properties)?;
    let _ = app.update(Message::Tool(ToolMessage::SelectTool(tool)));
    let _ = app.update(Message::Tool(ToolMessage::PrePlacementTab));
    reached(
        app.interaction_state.active_canvas().placement_paused,
        "Tab did not pause the placement",
    )
}

/// A custom column and three edited cells.
fn open_parameter_edits(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
) -> Result<(), String> {
    fixture::open_sample(app, r, fx);
    drive(app, Message::ParameterManager(ParameterManagerMsg::Open))?;
    for msg in [
        ParameterManagerMsg::NewKeyChanged("Tolerance".into()),
        ParameterManagerMsg::ColumnAdded,
    ] {
        let _ = app.update(Message::ParameterManager(msg));
    }
    let parts: Vec<Uuid> = app
        .document_state
        .active_engine()
        .ok_or("no active schematic engine")?
        .document()
        .symbols
        .iter()
        .take(3)
        .map(|s| s.uuid)
        .collect();
    for symbol_uuid in parts {
        let _ = app.update(Message::ParameterManager(ParameterManagerMsg::Edit {
            symbol_uuid,
            key: "Tolerance".into(),
            value: "1%".into(),
        }));
    }
    reached(
        app.ui_state.parameter_manager_open,
        "the Parameter Manager closed",
    )
}

/// The Remove dialog on a child sheet its parent places once.
fn open_remove_child(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    let dir = h::new_dir(fx, "child-sheet")?;
    let child = dir.join("linear-regulator.snxsch");
    h::copy_sample_sheet(fx, &child)?;
    let top = dir.join("top.snxsch");
    write_sheet(
        &top,
        root_sheet(vec![child_sheet("Power", "linear-regulator.snxsch", 101.6)]),
    )?;
    let prj = h::write_project(
        &dir,
        "linear-regulator",
        &["top.snxsch", "linear-regulator.snxsch"],
        None,
    )?;
    h::open(app, &prj)?;
    h::open(app, &child)?;
    h::open(app, &top)?;
    h::show_sheet(app, r);
    let leaf = project::sheet_key(app)?;
    project::tree_action(app, ProjectTreeAction::OpenRemoveDialog(leaf));
    reached(
        app.ui_state.remove_dialog.is_some(),
        "the Remove dialog did not open on the child sheet",
    )
}

/// One library of each kind; the in-project one is not on disk.
fn open_libraries(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    fixture::open_sample(app, r, fx);
    let entry = |path: PathBuf, kind| LibraryEntry {
        path,
        kind,
        library_id: None,
        unknown: Default::default(),
    };
    let entries = vec![
        entry(
            PathBuf::from("libs/Power.snxlib"),
            LibraryEntryKind::ProjectLocal,
        ),
        entry(fx.stock.clone(), LibraryEntryKind::Shared),
        entry(
            fx.stock.with_file_name("Gone.snxlib"),
            LibraryEntryKind::Global,
        ),
    ];
    app.document_state
        .projects
        .first_mut()
        .ok_or("the sample project is not loaded")?
        .data
        .libraries = entries;
    project::show_section(app, r, ProjectSection::Libraries)
}

/// A repository in the sample folder with its files committed, under an
/// identity of its own so the machine's git config is never needed.
fn commit_sample(fx: &Fixture) -> Result<(), String> {
    let git = |e: git2::Error| format!("git: {e}");
    let dir = fx.sample.parent().ok_or("the sample has no directory")?;
    let repo = git2::Repository::init(dir).map_err(git)?;
    let mut config = repo.config().map_err(git)?;
    config.set_str("user.name", "Audit").map_err(git)?;
    config
        .set_str("user.email", "audit@example.invalid")
        .map_err(git)?;
    let mut index = repo.index().map_err(git)?;
    index
        .add_all(["*"], git2::IndexAddOption::DEFAULT, None)
        .map_err(git)?;
    index.write().map_err(git)?;
    let tree = repo
        .find_tree(index.write_tree().map_err(git)?)
        .map_err(git)?;
    let who = git2::Signature::now("Audit", "audit@example.invalid").map_err(git)?;
    repo.commit(Some("HEAD"), &who, &who, "Add the sample", &tree, &[])
        .map_err(git)?;
    Ok(())
}

/// A root placing two copies of the sample, with unannotated parts and, on
/// the second sheet, a locked U1.
fn open_hierarchy(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    let dir = h::new_dir(fx, "hierarchy")?;
    let source = std::fs::read_to_string(fx.sample.with_extension("snxsch"))
        .map_err(|e| format!("read sample: {e}"))?;
    for (file, lock) in [("power.snxsch", false), ("mcu.snxsch", true)] {
        let mut sheet = SnxSchematic::parse(&source)
            .map_err(|e| format!("parse sample: {e:?}"))?
            .sheet;
        sheet.uuid = Uuid::now_v7();
        for part in &mut sheet.symbols {
            part.uuid = Uuid::now_v7();
            match part.reference.as_str() {
                "R1" => part.reference = "R?".to_string(),
                "C1" => part.reference = "C?".to_string(),
                "U1" => part.locked = lock,
                _ => {}
            }
        }
        write_sheet(&dir.join(file), sheet)?;
    }
    let top = dir.join("top.snxsch");
    write_sheet(
        &top,
        root_sheet(vec![
            child_sheet("Power", "power.snxsch", 50.8),
            child_sheet("MCU", "mcu.snxsch", 127.0),
        ]),
    )?;
    let prj = h::write_project(
        &dir,
        "dcdc-buck",
        &["top.snxsch", "power.snxsch", "mcu.snxsch"],
        None,
    )?;
    h::open(app, &prj)?;
    h::open(app, &top)?;
    h::show_sheet(app, r);
    Ok(())
}

/// An ERC run on the sample with C1 renamed to a second R1.
fn open_erc_findings(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    edit_sample_file(fx, |sheet| {
        if let Some(part) = sheet.symbols.iter_mut().find(|s| s.reference == "C1") {
            part.reference = "R1".to_string();
        }
    })?;
    fixture::open_sample(app, r, fx);
    helpers_d::sample_project(app, fx);
    settle_support::settle(app, Message::Erc(ErcMsg::Run));
    settle_support::settle(app, Message::Menu(MenuMessage::OpenErcPanel));
    fixture::select_panel(app, PanelKind::Erc)?;
    let _ = render::draw_main(r, app);
    reached(
        !app.ui_state.erc_violations.is_empty(),
        "the run found nothing on a sheet with a duplicate designator",
    )
}

/// The findings made stale by an edit, with the first row's waive form open.
fn open_erc_waive(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    open_erc_findings(app, r, fx)?;
    let _ = app.update(Message::CanvasEvent(CanvasEvent::Clicked {
        world_x: 88.9,
        world_y: 101.6,
    }));
    let _ = app.update(Message::Edit(EditMsg::RotateSelectedClockwise));
    let (key, message, sheet) = app
        .document_state
        .panel_ctx
        .erc_diagnostics
        .iter()
        .find_map(|e| {
            let key = e.waiver_key.clone()?;
            Some((key, e.message.clone(), e.sheet_path.clone()))
        })
        .ok_or("no finding can be waived")?;
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::ErcWaiver(
        ErcWaiverMsg::Started {
            key,
            message,
            sheet,
        },
    ))));
    let _ = render::draw_main(r, app);
    reached(
        app.ui_state.erc_waive_draft.is_some(),
        "Started opened no waive form",
    )
}

/// The primitives board with a VCC track 0.1 mm from its GND track.
fn open_drc_findings(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    let mut board = helpers_d::primitives_board()?;
    board.nets.push(
        serde_json::from_value(serde_json::json!({"number": 2, "name": "VCC"}))
            .map_err(|e| format!("net: {e}"))?,
    );
    let mut track = board.segments.first().cloned().ok_or("no track")?;
    track.uuid = Uuid::from_u128(0xD0B);
    track.start.y = 0.1;
    track.end.y = 0.1;
    track.net = 2;
    board.segments.push(track);
    let path = fx.pcb.with_file_name("drc-findings.snxpcb");
    let text = SnxPcb::new(board)
        .write_string()
        .map_err(|e| format!("encode board: {e:?}"))?;
    std::fs::write(&path, text).map_err(|e| format!("write board: {e}"))?;
    h::open(app, &path)?;
    h::show_board(app, r);
    command(app, "run_drc")?;
    fixture::select_panel(app, PanelKind::Drc)?;
    let _ = render::draw_main(r, app);
    let found = app
        .document_state
        .panel_ctx
        .pcb_drc
        .as_ref()
        .is_some_and(|d| d.ran && d.clearance + d.short > 0);
    reached(found, "the run reported no clearance or short finding")
}
