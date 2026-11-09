//! The change list as plain text (Report…). The same list always gives the
//! same bytes: no locale, no clock, no float formatting.

use std::fmt::Write as _;

use signex_types::pcb::PadNet;

use super::change::{ChangeReason, EcoBlock, EcoChange, EcoChangeList, EcoGroup, EcoNote};

/// The list as text: one section per group with a `[x]` / `[ ]` / `[-]`
/// (blocked) mark per row, then the notes.
pub fn report_text(list: &EcoChangeList) -> String {
    let mut out = String::new();
    let ticked = list.rows.iter().filter(|r| list.applies(r)).count();
    let _ = writeln!(out, "Update PCB from schematic");
    let _ = writeln!(out, "Board revision: {}", list.revision);
    let _ = writeln!(out, "Changes: {} of {} ticked", ticked, list.rows.len());
    let mut group: Option<EcoGroup> = None;
    for row in &list.rows {
        if group != Some(row.group) {
            group = Some(row.group);
            let _ = writeln!(out, "\n{}", row.group.label());
        }
        let mark = if row.blocked.is_some() {
            "[-]"
        } else if list.applies(row) {
            "[x]"
        } else {
            "[ ]"
        };
        let _ = write!(out, "  {mark} {}", describe(&row.change));
        if let Some(why) = &row.blocked {
            let _ = write!(out, " (blocked: {})", blocked_text(why));
        } else if row.on && !list.applies(row) {
            let _ = write!(out, " (skipped: a row it needs is not ticked)");
        }
        out.push('\n');
    }
    if !list.notes.is_empty() {
        let _ = writeln!(out, "\nNotes");
        for note in &list.notes {
            let _ = writeln!(out, "  - {}", note_text(note));
        }
    }
    out
}

fn net(n: &Option<PadNet>) -> &str {
    n.as_ref().map_or("(no net)", |n| n.name.as_str())
}

fn describe(change: &EcoChange) -> String {
    match change {
        EcoChange::AddFootprint { footprint } => {
            let source = footprint
                .library_ref
                .as_ref()
                .map_or_else(|| footprint.footprint_id.clone(), ToString::to_string);
            let mut text = format!("Add {} ({})", footprint.reference, footprint.value);
            if !source.is_empty() {
                text.push(' ');
                text.push_str(&source);
            }
            text
        }
        EcoChange::RemoveFootprint { reference, .. } => format!("Remove {reference}"),
        EcoChange::ChangeFootprint {
            reference,
            from,
            to,
            reason,
            ..
        } => {
            let why = match reason {
                ChangeReason::Footprint => "",
                ChangeReason::NewerVersion => " (newer library version)",
            };
            format!("Change {reference}: {from} -> {to}{why}")
        }
        EcoChange::SetDesignator { from, to, .. } => format!("Rename {from} -> {to}"),
        EcoChange::SetValue {
            reference,
            from,
            to,
            ..
        } => format!("Value of {reference}: {from} -> {to}"),
        EcoChange::Link { reference, .. } => format!("Link {reference} to its part"),
        EcoChange::AddNet { number, name } => format!("Add net {name} ({number})"),
        EcoChange::RemoveNet { number, name } => format!("Remove net {name} ({number})"),
        EcoChange::RenameNet { number, from, to } => {
            format!("Rename net {number}: {from} -> {to}")
        }
        EcoChange::PadNet {
            reference,
            pad_number,
            from,
            to,
            locked,
            ..
        } => format!(
            "Pad {reference}.{pad_number}: {} -> {}{}",
            net(from),
            net(to),
            if *locked { " (locked)" } else { "" }
        ),
    }
}

fn blocked_text(why: &EcoBlock) -> String {
    match why {
        EcoBlock::MissingFootprint(e) => format!("no footprint: {e}"),
        EcoBlock::NeedsAnnotation => "not annotated; run Annotate".into(),
        EcoBlock::DuplicateDesignator(d) => format!("designator {d} is used twice; run Annotate"),
        EcoBlock::PadsCannotCarry { number } => {
            format!("fewer pads numbered {number}; a net or lock would be lost")
        }
        EcoBlock::Unplaceable(e) => format!("cannot be placed: {e}"),
    }
}

fn note_text(note: &EcoNote) -> String {
    match note {
        EcoNote::UnreadableSheet(path) => format!(
            "a sheet could not be read ({}); nothing linked under it is removed",
            path.chain()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("/")
        ),
        EcoNote::PadLosesLock {
            reference,
            pad_number,
            ..
        } => format!(
            "{reference}.{pad_number} is locked and not on the new footprint; its lock goes with it"
        ),
        EcoNote::KeptUnlinked { reference, .. } => {
            format!("{reference} is not linked to a part and is kept")
        }
        EcoNote::Net(text) => text.clone(),
    }
}
