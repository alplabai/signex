//! The report as plain text (Export report…). The same report always gives
//! the same bytes: no locale, no clock, no float formatting.

use std::fmt::Write as _;

use super::{CompareAction, CompareObject, CompareReport, CompareRow, CompareValue, Direction};
use crate::pcb::eco::back::{BackBlock, BackNote};
use crate::pcb::eco::report::{blocked_text, note_text};

impl CompareReport {
    /// The report as text: the counts, one section per kind with each row's
    /// two sides and its choice (and blocked sides), then the notes.
    pub fn report_text(&self) -> String {
        let mut out = String::new();
        let counts = self.counts();
        let _ = writeln!(out, "Compare Board with Schematic");
        let _ = writeln!(out, "Board revision: {}", self.revision());
        let _ = writeln!(
            out,
            "Differences: {} ({} to PCB, {} to schematic, {} skipped)",
            counts.total, counts.to_pcb, counts.to_schematic, counts.skipped
        );
        if self.in_sync() {
            let c = self.compared;
            let _ = writeln!(
                out,
                "Board and schematic match: {} parts, {} nets and {} footprints compared.",
                c.parts, c.nets, c.footprints
            );
        }
        let mut kind = None;
        for row in &self.rows {
            if kind != Some(row.kind) {
                kind = Some(row.kind);
                let _ = writeln!(out, "\n{}", row.kind.label());
            }
            let _ = writeln!(
                out,
                "  {}: schematic {}, board {} -> {}",
                object_text(&row.object),
                side_text(&row.schematic, "not in schematic"),
                side_text(&row.board, "not on board"),
                self.choice_text(row)
            );
            if let Some(why) = &row.pcb_blocked {
                let _ = writeln!(out, "    Update PCB blocked: {}", blocked_text(why));
            }
            if let Some(why) = &row.back_blocked {
                let _ = writeln!(
                    out,
                    "    Update schematic blocked: {}",
                    back_block_text(why)
                );
            }
        }
        if !self.eco_notes().is_empty() || !self.back_notes().is_empty() {
            let _ = writeln!(out, "\nNotes");
            for note in self.eco_notes() {
                let _ = writeln!(out, "  - {}", note_text(note));
            }
            for note in self.back_notes() {
                let _ = writeln!(out, "  - {}", back_note_text(note));
            }
        }
        out
    }

    fn choice_text(&self, row: &CompareRow) -> String {
        let action = row.action(row.direction).map(CompareAction::label);
        match (row.direction, action) {
            (Direction::UpdatePcb, Some(a)) if self.waits(row) => {
                format!("Update PCB: {a} (skipped: a row it needs is skipped)")
            }
            (Direction::UpdatePcb, Some(a)) => format!("Update PCB: {a}"),
            (Direction::UpdateSchematic, Some(a)) => format!("Update schematic: {a}"),
            _ => "Skip".to_string(),
        }
    }
}

pub(super) fn object_text(object: &CompareObject) -> String {
    match object {
        CompareObject::Designator(s) | CompareObject::Net(s) => s.clone(),
        CompareObject::Pin { reference, pad } => format!("{reference} pin {pad}"),
    }
}

fn side_text(side: &Option<CompareValue>, absent: &str) -> String {
    match side {
        None => format!("({absent})"),
        Some(CompareValue::Text(s)) => s.clone(),
        Some(CompareValue::Part { value, footprint }) => format!("{value} {footprint}"),
        Some(CompareValue::NoNet) => "(no net)".to_string(),
        Some(CompareValue::Unlinked) => "(not linked)".to_string(),
    }
}

pub(super) fn back_block_text(why: &BackBlock) -> &'static str {
    match why {
        BackBlock::NeedsAnnotation => "the part is not annotated; run Annotate",
        BackBlock::DuplicateDesignator(_) => "the designator is used twice; run Annotate",
        BackBlock::BoardNotAnnotated => "the board designator is empty or holds ?",
        BackBlock::SharedSheet => "the sheet is placed more than once",
        BackBlock::ChannelDesignator => "a channel's designator comes from its sheet and naming",
        BackBlock::FootprintFromBoard => "taking a footprint from the board is not built yet",
    }
}

pub(super) fn back_note_text(note: &BackNote) -> String {
    match note {
        BackNote::UnreadableSheet(path) => format!(
            "a sheet could not be read ({})",
            path.chain()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("/")
        ),
        BackNote::Unlinked { reference } => format!("{reference} is not linked to a part"),
        BackNote::PartGone { reference } => {
            format!("{reference} is linked to a part that is not in the design")
        }
        BackNote::PartNotRead { designator } => {
            format!("part {designator} could not be read back from its sheet")
        }
        BackNote::SharedDesignator { designator, count } => {
            format!("{designator} would be on {count} parts")
        }
        BackNote::ExtraLink {
            reference,
            designator,
        } => format!("{reference} is linked to {designator} too; only the first is compared"),
    }
}
