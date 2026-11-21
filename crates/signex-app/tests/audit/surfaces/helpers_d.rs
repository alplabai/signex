//! Set-up steps the `annot_valid` and `pcb` rows share: running a menu
//! command, a project for the sample sheet, a board with one of every
//! primitive, and the checks that read which panel or form shows.

use crate::fixture::{self, Fixture};
use crate::{render, settle_support};
use pcb_geom::ClickMode;
use signex_app::app::{LoadedProject, Message, ProjectId, Signex, TabKind};
use signex_app::menu_bar::MenuMessage;
use signex_app::panels::PanelKind;
use signex_app::panels::pcb_primitive_properties::PcbPrimitiveFacts;
use signex_app::pcb_canvas::PcbCanvasMsg;
use signex_types::format::SnxPcb;
use signex_types::pcb::{PcbBoard, Point};
use signex_types::project::{ProjectData, SheetEntry};
use std::path::PathBuf;
use uuid::Uuid;

/// Run command `id` as its main-menu row does, with every task it starts.
pub fn command(app: &mut Signex, id: &str) -> Result<(), String> {
    let id = signex_app::keymap::AppCommandId::new(id)
        .ok()
        .ok_or_else(|| format!("no command id {id}"))?;
    settle_support::settle(app, Message::Menu(MenuMessage::Command(id)));
    Ok(())
}

/// Turn a state the surface needs into the row's open error.
pub fn reached(ok: bool, what: &str) -> Result<(), String> {
    if ok {
        Ok(())
    } else {
        Err(format!("open path unknown: {what}"))
    }
}

