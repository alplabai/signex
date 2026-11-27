//! Pin and gate swap on a board (#2410). Two pins of a component, or every
//! pin of two of its gates, trade nets on the board
//! (`signex_engine::pcb::pad_swap`) and the linked schematic part follows
//! (`signex_engine::part_swap`), as one undo step across both, the way a
//! footprint swap does (#2366). Which pins and gates may trade is the part's
//! own rule (`signex_types::part_swap`). A refusal changes nothing and says
//! why in Messages. Rules: `docs/PCB_TOOLS.md`, "Pin and gate swap".

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use iced::Task;
use serde::Serialize;
use signex_engine::part_swap::{self, PlacedPin};
use signex_engine::pcb::BoardCommand;
use signex_engine::pcb::pad_swap::{self, PadNetChange, SwapKind};
use signex_i18n::{command_ids, ids, t};
use signex_types::schematic::{SchematicSheet, Symbol};
use uuid::Uuid;

use crate::app::command::{CommandOutcome, OperandValue, translator};
use crate::app::state::pcb_edit::CheckedStep;
use crate::app::{Message, PartSwapMsg, Signex, UndoGroupKind};

/// What a swap trades.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SwapRequest {
    /// Two pins, by pad number.
    Pins(String, String),
    /// Two gates, by unit number.
    Gates(u32, u32),
    /// The gates the two pins (pad numbers) belong to.
    GatesOfPins(String, String),
}

impl SwapRequest {
    fn kind(&self) -> SwapKind {
        match self {
            Self::Pins(..) => SwapKind::Pins,
            Self::Gates(..) | Self::GatesOfPins(..) => SwapKind::Gates,
        }
    }
}

/// A pad whose net the swap changed, as the answer lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct SwappedPad {
    pub pad: String,
    pub from: Option<String>,
    pub to: Option<String>,
}

/// What a swap did, as the command answers it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct PartSwapReport {
    pub designator: String,
    /// `pins` or `gates`.
    pub kind: &'static str,
    /// The two pad numbers, or the two unit numbers.
    pub swapped: [String; 2],
    /// Each pad whose net changed; empty when they already shared nets.
    pub pads: Vec<SwappedPad>,
    /// The sheet that followed, when one had anything to rewire.
    pub sheet: Option<String>,
}

/// Both halves of a swap, built before anything runs.
struct SwapPlan {
    designator: String,
    kind: SwapKind,
    swapped: [String; 2],
    board: Vec<BoardCommand>,
    changes: Vec<PadNetChange>,
    sheet: Option<(PathBuf, SchematicSheet)>,
}

impl Signex {
    pub(crate) fn handle_part_swap(&mut self, msg: PartSwapMsg) -> Task<Message> {
        match msg {
            PartSwapMsg::Requested(kind) => self.swap_selected_pads(kind),
        }
    }

    /// `swap_pins` / `swap_gates` without operands: the two pads selected on
    /// the board in view name the component and the pins, or the gates they
    /// belong to.
    fn swap_selected_pads(&mut self, kind: SwapKind) -> Task<Message> {
        let Some(board) = self.active_tab_path().filter(|_| self.has_active_pcb()) else {
            return Task::none();
        };
        let pair = self
            .with_pcb_selection(|state, _| {
                crate::app::handlers::pcb_placement::selected_pad_pair(state.selection())
            })
            .flatten();
        let Some((footprint, pad_a, pad_b)) = pair else {
            self.refuse_part_swap(kind, "select two pads of one component on the board");
            return Task::none();
        };
        let numbers = self.document_state.pcb_board(&board).and_then(|b| {
            let fp = b.footprints.iter().find(|fp| fp.uuid == footprint)?;
            let number = |id: Uuid| {
                fp.pads
                    .iter()
                    .find(|p| p.uuid == id)
                    .map(|p| p.number.clone())
            };
            Some((number(pad_a)?, number(pad_b)?))
        });
        let Some((a, b)) = numbers else {
            return Task::none();
        };
        let request = match kind {
            SwapKind::Pins => SwapRequest::Pins(a, b),
            SwapKind::Gates => SwapRequest::GatesOfPins(a, b),
        };
        self.translate_part_swap(&board, footprint, &request)
    }

