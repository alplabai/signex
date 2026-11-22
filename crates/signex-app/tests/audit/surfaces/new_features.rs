//! Mockup groups `new-*` (rows m151-m193f), one file for all of them:
//! new-sch, new-release, new-advanced, new-pour, new-boardsketch,
//! new-pcbsim, new-fpsketch and new-gerber. The mockups draw their own demo
//! projects; each row opens the closest state on the fixture's documents.
//! A row whose surface the app does not have returns `not built`.

use super::checks;
use super::helpers_f::{self as h, REPEATED_SHEET};
use super::states_more;
use crate::fixture;
use crate::render::{self, Crop};
use crate::surface::{Check, Surface, WindowSel};
use signex_app::app::{
    CompareBoardMsg, CompiledDesignMsg, EditMsg, FileMsg, Message, PasteIntent, PasteRequest,
    PdfOptionMsg, PrintPreviewMsg, Signex, StatusBarRequest, UiMsg,
};
use signex_app::library::editor::footprint::parameters_table::SketchParametersMsg;
use signex_app::library::editor::footprint::{EditorMode, SketchTool};
use signex_app::library::messages::FootprintEditorMsg;
use signex_app::menu_bar::MenuMessage;
use signex_app::panels::PanelKind;
use signex_app::panels::compiled_design::CompiledStatus;
use signex_app::status_bar::BoardCounter;
use signex_types::schematic::{SchDrawing, SchematicSheet};

pub fn rows() -> Vec<Surface> {
    let mut rows = sch_rows();
    rows.extend(release_rows());
    rows.extend(advanced_rows());
    rows.extend(board_sketch_rows());
    rows.extend(pcb_sim_rows());
    rows.extend(footprint_sketch_rows());
    rows.extend(gerber_rows());
    rows
}

/// A row for a surface the app does not have: `open` says why.
fn missing(id: &'static str, title: &'static str, open: crate::surface::OpenFn) -> Surface {
    Surface {
        id,
        title,
        open,
        window: WindowSel::Auto,
        checks: &[],
    }
}

// ---- new-sch (m151-m158) --------------------------------------------------

fn sch_rows() -> Vec<Surface> {
    vec![
        Surface {
            id: "m151",
            title: "Multi-channel",
            // The repeated sheet symbol selected: Properties shows its Repeat
            // section. Other states: the Naming menu open
            // (CompiledDesignMsg::NamingMenuToggled), the compiled channels.
            open: super::states::open_repeated_selected,
            window: WindowSel::Auto,
            checks: MULTI_CHANNEL,
        },
        Surface {
            id: "m152",
            title: "Signal harnesses",
            // A harness connector with one entry, picked with a click:
            // Properties shows the harness. The definitions view is
            // HarnessPanelMsg::DefinitionsOpened.
            open: states_more::open_harness_selected,
            window: WindowSel::Auto,
            checks: HARNESS,
        },
        Surface {
            id: "m153",
            title: "Net directives",
            // A net blanket round the circuit, a differential pair mark and
            // Net Parameters on VBUS, picked with a click. The modal
            // (Design > Differential Pair Classes) is
            // `open_diff_pair_classes`.
            open: states_more::open_net_parameters_selected,
            window: WindowSel::Auto,
            checks: DIRECTIVES,
        },
        Surface {
            id: "m154",
            title: "Compiled Design",
            open: |app, r, fx| {
                h::open_multichannel(app, r, fx)?;
                h::command(app, "open_compiled_design")?;
                h::settle(app, CompiledDesignMsg::CompileRequested.into())
            },
            window: WindowSel::Auto,
            checks: COMPILED,
        },
        Surface {
            id: "m155",
            title: "Change Component + Smart Paste",
            // The Smart Paste dialog over a copy of R1. Change Component is
            // the part picker in its ChangeComponent mode (PlacedPartAction).
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                h::select_parts(app, &["R1"])?;
                let _ = app.update(Message::Edit(EditMsg::Copy));
                h::settle(app, Message::Edit(EditMsg::SmartPaste))?;
                if app.ui_state.smart_paste.is_none() {
                    // Nothing on the clipboard: the copy made here is pasted.
                    let request = PasteRequest {
                        intent: PasteIntent::SmartPaste,
                        epoch: app.interaction_state.placement_epoch,
                        path: app.document_state.active_path.clone(),
                        dialog: 0,
                        from_agent: false,
                    };
                    let _ = app.update(Message::Edit(EditMsg::PasteTextRead {
                        request,
                        text: None,
                    }));
                }
                Ok(())
            },
            window: WindowSel::Auto,
            checks: SMART_PASTE,
        },
        Surface {
            id: "m156",
            title: "Schematic tables + table of contents",
            // A filled table beside the circuit, picked with a click. The
            // contents sheet is `m156-contents`.
            open: states_more::open_table_selected,
            window: WindowSel::Auto,
            checks: TABLES,
        },
        Surface {
            id: "m157",
            title: "Bezier + dimension annotation",
            // The bezier picked with a click, so its handles show. The
            // dimension being placed is `m157-placing`.
            open: states_more::open_bezier_selected,
            window: WindowSel::Auto,
            checks: DRAWINGS,
        },
        Surface {
            id: "m158",
            title: "Named unions",
            // The Unions panel over a tagged union of C1, U1 and C2. The
            // Create Union popover is `m158-popover`.
            open: states_more::open_tagged_union,
            window: WindowSel::Auto,
            checks: UNIONS,
        },
    ]
}

