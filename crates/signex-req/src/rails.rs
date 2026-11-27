//! Rail budgets (#2943): what each power rail can supply against what its
//! parts draw, for the `rail_budget` requirement and the
//! `query_rail_budget` answer. A rail's voltage and tolerance are the
//! project's (`signex_types::project::RailSpec`); the current a part draws
//! from a rail is its parameter `Rail Load <net>`, the current a source part
//! can supply its parameter `Rail Supply <net>`, in A (`0.25`, `250mA`,
//! `250 uA`). Only fitted parts count. Missing or unreadable data is a
//! finding, never a pass. Rules: `docs/CLI.md`, "Rail budgets".

use std::collections::BTreeMap;

use serde_json::{Value, json};
use signex_bom::BomTable;
use signex_types::net_class::glob_matches;
use signex_types::project::{RailSpec, is_percent};

use crate::Miss;

/// The parameter a part names the current it draws from a rail by.
pub const LOAD_PARAMETER: &str = "Rail Load ";
/// The parameter a source part names the current it can supply by.
pub const SUPPLY_PARAMETER: &str = "Rail Supply ";

/// Below a nanoampere two currents are the same.
const SAME_A: f64 = 1e-9;
/// Below a microvolt two voltages are the same.
const SAME_V: f64 = 1e-6;

/// One part's current on a rail, in A.
#[derive(Debug, Clone, PartialEq)]
pub struct RailPart {
    /// The BOM line's references, comma-joined.
    pub designator: String,
    pub current_a: f64,
}

/// A rail parameter whose value is no current.
#[derive(Debug, Clone, PartialEq)]
pub struct UnreadCurrent {
    pub designator: String,
    pub parameter: String,
    pub text: String,
}

/// One rail: its spec, if the project sets one, and the parts on it.
#[derive(Debug, Clone, PartialEq)]
pub struct RailBudget {
    pub net: String,
    pub spec: Option<RailSpec>,
    pub sources: Vec<RailPart>,
    pub loads: Vec<RailPart>,
    pub unread: Vec<UnreadCurrent>,
}

impl RailBudget {
    fn new(net: &str, spec: Option<RailSpec>) -> Self {
        Self {
            net: net.to_string(),
            spec,
            sources: Vec::new(),
            loads: Vec::new(),
            unread: Vec::new(),
        }
    }

    /// What the loads draw together, in A.
    pub fn total_a(&self) -> f64 {
        self.loads.iter().map(|load| load.current_a).sum()
    }

    /// What the rail's one source can supply; `None` without exactly one.
    pub fn supply_a(&self) -> Option<f64> {
        match self.sources.as_slice() {
            [source] => Some(source.current_a),
            _ => None,
        }
    }

    /// Why the rail's budget cannot be told, one sentence each: no voltage,
    /// no source or more than one, no load, a parameter that is no current.
    pub fn missing_data(&self) -> Vec<String> {
        let net = &self.net;
        let mut out = Vec::new();
        if self.spec.is_none() {
            out.push(format!("Rail {net} has no voltage in the project's rails"));
        }
        match self.sources.len() {
            0 => out.push(format!(
                "No fitted part names a `{SUPPLY_PARAMETER}{net}` current, so rail {net} has no source"
            )),
            1 => {}
            many => out.push(format!(
                "Rail {net} has {many} sources ({}); a rail budget needs exactly one",
                designators(&self.sources)
            )),
        }
        if self.loads.is_empty() {
            out.push(format!(
                "No fitted part names a `{LOAD_PARAMETER}{net}` current, so rail {net} has no load"
            ));
        }
        for unread in &self.unread {
            out.push(format!(
                "{}'s `{}` is `{}`, not a current in A",
                unread.designator, unread.parameter, unread.text
            ));
        }
        out
    }

    /// The rail as the query answers it.
    pub fn to_json(&self) -> Value {
        let total = self.total_a();
        let supply = self.supply_a();
        json!({
            "net": self.net,
            "voltage": self.spec.map(|spec| spec.voltage),
            "tolerance_percent": self.spec.and_then(|spec| spec.tolerance_percent),
            "source": self.sources.first().filter(|_| self.sources.len() == 1).map(part_json),
            "sources": self.sources.iter().map(part_json).collect::<Vec<_>>(),
            "loads": self.loads.iter().map(part_json).collect::<Vec<_>>(),
            "total_a": total,
            "supply_a": supply,
            "headroom_a": supply.map(|supply| supply - total),
            "problems": self.missing_data(),
        })
    }
}

