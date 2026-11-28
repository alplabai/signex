//! Pin and gate swap started on the schematic (#1261). Two pins of a part,
//! or two of its gates, trade nets on the sheet as one undo step; nothing on
//! the board changes until Update PCB, whose change list then moves the pads'
//! nets. Which pins and gates may trade is the part's own rule, the one a
//! swap made on the board reads (`signex_engine::part_swap`).
//!
//! The Part Actions rows, the palette and a key send [`SheetSwapMsg`]; the
//! handler reads the clicked pin or the selected units and runs the
//! command's operand handler (`command::translator`), the one an agent or
//! the CLI runs. Rules: `docs/PCB_TOOLS.md`, "Pin and gate swap".

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use iced::Task;
use serde::Serialize;
use signex_engine::part_swap::{self, SheetSwapRequest};
use signex_i18n::{command_ids, ids, t};
use signex_types::schematic::{SchematicSheet, SelectedKind};
use uuid::Uuid;

use crate::app::command::{CommandOutcome, OperandValue, translator};
use crate::app::{Message, Signex};
use crate::canvas::CanvasSlot;

impl From<SheetSwapMsg> for Message {
    fn from(msg: SheetSwapMsg) -> Self {
        Self::Edit(crate::app::EditMsg::SheetSwap(msg))
    }
}

/// Swap Pins or Swap Gates was asked for on the sheet in view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SheetSwapMsg {
    /// `swap_schematic_pins` without operands: the clicked pin of the
    /// selected part and the one other pin of its swap group.
    PinsRequested,
    /// `swap_schematic_gates` without operands: the two selected units of
    /// one part, or one unit and the one other gate of its group.
    GatesRequested,
}

impl SheetSwapMsg {
    fn command(self) -> &'static str {
        match self {
            Self::PinsRequested => "swap_schematic_pins",
            Self::GatesRequested => "swap_schematic_gates",
        }
    }
}

/// Which swaps the sheet in view can make now (#1261).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct SwapReady {
    pub pins: bool,
    pub gates: bool,
}

/// The symbols selected on `canvas`, in selection order.
fn selected_symbols(canvas: &CanvasSlot) -> Vec<Uuid> {
    canvas
        .selected
        .iter()
        .filter(|item| item.kind == SelectedKind::Symbol)
        .map(|item| item.uuid)
        .collect()
}

/// What a swap on the schematic did, as the command answers it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct SheetSwapReport {
    pub document: String,
    pub designator: String,
    /// `pins` or `gates`.
    pub kind: &'static str,
    /// The two pin numbers, or the two unit numbers.
    pub swapped: [String; 2],
    /// Each pin whose net moved, with the pin it traded with.
    pub pairs: Vec<[String; 2]>,
    /// Whether the sheet changed: `false` when nothing ends on either pin.
    pub changed: bool,
}

impl Signex {
    pub(crate) fn handle_sheet_swap(&mut self, msg: SheetSwapMsg) -> Task<Message> {
        let (path, symbol, request) = match self.sheet_swap_in_view(msg) {
            Ok(found) => found,
            Err(why) => {
                self.refuse_sheet_swap(msg.command(), &why);
                return Task::none();
            }
        };
        let (pair, members) = match &request {
            SheetSwapRequest::Pins(a, b) => ("pins", format!("{a},{b}")),
            SheetSwapRequest::Gates(a, b) => ("units", format!("{a},{b}")),
        };
        let operands = vec![
            ("ids", OperandValue::Targets(vec![symbol])),
            (pair, OperandValue::Text(members)),
        ];
        let (outcome, task) = translator::run(self, msg.command(), &path, operands);
        if let CommandOutcome::Disabled { reason } = &outcome {
            let why = reason.render(&self.ui_state.i18n);
            self.refuse_sheet_swap(msg.command(), &why);
        }
        task
    }

    /// The sheet in view, the part and the swap the clicked pin or the
    /// selected units ask for.
    fn sheet_swap_in_view(
        &self,
        msg: SheetSwapMsg,
    ) -> Result<(PathBuf, Uuid, SheetSwapRequest), String> {
        let path = self
            .document_state
            .active_path
            .clone()
            .filter(|path| self.document_state.engines.contains_key(path))
            .ok_or_else(|| "open a sheet to swap on it".to_string())?;
        let doc = self.document_state.engines[&path].document();
        let canvas = self.interaction_state.active_canvas();
        let (symbol, request) = match msg {
            SheetSwapMsg::PinsRequested => self.clicked_pin_swap(doc, canvas),
            SheetSwapMsg::GatesRequested => {
                part_swap::gates_of_selection(doc, &selected_symbols(canvas))
                    .map_err(|refusal| refusal.to_string())
            }
        }?;
        Ok((path, symbol, request))
    }

