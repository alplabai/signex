//! The board Tools edits (#2408, #2409) as the app runs them: Via
//! Stitching, Add and Remove Teardrops, Gloss Selected, Board Shape from
//! Selection, Keepout from Selection and Arrange in Box.
//!
//! A command from a menu, the board's right-click menu, the palette or a
//! key acts on the board in view and its selection; the same command with
//! operands (`operands.rs`) acts on the board an agent names. Both build the
//! edit in `signex_engine::pcb::board_tools` and run it here as one undo
//! step through the board's mutation gateway. Gloss, Board Shape, Keepout
//! and Arrange in Box go one step further: the GUI fills in the board and
//! the items the person picked and runs the operand handler itself
//! (`command::translator`, ADR-0006 slice 6). Copper keeps the clearance the
//! board, its project's net classes and the written DRC rules ask for.
//! Behaviour:
//! `docs/PCB_TOOLS.md`, "Board Tools".

mod arrange_box;
mod operands;

use std::collections::BTreeSet;
use std::path::Path;

use iced::Task;
use pcb_geom::ItemRef;
use signex_drc::DrcRules;
use signex_engine::pcb::board_tools::{
    StitchArea, ToolEdit, ToolRefusal, ToolReport, add_teardrops, default_stitch_pitch_mm,
    remove_teardrops,
};
use signex_i18n::{ids, t};
use signex_types::pcb::PcbBoard;
use uuid::Uuid;

pub(crate) use arrange_box::ArrangeBox;
pub(crate) use operands::{
    add_teardrops_by_operands, arrange_in_box_by_operands, board_shape_by_operands,
    gloss_by_operands, keepout_by_operands, remove_teardrops_by_operands, stitch_by_operands,
};

use crate::app::command::{CommandOutcome, OperandValue, translator};
use crate::app::{BoardToolsMsg, Message, Signex};

/// The catalog command of each tool, for its name in Messages.
fn command_of(event: BoardToolsMsg) -> &'static str {
    match event {
        BoardToolsMsg::StitchRequested => "stitch_vias",
        BoardToolsMsg::TeardropsAddRequested => "add_teardrops",
        BoardToolsMsg::TeardropsRemoveRequested => "remove_teardrops",
        BoardToolsMsg::GlossRequested => "gloss_selected_routes",
        BoardToolsMsg::BoardShapeRequested => "board_shape_from_selection",
        BoardToolsMsg::KeepoutRequested => "keepout_from_selection",
        BoardToolsMsg::ArrangeRequested => "arrange_in_box",
    }
}

/// The selection of the board in view, split by what the tools read.
#[derive(Debug, Default)]
struct Picked {
    tracks: BTreeSet<Uuid>,
    zones: BTreeSet<Uuid>,
    footprints: BTreeSet<Uuid>,
    /// The nets of everything selected, no net left out.
    nets: BTreeSet<u32>,
    /// Whether anything at all is selected.
    is_any: bool,
}

impl Picked {
    fn of(board: &PcbBoard, items: &[ItemRef]) -> Self {
        let mut picked = Picked {
            is_any: !items.is_empty(),
            ..Picked::default()
        };
        for item in items {
            match *item {
                ItemRef::Track(id) => {
                    picked.tracks.insert(id);
                    let net = board.segments.iter().find(|s| s.uuid == id).map(|s| s.net);
                    picked.nets.extend(net);
                }
                ItemRef::Via(id) => {
                    let net = board.vias.iter().find(|v| v.uuid == id).map(|v| v.net);
                    picked.nets.extend(net);
                }
                ItemRef::Zone { zone, .. } => {
                    picked.zones.insert(zone);
                    let net = board.zones.iter().find(|z| z.uuid == zone).map(|z| z.net);
                    picked.nets.extend(net);
                }
                ItemRef::Pad { footprint, .. } | ItemRef::Footprint(footprint) => {
                    picked.footprints.insert(footprint);
                    let pads = board
                        .footprints
                        .iter()
                        .filter(|fp| fp.uuid == footprint)
                        .flat_map(|fp| fp.pads.iter());
                    picked
                        .nets
                        .extend(pads.filter_map(|p| p.net.as_ref().map(|n| n.number)));
                }
                // #2373: a copper arc or free pad brings its net; strings and
                // dimensions carry none.
                ItemRef::Arc(_) | ItemRef::FreePad(_) => {
                    picked.nets.extend(pcb_geom::item_net(board, *item));
                }
                ItemRef::Text(_)
                | ItemRef::Dimension(_)
                | ItemRef::Graphic { .. }
                | ItemRef::FootprintText { .. } => {}
            }
        }
        picked.nets.remove(&0);
        picked
    }
}