fn part_json(part: &RailPart) -> Value {
    json!({"designator": part.designator, "current_a": part.current_a})
}

fn designators(parts: &[RailPart]) -> String {
    let names: Vec<&str> = parts.iter().map(|part| part.designator.as_str()).collect();
    names.join(", ")
}

/// Every rail, by name: those the project lists and those a fitted part's
/// parameter names. `parts` is the ungrouped BOM (one line per part).
pub fn rail_budgets(rails: &BTreeMap<String, RailSpec>, parts: &BomTable) -> Vec<RailBudget> {
    let mut found: BTreeMap<String, RailBudget> = rails
        .iter()
        .map(|(net, spec)| (net.clone(), RailBudget::new(net, Some(*spec))))
        .collect();
    for row in parts.rows.iter().filter(|row| row.fitted_qty > 0) {
        let designator = row.references.join(",");
        for (parameter, text) in &row.custom {
            let Some((net, supply)) = rail_parameter(parameter) else {
                continue;
            };
            let budget = found
                .entry(net.to_string())
                .or_insert_with(|| RailBudget::new(net, None));
            let Some(current) = parse_current_a(text) else {
                budget.unread.push(UnreadCurrent {
                    designator: designator.clone(),
                    parameter: parameter.clone(),
                    text: text.clone(),
                });
                continue;
            };
            let part = RailPart {
                designator: designator.clone(),
                current_a: current * f64::from(row.fitted_qty),
            };
            if supply {
                budget.sources.push(part);
            } else {
                budget.loads.push(part);
            }
        }
    }
    found.into_values().collect()
}

/// The rail a parameter names and whether it is a supply; `None` for any
/// other parameter.
fn rail_parameter(parameter: &str) -> Option<(&str, bool)> {
    let (net, supply) = if let Some(net) = parameter.strip_prefix(SUPPLY_PARAMETER) {
        (net, true)
    } else {
        (parameter.strip_prefix(LOAD_PARAMETER)?, false)
    };
    let net = net.trim();
    (!net.is_empty()).then_some((net, supply))
}

/// A current in A from a parameter's text: a number, optionally followed by
/// `A`, `mA`, `uA` or `µA`; finite and not negative.
pub fn parse_current_a(text: &str) -> Option<f64> {
    let text = text.trim();
    let (number, per_amp) = [("mA", 1e3), ("uA", 1e6), ("µA", 1e6), ("A", 1.0)]
        .iter()
        .find_map(|&(unit, per_amp)| text.strip_suffix(unit).map(|number| (number, per_amp)))
        .unwrap_or((text, 1.0));
    let value: f64 = number.trim().parse().ok()?;
    let current = value / per_amp;
    (current.is_finite() && current >= 0.0).then_some(current)
}

/// What a `rail_budget` requirement asks.
pub(crate) struct RailWanted<'a> {
    pub(crate) net: &'a str,
    pub(crate) voltage: Option<f64>,
    pub(crate) tolerance_percent: Option<f64>,
    pub(crate) max_current_a: Option<f64>,
    pub(crate) margin_percent: Option<f64>,
}

impl RailWanted<'_> {
    /// Why the requirement cannot be checked as written.
    fn problems(&self) -> Vec<Miss> {
        let mut out = Vec::new();
        let not_checked =
            |what: String| Miss::new(format!("{what}, so the requirement was not checked"));
        if self.net.trim().is_empty() {
            out.push(not_checked("`net` is empty".into()));
        }
        if self.voltage.is_some_and(|v| !v.is_finite() || v == 0.0) {
            out.push(not_checked("`voltage` is no voltage".into()));
        }
        if self.tolerance_percent.is_some() && self.voltage.is_none() {
            out.push(not_checked(
                "`tolerance_percent` is set without a `voltage`".into(),
            ));
        }
        for (name, value) in [
            ("tolerance_percent", self.tolerance_percent),
            ("margin_percent", self.margin_percent),
        ] {
            if value.is_some_and(|v| !is_percent(v)) {
                out.push(not_checked(format!("`{name}` is not from 0 to below 100")));
            }
        }
        if self
            .max_current_a
            .is_some_and(|a| !a.is_finite() || a <= 0.0)
        {
            out.push(not_checked("`max_current_a` is no current above 0".into()));
        }
        out
    }
}