/// The sample sheet's project, made active and owning every tab. The
/// bundled `.snxprj` is empty, so a project is added in memory when the
/// open did not load one; nothing is written.
pub fn sample_project(app: &mut Signex, fx: &Fixture) -> ProjectId {
    if let Some(id) = app
        .document_state
        .active_project
        .or_else(|| app.document_state.projects.first().map(|p| p.id))
    {
        app.document_state.active_project = Some(id);
        for tab in &mut app.document_state.tabs {
            tab.project_id = Some(id);
        }
        return id;
    }
    let dir = fx.sample.parent().map(PathBuf::from).unwrap_or_default();
    let sheet = "linear-regulator.snxsch".to_string();
    let data = ProjectData {
        document_id: None,
        name: "linear-regulator".to_string(),
        description: None,
        dir: dir.to_string_lossy().into_owned(),
        schematic_root: Some(sheet.clone()),
        pcb_file: None,
        sheets: vec![SheetEntry {
            name: sheet.clone(),
            filename: sheet,
            symbols_count: 0,
            wires_count: 0,
            labels_count: 0,
            unknown: Default::default(),
        }],
        variant_definitions: Vec::new(),
        active_variant: None,
        libraries: Vec::new(),
        enable_git: false,
        net_classes: Vec::new(),
        rules: None,
        annotation: None,
        parameters: Default::default(),
        schematic_defaults: Default::default(),
        project_defaults: Default::default(),
        agent_journal: Default::default(),
        schematic_design: Default::default(),
        fab_outputs: None,
        router_tool_diameter: None,
        requirements: Default::default(),
        unknown: Default::default(),
    };
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id,
        path: fx.sample.clone(),
        data,
        pending_libraries: Default::default(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
    app.document_state.active_project = Some(id);
    for tab in &mut app.document_state.tabs {
        tab.project_id = Some(id);
    }
    id
}

/// The loaded project `id`, to edit its data in memory.
pub fn project_mut(app: &mut Signex, id: ProjectId) -> Option<&mut LoadedProject> {
    app.document_state.projects.iter_mut().find(|p| p.id == id)
}

/// The sample open with its project, the schematic tab active; the board
/// copy saved beside it as the project's board.
pub fn sample_with_board(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
) -> Result<ProjectId, String> {
    fixture::open_sample(app, r, fx);
    fixture::mount_stock(app, fx)?;
    let id = sample_project(app, fx);
    let board = fx.sample.with_extension("snxpcb");
    std::fs::copy(&fx.pcb, &board).map_err(|e| format!("copy board: {e}"))?;
    if let Some(project) = project_mut(app, id) {
        project.data.pcb_file = Some("linear-regulator.snxpcb".to_string());
    }
    Ok(id)
}

// ---- a board with one of every primitive ----

const FOOTPRINT: Uuid = Uuid::from_u128(0xD01);
const PAD_TH: Uuid = Uuid::from_u128(0xD02);
const PAD_SMD: Uuid = Uuid::from_u128(0xD03);
const TRACK: Uuid = Uuid::from_u128(0xD04);
const VIA: Uuid = Uuid::from_u128(0xD05);
const ARC: Uuid = Uuid::from_u128(0xD06);
const POUR: Uuid = Uuid::from_u128(0xD07);
const TEXT: Uuid = Uuid::from_u128(0xD08);
const DIM: Uuid = Uuid::from_u128(0xD09);
const GRAPHIC: Uuid = Uuid::from_u128(0xD0A);

fn square(x: f64, y: f64, side: f64) -> serde_json::Value {
    serde_json::json!([
        {"x": x, "y": y}, {"x": x + side, "y": y},
        {"x": x + side, "y": y + side}, {"x": x, "y": y + side}
    ])
}

/// U1 at (60, 10) with a through-hole pad 2 mm left and an SMD pad 2 mm
/// right of its origin; a GND track along y = 0 to x = 10; a via at (20, 0);
/// an arc round (20, 20); a GND pour square at (50, 50); a string at
/// (5, 30); a dimension along y = 10 from x = 30 to 40; a silkscreen line
/// along y = 40. No two overlap, so one click picks one item.
pub fn primitives_board() -> Result<PcbBoard, String> {
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(0xD00),
        "setup": {"grid_size": 1.0, "trace_width": 0.25},
        "nets": [{"number": 1, "name": "GND"}],
        "footprints": [{
            "uuid": FOOTPRINT, "reference": "U1", "value": "MCU",
            "footprint_id": "QFN-48", "position": {"x": 60.0, "y": 10.0},
            "layer": "TopCopper",
            "pads": [
                {"uuid": PAD_TH, "number": "1", "pad_type": "thru", "shape": "circle",
                 "position": {"x": -2.0, "y": 0.0}, "size": {"x": 1.2, "y": 1.2},
                 "drill": {"diameter": 0.5, "shape": ""},
                 "layers": ["TopCopper", "BottomCopper"], "net": null},
                {"uuid": PAD_SMD, "number": "2", "pad_type": "smd", "shape": "rect",
                 "position": {"x": 2.0, "y": 0.0}, "size": {"x": 1.0, "y": 0.6},
                 "drill": null, "layers": ["TopCopper"], "net": null}
            ]
        }],
        "segments": [{"uuid": TRACK, "start": {"x": 0.0, "y": 0.0},
                      "end": {"x": 10.0, "y": 0.0}, "width": 0.25,
                      "layer": "TopCopper", "net": 1}],
        "vias": [{"uuid": VIA, "position": {"x": 20.0, "y": 0.0}, "diameter": 0.6,
                  "drill": 0.3, "layers": ["TopCopper", "BottomCopper"], "net": 1}],
        "zones": [{"uuid": POUR, "net": 1, "net_name": "GND", "layer": "TopCopper",
                   "fill_type": "pour", "outline": square(50.0, 50.0, 10.0)}],
        "texts": [{"uuid": TEXT, "text": "A", "position": {"x": 5.0, "y": 30.0},
                   "layer": "TopSilkscreen", "font_size": 1.0, "rotation": 0.0}],
        "graphics": [{"uuid": GRAPHIC, "graphic_type": "line", "layer": "TopSilkscreen",
                      "width": 0.2, "start": {"x": 0.0, "y": 40.0},
                      "end": {"x": 10.0, "y": 40.0}}],
        "free": {
            "arcs": [{"uuid": ARC, "start": {"x": 25.0, "y": 20.0},
                      "mid": {"x": 20.0, "y": 25.0}, "end": {"x": 15.0, "y": 20.0},
                      "width": 0.3, "layer": "TopCopper", "net": 0}],
            "dimensions": [{"uuid": DIM, "start": {"x": 30.0, "y": 10.0},
                            "end": {"x": 40.0, "y": 10.0}, "offset": 2.0,
                            "layer": "Dimensions", "line_width": 0.1,
                            "text_height": 1.0, "precision": 2}]
        }
    }))
    .map_err(|e| format!("primitives board: {e}"))
}

/// Save [`primitives_board`] beside the fixture board and open it, fitted.
pub fn open_primitives(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
) -> Result<(), String> {
    let path = fx.pcb.with_file_name("primitives.snxpcb");
    let text = SnxPcb::new(primitives_board()?)
        .write_string()
        .map_err(|e| format!("encode board: {e:?}"))?;
    std::fs::write(&path, text).map_err(|e| format!("write board: {e}"))?;
    settle_support::settle(
        app,
        Message::File(signex_app::app::FileMsg::Opened(Some(path))),
    );
    let _ = render::draw_main(r, app);
    app.interaction_state.pcb_canvas.fit_to_board();
    let _ = render::draw_main(r, app);
    Ok(())
}