const MULTI_CHANNEL: &[Check] = &[("the Repeat switch off writes the plain name", repeat_off)];
const HARNESS: &[Check] = &[("one connector with one entry", one_harness_entry)];
const DIRECTIVES: &[Check] = &[("three directives are placed", three_directives)];
const COMPILED: &[Check] = &[
    ("the panel is docked", compiled_docked),
    ("the compile is ready", compiled_ready),
];
const SMART_PASTE: &[Check] = &[
    ("the Smart Paste dialog is open", smart_paste_open),
    ("Cancel closes it", smart_paste_cancel),
];
const TABLES: &[Check] = &[("a table is placed", has_table)];
const DRAWINGS: &[Check] = &[("a bezier and a dimension are placed", bezier_and_dimension)];
const UNIONS: &[Check] = &[
    ("the union is made", one_union),
    ("the Unions panel is docked", unions_docked),
];

fn sheet(app: &Signex) -> Result<&SchematicSheet, String> {
    Ok(app
        .document_state
        .active_engine()
        .ok_or("no active sheet")?
        .document())
}

fn repeat_off(app: &mut Signex) -> Result<(), String> {
    h::settle(
        app,
        CompiledDesignMsg::RepeatToggled {
            sheet_symbol: REPEATED_SHEET,
            on: false,
        }
        .into(),
    )?;
    let name = sheet(app)?
        .child_sheets
        .first()
        .map(|c| c.name.clone())
        .ok_or("no sheet symbol on the active sheet")?;
    if name.starts_with("Repeat(") {
        Err(format!("the designator is still {name}"))
    } else {
        Ok(())
    }
}

fn one_harness_entry(app: &mut Signex) -> Result<(), String> {
    match sheet(app)?.harness.connectors.as_slice() {
        [c] if c.entries.len() == 1 => Ok(()),
        [c] => Err(format!("the connector has {} entries", c.entries.len())),
        other => Err(format!("{} connectors", other.len())),
    }
}

fn three_directives(app: &mut Signex) -> Result<(), String> {
    match sheet(app)?.directives.len() {
        3 => Ok(()),
        n => Err(format!("{n} directives placed")),
    }
}

fn compiled_docked(app: &mut Signex) -> Result<(), String> {
    h::docked(app, PanelKind::CompiledDesign)
}

fn compiled_ready(app: &mut Signex) -> Result<(), String> {
    let status = &app.document_state.panel_ctx.compiled_design.panel.status;
    if *status == CompiledStatus::Ready {
        Ok(())
    } else {
        Err(format!("status is {status:?}"))
    }
}

fn smart_paste_open(app: &mut Signex) -> Result<(), String> {
    match app.ui_state.smart_paste {
        Some(_) => Ok(()),
        None => Err("smart_paste is None".into()),
    }
}

