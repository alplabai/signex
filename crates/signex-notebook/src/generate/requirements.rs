//! Requirements and how each was met (mockup row 201, table 1): the
//! statement, the parts and blocks that meet it, the evidence and the
//! result of the requirement checks.
//!
//! A block meets a requirement when it names the requirement's id in its
//! title or text. Measurements that name it are its evidence; with none, the
//! calculations that name it are. The parts those blocks reference, and the
//! other calculations and decisions that name it, are what meets it.

use signex_types::check::CheckReport;
use signex_types::project::{RequirementEntry, RequirementKind};

use super::blocks::{block_fields, cites, label};
use crate::annotation::{Annotation, AnnotationKind};

/// Where one requirement stands after the requirement checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequirementResult {
    Pass,
    Fail,
    /// No check report, or the requirement is turned off.
    NotChecked,
}

impl RequirementResult {
    /// The word the table prints: `Pass`, `Fail`, `Not checked`.
    pub fn word(self) -> &'static str {
        match self {
            RequirementResult::Pass => "Pass",
            RequirementResult::Fail => "Fail",
            RequirementResult::NotChecked => "Not checked",
        }
    }
}

/// One row of the requirements table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequirementRow {
    /// The requirement's id; an entry with none is named by its position
    /// (`#3`).
    pub id: String,
    pub statement: String,
    /// Parts, then calculation and decision ids, first mention first.
    pub met_by: Vec<String>,
    /// Block ids, each with its reading when it records one
    /// (`MEAS-007 3.310 V`).
    pub evidence: Vec<String>,
    pub result: RequirementResult,
}

/// The rows of `entries`, in list order, with each one's result in `report`
/// (`None`: no requirement run was read) and its blocks among `blocks`.
pub fn requirement_rows(
    entries: &[RequirementEntry],
    report: Option<&CheckReport>,
    blocks: &[Annotation],
) -> Vec<RequirementRow> {
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let id = entry
                .id()
                .map(str::trim)
                .filter(|id| !id.is_empty())
                .map_or_else(|| format!("#{}", index + 1), str::to_string);
            let statement = entry.requirement().map_or_else(
                || "Could not be read from the project file".to_string(),
                |r| statement(&r.kind),
            );
            let citing: Vec<&Annotation> = blocks.iter().filter(|b| cites(b, &id)).collect();
            let (met_by, evidence) = met_by_and_evidence(&citing);
            RequirementRow {
                result: result_of(&id, entry, report),
                id,
                statement,
                met_by,
                evidence,
            }
        })
        .collect()
}

/// The result of the requirement `id` in `report`: a finding under its rule
/// fails it, its rule in `passed` passes it.
fn result_of(
    id: &str,
    entry: &RequirementEntry,
    report: Option<&CheckReport>,
) -> RequirementResult {
    let Some(report) = report else {
        return RequirementResult::NotChecked;
    };
    let rule = format!("req::{id}");
    if report.findings.iter().any(|f| f.rule == rule) {
        RequirementResult::Fail
    } else if report.passed.contains(&rule) {
        RequirementResult::Pass
    } else if entry.requirement().is_none() {
        // An entry that cannot be read is never checked, and the check
        // files a finding for it under a rule of its own.
        RequirementResult::Fail
    } else {
        RequirementResult::NotChecked
    }
}

fn met_by_and_evidence(citing: &[&Annotation]) -> (Vec<String>, Vec<String>) {
    let of_kind = |kind| citing.iter().filter(move |b| b.kind == kind);
    let measured: Vec<&&Annotation> = of_kind(AnnotationKind::Measurement).collect();
    let evidence_kind = if measured.is_empty() {
        AnnotationKind::Calculation
    } else {
        AnnotationKind::Measurement
    };
    let evidence: Vec<String> = of_kind(evidence_kind)
        .map(|block| match block_fields(block).reading() {
            Some(reading) => format!("{} {reading}", label(block)),
            None => label(block),
        })
        .collect();
    let mut met_by: Vec<String> = Vec::new();
    let mut push = |name: String| {
        if !met_by.contains(&name) {
            met_by.push(name);
        }
    };
    for block in citing {
        for part in block.references.iter().filter_map(|r| r.component()) {
            push(part.to_string());
        }
    }
    for block in citing {
        let meets = matches!(
            block.kind,
            AnnotationKind::Calculation | AnnotationKind::Decision
        );
        if meets && block.kind != evidence_kind {
            push(label(block));
        }
    }
    (met_by, evidence)
}

/// What a requirement asks, in a line (`ERC has no errors and no warnings`).
pub fn statement(kind: &RequirementKind) -> String {
    let clean = |check: &str, allow_warnings: bool| {
        if allow_warnings {
            format!("{check} has no errors")
        } else {
            format!("{check} has no errors and no warnings")
        }
    };
    match kind {
        RequirementKind::ErcClean { allow_warnings } => clean("ERC", *allow_warnings),
        RequirementKind::DrcClean { allow_warnings } => clean("DRC", *allow_warnings),
        RequirementKind::BoardSize {
            max_width_mm,
            max_height_mm,
        } => format!("Board at most {max_width_mm} × {max_height_mm} mm"),
        RequirementKind::PartsState { states } => {
            format!("Every bought part is {}", states.join(" or "))
        }
        RequirementKind::NetClass {
            nets,
            class,
            min_trace_width_mm,
            min_clearance_mm,
        } => {
            let mut line = format!("Nets {nets}");
            if let Some(class) = class {
                line.push_str(&format!(" in class {class}"));
            }
            if let Some(width) = min_trace_width_mm {
                line.push_str(&format!(", track at least {width} mm"));
            }
            if let Some(clearance) = min_clearance_mm {
                line.push_str(&format!(", clearance at least {clearance} mm"));
            }
            line
        }
        RequirementKind::BomCost {
            quantity,
            currency,
            max_unit_cost,
        } => format!("BOM at most {max_unit_cost} {currency} per board at {quantity} boards"),
        // #2943: the rail budget and the copper layer count.
        RequirementKind::RailBudget {
            net,
            voltage,
            tolerance_percent,
            max_current_a,
            margin_percent,
        } => {
            let mut line = format!("Rail {net} covers its load");
            if let Some(voltage) = voltage {
                line.push_str(&format!(", at {voltage} V"));
                if let Some(tolerance) = tolerance_percent {
                    line.push_str(&format!(" ± {tolerance} %"));
                }
            }
            if let Some(current) = max_current_a {
                line.push_str(&format!(", load at most {current} A"));
            }
            if let Some(margin) = margin_percent {
                line.push_str(&format!(", with {margin} % margin"));
            }
            line
        }
        RequirementKind::LayerCount {
            min_layers,
            max_layers,
        } => match (min_layers, max_layers) {
            (Some(min), Some(max)) => format!("Board has {min} to {max} copper layers"),
            (Some(min), None) => format!("Board has at least {min} copper layers"),
            (None, Some(max)) => format!("Board has at most {max} copper layers"),
            (None, None) => "Board copper layers".to_string(),
        },
    }
}
