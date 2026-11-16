//! Length tuning from the PCB Active Bar's Tune group, the palette or an
//! agent (#2389). Length Tuning brings the selected tracks' nets (or the
//! net an agent names) to a target length, by default the longest of
//! them; Differential Pair Length Tuning lengthens the shorter net of a
//! pair to the other's length. The accordions are
//! `signex_engine::pcb::route::{tune_nets, tune_pair}`, kept clear of other
//! nets by the board's rules, and land as one undo step checked against
//! the revision they were computed on.

use std::path::Path;

use iced::Task;
use pcb_geom::{CopperItem, ItemRef, net_routed_length_mm, nm_to_mm};
use serde_json::json;
use signex_drc::DrcRules;
use signex_engine::pcb::BoardCommand;
use signex_engine::pcb::route::{Meander, TuneRefusal, diff_pair_of, tune_nets, tune_pair};
use signex_i18n::{MessageId, command_ids, ids, t};
use signex_types::net_class::NetClass;
use signex_types::pcb::PcbBoard;
use uuid::Uuid;

use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::state::pcb_edit::CheckedStep;
use crate::app::{Message, Signex, TuneMsg};

/// How tall a bump may grow, in accordion pitches; the track panel's
/// Tune Net To uses the same.
const AMPLITUDE_PITCHES: f64 = 4.0;

/// What a tuning landed: the nets lengthened and the length they reach.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Tuned {
    pub nets: Vec<u32>,
    pub length_mm: f64,
}

/// What to tune.
#[derive(Debug, Clone, Copy, PartialEq)]
enum TuneAsk<'a> {
    /// These nets to `target` mm, else to the longest of them.
    Nets(&'a [u32], Option<f64>),
    /// The pair `net` belongs to.
    Pair(u32),
}

impl TuneMsg {
    /// The command's name, as its Tune row reads.
    fn name(self) -> MessageId {
        match self {
            Self::NetLengthRequested => command_ids::CMD_MENU_TUNE_NET_LENGTH,
            Self::DiffPairRequested => command_ids::CMD_MENU_TUNE_DIFF_PAIR,
        }
    }
}

impl Signex {
    /// A Tune row ran on the board in view: tune the selected tracks' nets
    /// and say in Messages what was done, or why nothing was.
    pub(crate) fn handle_tune(&mut self, event: TuneMsg) {
        let what = t!(self.ui_state.i18n, event.name()).into_string();
        let Some(path) = self.active_tab_path().filter(|_| self.has_active_pcb()) else {
            return;
        };
        let nets = self.selected_track_nets();
        if nets.is_empty() {
            let line = t!(self.ui_state.i18n, ids::PCB_TUNE_NEEDS_TRACKS, "what" => what);
            crate::diagnostics::log_warning(line.into_string());
            return;
        }
        let ask = match event {
            TuneMsg::NetLengthRequested => TuneAsk::Nets(&nets, None),
            TuneMsg::DiffPairRequested => TuneAsk::Pair(nets[0]),
        };
        match self.tune_board(&path, ask, &what) {
            Ok(tuned) => {
                let length = format!("{:.3}", tuned.length_mm);
                let line = t!(
                    self.ui_state.i18n,
                    ids::PCB_TUNE_DONE,
                    "what" => what,
                    "count" => tuned.nets.len(),
                    "length" => length
                );
                crate::diagnostics::log_info(line.into_string());
            }
            Err(why) => {
                let line =
                    t!(self.ui_state.i18n, ids::PCB_TUNE_REFUSED, "what" => what, "why" => why);
                crate::diagnostics::log_warning(line.into_string());
            }
        }
    }

    /// The nets of the tracks selected on the board in view, in board order,
    /// each once.
    fn selected_track_nets(&mut self) -> Vec<u32> {
        let picked = self.with_pcb_selection(|state, board| {
            let tracks: Vec<uuid::Uuid> = state
                .selection()
                .items()
                .filter_map(|item| match item {
                    ItemRef::Track(id) => Some(id),
                    _ => None,
                })
                .collect();
            let mut nets: Vec<u32> = Vec::new();
            for segment in &board.segments {
                if tracks.contains(&segment.uuid)
                    && segment.net != 0
                    && !nets.contains(&segment.net)
                {
                    nets.push(segment.net);
                }
            }
            nets
        });
        picked.unwrap_or_default()
    }

