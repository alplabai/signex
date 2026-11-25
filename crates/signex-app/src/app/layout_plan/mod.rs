//! The board's layout plan (#2942): `layout_plan_get`, `layout_plan_set` and
//! `layout_plan_status` from their operands. The plan is board data
//! (`BoardOptions::layout_plan`, `signex_types::pcb_layout_plan`), changed
//! only through `BoardCommand::SetBoardOptions`, one undo step per set; the
//! status is `signex_drc::layout_plan`. Each answers with JSON on every
//! surface. See `docs/CLI.md`, "Layout plan".

#[cfg(test)]
mod tests;

use std::path::Path;

use iced::Task;
use serde_json::{Map, Value, json};
use signex_drc::layout_plan::{
    PlanInput, StageStatus, is_known_rule, missing_references, plan_status,
};
use signex_engine::pcb::BoardCommand;
use signex_i18n::{ids, t};
use signex_types::pcb_layout_plan::{LayoutPlan, LayoutSetting, LayoutStage};

use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::state::pcb_edit::CheckedStep;
use crate::app::{Message, Signex};

type Ran = (CommandOutcome, Task<Message>);

fn answered(result: Result<Value, String>) -> Ran {
    let outcome = match result {
        Ok(answer) => CommandOutcome::Answered(answer),
        Err(reason) => CommandOutcome::disabled(reason),
    };
    (outcome, Task::none())
}

fn text<'a>(args: &'a CommandArgs, name: &str) -> Option<&'a str> {
    match args.operand(name) {
        Some(OperandValue::Text(text)) => Some(text.as_str()),
        _ => None,
    }
}

/// The settings of `stage` by operand name, every one listed.
fn settings_json(plan: &LayoutPlan, stage: LayoutStage) -> Value {
    let mut out = Map::new();
    for setting in stage.settings() {
        let value = match setting {
            LayoutSetting::Weights => {
                let weights: Map<String, Value> = plan
                    .placement
                    .weights
                    .iter()
                    .map(|(priority, weight)| (priority.token().to_string(), json!(weight)))
                    .collect();
                Value::Object(weights)
            }
            LayoutSetting::Rooms => json!(plan.placement.rooms),
            LayoutSetting::Keep => json!(plan.placement.keep),
            LayoutSetting::Nets => json!(plan.routing.nets),
            LayoutSetting::Skip => json!(plan.routing.skip),
            LayoutSetting::Pours => json!(plan.pours.pours),
            LayoutSetting::Rules => json!(plan.checks.rules),
            LayoutSetting::Requirements => json!(plan.checks.requirements),
        };
        out.insert(setting.operand().to_string(), value);
    }
    Value::Object(out)
}

fn status_json(status: &StageStatus) -> Value {
    let mut out = json!({
        "stage": status.stage.token(),
        "state": status.state.token(),
        "reasons": status.reasons,
    });
    for (name, count) in &status.counts {
        out[*name] = json!(count);
    }
    out
}

impl Signex {
    /// The board's plan, its revision and what the plan names that the
    /// board or its project does not have.
    fn layout_plan_answer(&self, path: &Path) -> Result<Value, String> {
        let engine = self
            .document_state
            .board_engine(path)
            .ok_or_else(|| format!("no PCB is open at {}", path.display()))?;
        let plan = &engine.board().options.layout_plan;
        let stages: Vec<Value> = LayoutStage::ALL
            .into_iter()
            .map(|stage| {
                json!({
                    "stage": stage.token(),
                    "set": !plan.stage_is_empty(stage),
                    "settings": settings_json(plan, stage),
                })
            })
            .collect();
        let requirements = self.board_requirement_ids(path);
        let input = self.plan_input(path, engine.board(), &requirements, false);
        Ok(json!({
            "document": path.display().to_string(),
            "revision": engine.revision(),
            "empty": plan.is_empty(),
            "stages": stages,
            "warnings": missing_references(plan, &input),
        }))
    }

