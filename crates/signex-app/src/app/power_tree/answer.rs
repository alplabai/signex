//! The `query_power_tree` answer (#1313): the tree as JSON, each part named
//! with the sheet and the symbol a click on it goes to. Shape:
//! `docs/POWER_TREE.md`, "The answer".

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use signex_net::SheetKey;
use signex_net::power_tree::{
    BudgetLevel, Converter, CurrentFrom, PowerProblem, PowerRail, PowerTopology, PowerTree,
    SheetBudget, VoltageFrom,
};
use uuid::Uuid;

/// What the answer names besides the tree.
pub(super) struct AnswerContext<'a> {
    pub(super) project: &'a Path,
    pub(super) dir: &'a Path,
    /// The variant read; `None` is Base.
    pub(super) variant: Option<&'a str>,
    /// Each sheet's file.
    pub(super) paths: &'a HashMap<SheetKey, PathBuf>,
    pub(super) warnings: Vec<String>,
}

/// The whole answer.
pub(super) fn power_tree_answer(
    topology: &PowerTopology,
    tree: &PowerTree,
    budgets: &[SheetBudget],
    context: AnswerContext<'_>,
) -> Value {
    let parts = Parts {
        topology,
        context: &context,
    };
    json!({
        "project": context.project.display().to_string(),
        "variant": context.variant,
        "roots": tree.roots,
        "grounds": topology.grounds,
        "rails": tree.rails.iter().map(|rail| parts.rail(rail)).collect::<Vec<_>>(),
        "converters": tree.converters.iter().map(|c| parts.converter(c)).collect::<Vec<_>>(),
        // Not `sheets`: the command line filters a `sheets` list as documents
        // the call named (docs/CLI.md, "Only this run's folders are shown").
        "sheet_budgets": budgets.iter().map(|b| parts.sheet(b, tree)).collect::<Vec<_>>(),
        "warnings": context.warnings,
    })
}

struct Parts<'a> {
    topology: &'a PowerTopology,
    context: &'a AnswerContext<'a>,
}

impl Parts<'_> {
    /// Part `at` of the topology: its designator and value, and the sheet
    /// and symbol a click on it goes to.
    fn part(&self, at: usize) -> Value {
        let part = &self.topology.parts[at];
        json!({
            "designator": part.designator,
            "value": part.value,
            "sheet": part.sheet_id.to_string(),
            "sheet_path": self.sheet_path(&part.sheet),
            "symbol": part.units.first().map(Uuid::to_string),
        })
    }

    fn designators(&self, parts: &[usize]) -> Vec<&str> {
        parts
            .iter()
            .map(|&at| self.topology.parts[at].designator.as_str())
            .collect()
    }

    /// The sheet's file relative to the project folder, `/`-separated.
    fn sheet_path(&self, sheet: &SheetKey) -> Option<String> {
        let path = self.context.paths.get(sheet)?;
        let relative = path.strip_prefix(self.context.dir).unwrap_or(path);
        Some(relative.to_string_lossy().replace('\\', "/"))
    }

    fn rail(&self, rail: &PowerRail) -> Value {
        let limit = rail.limit_a();
        let level = limit.and_then(|limit| BudgetLevel::of(rail.total_a, limit));
        let sources: Vec<Value> = rail
            .sources
            .iter()
            .map(|source| {
                json!({
                    "part": self.part(source.part),
                    "limit_a": source.limit_a,
                    "converter": source.is_converter,
                })
            })
            .collect();
        let loads: Vec<Value> = rail
            .loads
            .iter()
            .map(|load| {
                let share = load
                    .current_a
                    .filter(|_| rail.total_a > 0.0)
                    .map(|current| current / rail.total_a);
                json!({
                    "part": self.part(load.part),
                    "current_a": load.current_a,
                    "current_from": load.from.map(current_from),
                    "share_of_rail": share,
                })
            })
            .collect();
        json!({
            "net": rail.net,
            "voltage": rail.voltage.map(|v| v.volts),
            "tolerance_percent": rail.voltage.and_then(|v| v.tolerance_percent),
            "voltage_from": rail.voltage.map(|v| voltage_from(v.from)),
            "sources": sources,
            "loads": loads,
            "total_a": rail.total_a,
            "complete": rail.is_complete,
            "limit_a": limit,
            "headroom_a": rail.headroom_a(),
            "level": level.map(level_token),
            "fed_from": rail.fed_from,
            "problems": rail.problems.iter().map(problem).collect::<Vec<_>>(),
        })
    }

    fn converter(&self, converter: &Converter) -> Value {
        json!({
            "part": self.part(converter.part),
            "inputs": converter.inputs,
            "outputs": converter.outputs,
            "efficiency": converter.efficiency,
            "output_w": converter.output_w,
            "input_w": converter.input_w,
            "loss_w": converter.loss_w(),
        })
    }

    fn sheet(&self, budget: &SheetBudget, tree: &PowerTree) -> Value {
        let path = self
            .topology
            .parts
            .iter()
            .find(|part| part.sheet_id == budget.sheet)
            .and_then(|part| self.sheet_path(&part.sheet));
        let draws: Vec<Value> = budget
            .draws
            .iter()
            .map(|draw| {
                json!({
                    "net": draw.net,
                    "current_a": draw.current_a,
                    "complete": draw.is_complete,
                    "parts": self.designators(&draw.parts),
                })
            })
            .collect();
        let feeds: Vec<Value> = budget
            .feeds
            .iter()
            .map(|feed| {
                json!({
                    "net": feed.net,
                    "limit_a": feed.limit_a,
                    "parts": self.designators(&feed.parts),
                })
            })
            .collect();
        json!({
            "sheet": budget.sheet.to_string(),
            "sheet_path": path,
            "draws": draws,
            "feeds": feeds,
            "power_w": budget.power_w(tree),
        })
    }
}

fn voltage_from(from: VoltageFrom) -> &'static str {
    match from {
        VoltageFrom::Project => "project",
        VoltageFrom::NetName => "net_name",
    }
}

fn current_from(from: CurrentFrom) -> &'static str {
    match from {
        CurrentFrom::Parameter => "parameter",
        CurrentFrom::Converter => "converter",
    }
}

fn level_token(level: BudgetLevel) -> &'static str {
    match level {
        BudgetLevel::Fine => "fine",
        BudgetLevel::Close => "close",
        BudgetLevel::Over => "over",
    }
}

/// A problem as `{"kind": ...}` with what it names; the kinds are listed in
/// `docs/POWER_TREE.md`.
fn problem(problem: &PowerProblem) -> Value {
    match problem {
        PowerProblem::NoVoltage => json!({"kind": "no_voltage"}),
        PowerProblem::NoSource => json!({"kind": "no_source"}),
        PowerProblem::SeveralSources(count) => {
            json!({"kind": "several_sources", "count": count})
        }
        PowerProblem::UnknownLoad { designator } => {
            json!({"kind": "unknown_load", "designator": designator})
        }
        PowerProblem::UnreadParameter {
            designator,
            parameter,
            text,
        } => json!({
            "kind": "unread_parameter",
            "designator": designator,
            "parameter": parameter,
            "text": text,
        }),
        PowerProblem::UnknownInput { designator } => {
            json!({"kind": "unknown_input", "designator": designator})
        }
    }
}