fn smart_paste_cancel(app: &mut Signex) -> Result<(), String> {
    let _ = app.update(Message::Edit(EditMsg::SmartPasteDialog(
        signex_app::smart_paste::SmartPasteMsg::Cancelled,
    )));
    match app.ui_state.smart_paste {
        None => Ok(()),
        Some(_) => Err("the dialog is still open".into()),
    }
}

fn has_table(app: &mut Signex) -> Result<(), String> {
    let drawings = &sheet(app)?.drawings;
    if drawings.iter().any(|d| matches!(d, SchDrawing::Table(_))) {
        Ok(())
    } else {
        Err("no table on the sheet".into())
    }
}

fn bezier_and_dimension(app: &mut Signex) -> Result<(), String> {
    let drawings = &sheet(app)?.drawings;
    let bezier = drawings.iter().any(|d| matches!(d, SchDrawing::Bezier(_)));
    let dimension = drawings
        .iter()
        .any(|d| matches!(d, SchDrawing::Dimension(_)));
    match (bezier, dimension) {
        (true, true) => Ok(()),
        _ => Err(format!("bezier {bezier}, dimension {dimension}")),
    }
}

fn one_union(app: &mut Signex) -> Result<(), String> {
    match sheet(app)?.unions.as_slice() {
        [u] if u.members.len() == 3 => Ok(()),
        [u] => Err(format!("the union has {} members", u.members.len())),
        other => Err(format!("{} unions", other.len())),
    }
}

fn unions_docked(app: &mut Signex) -> Result<(), String> {
    h::docked(app, PanelKind::Unions)
}

// ---- new-release (m161-m168) ----------------------------------------------

fn release_rows() -> Vec<Surface> {
    vec![
        Surface {
            id: "m161",
            title: "Command Log",
            // Two commands run first, the second refused (nothing to undo),
            // so the log shows an ok and a failed row.
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                h::command(app, "cycle_selection_mode")?;
                h::command(app, "undo")?;
                h::open_panel(app, PanelKind::CommandLog);
                Ok(())
            },
            window: WindowSel::Auto,
            checks: COMMAND_LOG,
        },
        Surface {
            id: "m162",
            title: "Compare Board with Schematic",
            open: |app, r, fx| {
                h::open_board_project(app, r, fx, false)?;
                h::command(app, "compare_board_with_schematic")
            },
            window: WindowSel::Auto,
            checks: COMPARE,
        },
        Surface {
            id: "m163",
            title: "Rooms from Sheets",
            // Design > Rooms > Create Rooms from Sheets on a project whose
            // root places two sheets. The Place > Room tool is
            // `place_board_room`.
            open: states_more::open_create_rooms,
            window: WindowSel::Auto,
            checks: BOARD,
        },
        Surface {
            id: "m164",
            title: "PCB Status-bar Counters",
            // The status bar under the board; a check opens a counter's
            // popover after the capture.
            open: |app, r, fx| {
                fixture::open_board(app, r, fx);
                Ok(())
            },
            window: WindowSel::MainCrop(Crop {
                x: 0,
                y: render::HEIGHT - 48,
                w: render::WIDTH,
                h: 48,
            }),
            checks: COUNTERS,
        },
        Surface {
            id: "m165",
            title: "Annotations",
            open: |app, r, fx| {
                h::open_notebook_project(app, r, fx)?;
                h::settle(app, Message::Menu(MenuMessage::OpenAnnotationsPanel))
            },
            window: WindowSel::Auto,
            checks: ANNOTATIONS,
        },
        Surface {
            id: "m166",
            title: "Workspace Layouts",
            // Window > Layouts > Manage Layouts. The Window > Layouts menu
            // with its hovered-row preview is menu-bar widget state, which
            // update cannot open.
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                h::command(app, "manage_workspace_layouts")
            },
            window: WindowSel::Auto,
            checks: LAYOUTS,
        },
        Surface {
            id: "m167",
            title: "Export Job naming",
            // A naming card: what the Output Jobs surface is called, shown on
            // the Output Jobs panel of the sample.
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                h::open_panel(app, PanelKind::OutputJobs);
                Ok(())
            },
            window: WindowSel::Auto,
            checks: OUTPUT_JOBS,
        },
        Surface {
            id: "m168",
            title: "PDF options: watermark, layers, bookmarks",
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                h::settle(app, Message::PrintPreview(PrintPreviewMsg::Requested))?;
                for msg in [
                    PdfOptionMsg::WatermarkToggled(true),
                    PdfOptionMsg::PdfLayersToggled(true),
                    PdfOptionMsg::BookmarkSheetsToggled(true),
                ] {
                    h::settle(app, Message::from(msg))?;
                }
                states_more::fold_pdf_sections(app, r)
            },
            window: WindowSel::Auto,
            checks: PDF,
        },
    ]
}

