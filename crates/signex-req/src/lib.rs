//! The project requirement checks (#2339): a project's requirements list
//! (`signex_types::project::Requirement`) evaluated as deterministic
//! checks, each passing or failing with evidence, in the shared check shape
//! (`signex_types::check`). Pure: the app prepares the facts (the ERC and
//! DRC reports, the board, the BOM, the library rows' lifecycle states, the
//! cached prices) and this crate decides. A requirement that cannot be
//! evaluated is a finding, never a pass. Kinds, fields and the report:
//! `docs/CLI.md`, "Requirement checks".

mod board;
mod bom;
mod parts;
mod rails;
mod runs;

pub use parts::{LIFECYCLE_STATES, PartMiss, PartProblem, PartsShortfall, parts_shortfall};
pub use rails::{
    LOAD_PARAMETER, RailBudget, RailPart, SUPPLY_PARAMETER, UnreadCurrent, parse_current_a,
    rail_budgets,
};

use std::collections::{BTreeMap, HashMap, HashSet};

use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use signex_bom::{BomTable, PriceBook};
use signex_types::check::{CheckReport, Finding, Severity};
use signex_types::net_class::NetClass;
use signex_types::pcb::PcbBoard;
use signex_types::project::{RailSpec, Requirement, RequirementEntry, RequirementKind};

/// The id a report names its check by.
pub const REQUIREMENTS_CHECK: &str = "requirements";

/// How many hex digits of the SHA-256 a finding key keeps (as ERC and DRC).
const KEY_HEX_DIGITS: usize = 32;

/// What the check needs the app to prepare, by the requirements it runs.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Needs {
    pub erc: bool,
    pub drc: bool,
    /// The board itself (its outline, its nets).
    pub board: bool,
    /// The parts the BOM buys, one row each.
    pub parts: bool,
    /// The grouped BOM and the cached prices.
    pub bom: bool,
}

impl Needs {
    /// What `entries` need; an unreadable or turned-off entry needs nothing.
    pub fn of(entries: &[RequirementEntry]) -> Self {
        let mut needs = Self::default();
        let enabled = entries
            .iter()
            .filter_map(RequirementEntry::requirement)
            .filter(|requirement| requirement.enabled);
        for requirement in enabled {
            match requirement.kind {
                RequirementKind::ErcClean { .. } => needs.erc = true,
                RequirementKind::DrcClean { .. } => needs.drc = true,
                RequirementKind::BoardSize { .. }
                | RequirementKind::NetClass { .. }
                | RequirementKind::LayerCount { .. } => needs.board = true,
                // A rail's loads and sources are parameters of the parts.
                RequirementKind::PartsState { .. } | RequirementKind::RailBudget { .. } => {
                    needs.parts = true
                }
                RequirementKind::BomCost { .. } => needs.bom = true,
            }
        }
        needs
    }
}

/// The project's board, as the app found it.
#[derive(Debug, Clone)]
pub enum Board {
    /// The project lists no board.
    NotListed,
    Read {
        path: String,
        board: Box<PcbBoard>,
    },
    Unreadable {
        path: String,
        why: String,
    },
}

/// Everything a run checks. A fact is `None` when the app did not prepare it
/// (no requirement needed it).
#[derive(Debug, Clone)]
pub struct Facts {
    /// The project file, as the report names it: the `sheet` of a finding
    /// about the project and part of every key.
    pub project: String,
    pub requirements: Vec<RequirementEntry>,
    /// The stored `requirements` value is not a list.
    pub unreadable_list: bool,
    pub erc: Option<Result<CheckReport, String>>,
    /// The DRC run of the project's board; `None` also when it lists none.
    pub drc: Option<Result<CheckReport, String>>,
    pub board: Board,
    pub classes: Vec<NetClass>,
    /// The parts the BOM buys, one row per part (an ungrouped BOM).
    pub parts: Option<Result<BomTable, String>>,
    /// Lifecycle state name (`released`) by library row key
    /// (`signex_bom::library_row_key`), for the rows of the open libraries.
    pub row_states: HashMap<String, String>,
    /// The grouped BOM, unpriced.
    pub bom: Option<Result<BomTable, String>>,
    pub prices: PriceBook,
    /// The project's rails by net name (#2943), or why they cannot be read.
    pub rails: Result<BTreeMap<String, RailSpec>, String>,
    /// What the app could not prepare completely, said in the report.
    pub warnings: Vec<String>,
}

/// One way a requirement is not met, before it is made a finding.
#[derive(Debug, Clone)]
pub(crate) struct Miss {
    pub(crate) message: String,
    pub(crate) evidence: Map<String, Value>,
    /// The document the finding is filed under; `None`: the project file.
    pub(crate) sheet: Option<String>,
    /// What tells this miss apart from the requirement's others (a part's
    /// designator, a net); part of the key.
    pub(crate) subject: String,
}