/// `rail_budget`. `rails` is the project's rails or why they cannot be
/// read; `parts` the ungrouped BOM.
pub(crate) fn rail_budget(
    rails: &Result<BTreeMap<String, RailSpec>, String>,
    parts: Option<&Result<BomTable, String>>,
    wanted: &RailWanted,
) -> Vec<Miss> {
    let problems = wanted.problems();
    if !problems.is_empty() {
        return problems;
    }
    let rails = match rails {
        Ok(rails) => rails,
        Err(why) => {
            let miss = Miss::new(format!("The project's rails could not be read: {why}"));
            return vec![miss.with("reason", json!(why))];
        }
    };
    let table = match parts {
        None => return vec![Miss::new("The parts were not prepared")],
        Some(Err(why)) => {
            let miss = Miss::new(format!("The parts could not be listed: {why}"));
            return vec![miss.with("reason", json!(why))];
        }
        Some(Ok(table)) => table,
    };
    let matched: Vec<RailBudget> = rail_budgets(rails, table)
        .into_iter()
        .filter(|budget| glob_matches(wanted.net, &budget.net))
        .collect();
    if matched.is_empty() {
        let miss = Miss::new(format!("No rail matches {}", wanted.net));
        return vec![miss.with("net", json!(wanted.net))];
    }
    matched
        .iter()
        .flat_map(|budget| rail_misses(budget, wanted))
        .collect()
}

/// What one rail misses of `wanted`.
fn rail_misses(budget: &RailBudget, wanted: &RailWanted) -> Vec<Miss> {
    let net = budget.net.as_str();
    let with_rail = |miss: Miss, about: &str| {
        miss.with("rail", rail_evidence(budget))
            .about(format!("rail:{net}:{about}"))
    };
    let mut misses: Vec<Miss> = budget
        .missing_data()
        .into_iter()
        .map(|message| {
            let about = format!("data:{message}");
            with_rail(Miss::new(message), &about)
        })
        .collect();
    if let (Some(want), Some(spec)) = (wanted.voltage, budget.spec)
        && let Some(message) = voltage_miss(net, &spec, want, wanted.tolerance_percent)
    {
        let miss = Miss::new(message)
            .with("voltage", json!(spec.voltage))
            .with("tolerance_percent", json!(spec.tolerance_percent))
            .with("wanted_voltage", json!(want))
            .with("wanted_tolerance_percent", json!(wanted.tolerance_percent));
        misses.push(with_rail(miss, "voltage"));
    }
    let Some(supply) = budget.supply_a() else {
        return misses;
    };
    let margin = wanted.margin_percent.unwrap_or(0.0);
    let mut limit = supply * (1.0 - margin / 100.0);
    if let Some(cap) = wanted.max_current_a {
        limit = limit.min(cap);
    }
    let total = budget.total_a();
    if total <= limit + SAME_A {
        return misses;
    }
    let miss = Miss::new(format!(
        "Rail {net} draws {} A, over its limit of {} A",
        amps(total),
        amps(limit)
    ))
    .with("total_a", json!(total))
    .with("limit_a", json!(limit))
    .with("supply_a", json!(supply))
    .with("margin_percent", json!(wanted.margin_percent))
    .with("max_current_a", json!(wanted.max_current_a));
    misses.push(with_rail(miss, "load"));
    misses
}

/// Why the rail's voltage does not match: the nominal against `want`, or
/// the rail's band against `want` ± `tolerance` percent.
fn voltage_miss(net: &str, spec: &RailSpec, want: f64, tolerance: Option<f64>) -> Option<String> {
    let Some(tolerance) = tolerance else {
        return ((spec.voltage - want).abs() > SAME_V)
            .then(|| format!("Rail {net} is {} V, not {} V", spec.voltage, want));
    };
    let spread = want.abs() * tolerance / 100.0;
    let (low, high) = spec.band();
    let inside = low >= want - spread - SAME_V && high <= want + spread + SAME_V;
    (!inside)
        .then(|| format!("Rail {net} spans {low} V to {high} V, outside {want} V ± {tolerance} %"))
}

/// A rail as a finding's evidence: the rail, its source, each load with its
/// current, the total and what the source supplies.
fn rail_evidence(budget: &RailBudget) -> Value {
    let mut rail = budget.to_json();
    if let Value::Object(map) = &mut rail {
        map.remove("problems");
    }
    rail
}

/// `value` in A as a person reads it: up to four decimals, no trailing zeros.
fn amps(value: f64) -> String {
    let text = format!("{value:.4}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

#[cfg(test)]
mod tests;