const COMMAND_LOG: &[Check] = &[
    ("the log has rows", command_log_rows),
    ("the Command Log is docked", command_log_docked),
];
const COMPARE: &[Check] = &[
    ("the compare dialog is open", compare_open),
    ("Close closes it", compare_close),
];
const BOARD: &[Check] = &[("a board tab is active", checks::board_tab_active)];
const COUNTERS: &[Check] = &[
    ("a board tab is active", checks::board_tab_active),
    ("the DRC counter opens its popover", drc_popover),
];
const ANNOTATIONS: &[Check] = &[
    ("the Annotations panel is docked", annotations_docked),
    ("it lists the notebook's blocks", annotations_listed),
];
const LAYOUTS: &[Check] = &[("Manage Layouts is open", manage_layouts_open)];
const OUTPUT_JOBS: &[Check] = &[("the Output Jobs panel is docked", output_jobs_docked)];
const PDF: &[Check] = &[("the watermark is on", watermark_on)];

fn command_log_rows(app: &mut Signex) -> Result<(), String> {
    match app.ui_state.command_log.log.rows().count() {
        0 => Err("the log is empty".into()),
        _ => Ok(()),
    }
}

fn command_log_docked(app: &mut Signex) -> Result<(), String> {
    h::docked(app, PanelKind::CommandLog)
}

fn compare_open(app: &mut Signex) -> Result<(), String> {
    match app.ui_state.compare_board {
        Some(_) => Ok(()),
        None => Err("compare_board is None".into()),
    }
}

fn compare_close(app: &mut Signex) -> Result<(), String> {
    h::settle(app, Message::from(CompareBoardMsg::CloseRequested))?;
    match app.ui_state.compare_board {
        None => Ok(()),
        Some(_) => Err("the dialog is still open".into()),
    }
}

fn drc_popover(app: &mut Signex) -> Result<(), String> {
    let window = app.ui_state.main_window_id;
    let _ = app.update(Message::Ui(UiMsg::StatusBar(
        StatusBarRequest::BoardCounterClicked {
            window,
            counter: BoardCounter::Drc,
        },
    )));
    match app.ui_state.board_status.popover {
        Some(_) => Ok(()),
        None => Err("no popover opened".into()),
    }
}

fn annotations_docked(app: &mut Signex) -> Result<(), String> {
    h::docked(app, PanelKind::Annotations)
}

fn annotations_listed(app: &mut Signex) -> Result<(), String> {
    match app.document_state.panel_ctx.annotations.total {
        0 => Err("the panel lists no block".into()),
        _ => Ok(()),
    }
}

fn manage_layouts_open(app: &mut Signex) -> Result<(), String> {
    match app.ui_state.workspace_layouts.manage {
        Some(_) => Ok(()),
        None => Err("workspace_layouts.manage is None".into()),
    }
}

fn output_jobs_docked(app: &mut Signex) -> Result<(), String> {
    h::docked(app, PanelKind::OutputJobs)
}

fn watermark_on(app: &mut Signex) -> Result<(), String> {
    let preview = app
        .document_state
        .preview
        .as_ref()
        .ok_or("the PDF dialog is not open")?;
    if preview.dialog.watermark.is_on {
        Ok(())
    } else {
        Err("the watermark is off".into())
    }
}

// ---- new-advanced (m171-m177) and new-pour (m178) -------------------------

