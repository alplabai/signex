//! Track Properties for a differential pair (#2585, mockup row 95): the
//! pair, the paired net and the gap between the two tracks' copper, and a
//! Width that reaches both tracks of the pair. The pair is the router's
//! (`diff_pair_of`), the coupled track and the gap are
//! `signex_engine::pcb::route::pair_tracks`; nothing here measures
//! geometry. Width runs through one path for the panel and for
//! `set_diff_pair_width`, as one board undo step.

use std::path::Path;

use iced::Task;
use pcb_geom::ItemRef;
use serde_json::json;
use signex_engine::pcb::primitive_edit::{TrackEdit, track_edit_command};
use signex_engine::pcb::route::{PairNets, diff_pair_of, pair_name, shared_gap_mm, with_partners};
use signex_i18n::{ids, t};
use signex_types::net_class::NetClass;
use signex_types::pcb::{PcbBoard, Segment};
use uuid::Uuid;

use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::pcb_tune::{net_named, outcome, text_operand};
use crate::app::state::pcb_edit::CheckedStep;
use crate::app::{Message, Signex};
use crate::panels::pcb_primitive_properties::{PairFacts, PrimitiveTarget};

/// The pair every one of `tracks` belongs to; `None` when one is on no
/// pair's net or they span two pairs.
fn pair_of_tracks(board: &PcbBoard, classes: &[NetClass], tracks: &[&Segment]) -> Option<PairNets> {
    let first = tracks.first()?;
    let pair = diff_pair_of(board, classes, first.net)?;
    tracks
        .iter()
        .all(|s| pair.nets().contains(&s.net))
        .then_some(pair)
}

fn net_name(board: &PcbBoard, net: u32) -> String {
    board
        .nets
        .iter()
        .find(|n| n.number == net)
        .map_or_else(|| net.to_string(), |n| n.name.clone())
}

/// What the Track panel shows of the pair `tracks` belong to.
pub(super) fn pair_facts(
    board: &PcbBoard,
    classes: &[NetClass],
    tracks: &[&Segment],
) -> Option<PairFacts> {
    let pair = pair_of_tracks(board, classes, tracks)?;
    let positive = net_name(board, pair.positive);
    let negative = net_name(board, pair.negative);
    let has = |net: u32| tracks.iter().any(|s| s.net == net);
    let paired_net = match (has(pair.positive), has(pair.negative)) {
        (true, false) => Some(negative.clone()),
        (false, true) => Some(positive.clone()),
        _ => None,
    };
    let ids: Vec<Uuid> = tracks.iter().map(|s| s.uuid).collect();
    Some(PairFacts {
        name: pair_name(&positive, &negative),
        paired_net,
        gap_mm: shared_gap_mm(board, pair, &ids),
        tracks: with_partners(board, pair, &ids),
        positive,
        negative,
    })
}

/// What a pair width edit did.
struct PairWidened {
    pair: PairNets,
    tracks: Vec<Uuid>,
}

impl Signex {
    /// Enter in Width on a pair's tracks: both tracks of the pair take the
    /// width. `false` when the target is not a pair's tracks, so the plain
    /// track edit runs instead.
    pub(super) fn widen_pair_tracks(&mut self, target: &PrimitiveTarget, width: f64) -> bool {
        if target.is_placement() {
            return false;
        }
        let tracks: Vec<Uuid> = target
            .items
            .iter()
            .filter_map(|item| match item {
                ItemRef::Track(id) => Some(*id),
                _ => None,
            })
            .collect();
        if tracks.len() != target.items.len() {
            return false;
        }
        let Some((pair, reach)) = self.pair_reach(&target.board, &tracks) else {
            return false;
        };
        if let Err(why) = self.set_pair_width(&target.board, pair, &reach, width, true) {
            self.refuse_primitive_edit(ids::PCB_PRIM_WIDTH, why);
        }
        true
    }

    /// The pair of `tracks` on the board at `path` and the tracks an edit
    /// of them reaches (theirs and their partners).
    fn pair_reach(&self, path: &Path, tracks: &[Uuid]) -> Option<(PairNets, Vec<Uuid>)> {
        let board = self.document_state.pcb_board(path)?;
        let segments: Vec<&Segment> = board
            .segments
            .iter()
            .filter(|s| tracks.contains(&s.uuid))
            .collect();
        let pair = pair_of_tracks(board, self.board_net_classes(path), &segments)?;
        Some((pair, with_partners(board, pair, tracks)))
    }