    /// The ids of the requirements of the board's project.
    fn board_requirement_ids(&self, board: &Path) -> Vec<String> {
        self.document_state
            .board_project(board, None)
            .map(|project| {
                project
                    .data
                    .requirements
                    .iter()
                    .filter_map(|entry| entry.id().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// What the status reads besides the plan; the DRC run only when
    /// `with_drc` and it ran on the board as it is now.
    fn plan_input<'a>(
        &'a self,
        path: &Path,
        board: &'a signex_types::pcb::PcbBoard,
        requirements: &'a [String],
        with_drc: bool,
    ) -> PlanInput<'a> {
        let net_classes = self.board_net_classes(path);
        let revision = self.document_state.pcb_revisions.get(path).copied();
        let drc = self
            .interaction_state
            .pcb_drc
            .done_for(path)
            .filter(|done| with_drc && revision.is_some_and(|r| !done.is_stale(r, net_classes)))
            .map(|done| &done.report);
        PlanInput {
            board,
            net_classes,
            requirements,
            drc,
        }
    }

    /// Each stage of the board's plan: not started, done or failing, and why.
    fn layout_plan_status_answer(&self, path: &Path) -> Result<Value, String> {
        let engine = self
            .document_state
            .board_engine(path)
            .ok_or_else(|| format!("no PCB is open at {}", path.display()))?;
        let plan = &engine.board().options.layout_plan;
        let requirements = self.board_requirement_ids(path);
        let input = self.plan_input(path, engine.board(), &requirements, true);
        let all = plan_status(plan, &input);
        let next = all
            .iter()
            .find(|status| status.state != signex_drc::layout_plan::StageState::Done)
            .map(|status| status.stage.token());
        let stages: Vec<Value> = all
            .iter()
            .map(|status| {
                let mut out = status_json(status);
                out["set"] = json!(!plan.stage_is_empty(status.stage));
                out
            })
            .collect();
        Ok(json!({
            "document": path.display().to_string(),
            "revision": engine.revision(),
            "drc_current": input.drc.is_some(),
            "stages": stages,
            "next": next,
        }))
    }

    /// Set `plan` on the board at `path` as one undo step; whether it
    /// changed. The board's DRC run stays current: the plan is not checked.
    fn apply_layout_plan(&mut self, path: &Path, plan: LayoutPlan) -> Result<bool, String> {
        let Some(engine) = self.document_state.board_engine(path) else {
            return Err(format!("no PCB is open at {}", path.display()));
        };
        let board = engine.board();
        if board.options.layout_plan == plan {
            return Ok(false);
        }
        let mut options = board.options.clone();
        options.layout_plan = plan;
        let label = t!(self.ui_state.i18n, ids::PCB_BOARD_LAYOUT_PLAN_UNDO).into_string();
        let step = CheckedStep {
            label: &label,
            expected_revision: engine.revision(),
        };
        let from = self.document_state.pcb_revisions.get(path).copied();
        let command = BoardCommand::SetBoardOptions {
            options: Box::new(options),
        };
        self.execute_board_commands(path, vec![command], Some(step))
            .map_err(|e| e.to_string())?;
        let to = self.document_state.pcb_revisions.get(path).copied();
        if let (Some(from), Some(to)) = (from, to) {
            self.interaction_state
                .pcb_drc
                .carry_revision(path, from, to);
        }
        Ok(true)
    }
}

/// The plan `args` asks for, from the board's `plan`: the stage cleared
/// (`remove=true`) or each of its settings given set. A setting of another
/// stage, an unknown stage, a built-in rule this build does not have, or
/// nothing to set is refused.
fn edited_plan(plan: &LayoutPlan, args: &CommandArgs) -> Result<(LayoutStage, LayoutPlan), String> {
    let stages: Vec<&str> = LayoutStage::ALL.iter().map(|s| s.token()).collect();
    let stage = text(args, "stage")
        .and_then(LayoutStage::from_token)
        .ok_or_else(|| format!("`stage` is one of {}", stages.join(", ")))?;
    let given: Vec<(LayoutSetting, &str)> = LayoutSetting::ALL
        .into_iter()
        .filter_map(|setting| text(args, setting.operand()).map(|value| (setting, value)))
        .collect();
    if let Some((setting, _)) = given.iter().find(|(s, _)| s.stage() != stage) {
        return Err(format!(
            "`{}` is a setting of the {} stage, not of {}",
            setting.operand(),
            setting.stage().token(),
            stage.token()
        ));
    }
    let mut edited = plan.clone();
    let remove = matches!(args.operand("remove"), Some(OperandValue::Bool(true)));
    if remove {
        if !given.is_empty() {
            return Err("`remove` clears the whole stage; give it no setting".into());
        }
        edited.clear(stage);
        return Ok((stage, edited));
    }
    if given.is_empty() {
        let names: Vec<&str> = stage.settings().map(|s| s.operand()).collect();
        return Err(format!(
            "give a setting of the {} stage ({}) or `remove=true`",
            stage.token(),
            names.join(", ")
        ));
    }
    for (setting, value) in given {
        edited.set(setting, value).map_err(|e| e.0)?;
    }
    if let Some(rule) = edited.checks.rules.iter().find(|id| !is_known_rule(id)) {
        return Err(format!("`{rule}` is not a DRC rule of this build"));
    }
    Ok((stage, edited))
}

impl Signex {
    /// The queries `layout_plan_get document=<board>` (the plan, every stage
    /// and setting) and `layout_plan_status document=<board>` (each stage
    /// judged, and the first one not done).
    pub(crate) fn layout_plan_query(&self, id: &str, args: &CommandArgs) -> Result<Value, String> {
        let path = self.named_output_board(args)?;
        if id == "layout_plan_status" {
            self.layout_plan_status_answer(&path)
        } else {
            self.layout_plan_answer(&path)
        }
    }
}

/// `layout_plan_set document=<board> stage=<stage> [<setting>=<list>]...
/// [remove=true]`: one stage of the plan as one undo step, answered with
/// the plan as `layout_plan_get` reads it and whether it changed.
pub(crate) fn layout_plan_set(app: &mut Signex, args: &CommandArgs) -> Ran {
    let result = app.named_output_board(args).and_then(|path| {
        let plan = app
            .document_state
            .pcb_board(&path)
            .map(|board| board.options.layout_plan.clone())
            .ok_or_else(|| format!("no PCB is open at {}", path.display()))?;
        let (stage, edited) = edited_plan(&plan, args)?;
        let changed = app.apply_layout_plan(&path, edited)?;
        let mut answer = app.layout_plan_answer(&path)?;
        answer["stage"] = json!(stage.token());
        answer["changed"] = json!(changed);
        Ok(answer)
    });
    answered(result)
}