fn advanced_rows() -> Vec<Surface> {
    vec![
        missing("m171", "Block Diagram + Power Tree", |_, _, _| {
            Err("not built: no block diagram document (.snxblk) or tab".into())
        }),
        missing("m172", "Simulation View", |_, _, _| {
            Err("not built: no simulation document or editor mode".into())
        }),
        missing("m173", "Plugin Manager", |_, _, _| {
            Err("not built: no Plugin Manager modal or plugins command".into())
        }),
        missing("m174", "Render Manager", |_, _, _| {
            Err("not built: no Render Manager tab or command".into())
        }),
        Surface {
            id: "m175",
            title: "3D View Toolbar Extras",
            open: |app, r, fx| {
                fixture::open_board(app, r, fx);
                h::command(app, "view_board_3d")?;
                let _ = render::draw_main(r, app);
                Ok(())
            },
            window: WindowSel::Auto,
            checks: BOARD_3D,
        },
        missing("m176", "Board Materials", |_, _, _| {
            Err("not built: the Layer Stack panel has no Finish sub-tab".into())
        }),
        missing("m177", "Layer Stack Material Library", |_, _, _| {
            Err("not built: the Layer Stack panel has no Materials or Templates sub-tab".into())
        }),
        missing("m178", "Polygon Manager", |_, _, _| {
            Err("not built: no Polygon Manager tab or command".into())
        }),
    ]
}

const BOARD_3D: &[Check] = &[("the board is in 3D", in_3d)];

fn in_3d(app: &mut Signex) -> Result<(), String> {
    match app.interaction_state.board_3d {
        Some(_) => Ok(()),
        None => Err("board_3d is None".into()),
    }
}

// ---- new-boardsketch (m179-m179f) -----------------------------------------

/// The fixture board in sketch mode.
fn open_board_sketch(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &fixture::Fixture,
) -> Result<(), String> {
    fixture::open_board(app, r, fx);
    h::command(app, "board_sketch")?;
    let _ = render::draw_main(r, app);
    Ok(())
}

/// A 10 x 10 mm square on BOARD and a hole on HOLES.
const SKETCH_DXF: &str = "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n\
0\nSECTION\n2\nENTITIES\n\
0\nLINE\n8\nBOARD\n10\n0\n20\n0\n11\n10\n21\n0\n\
0\nLINE\n8\nBOARD\n10\n10\n20\n0\n11\n10\n21\n10\n\
0\nLINE\n8\nBOARD\n10\n10\n20\n10\n11\n0\n21\n10\n\
0\nLINE\n8\nBOARD\n10\n0\n20\n10\n11\n0\n21\n0\n\
0\nCIRCLE\n8\nHOLES\n10\n5\n20\n5\n40\n1\n\
0\nENDSEC\n0\nEOF\n";

fn open_board_sketch_dxf(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &fixture::Fixture,
) -> Result<(), String> {
    open_board_sketch(app, r, fx)?;
    let board = app
        .interaction_state
        .board_sketch
        .as_ref()
        .ok_or("no board sketch")?
        .board_path
        .clone();
    let source = signex_engine::pcb::dxf::DxfSource::read(SKETCH_DXF).map_err(|e| e.to_string())?;
    let _ = app.update(signex_app::app::Message::Pcb(
        signex_app::app::PcbMsg::DxfImport(signex_app::app::DxfImportMsg::SketchRead {
            board,
            path: "lid.dxf".into(),
            source: Ok(std::sync::Arc::new(source)),
        }),
    ));
    let _ = render::draw_main(r, app);
    Ok(())
}

fn board_sketch_rows() -> Vec<Surface> {
    vec![
        Surface {
            id: "m179",
            title: "Board sketch mode (full sketcher)",
            open: open_board_sketch,
            window: WindowSel::Auto,
            checks: BOARD_SKETCH,
        },
        Surface {
            id: "m179b",
            title: "References to board features",
            // There is no Create > Project tool; the Line tool snapping to
            // board features is the nearest state there is.
            open: |app, r, fx| {
                open_board_sketch(app, r, fx)?;
                let tool = FootprintEditorMsg::SketchSetTool(SketchTool::Line);
                h::fp_edit(app, &fx.pcb, tool);
                Ok(())
            },
            window: WindowSel::Auto,
            checks: BOARD_SKETCH,
        },
        Surface {
            id: "m179c",
            title: "Import DXF into the sketch",
            open: open_board_sketch_dxf,
            window: WindowSel::Auto,
            checks: BOARD_SKETCH,
        },
        Surface {
            id: "m179d",
            title: "Outline checks",
            // A drawn outline with a sharp inside corner, under a fab profile
            // that names the router, so every check runs.
            open: super::states_sketch::open_outline_checks,
            window: WindowSel::Auto,
            checks: BOARD_SKETCH,
        },
        Surface {
            id: "m179e",
            title: "Mechanical constraints",
            // Modify > Rectangular Pattern armed. The enclosure body
            // (Create > Project > Enclosure body) is not built.
            open: |app, r, fx| {
                open_board_sketch(app, r, fx)?;
                let tool = FootprintEditorMsg::SketchSetTool(SketchTool::RectPattern);
                h::fp_edit(app, &fx.pcb, tool);
                Ok(())
            },
            window: WindowSel::Auto,
            checks: BOARD_SKETCH,
        },
        missing(
            "m179f",
            "Edit in place with a live 3D outline",
            |_, _, _| Err("not built: board sketch mode has no live 3D outline view".into()),
        ),
    ]
}

