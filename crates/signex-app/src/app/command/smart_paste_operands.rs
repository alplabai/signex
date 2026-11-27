//! `smart_paste` from its operands (#2665): the Smart Paste dialog's choices
//! (mockup row 155) as `key=value`, so a CLI run can paste once at an offset
//! or as an array with the dialog's numbering and net names. Without operands
//! a headless `smart_paste` keeps its offset paste beside the sheet's content
//! (DECISIONS 2026-11-19). Rules: docs/CLI.md, "Smart Paste".

use iced::Task;
use serde_json::{Value, json};
use signex_i18n::{Translatable, ids};
use signex_types::schematic::Point;

use super::placement_operands::run_edit;
use super::queries::text_operand;
use super::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::handlers::paste::{PasteSeed, PlacedPaste};
use crate::app::{Message, Signex};
use crate::smart_paste::SmartPasteState;
use crate::smart_paste::plan::{
    DesignatorChoice, InputProblem, MAX_COPIES, NetAfter, NetChoice, PlaceMode,
};

type Ran = (CommandOutcome, Task<Message>);

/// `smart_paste [at=x,y] [delta=dx,dy] [copies=n] [designators=..] [nets=..]
/// [net_pattern=..] [net_start=n] [document=..]`: the in-app copy placed at
/// once as one undo step. Answers with what landed.
pub(super) fn smart_paste(app: &mut Signex, args: &CommandArgs) -> Ran {
    run_edit(app, args, |app| app.smart_paste_from_operands(args))
}

impl Signex {
    fn smart_paste_from_operands(&mut self, args: &CommandArgs) -> Result<CommandOutcome, String> {
        let target = self
            .document_state
            .edit_path()
            .ok_or("no schematic is open")?
            .to_path_buf();
        let Some(copied) = self.interaction_state.clipboard.as_ref() else {
            return Ok(CommandOutcome::Disabled {
                reason: Translatable::new(ids::SMART_PASTE_NOTHING_COPIED),
            });
        };
        let step = self.smart_paste_step_mm(copied.payload.fragment());
        let mut state = SmartPasteState::new(target.clone(), step);
        choose(&mut state, args)?;
        state.input().map_err(problem_text)?;
        let at = anchor(args)?;
        let seed = self.paste_seed_of(&target)?;
        let placed = match self.smart_paste_at_once(&mut state, &seed, at) {
            Ok(placed) => placed,
            Err(reason) => return Ok(CommandOutcome::Disabled { reason }),
        };
        Ok(CommandOutcome::Answered(answer(&target, &placed)))
    }

    /// The project of `target` as a paste reads it: the prefetched read, or
    /// memory when every sheet is open; a loose sheet counts alone. A project
    /// with a closed sheet not read yet is refused here, and `run_operands`
    /// reads it and runs the command again.
    fn paste_seed_of(&self, target: &std::path::Path) -> Result<PasteSeed, String> {
        if self.document_state.document_project(target).is_none() {
            return Ok(PasteSeed::default());
        }
        self.edit_project_set(target).map(|set| PasteSeed::of(&set))
    }
}

/// The dialog's fields as the operands set them; a choice left out keeps the
/// dialog's default.
fn choose(state: &mut SmartPasteState, args: &CommandArgs) -> Result<(), String> {
    if let Some(copies) = text_operand(args, "copies") {
        state.mode = PlaceMode::Array;
        state.copies = copies.to_owned();
    }
    if let Some(delta) = text_operand(args, "delta") {
        let (x, y) = delta
            .split_once(',')
            .ok_or_else(|| format!("`delta` must be <x>,<y> in mm, not `{delta}`"))?;
        let (x, y) = (x.trim().to_owned(), y.trim().to_owned());
        match state.mode {
            PlaceMode::Offset => (state.offset_x, state.offset_y) = (x, y),
            PlaceMode::Array => (state.step_x, state.step_y) = (x, y),
        }
    }
    if let Some(designators) = text_operand(args, "designators") {
        state.designators = match designators.trim() {
            "next_free" => DesignatorChoice::NextFree,
            "keep_if_free" => DesignatorChoice::KeepIfFree,
            other => {
                return Err(format!(
                    "`designators` is `next_free` or `keep_if_free`, not `{other}`"
                ));
            }
        };
    }
    if let Some(nets) = text_operand(args, "nets") {
        state.nets = match nets.trim() {
            "keep" => NetChoice::Keep,
            "rewrite" => NetChoice::Rewrite,
            other => return Err(format!("`nets` is `keep` or `rewrite`, not `{other}`")),
        };
    }
    if let Some(pattern) = text_operand(args, "net_pattern") {
        state.pattern = pattern.to_owned();
    }
    if let Some(start) = text_operand(args, "net_start") {
        state.start = start.to_owned();
    }
    Ok(())
}

/// Why the choices cannot be pasted, naming the operand.
fn problem_text(problem: InputProblem) -> String {
    match problem {
        InputProblem::Distance => {
            "`delta` must be two numbers in mm, each within ±10000".to_string()
        }
        InputProblem::Copies => {
            format!("`copies` must be a whole number from 1 to {MAX_COPIES}")
        }
        InputProblem::Start => "`net_start` must be a whole number".to_string(),
        InputProblem::Pattern => "`net_pattern` would give two nets one name: it needs \
             `${n}` for several copies and `${Net}` for several signal nets"
            .to_string(),
    }
}

/// Where the click lands: `at` as a coordinate, `None` for the copy's own
/// place.
fn anchor(args: &CommandArgs) -> Result<Option<Point>, String> {
    match args.operand("at") {
        None => Ok(None),
        Some(OperandValue::Point(x, y)) => Ok(Some(Point::new(*x, *y))),
        Some(_) => Err("`at` is a coordinate here, not a pin reference".to_string()),
    }
}

/// The answer: the sheet, the anchor, the layout and what each copy calls
/// the copied parts and nets.
fn answer(target: &std::path::Path, placed: &PlacedPaste) -> Value {
    let designators: Vec<Value> = placed
        .designators
        .iter()
        .map(|(from, to)| json!({ "from": from, "to": to }))
        .collect();
    let nets: Vec<Value> = placed
        .nets
        .iter()
        .map(|(net, after)| match after {
            NetAfter::Joins => json!({ "net": net, "after": "joins" }),
            NetAfter::Power => json!({ "net": net, "after": "power" }),
            NetAfter::Renamed(names) => json!({ "net": net, "after": names }),
        })
        .collect();
    json!({
        "document": target.display().to_string(),
        "at": [placed.at.x, placed.at.y],
        "copies": placed.copies,
        "delta": [placed.delta.0, placed.delta.1],
        "items": placed.items,
        "designators": designators,
        "nets": nets,
    })
}