    /// Run the swap `request` of the menu through the command's operand
    /// handler (`command::translator`), on the clicked component by its uuid
    /// (`ids`) and by pad or unit numbers. It is planned here first, so a
    /// refusal reads as the menu's always did and the gates of two pads are
    /// known as units.
    /// shortcut: the plan is built twice, here and in the handler; drop
    /// this one if the handler ever takes pads.
    fn translate_part_swap(
        &mut self,
        board: &Path,
        footprint: Uuid,
        request: &SwapRequest,
    ) -> Task<Message> {
        let kind = request.kind();
        let plan = match self.plan_part_swap(board, footprint, request) {
            Ok(plan) => plan,
            Err(why) => {
                self.refuse_part_swap(kind, &why);
                return Task::none();
            }
        };
        let (command, pair) = match kind {
            SwapKind::Pins => ("swap_pins", "pins"),
            SwapKind::Gates => ("swap_gates", "units"),
        };
        let operands = vec![
            ("ids", OperandValue::Targets(vec![footprint])),
            (pair, OperandValue::Text(plan.swapped.join(","))),
        ];
        let (outcome, task) = translator::run(self, command, board, operands);
        self.say_part_swap_refusal(kind, &outcome);
        task
    }

    /// Say in Messages why the menu's swap did nothing, when `outcome` is a
    /// refusal: of its operands, or of the board or the schematic, which
    /// `swap_part` leaves to the menu.
    pub(super) fn say_part_swap_refusal(&self, kind: SwapKind, outcome: &CommandOutcome) {
        if let CommandOutcome::Disabled { reason } = outcome {
            let why = reason.render(&self.ui_state.i18n);
            self.refuse_part_swap(kind, &why);
        }
    }

    /// Swap `request` on `footprint` of `board` and its schematic part, as
    /// one undo step. A refusal is returned, and said in Messages unless the
    /// board menu runs it (`agent.from_view`), which says it itself.
    pub(crate) fn swap_part(
        &mut self,
        board: &Path,
        footprint: Uuid,
        request: &SwapRequest,
    ) -> Result<PartSwapReport, String> {
        let kind = request.kind();
        let result = self.run_part_swap(board, footprint, request);
        if let Err(why) = &result
            && !self.agent.from_view
        {
            self.refuse_part_swap(kind, why);
        }
        result
    }