const BOARD_SKETCH: &[Check] = &[("the board is in sketch mode", in_board_sketch)];

fn in_board_sketch(app: &mut Signex) -> Result<(), String> {
    match app.interaction_state.board_sketch {
        Some(_) => Ok(()),
        None => Err("board_sketch is None".into()),
    }
}

// ---- new-pcbsim (m181-m186) -----------------------------------------------

const NO_SIM: &str =
    "not built: no board simulation (no Simulate Board command, run or results tab)";

fn pcb_sim_rows() -> Vec<Surface> {
    vec![
        missing("m181", "Simulation setup wizard", |_, _, _| {
            Err(NO_SIM.into())
        }),
        missing("m182", "DC IR drop results", |_, _, _| Err(NO_SIM.into())),
        missing("m183", "Thermal results", |_, _, _| Err(NO_SIM.into())),
        missing("m184", "Signal integrity", |_, _, _| Err(NO_SIM.into())),
        missing("m185", "PDN impedance", |_, _, _| Err(NO_SIM.into())),
        missing("m186", "Antenna & RF", |_, _, _| Err(NO_SIM.into())),
    ]
}

// ---- new-fpsketch (m187-m192) ---------------------------------------------

/// The first stock footprint open in its editor.
fn open_footprint(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &fixture::Fixture,
) -> Result<(), String> {
    fixture::mount_stock(app, fx)?;
    let path = fx.footprint.clone();
    h::settle(app, Message::File(FileMsg::Opened(Some(path))))?;
    let _ = render::draw_main(r, app);
    Ok(())
}

/// The first stock footprint in sketch mode.
fn open_footprint_sketch(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &fixture::Fixture,
) -> Result<(), String> {
    open_footprint(app, r, fx)?;
    let mode = FootprintEditorMsg::SetMode(EditorMode::Sketch);
    h::fp_edit(app, &fx.footprint, mode);
    let _ = render::draw_main(r, app);
    Ok(())
}