/// The clearance rule copper tools keep on `board`: the board's clearance,
/// its project's net classes and the written rules, the rules a design
/// rule check of the board applies. A clearance the geometry cannot hold
/// is refused, never replaced by a default that would drop the rules.
pub(crate) fn clearance_rules(
    app: &Signex,
    path: &Path,
    board: &PcbBoard,
) -> Result<DrcRules, ToolRefusal> {
    app.board_design_rules(path, board)
        .map(|(rules, _)| rules)
        .map_err(ToolRefusal::Rules)
}

/// The gap copper of nets `a` and `b` keeps under `rules`; none without.
fn pair_gap(rules: Option<&DrcRules>, a: u32, b: u32) -> i64 {
    rules.map_or(0, |r| r.net_pair_clearance(a, b))
}

/// Whether the tool lays copper and so needs the clearance rule.
fn lays_copper(event: BoardToolsMsg) -> bool {
    matches!(
        event,
        BoardToolsMsg::StitchRequested
            | BoardToolsMsg::TeardropsAddRequested
            | BoardToolsMsg::GlossRequested
    )
}

/// The items of the selection a translated tool acts on, as its `ids`;
/// `None` for a tool the GUI runs itself. Only what the menu row acted on
/// before the translator is passed, so the handler's all-or-nothing check
/// never refuses a selection the row accepted: the tracks of the board for
/// Gloss and Board Shape; for a keepout the zones of the board when any is
/// selected (the lines are then left out, as before), else the tracks.
fn translated_targets(
    event: BoardToolsMsg,
    board: &PcbBoard,
    picked: &Picked,
) -> Option<Vec<Uuid>> {
    let tracks = || {
        let on_board = |id: &&Uuid| board.segments.iter().any(|s| s.uuid == **id);
        picked.tracks.iter().filter(on_board).copied().collect()
    };
    match event {
        BoardToolsMsg::GlossRequested | BoardToolsMsg::BoardShapeRequested => Some(tracks()),
        BoardToolsMsg::KeepoutRequested => {
            let on_board = |id: &&Uuid| board.zones.iter().any(|z| z.uuid == **id);
            let zones: Vec<Uuid> = picked.zones.iter().filter(on_board).copied().collect();
            Some(if zones.is_empty() { tracks() } else { zones })
        }
        BoardToolsMsg::StitchRequested
        | BoardToolsMsg::TeardropsAddRequested
        | BoardToolsMsg::TeardropsRemoveRequested
        | BoardToolsMsg::ArrangeRequested => None,
    }
}

/// `ids` for `items`; none when nothing is named, as a caller leaves it out.
fn targets_operand(items: Vec<Uuid>) -> Vec<(&'static str, OperandValue)> {
    if items.is_empty() {
        return Vec::new();
    }
    vec![("ids", OperandValue::Targets(items))]
}

impl Signex {
    /// A board Tools command ran on the board in view.
    pub(crate) fn handle_board_tools(&mut self, event: BoardToolsMsg) -> Task<Message> {
        if event == BoardToolsMsg::ArrangeRequested {
            self.arm_arrange_box();
            return Task::none();
        }
        let Some(path) = self.active_tab_path().filter(|_| self.has_active_pcb()) else {
            return Task::none();
        };
        let items = self
            .with_pcb_selection(|sel, _| sel.selection().items().collect::<Vec<_>>())
            .unwrap_or_default();
        let Some(board) = self.document_state.pcb_board(&path) else {
            return Task::none();
        };
        let picked = Picked::of(board, &items);
        if let Some(ids) = translated_targets(event, board, &picked) {
            return self.run_translated_tool(command_of(event), &path, targets_operand(ids));
        }
        let rules = match clearance_rules(self, &path, board) {
            Ok(rules) => Some(rules),
            Err(refusal) if lays_copper(event) => {
                let ran = self.run_board_tool(&path, Err(refusal));
                self.say_board_tool(command_of(event), &ran);
                return Task::none();
            }
            Err(_) => None,
        };
        let gap = |a: u32, b: u32| pair_gap(rules.as_ref(), a, b);
        let nets = (!picked.nets.is_empty()).then_some(&picked.nets);
        let built = match event {
            BoardToolsMsg::StitchRequested => stitch_selection(board, &picked, gap),
            BoardToolsMsg::TeardropsAddRequested => add_teardrops(board, nets, gap, Uuid::now_v7),
            BoardToolsMsg::TeardropsRemoveRequested => remove_teardrops(board, nets),
            // Translated above; Arrange arms its box.
            BoardToolsMsg::GlossRequested
            | BoardToolsMsg::BoardShapeRequested
            | BoardToolsMsg::KeepoutRequested
            | BoardToolsMsg::ArrangeRequested => return Task::none(),
        };
        let ran = self.run_board_tool(&path, built);
        self.say_board_tool(command_of(event), &ran);
        Task::none()
    }