impl Miss {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            evidence: Map::new(),
            sheet: None,
            subject: String::new(),
        }
    }

    pub(crate) fn with(mut self, key: &str, value: Value) -> Self {
        self.evidence.insert(key.to_string(), value);
        self
    }

    pub(crate) fn on(mut self, sheet: &str) -> Self {
        self.sheet = Some(sheet.to_string());
        self
    }

    pub(crate) fn about(mut self, subject: impl Into<String>) -> Self {
        self.subject = subject.into();
        self
    }
}

/// Run every requirement of `facts` and say what it found: a finding per
/// way a requirement is not met (rule `req::<id>`, the requirement's own
/// severity), the requirements met in `passed`, in list order.
pub fn check_report(facts: &Facts) -> CheckReport {
    let mut findings = Vec::new();
    let mut passed = Vec::new();
    let mut seen: HashSet<&str> = HashSet::new();
    let mut turned_off = Vec::new();
    if facts.unreadable_list {
        let miss =
            Miss::new("The project's requirements are not a list, so none of them was checked");
        findings.push(finding(
            facts,
            "req::requirements",
            Severity::Error,
            "",
            miss,
        ));
    }
    for (index, entry) in facts.requirements.iter().enumerate() {
        let label = entry_label(entry, index);
        let rule = entry_rule(entry, index);
        let severity = entry
            .requirement()
            .map_or(Severity::Error, |requirement| requirement.severity);
        // Readable or not, an entry's id is taken; a later entry with the
        // same id is not checked and its rule never counts as passed.
        let id = entry.id().map(str::trim).filter(|id| !id.is_empty());
        if let Some(id) = id
            && !seen.insert(id)
        {
            let miss = Miss::new(format!(
                "Requirement {id} uses an id an earlier requirement has, so it was not checked"
            ))
            .with("index", json!(index));
            let subject = format!("duplicate {index}");
            findings.push(finding(facts, &rule, severity, &subject, miss));
            passed.retain(|done| done != &rule);
            continue;
        }
        let requirement = match entry {
            RequirementEntry::Unreadable { reason, .. } => {
                let miss = Miss::new(format!(
                    "Requirement {label} could not be read ({reason}), so it was not checked"
                ))
                .with("reason", json!(reason))
                .with("index", json!(index));
                let subject = format!("unreadable {index}");
                findings.push(finding(facts, &rule, Severity::Error, &subject, miss));
                continue;
            }
            RequirementEntry::Known { requirement, .. } => requirement,
        };
        if id.is_none() {
            let miss = Miss::new(format!(
                "Requirement {label} has no id, so it was not checked"
            ));
            findings.push(finding(facts, &rule, severity, "no id", miss));
            continue;
        }
        if !requirement.enabled {
            turned_off.push(format!(
                "Requirement {label} is turned off, so it was not checked"
            ));
            continue;
        }
        let misses = evaluate(requirement, facts);
        if misses.is_empty() {
            passed.push(rule);
            continue;
        }
        for miss in misses {
            let miss = miss.with("kind", json!(requirement.kind.name()));
            let subject = miss.subject.clone();
            findings.push(finding(facts, &rule, requirement.severity, &subject, miss));
        }
    }
    let mut warnings = facts.warnings.clone();
    if facts.requirements.is_empty() && !facts.unreadable_list {
        warnings.push("The project lists no requirements".to_string());
    }
    warnings.extend(turned_off);
    warnings.extend(run_warnings("ERC", facts.erc.as_ref()));
    warnings.extend(run_warnings("DRC", facts.drc.as_ref()));
    CheckReport::new(
        REQUIREMENTS_CHECK,
        findings,
        Vec::new(),
        Vec::new(),
        warnings,
    )
    .with_passed(passed)
    .with_rule_set(rule_set(facts))
}

/// The width and height in mm of the board outline's bounding box, as
/// `board_size` measures it; `None` without a board read or an outline to
/// measure.
pub fn board_extent_mm(board: &Board) -> Option<(f64, f64)> {
    match board {
        Board::Read { board, .. } => board::outline_extent(board),
        Board::NotListed | Board::Unreadable { .. } => None,
    }
}

/// The `query_rail_budget` answer's rails (#2943): every rail the project
/// lists or a fitted part names, by name, each with its voltage, source,
/// loads, total and headroom, and what keeps its budget from being told.
pub fn rail_budget_answer(rails: &BTreeMap<String, RailSpec>, parts: &BomTable) -> Value {
    let budgets = rail_budgets(rails, parts);
    let listed: Vec<Value> = budgets.iter().map(RailBudget::to_json).collect();
    json!({"count": listed.len(), "rails": listed})
}