fn footprint_sketch_rows() -> Vec<Surface> {
    vec![
        Surface {
            id: "m187",
            title: "Footprint sketch mode workspace",
            open: open_footprint_sketch,
            window: WindowSel::Auto,
            checks: FP_SKETCH,
        },
        Surface {
            id: "m188",
            title: "Constraints and snap glyphs",
            // A rectangle drawn in sketch mode shows its constraint glyphs;
            // the snap glyphs at the cursor need a hover the capture lacks.
            open: |app, r, fx| {
                open_footprint_sketch(app, r, fx)?;
                let path = fx.footprint.clone();
                let tool = FootprintEditorMsg::SketchSetTool(SketchTool::Rectangle);
                h::fp_edit(app, &path, tool);
                for (x_mm, y_mm) in [(-4.0, -3.0), (4.0, 3.0)] {
                    let click = FootprintEditorMsg::SketchToolClick {
                        x_mm,
                        y_mm,
                        snap_id: None,
                    };
                    h::fp_edit(app, &path, click);
                }
                h::fp_edit(app, &path, FootprintEditorMsg::ToolEscape);
                Ok(())
            },
            window: WindowSel::Auto,
            checks: FP_SKETCH,
        },
        Surface {
            id: "m189",
            title: "Sketch parameters table",
            open: |app, r, fx| {
                open_footprint_sketch(app, r, fx)?;
                let toggle =
                    FootprintEditorMsg::SketchParameters(SketchParametersMsg::TableToggled);
                h::fp_edit(app, &fx.footprint, toggle);
                Ok(())
            },
            window: WindowSel::Auto,
            checks: PARAMETERS,
        },
        Surface {
            id: "m190",
            title: "Bake: roles, preview, errors, result",
            // The bake preview card open. The error and result states need a
            // sketch that refuses, or a Bake, which leaves the mode.
            open: |app, r, fx| {
                open_footprint_sketch(app, r, fx)?;
                let toggle = FootprintEditorMsg::SketchBakePreviewToggled;
                h::fp_edit(app, &fx.footprint, toggle);
                Ok(())
            },
            window: WindowSel::Auto,
            checks: BAKE_PREVIEW,
        },
        Surface {
            id: "m191",
            title: "Mirror (B1) and patterns",
            // A pad row mirrored about a centerline with the armed tool.
            open: super::states_sketch::open_mirrored_row,
            window: WindowSel::Auto,
            checks: MIRROR,
        },
        Surface {
            id: "m192",
            title: "Pads <-> Sketch",
            // A pad selected in the pads view: Properties offers Edit in
            // Sketch. The sketch side's Jump to pad needs an entity linked to
            // a pad (PadLinkMsg::PadLinkPressed).
            open: |app, r, fx| {
                open_footprint(app, r, fx)?;
                h::fp_edit(app, &fx.footprint, FootprintEditorMsg::SelectPad(Some(0)));
                let _ = render::draw_main(r, app);
                Ok(())
            },
            window: WindowSel::Auto,
            checks: &[],
        },
    ]
}

const FP_SKETCH: &[Check] = &[("the footprint is in sketch mode", fp_in_sketch)];
const PARAMETERS: &[Check] = &[("the parameters table is open", parameters_open)];
const BAKE_PREVIEW: &[Check] = &[("the bake preview is open", bake_preview_open)];
const MIRROR: &[Check] = &[("the Mirror tool is armed", mirror_armed)];

fn footprint_editor(app: &Signex) -> Result<&signex_app::app::FootprintDocument, String> {
    app.document_state
        .footprint_editors
        .values()
        .next()
        .ok_or_else(|| "no footprint editor is open".to_string())
}

fn fp_in_sketch(app: &mut Signex) -> Result<(), String> {
    match footprint_editor(app)?.state.mode {
        EditorMode::Sketch => Ok(()),
        mode => Err(format!("the editor is in {mode:?}")),
    }
}

fn parameters_open(app: &mut Signex) -> Result<(), String> {
    match footprint_editor(app)?.state.parameters_table {
        Some(_) => Ok(()),
        None => Err("parameters_table is None".into()),
    }
}

fn bake_preview_open(app: &mut Signex) -> Result<(), String> {
    if footprint_editor(app)?.state.sketch_exit.preview {
        Ok(())
    } else {
        Err("sketch_exit.preview is false".into())
    }
}

fn mirror_armed(app: &mut Signex) -> Result<(), String> {
    match footprint_editor(app)?.state.active_tool {
        SketchTool::Mirror => Ok(()),
        tool => Err(format!("the active tool is {tool:?}")),
    }
}

// ---- new-gerber (m193-m193f) ----------------------------------------------

const NO_VIEWER: &str =
    "not built: no Gerber Viewer tab; only its Messages rows (app/gerber_read.rs) exist";

fn gerber_rows() -> Vec<Surface> {
    vec![
        missing("m193", "Gerber Viewer tab", |_, _, _| Err(NO_VIEWER.into())),
        missing("m193b", "Opened from an Export Job run", |_, _, _| {
            Err(NO_VIEWER.into())
        }),
        missing("m193c", "Measure", |_, _, _| Err(NO_VIEWER.into())),
        missing("m193d", "Compare with board", |_, _, _| {
            Err(NO_VIEWER.into())
        }),
        missing("m193e", "Read errors", |_, _, _| Err(NO_VIEWER.into())),
        missing("m193f", "Empty state", |_, _, _| Err(NO_VIEWER.into())),
    ]
}