/// Click the board at `(x, y)` mm, replacing the selection.
pub fn board_click(app: &mut Signex, r: &mut iced::Renderer, x: f64, y: f64) {
    settle_support::settle(
        app,
        Message::PcbCanvas(PcbCanvasMsg::Clicked {
            at: Point::new(x, y),
            tolerance_mm: 0.05,
            mode: ClickMode::Replace,
        }),
    );
    let _ = render::draw_main(r, app);
}

/// The middle of the first stroke the primitives board's dimension draws,
/// where a click picks the dimension.
pub fn dimension_hit() -> Result<(f64, f64), String> {
    let board = primitives_board()?;
    let dimension = board.free.dimensions.first().ok_or("no dimension")?;
    let drawing = dimension.drawing().ok_or("the dimension draws nothing")?;
    let (a, b) = drawing.strokes.first().copied().ok_or("no stroke")?;
    Ok(((a.x + b.x) / 2.0, (a.y + b.y) / 2.0))
}

// ---- checks ----

/// The Properties form the board selection shows, by its kind.
fn primitive_kind(app: &Signex) -> Option<&'static str> {
    let facts = app
        .document_state
        .panel_ctx
        .pcb_board
        .as_ref()
        .and_then(|b| b.primitive.as_ref())?;
    Some(match facts {
        PcbPrimitiveFacts::Track(_) => "Track",
        PcbPrimitiveFacts::Via(_) => "Via",
        PcbPrimitiveFacts::Pad(_) => "Pad",
        PcbPrimitiveFacts::Zone(_) => "Zone",
        PcbPrimitiveFacts::Arc(_) => "Arc",
        PcbPrimitiveFacts::Text(_) => "Text",
        PcbPrimitiveFacts::Dimension(_) => "Dimension",
        PcbPrimitiveFacts::Graphic(_) => "Graphic",
    })
}

fn primitive_is(app: &Signex, want: &str) -> Result<(), String> {
    match primitive_kind(app) {
        Some(kind) if kind == want => Ok(()),
        Some(kind) => Err(format!("Properties shows a {kind}, not a {want}")),
        None => Err(format!(
            "Properties shows no primitive form; expected {want}"
        )),
    }
}

pub fn track_form(app: &mut Signex) -> Result<(), String> {
    primitive_is(app, "Track")
}

pub fn via_form(app: &mut Signex) -> Result<(), String> {
    primitive_is(app, "Via")
}

pub fn pad_form(app: &mut Signex) -> Result<(), String> {
    primitive_is(app, "Pad")
}

pub fn zone_form(app: &mut Signex) -> Result<(), String> {
    primitive_is(app, "Zone")
}

pub fn arc_form(app: &mut Signex) -> Result<(), String> {
    primitive_is(app, "Arc")
}

pub fn text_form(app: &mut Signex) -> Result<(), String> {
    primitive_is(app, "Text")
}

pub fn dimension_form(app: &mut Signex) -> Result<(), String> {
    primitive_is(app, "Dimension")
}

pub fn graphic_form(app: &mut Signex) -> Result<(), String> {
    primitive_is(app, "Graphic")
}

pub fn component_form(app: &mut Signex) -> Result<(), String> {
    let shown = app
        .document_state
        .panel_ctx
        .pcb_board
        .as_ref()
        .is_some_and(|b| b.component.is_some());
    if shown {
        Ok(())
    } else {
        Err("Properties shows no Component form".into())
    }
}

/// Is a panel of `kind` in any dock?
pub fn panel_shown(app: &Signex, kind: PanelKind) -> Result<(), String> {
    match app.document_state.dock.locate(kind) {
        Some(_) => Ok(()),
        None => Err(format!("no {kind:?} panel in any dock")),
    }
}

/// Is the active tab the Rules tab of a project?
pub fn rules_tab_active(app: &mut Signex) -> Result<(), String> {
    let d = &app.document_state;
    match d.tabs.get(d.active_tab).map(|t| &t.kind) {
        Some(TabKind::Rules(_)) => Ok(()),
        Some(kind) => Err(format!("active tab is {kind:?}, not a Rules tab")),
        None => Err("no active tab; expected a Rules tab".into()),
    }
}