/// What `requirement` finds unmet.
fn evaluate(requirement: &Requirement, facts: &Facts) -> Vec<Miss> {
    match &requirement.kind {
        RequirementKind::ErcClean { allow_warnings } => {
            runs::erc_clean(facts.erc.as_ref(), *allow_warnings)
        }
        RequirementKind::DrcClean { allow_warnings } => {
            runs::drc_clean(&facts.board, facts.drc.as_ref(), *allow_warnings)
        }
        RequirementKind::BoardSize {
            max_width_mm,
            max_height_mm,
        } => board::board_size(&facts.board, *max_width_mm, *max_height_mm),
        RequirementKind::PartsState { states } => {
            parts::parts_state(facts.parts.as_ref(), &facts.row_states, states)
        }
        RequirementKind::NetClass {
            nets,
            class,
            min_trace_width_mm,
            min_clearance_mm,
        } => board::net_class(
            &facts.board,
            &facts.classes,
            &board::NetClassWanted {
                nets,
                class: class.as_deref(),
                min_trace_width_mm: *min_trace_width_mm,
                min_clearance_mm: *min_clearance_mm,
            },
        ),
        RequirementKind::BomCost {
            quantity,
            currency,
            max_unit_cost,
        } => bom::bom_cost(
            facts.bom.as_ref(),
            &facts.prices,
            &bom::Budget {
                quantity: *quantity,
                currency,
                max_unit_cost: *max_unit_cost,
            },
        ),
        RequirementKind::RailBudget {
            net,
            voltage,
            tolerance_percent,
            max_current_a,
            margin_percent,
        } => rails::rail_budget(
            &facts.rails,
            facts.parts.as_ref(),
            &rails::RailWanted {
                net,
                voltage: *voltage,
                tolerance_percent: *tolerance_percent,
                max_current_a: *max_current_a,
                margin_percent: *margin_percent,
            },
        ),
        RequirementKind::LayerCount {
            min_layers,
            max_layers,
        } => board::layer_count(&facts.board, *min_layers, *max_layers),
    }
}

/// The rule a report files entry `index` under: `req::<id>`, or
/// `req::#<place>` for an entry with no id. Readers of a report look an
/// entry's verdict up by this name.
pub fn entry_rule(entry: &RequirementEntry, index: usize) -> String {
    format!("req::{}", entry_label(entry, index))
}

/// How a finding names an entry: its id, else its place in the list (`#3`).
fn entry_label(entry: &RequirementEntry, index: usize) -> String {
    entry
        .id()
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map_or_else(|| format!("#{}", index + 1), str::to_string)
}

/// `miss` as a finding of `rule`.
fn finding(facts: &Facts, rule: &str, severity: Severity, subject: &str, miss: Miss) -> Finding {
    let sheet = miss.sheet.clone().unwrap_or_else(|| facts.project.clone());
    Finding {
        rule: rule.to_string(),
        severity,
        key: key(&facts.project, rule, subject),
        message: miss.message,
        evidence: Value::Object(miss.evidence),
        items: Vec::new(),
        sheet,
    }
}

/// The project file, the rule and the subject: the same finding keeps its
/// key whatever its measured values are.
fn key(project: &str, rule: &str, subject: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(project.as_bytes());
    hasher.update(b"\n");
    hasher.update(rule.as_bytes());
    hasher.update(b"\nsubject:");
    hasher.update(subject.as_bytes());
    let mut hex = format!("{:x}", hasher.finalize());
    hex.truncate(KEY_HEX_DIGITS);
    hex
}

/// What a check run this one used said it could not do, named by the run.
fn run_warnings(name: &str, run: Option<&Result<CheckReport, String>>) -> Vec<String> {
    match run {
        Some(Ok(report)) => report
            .warnings
            .iter()
            .map(|warning| format!("{name}: {warning}"))
            .collect(),
        _ => Vec::new(),
    }
}

/// Where the requirements came from: the project, how many, and the hash of
/// the list as stored (each entry's value as read, keys sorted), so two runs
/// over different lists can be told apart.
fn rule_set(facts: &Facts) -> Value {
    let text = serde_json::to_string(&facts.requirements).unwrap_or_default();
    let digest = format!("{:x}", Sha256::digest(text.as_bytes()));
    json!({
        "source": "project",
        "requirements": facts.requirements.len(),
        "sha256": digest,
    })
}

/// `value` in mm as a person reads it: up to three decimals, no trailing
/// zeros.
pub(crate) fn mm(value: f64) -> String {
    let text = format!("{value:.3}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

#[cfg(test)]
mod tests;