    /// Give `tracks` of `pair` on the board at `path` the width `width_mm`,
    /// as one board undo step labelled Width. `from_panel` marks the step
    /// as a Properties edit for the Command Log, as every panel edit is.
    fn set_pair_width(
        &mut self,
        path: &Path,
        pair: PairNets,
        tracks: &[Uuid],
        width_mm: f64,
        from_panel: bool,
    ) -> Result<PairWidened, String> {
        let Some(engine) = self.document_state.board_engine(path) else {
            return Err(crate::app::BoardEditError::NotLoaded(path.to_path_buf()).to_string());
        };
        let board = engine.board();
        if board.unknown.newer_minor_source.is_some() {
            return Err(t!(self.ui_state.i18n, ids::PCB_PRIM_READ_ONLY).into_string());
        }
        let revision = engine.revision();
        let command = track_edit_command(board, tracks, TrackEdit::Width(width_mm))
            .map_err(|refusal| refusal.to_string())?;
        let done = PairWidened {
            pair,
            tracks: tracks.to_vec(),
        };
        let Some(command) = command else {
            return Ok(done);
        };
        let label = t!(self.ui_state.i18n, ids::PCB_PRIM_WIDTH).into_string();
        let step = CheckedStep {
            label: &label,
            expected_revision: revision,
        };
        if from_panel {
            self.ui_state.command_replay.mark_board_properties_edit();
        }
        self.execute_board_commands(path, vec![command], Some(step))
            .map_err(|error| error.to_string())?;
        Ok(done)
    }
}

/// `set_diff_pair_width document=<board> [net=<name>] width=<mm>`: every
/// track of both nets of the pair `net` belongs to, else the selected
/// tracks and the tracks coupled to them, take `width`. The answer names
/// the pair, its nets, the width, the tracks changed and the gap now.
pub(crate) fn set_diff_pair_width(
    app: &mut Signex,
    args: &CommandArgs,
) -> (CommandOutcome, Task<Message>) {
    outcome(pair_width_answer(app, args))
}

fn pair_width_answer(app: &mut Signex, args: &CommandArgs) -> Result<serde_json::Value, String> {
    let path = app.named_output_board(args)?;
    let text = text_operand(args, "width").ok_or("name the width in mm with `width`")?;
    let width = text
        .trim_end_matches("mm")
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|w| w.is_finite() && *w > 0.0)
        .ok_or_else(|| format!("`width` is a length in mm above zero, not `{text}`"))?;
    let tracks: Vec<Uuid> = match text_operand(args, "net") {
        Some(name) => {
            let net = net_named(app, &path, name)?;
            let board = app
                .document_state
                .pcb_board(&path)
                .ok_or("the board is gone")?;
            let pair = diff_pair_of(board, app.board_net_classes(&path), net)
                .ok_or_else(|| format!("`{name}` is in no differential pair"))?;
            board
                .segments
                .iter()
                .filter(|s| pair.nets().contains(&s.net))
                .map(|s| s.uuid)
                .collect()
        }
        None => selected_tracks_on(app, &path),
    };
    if tracks.is_empty() {
        return Err("name a net of the pair with `net`, or select its tracks".to_string());
    }
    let (pair, reach) = app
        .pair_reach(&path, &tracks)
        .ok_or("the tracks are not the tracks of one differential pair")?;
    let done = app.set_pair_width(&path, pair, &reach, width, false)?;
    let board = app
        .document_state
        .pcb_board(&path)
        .ok_or("the board is gone")?;
    let (positive, negative) = (
        net_name(board, done.pair.positive),
        net_name(board, done.pair.negative),
    );
    Ok(json!({
        "document": path.display().to_string(),
        "pair": pair_name(&positive, &negative),
        "positive": positive,
        "negative": negative,
        "width_mm": width,
        "tracks": done.tracks.iter().map(Uuid::to_string).collect::<Vec<_>>(),
        "gap_mm": shared_gap_mm(board, done.pair, &done.tracks),
    }))
}

/// The tracks selected on the board at `path`; none when the selection is
/// another board's.
fn selected_tracks_on(app: &Signex, path: &Path) -> Vec<Uuid> {
    let state = &app.interaction_state.pcb_selection;
    if state.document() != Some(path) {
        return Vec::new();
    }
    state
        .selection()
        .items()
        .filter_map(|item| match item {
            ItemRef::Track(id) => Some(id),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
#[path = "pair_tests.rs"]
mod tests;
