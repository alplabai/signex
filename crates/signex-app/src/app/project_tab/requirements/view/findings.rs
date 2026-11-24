//! A failed card's findings as one sentence (mockup row 38): a
//! `parts_state` run counts the parts outside the allowed states and says
//! what each line is; any other requirement's messages are joined.

use signex_i18n::{I18n, ListKind, ids, t};
use signex_req::{PartMiss, PartProblem, parts_shortfall};
use signex_types::check::Finding;

/// The findings a run filed under one requirement as one sentence. `None`
/// for no findings.
pub(super) fn findings_sentence(findings: &[&Finding], i18n: &I18n) -> Option<String> {
    if findings.is_empty() {
        return None;
    }
    if let Some(shortfall) = parts_shortfall(findings) {
        let lines: Vec<String> = shortfall
            .lines
            .iter()
            .map(|line| part_line(line, i18n))
            .collect();
        let sentence = t!(
            i18n,
            ids::REQUIREMENTS_PARTS_UNMET,
            "missed" => shortfall.missed,
            "of" => shortfall.of,
            "allowed" => i18n.list(shortfall.allowed.as_slice(), ListKind::Or),
            "lines" => i18n.list(lines.as_slice(), ListKind::And),
        );
        return Some(sentence.into_string());
    }
    let mut joined: Option<String> = None;
    for finding in findings {
        let clause = finding.message.trim().trim_end_matches('.');
        joined = Some(match joined {
            None => clause.to_string(),
            Some(head) => t!(
                i18n,
                ids::REQUIREMENTS_FINDINGS_NEXT,
                "head" => head,
                "next" => mid_sentence(clause, i18n),
            )
            .into_string(),
        });
    }
    joined
        .map(|findings| t!(i18n, ids::REQUIREMENTS_FINDINGS, "findings" => findings).into_string())
}

/// What is wrong with one BOM line's parts, as a clause.
fn part_line(line: &PartMiss, i18n: &I18n) -> String {
    let parts = line.designator.as_str();
    let words = match &line.problem {
        PartProblem::Unplaced => t!(
            i18n,
            ids::REQUIREMENTS_PART_UNPLACED,
            "parts" => parts,
            "count" => line.count,
        ),
        PartProblem::NoLibrary => t!(
            i18n,
            ids::REQUIREMENTS_PART_NO_LIBRARY,
            "parts" => parts,
            "count" => line.count,
        ),
        PartProblem::State(state) => t!(
            i18n,
            ids::REQUIREMENTS_PART_STATE,
            "parts" => parts,
            "count" => line.count,
            "state" => state.as_str(),
        ),
    };
    words.into_string()
}

/// `clause` as it reads after a semicolon: an opening word in sentence case
/// (`The`, `Net`) goes lower case; a name (`R2`, `DRC`) stays as written.
fn mid_sentence(clause: &str, i18n: &I18n) -> String {
    let mut chars = clause.chars();
    match (chars.next(), chars.next()) {
        (Some(first), Some(second)) if first.is_uppercase() && second.is_lowercase() => {
            let rest = &clause[first.len_utf8()..];
            let mut out = i18n.lower_for_display(&first.to_string());
            out.push_str(rest);
            out
        }
        _ => clause.to_string(),
    }
}