    fn run_part_swap(
        &mut self,
        board: &Path,
        footprint: Uuid,
        request: &SwapRequest,
    ) -> Result<PartSwapReport, String> {
        let plan = self.plan_part_swap(board, footprint, request)?;
        let what = self.part_swap_label(plan.kind);
        let mut report = PartSwapReport {
            designator: plan.designator.clone(),
            kind: match plan.kind {
                SwapKind::Pins => "pins",
                SwapKind::Gates => "gates",
            },
            swapped: plan.swapped.clone(),
            pads: Vec::new(),
            sheet: None,
        };
        if plan.board.is_empty() {
            let id = match plan.kind {
                SwapKind::Pins => ids::PCB_PART_SWAP_NOTHING,
                SwapKind::Gates => ids::PCB_GATES_SWAP_NOTHING,
            };
            self.report_part_swap(id, &plan.designator, &plan.swapped);
            return Ok(report);
        }
        let revision = self
            .document_state
            .board_engine(board)
            .map(|engine| engine.revision())
            .ok_or_else(|| "the board is not open".to_string())?;
        let step = CheckedStep {
            label: &what,
            expected_revision: revision,
        };
        match plan.sheet {
            None => {
                self.execute_board_commands(board, plan.board, Some(step))
                    .map_err(|error| error.to_string())?;
            }
            Some((path, sheet)) => {
                let group = self.mint_undo_group();
                let commands = vec![signex_engine::Command::ReplaceDocument { document: sheet }];
                let sheets = BTreeMap::from([(path.clone(), commands)]);
                let changed = self.run_linked_footprints(sheets, group, &what);
                if changed.is_empty() {
                    return Err(format!(
                        "the sheet of {} could not be rewired; nothing changed",
                        plan.designator
                    ));
                }
                if let Err(error) =
                    self.execute_board_commands_grouped(board, plan.board, step, group)
                {
                    self.roll_back_linked_footprints(&changed, group, &what);
                    return Err(error.to_string());
                }
                let board = board.to_path_buf();
                self.commit_linked_footprints(changed, group, board, UndoGroupKind::PartSwap);
                report.sheet = Some(path.display().to_string());
            }
        }
        report.pads = plan
            .changes
            .into_iter()
            .map(|change| SwappedPad {
                pad: change.pad,
                from: change.from,
                to: change.to,
            })
            .collect();
        let id = match plan.kind {
            SwapKind::Pins => ids::PCB_PINS_SWAPPED,
            SwapKind::Gates => ids::PCB_GATES_SWAPPED,
        };
        self.report_part_swap(id, &plan.designator, &plan.swapped);
        self.sync_pcb_board_summary();
        Ok(report)
    }

