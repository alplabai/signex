//! Presets (#2398): rule text applied in one step after a row-by-row diff
//! (added, changed and removed rules). A generic manufacturing tier
//! (`FabTemplate`) merges its rules into the project's text; a rule profile
//! (`.snxrules`) replaces it, as "Update from profile" does. See
//! `docs/DRC_RULES.md`, "Presets".

use std::collections::BTreeSet;

use signex_types::project::FabTemplate;

use super::edit::{EditError, splice};
use super::table;

/// How a preset's text meets the project's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresetMode {
    /// Its rules replace the project's rules of the same name and are added
    /// otherwise; the rest stay.
    Merge,
    /// Its text replaces the project's.
    Replace,
}

/// The stable id of a generic tier (`standard`, `advanced`, `hdi`).
pub const fn tier_id(tier: FabTemplate) -> &'static str {
    match tier {
        FabTemplate::Standard => "standard",
        FabTemplate::Advanced => "advanced",
        FabTemplate::Hdi => "hdi",
    }
}

pub fn tier_from_id(id: &str) -> Option<FabTemplate> {
    FabTemplate::ALL.into_iter().find(|t| tier_id(*t) == id)
}

/// A tier's minimums as rule text: one board-wide rule per base rule a
/// maker limits, named `fab_*` (the fab floor, #2311). Silkscreen and
/// mask slivers are warnings; the rest are errors.
pub fn tier_text(tier: FabTemplate) -> String {
    let c = tier.capabilities();
    format!(
        "rule fab_clearance on clearance require clearance >= {} then error\n\
         rule fab_track on track require width >= {} then error\n\
         rule fab_via on via require diameter >= {} and drill >= {} then error\n\
         rule fab_hole on hole require diameter >= {} and annular >= {} and spacing >= {} \
         then error\n\
         rule fab_edge on edge require clearance >= {} then error\n\
         rule fab_mask on mask require sliver >= {} then warning\n\
         rule fab_silk on silk require pad_clearance >= {} then warning\n",
        mm(c.clearance),
        mm(c.track_width),
        mm(c.via_diameter),
        mm(c.via_drill),
        mm(c.hole),
        mm(c.annular_ring),
        mm(c.hole_to_hole),
        mm(c.edge_clearance),
        mm(c.mask_sliver),
        mm(c.silk_to_pad),
    )
}

/// `0.15mm`: millimetres without float noise or trailing zeros.
fn mm(value: f64) -> String {
    let text = format!("{value:.4}");
    let trimmed = text.trim_end_matches('0').trim_end_matches('.');
    format!("{trimmed}mm")
}

/// `current` after `preset` meets it by `mode`. Fails when either text
/// has a syntax error (no rule can be matched by name).
pub fn preset_result(current: &str, preset: &str, mode: PresetMode) -> Result<String, EditError> {
    let incoming = table(preset);
    if incoming.is_unparsed() {
        return Err(EditError::Invalid(
            incoming.syntax.iter().map(|d| d.render(preset)).collect(),
        ));
    }
    if mode == PresetMode::Replace {
        return Ok(preset.to_string());
    }
    if table(current).is_unparsed() {
        return Err(EditError::Unparsed);
    }
    let mut text = current.to_string();
    for row in &incoming.rows {
        let rule = row.text(preset);
        let rows = table(&text);
        match rows.row(&row.name) {
            Some(held) => text = splice(&text, &held.span, rule),
            None => {
                if !text.is_empty() && !text.ends_with('\n') {
                    text.push('\n');
                }
                text.push_str(rule);
                text.push('\n');
            }
        }
    }
    Ok(text)
}

/// What a preset does to one rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowChangeKind {
    Added,
    Changed,
    Removed,
}

impl RowChangeKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            RowChangeKind::Added => "added",
            RowChangeKind::Changed => "changed",
            RowChangeKind::Removed => "removed",
        }
    }
}

/// One rule a preset adds, changes or removes, with its text before and
/// after.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowChange {
    pub name: String,
    pub kind: RowChangeKind,
    pub from: Option<String>,
    pub to: Option<String>,
}

/// The rules that differ between `before` and `after`, by name: removed
/// ones in `before`'s order, then changed and added ones in `after`'s.
/// Rules equal but for spacing and line breaks are not changes.
pub fn diff_rows(before: &str, after: &str) -> Vec<RowChange> {
    let (old, new) = (table(before), table(after));
    let names: BTreeSet<&str> = new.rows.iter().map(|r| r.name.as_str()).collect();
    let mut out: Vec<RowChange> = old
        .rows
        .iter()
        .filter(|r| !names.contains(r.name.as_str()))
        .map(|r| RowChange {
            name: r.name.clone(),
            kind: RowChangeKind::Removed,
            from: Some(r.text(before).to_string()),
            to: None,
        })
        .collect();
    for row in &new.rows {
        let to = row.text(after).to_string();
        match old.row(&row.name) {
            None => out.push(RowChange {
                name: row.name.clone(),
                kind: RowChangeKind::Added,
                from: None,
                to: Some(to),
            }),
            Some(held) => {
                let from = held.text(before).to_string();
                if words(&from) != words(&to) {
                    out.push(RowChange {
                        name: row.name.clone(),
                        kind: RowChangeKind::Changed,
                        from: Some(from),
                        to: Some(to),
                    });
                }
            }
        }
    }
    out
}

fn words(text: &str) -> Vec<&str> {
    text.split_whitespace().collect()
}