    /// Run `ask` on the board at `path` as one undo step labelled `what`.
    fn tune_board(&mut self, path: &Path, ask: TuneAsk<'_>, what: &str) -> Result<Tuned, String> {
        let Some(engine) = self.document_state.board_engine(path) else {
            return Err(crate::app::BoardEditError::NotLoaded(path.to_path_buf()).to_string());
        };
        let board = engine.board();
        if board.unknown.newer_minor_source.is_some() {
            return Err(t!(self.ui_state.i18n, ids::PCB_PRIM_READ_ONLY).into_string());
        }
        let revision = engine.revision();
        let (rules, classes) = self.board_design_rules(path, board)?;
        if let TuneAsk::Pair(net) = ask
            && diff_pair_of(board, &classes, net).is_none()
        {
            let name = board
                .nets
                .iter()
                .find(|n| n.number == net)
                .map_or_else(|| net.to_string(), |n| n.name.clone());
            let why = t!(self.ui_state.i18n, ids::PCB_ROUTE_NO_PAIR, "net" => name);
            return Err(why.into_string());
        }
        let (commands, tuned) =
            tune_commands(board, &rules, &classes, ask).map_err(|why| why.to_string())?;
        let step = CheckedStep {
            label: what,
            expected_revision: revision,
        };
        self.execute_board_commands(path, commands, Some(step))
            .map_err(|error| error.to_string())?;
        self.refresh_pcb_highlight();
        Ok(tuned)
    }
}

/// The commands that tune `ask` on `board`, and what they reach.
fn tune_commands(
    board: &PcbBoard,
    rules: &DrcRules,
    classes: &[NetClass],
    ask: TuneAsk<'_>,
) -> Result<(Vec<BoardCommand>, Tuned), TuneRefusal> {
    let clearance = |a: &CopperItem, b: &CopperItem| rules.clearance_between(a, b);
    let pitch = |nets: &[u32]| {
        let width = board
            .segments
            .iter()
            .filter(|s| nets.contains(&s.net))
            .map(|s| s.width.abs())
            .fold(0.0, f64::max);
        // Legs a track width and a clearance apart, on either side.
        2.0 * (width + nm_to_mm(rules.clearance))
    };
    let meander = |pitch_mm: f64| Meander {
        clearance: &clearance,
        reach: rules.widest_clearance(),
        pitch_mm,
        amplitude_mm: AMPLITUDE_PITCHES * pitch_mm,
    };
    match ask {
        TuneAsk::Nets(nets, target) => {
            let target = match target {
                Some(target) => target,
                None => nets
                    .iter()
                    .filter_map(|&net| net_routed_length_mm(board, net))
                    .fold(0.0, f64::max),
            };
            let commands = tune_nets(board, nets, target, &meander(pitch(nets)), Uuid::now_v7)?;
            let tuned = Tuned {
                nets: nets.to_vec(),
                length_mm: target,
            };
            Ok((commands, tuned))
        }
        TuneAsk::Pair(net) => {
            // The caller said already when the net is in no pair.
            let pair = diff_pair_of(board, classes, net).ok_or(TuneRefusal::NoNet)?;
            let meander = meander(pitch(&pair.nets()));
            let (commands, done) = tune_pair(board, pair, &meander, Uuid::now_v7)?;
            let tuned = Tuned {
                nets: vec![done.net],
                length_mm: done.length_mm,
            };
            Ok((commands, tuned))
        }
    }
}

/// `tune_net_length document=<board> [net=<name>] [value=<mm>]`: tune the
/// named net (else the selected tracks' nets) to `value` (else the longest
/// of them). The answer names the nets and the length they reach.
pub(crate) fn tune_net_length(
    app: &mut Signex,
    args: &CommandArgs,
) -> (CommandOutcome, Task<Message>) {
    outcome(net_length_answer(app, args))
}