    /// Both halves of the swap, checked: the part's swap groups, the board's
    /// locks and switches, and what the sheet can move.
    fn plan_part_swap(
        &self,
        board: &Path,
        footprint: Uuid,
        request: &SwapRequest,
    ) -> Result<SwapPlan, String> {
        let pcb = self
            .document_state
            .pcb_board(board)
            .ok_or_else(|| "the board is not open".to_string())?;
        let fp = pcb
            .footprints
            .iter()
            .find(|fp| fp.uuid == footprint)
            .ok_or_else(|| "the component is no longer on the board".to_string())?;
        let designator = signex_engine::pcb::placement::name(fp);
        let link = fp.schematic_link.as_ref().ok_or_else(|| {
            format!("{designator} has no schematic part to say which pins and gates may swap")
        })?;
        let (path, part) = self.linked_part_sheet(link).ok_or_else(|| {
            format!("the sheet of {designator} is not open; open it to swap its pins or gates")
        })?;
        let doc = self
            .document_state
            .engines
            .get(&path)
            .map(|engine| engine.document())
            .ok_or_else(|| format!("the sheet of {designator} is not open"))?;
        if doc.is_read_only() {
            return Err(format!("the sheet of {designator} opened read-only"));
        }
        let units: Vec<&Symbol> = signex_engine::placed_unit_ids(doc, part)
            .into_iter()
            .filter_map(|id| doc.symbols.iter().find(|s| s.uuid == id))
            .collect();
        let first = *units
            .first()
            .ok_or_else(|| format!("{designator} is not placed on its sheet"))?;
        let lib = doc
            .resolve_lib_symbol(first)
            .ok_or_else(|| format!("the library symbol of {designator} is not on its sheet"))?;
        let gates = |a: u32, b: u32| -> Result<SwapPlan, String> {
            part_swap::check_gate_group(first, &designator, a, b)
                .map_err(|refusal| refusal.to_string())?;
            let pairs = part_swap::gate_pin_pairs(&lib, &designator, a, b)
                .map_err(|refusal| refusal.to_string())?;
            let swap = pad_swap::pad_swap(pcb, footprint, &pairs, SwapKind::Gates)
                .map_err(|refusal| refusal.to_string())?;
            if link
                .units
                .iter()
                .any(|u| !doc.symbols.iter().any(|s| s.uuid == *u))
            {
                return Err(format!(
                    "the gates of {designator} are on more than one sheet; swap them on the sheet"
                ));
            }
            let trades: Vec<(Uuid, u32)> = units
                .iter()
                .filter_map(|s| match s.unit {
                    u if u == a => Some((s.uuid, b)),
                    u if u == b => Some((s.uuid, a)),
                    _ => None,
                })
                .collect();
            let sheet = if trades.is_empty() {
                None
            } else {
                let swapped =
                    part_swap::swap_units(doc, &trades).map_err(|refusal| refusal.to_string())?;
                Some((path.clone(), swapped))
            };
            Ok(SwapPlan {
                designator: designator.clone(),
                kind: SwapKind::Gates,
                swapped: [a.to_string(), b.to_string()],
                board: swap.commands,
                changes: swap.changes,
                sheet,
            })
        };
        match request {
            SwapRequest::Gates(a, b) => gates(*a, *b),
            SwapRequest::GatesOfPins(a, b) => {
                let unit = |pin: &str| match part_swap::unit_of_pin(&lib, pin) {
                    Some(0) => Err(format!("pin {pin} of {designator} belongs to every gate")),
                    Some(unit) => Ok(unit),
                    None => Err(format!("{designator} has no pin {pin}")),
                };
                let (gate_a, gate_b) = (unit(a.as_str())?, unit(b.as_str())?);
                if gate_a == gate_b {
                    return Err(format!(
                        "pins {a} and {b} of {designator} are on one gate; pick a pin of each gate"
                    ));
                }
                gates(gate_a, gate_b)
            }
            SwapRequest::Pins(a, b) => {
                part_swap::check_pin_group(first, &designator, a, b)
                    .map_err(|refusal| refusal.to_string())?;
                let pairs = [(a.clone(), b.clone())];
                let swap = pad_swap::pad_swap(pcb, footprint, &pairs, SwapKind::Pins)
                    .map_err(|refusal| refusal.to_string())?;
                let holding = |pin: &str| {
                    part_swap::unit_holding(&units, &lib, pin, &designator)
                        .map_err(|refusal| refusal.to_string())
                };
                let pin_a = PlacedPin {
                    symbol: holding(a.as_str())?,
                    number: a,
                };
                let pin_b = PlacedPin {
                    symbol: holding(b.as_str())?,
                    number: b,
                };
                let sheet = part_swap::rewire_pins(doc, pin_a, pin_b)
                    .map_err(|refusal| refusal.to_string())?;
                Ok(SwapPlan {
                    designator: designator.clone(),
                    kind: SwapKind::Pins,
                    swapped: [a.clone(), b.clone()],
                    board: swap.commands,
                    changes: swap.changes,
                    sheet: sheet.map(|sheet| (path.clone(), sheet)),
                })
            }
        }
    }

    /// The command's menu name, which labels its undo step.
    fn part_swap_label(&self, kind: SwapKind) -> String {
        let id = match kind {
            SwapKind::Pins => command_ids::CMD_MENU_SWAP_PINS,
            SwapKind::Gates => command_ids::CMD_MENU_SWAP_GATES,
        };
        t!(self.ui_state.i18n, id).into_string()
    }

    fn refuse_part_swap(&self, kind: SwapKind, why: &str) {
        let what = self.part_swap_label(kind);
        let line = t!(
            self.ui_state.i18n,
            ids::PCB_PART_SWAP_REFUSED,
            "what" => what.as_str(),
            "why" => why
        );
        crate::diagnostics::log_warning(line.into_string());
    }

    fn report_part_swap(
        &self,
        id: signex_i18n::MessageId,
        designator: &str,
        swapped: &[String; 2],
    ) {
        let line = t!(
            self.ui_state.i18n,
            id,
            "designator" => designator,
            "a" => swapped[0].as_str(),
            "b" => swapped[1].as_str()
        );
        crate::diagnostics::log_info(line.into_string());
    }
}