    /// Run the board tool `command` through its operand handler on the board
    /// at `path`, with `operands` filled in from the board in view, and say
    /// in Messages what it did or why not, as the menu row always has.
    pub(crate) fn run_translated_tool(
        &mut self,
        command: &'static str,
        path: &Path,
        operands: Vec<(&'static str, OperandValue)>,
    ) -> Task<Message> {
        let (outcome, task) = translator::run(self, command, path, operands);
        let ran = match outcome {
            CommandOutcome::Answered(answer) => {
                match serde_json::from_value::<ToolReport>(answer) {
                    Ok(report) => Ok(report),
                    // The edit has landed: say so, never a refusal.
                    Err(error) => {
                        debug_assert!(false, "{command}: unreadable report: {error}");
                        crate::diagnostics::log_warning(format!(
                            "{command} ran, but what it did could not be read: {error}"
                        ));
                        return task;
                    }
                }
            }
            CommandOutcome::Disabled { reason } => Err(reason.render(&self.ui_state.i18n)),
            // A board tool reads nothing first, so it never waits; were it
            // to, the read runs it again (`translator::run` leaves no other).
            other => {
                debug_assert!(false, "{command}: a board tool waited: {other:?}");
                return task;
            }
        };
        self.say_board_tool(command, &ran);
        task
    }

    /// Run a built tool edit on the board at `path` as one undo step. What it
    /// did, or why it did nothing.
    pub(crate) fn run_board_tool(
        &mut self,
        path: &Path,
        built: Result<ToolEdit, ToolRefusal>,
    ) -> Result<ToolReport, String> {
        let edit = built.map_err(|refusal| refusal.to_string())?;
        if edit.commands.is_empty() {
            return Ok(edit.report);
        }
        self.execute_board_commands(path, edit.commands, None)
            .map_err(|error| error.to_string())?;
        if self.active_tab_path().as_deref() == Some(path) {
            self.refresh_pcb_highlight();
        }
        Ok(edit.report)
    }

    /// The Messages line for a tool's result.
    pub(crate) fn say_board_tool(&self, command: &str, ran: &Result<ToolReport, String>) {
        let i18n = &self.ui_state.i18n;
        let tool = crate::menu_bar::cmd_label(i18n, command, command);
        match ran {
            Ok(report) => {
                crate::diagnostics::log_info(
                    t!(i18n, ids::PCB_BOARD_TOOL_DONE,
                        "tool" => tool.clone(),
                        "added" => report.added.len(),
                        "removed" => report.removed.len(),
                        "changed" => report.changed.len() + report.outline_points.min(1))
                    .into_string(),
                );
                if report.skipped > 0 {
                    crate::diagnostics::log_warning(
                        t!(i18n, ids::PCB_BOARD_TOOL_SKIPPED,
                            "tool" => tool, "count" => report.skipped)
                        .into_string(),
                    );
                }
            }
            Err(reason) => crate::diagnostics::log_warning(
                t!(i18n, ids::PCB_BOARD_TOOL_REFUSED, "tool" => tool, "reason" => reason.clone())
                    .into_string(),
            ),
        }
    }
}

/// Stitch the selected areas with their nets' vias, or, with nothing
/// selected, every pour that has a net: each at the default grid step. A
/// selection with no area in it is refused.
fn stitch_selection<F: Fn(u32, u32) -> i64 + Copy>(
    board: &PcbBoard,
    picked: &Picked,
    gap: F,
) -> Result<ToolEdit, ToolRefusal> {
    if picked.is_any && picked.zones.is_empty() {
        return Err(ToolRefusal::NothingSelected);
    }
    let areas: Vec<Uuid> = if picked.zones.is_empty() {
        board
            .zones
            .iter()
            .filter(|z| z.is_pour() && z.net != 0)
            .map(|z| z.uuid)
            .collect()
    } else {
        picked.zones.iter().copied().collect()
    };
    if areas.is_empty() {
        return Err(ToolRefusal::NothingSelected);
    }
    let pitch = default_stitch_pitch_mm(board);
    let mut staged = board.clone();
    let mut edit = ToolEdit {
        commands: Vec::new(),
        report: ToolReport::default(),
    };
    let mut last_refusal = None;
    for area in areas {
        let built = signex_engine::pcb::board_tools::stitch_vias(
            &staged,
            StitchArea::Zone(area),
            None,
            pitch,
            gap,
            Uuid::now_v7,
        );
        match built {
            Ok(part) => {
                // Later areas keep off the vias earlier ones placed.
                for command in &part.commands {
                    if let Err(error) = command.clone().apply(&mut staged) {
                        return Err(ToolRefusal::Path(error.to_string()));
                    }
                }
                edit.commands.extend(part.commands);
                edit.report.added.extend(part.report.added);
            }
            Err(refusal) => last_refusal = Some(refusal),
        }
    }
    match (edit.commands.is_empty(), last_refusal) {
        (true, Some(refusal)) => Err(refusal),
        (true, None) => Err(ToolRefusal::NothingFits),
        _ => Ok(edit),
    }
}

#[cfg(test)]
mod copper_tests;
#[cfg(test)]
mod placement_tests;
#[cfg(test)]
mod targets_tests;
#[cfg(test)]
mod translator_tests;