    /// The pin swap the clicked pin of the part selected on `canvas` asks
    /// for, with the part's unit.
    fn clicked_pin_swap(
        &self,
        doc: &SchematicSheet,
        canvas: &CanvasSlot,
    ) -> Result<(Uuid, SheetSwapRequest), String> {
        let clicked = self
            .interaction_state
            .clicked_pin
            .as_ref()
            .filter(|pin| selected_symbols(canvas).contains(&pin.symbol))
            .ok_or_else(|| "click the pin of the part to swap".to_string())?;
        part_swap::pins_of_clicked_pin(doc, clicked.symbol, &clicked.pin)
            .map(|request| (clicked.symbol, request))
            .map_err(|refusal| refusal.to_string())
    }

    /// Which swaps the sheet `doc` shown on `canvas` can make now; the canvas
    /// menu's Swap Pins and Swap Gates rows are live by it.
    pub(crate) fn sheet_swap_ready(&self, doc: &SchematicSheet, canvas: &CanvasSlot) -> SwapReady {
        SwapReady {
            pins: self.clicked_pin_swap(doc, canvas).is_ok(),
            gates: part_swap::gates_of_selection(doc, &selected_symbols(canvas)).is_ok(),
        }
    }

    /// Make `request` on the part `symbol` of the sheet at `path`, as one
    /// undo step of that sheet. A refusal changes nothing.
    pub(crate) fn swap_on_sheet_at(
        &mut self,
        path: &Path,
        symbol: Uuid,
        request: &SheetSwapRequest,
    ) -> Result<SheetSwapReport, String> {
        if let Some(reason) = self.refuse_if_read_only_at(path) {
            return Err(reason);
        }
        if !self.agent.from_view
            && let Some(reason) = self.person_busy_on(path)
        {
            return Err(reason.to_string());
        }
        let doc = self
            .document_state
            .engines
            .get(path)
            .map(|engine| engine.document())
            .ok_or_else(|| format!("{} is not loaded", path.display()))?;
        let swap = part_swap::swap_on_sheet(doc, symbol, request)
            .map_err(|refusal| refusal.to_string())?;
        let (kind, swapped) = match request {
            SheetSwapRequest::Pins(a, b) => ("pins", [a.clone(), b.clone()]),
            SheetSwapRequest::Gates(a, b) => ("gates", [a.to_string(), b.to_string()]),
        };
        let changed = match swap.sheet {
            Some(document) => {
                let command = signex_engine::Command::ReplaceDocument {
                    document: Box::new(document),
                };
                self.execute_on_sheet(path, command)?
            }
            None => false,
        };
        let line = match (kind, changed) {
            ("gates", _) => ids::PCB_SCHEMATIC_GATES_SWAPPED,
            (_, true) => ids::PCB_SCHEMATIC_PINS_SWAPPED,
            (_, false) => ids::PCB_SCHEMATIC_SWAP_NOTHING,
        };
        let said = t!(
            self.ui_state.i18n,
            line,
            "designator" => swap.designator.as_str(),
            "a" => swapped[0].as_str(),
            "b" => swapped[1].as_str()
        );
        crate::diagnostics::log_info(said.into_string());
        Ok(SheetSwapReport {
            document: path.display().to_string(),
            designator: swap.designator,
            kind,
            swapped,
            pairs: swap.pairs.into_iter().map(|(a, b)| [a, b]).collect(),
            changed,
        })
    }

    /// Say in Messages why the swap `command` did nothing.
    pub(crate) fn refuse_sheet_swap(&self, command: &str, why: &str) {
        let label = match command {
            "swap_schematic_pins" => command_ids::CMD_MENU_SWAP_SCHEMATIC_PINS,
            _ => command_ids::CMD_MENU_SWAP_SCHEMATIC_GATES,
        };
        let what = t!(self.ui_state.i18n, label).into_string();
        let line = t!(
            self.ui_state.i18n,
            ids::PCB_PART_SWAP_REFUSED,
            "what" => what.as_str(),
            "why" => why
        );
        crate::diagnostics::log_warning(line.into_string());
    }
}

/// The refusal of the swap `command` as its catalog message, the Messages
/// line's words: "Swap Pins did nothing: `why`".
pub(crate) fn sheet_swap_refusal(command: &str, why: String) -> signex_i18n::Translatable {
    let label = match command {
        "swap_schematic_pins" => command_ids::CMD_MENU_SWAP_SCHEMATIC_PINS,
        _ => command_ids::CMD_MENU_SWAP_SCHEMATIC_GATES,
    };
    signex_i18n::Translatable::new(ids::PCB_PART_SWAP_REFUSED)
        .arg("what", signex_i18n::Translatable::new(label))
        .arg("why", why)
}