fn net_length_answer(app: &mut Signex, args: &CommandArgs) -> Result<serde_json::Value, String> {
    let path = app.named_board(args)?;
    let target = match text_operand(args, "value") {
        Some(text) => Some(parse_length(text)?),
        None => None,
    };
    let nets = match text_operand(args, "net") {
        Some(name) => vec![net_named(app, &path, name)?],
        None => selected_nets_on(app, &path),
    };
    if nets.is_empty() {
        return Err("name a net with `net`, or select a track of each net".to_string());
    }
    let what = t!(app.ui_state.i18n, command_ids::CMD_MENU_TUNE_NET_LENGTH).into_string();
    let tuned = app.tune_board(&path, TuneAsk::Nets(&nets, target), &what)?;
    Ok(answer(app, &path, &tuned))
}

/// `tune_diff_pair document=<board> [net=<name>]`: lengthen the shorter
/// net of the pair `net` (else the selected track's net) belongs to.
pub(crate) fn tune_diff_pair(
    app: &mut Signex,
    args: &CommandArgs,
) -> (CommandOutcome, Task<Message>) {
    outcome(diff_pair_answer(app, args))
}

fn diff_pair_answer(app: &mut Signex, args: &CommandArgs) -> Result<serde_json::Value, String> {
    let path = app.named_board(args)?;
    let net = match text_operand(args, "net") {
        Some(name) => net_named(app, &path, name)?,
        None => selected_nets_on(app, &path)
            .first()
            .copied()
            .ok_or_else(|| {
                "name a net of the pair with `net`, or select one of its tracks".to_string()
            })?,
    };
    let what = t!(app.ui_state.i18n, command_ids::CMD_MENU_TUNE_DIFF_PAIR).into_string();
    let tuned = app.tune_board(&path, TuneAsk::Pair(net), &what)?;
    Ok(answer(app, &path, &tuned))
}

fn outcome(ran: Result<serde_json::Value, String>) -> (CommandOutcome, Task<Message>) {
    match ran {
        Ok(answer) => (CommandOutcome::Answered(answer), Task::none()),
        Err(reason) => (CommandOutcome::Disabled { reason }, Task::none()),
    }
}

/// The JSON answer: the board, and each tuned net by name with its length
/// now.
fn answer(app: &Signex, path: &Path, tuned: &Tuned) -> serde_json::Value {
    let board = app.document_state.pcb_board(path);
    let nets: Vec<serde_json::Value> = tuned
        .nets
        .iter()
        .map(|&net| {
            let name = board
                .and_then(|b| b.nets.iter().find(|n| n.number == net))
                .map(|n| n.name.clone())
                .unwrap_or_default();
            let length = board.and_then(|b| net_routed_length_mm(b, net));
            json!({ "net": name, "length_mm": length })
        })
        .collect();
    json!({
        "document": path.display().to_string(),
        "target_mm": tuned.length_mm,
        "nets": nets,
    })
}

/// The selected tracks' nets when the board at `path` is the one in view;
/// none for another board, whose selection is not this one.
fn selected_nets_on(app: &mut Signex, path: &Path) -> Vec<u32> {
    if app.active_tab_path().as_deref() == Some(path) {
        app.selected_track_nets()
    } else {
        Vec::new()
    }
}

fn text_operand<'a>(args: &'a CommandArgs, name: &str) -> Option<&'a str> {
    match args.operand(name) {
        Some(OperandValue::Text(text)) => Some(text.trim()).filter(|t| !t.is_empty()),
        _ => None,
    }
}

/// `value` as a length in mm: a positive finite number, `mm` optional.
fn parse_length(text: &str) -> Result<f64, String> {
    let number = text.trim_end_matches("mm").trim();
    number
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && *v > 0.0)
        .ok_or_else(|| format!("`value` is a length in mm above zero, not `{text}`"))
}

/// The number of the net called `name` on the board at `path`.
fn net_named(app: &Signex, path: &Path, name: &str) -> Result<u32, String> {
    app.document_state
        .pcb_board(path)
        .and_then(|board| board.nets.iter().find(|n| n.name == name && n.number != 0))
        .map(|n| n.number)
        .ok_or_else(|| format!("the board has no net called `{name}`"))
}
