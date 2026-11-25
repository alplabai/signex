//! The `Needs` reasons are catalog ids now (ADR-0006 slice 7): each renders
//! in en-US byte-identical to the English literal it replaced, and the agent
//! reply and the Messages line built from one read as before.

use signex_i18n::{RenderTranslatable, Translatable};

use super::needs::Needs;
use super::{CommandArgs, CommandOutcome};
use crate::app::Signex;
use crate::keymap::AppCommandId;

/// The English each reason was before the catalog. A `match`, so a new need
/// cannot be left out.
fn old_reason(need: Needs) -> &'static str {
    match need {
        Needs::Planned => "Planned, not available yet",
        Needs::SaveTarget => {
            "Save needs a schematic or editor document, or a project with unsaved changes"
        }
        Needs::SaveAsTarget => "Save As needs a schematic, symbol or footprint document",
        // New with `save_all` (#2844): it replaced no English literal.
        Needs::UnsavedWork => "Nothing is unsaved",
        Needs::PrintTarget => "Print needs an open schematic or PCB",
        Needs::Document => "No document is open",
        Needs::Undo => "Nothing to undo",
        Needs::Redo => "Nothing to redo",
        Needs::CopyTarget => "Nothing is selected to copy",
        Needs::CutTarget => "Nothing is selected to cut, or the selection is locked",
        Needs::PasteTarget => {
            "Nothing to paste here: no schematic is open, or the clipboard holds no board objects"
        }
        Needs::SelectAllTarget => "No schematic or PCB is open",
        Needs::DrawingSelection => "No drawing is selected",
        Needs::SelectedObject => "No object is selected",
        Needs::Schematic => "No schematic is open",
        Needs::ParentSheet => "The active sheet has no parent sheet",
        Needs::DeleteTarget => "Nothing is selected to delete, or the selection is locked",
        Needs::VertexTarget => "Nothing is selected and no wire is being drawn",
        Needs::DrawInProgress => "No wire, bus or polyline is being drawn",
        Needs::Sheets => "No schematic or project is open",
        Needs::ActiveSheet => "No sheet is open",
        Needs::Canvas => "No schematic or PCB is open",
        Needs::FitTarget => "No schematic or PCB is open",
        Needs::FootprintEditor => "No footprint editor is open",
        Needs::Pcb => "No PCB is open",
        Needs::NetClassTable => "No schematic or PCB is open",
        Needs::EditablePcb => "No PCB is open, or the board is read-only",
        Needs::MeasureTarget => "No schematic or PCB is in view",
        Needs::TurnedPcbView => "The board view is not rotated",
        // New with the 3D view's keys (mockup row 175): it replaced no
        // English literal.
        Needs::Board3dView => "This needs the board's 3D view",
        Needs::Turn => "Nothing is selected or in hand to turn, or the selection is locked",
        Needs::MoveSelection => "Nothing is selected to move, or the selection is locked",
        Needs::MoveObject => "No schematic or PCB is open",
        Needs::SimilarSeed => "Nothing is selected on a sheet to find similar objects to",
        Needs::PcbPlacement => "No unlocked footprint is selected on a PCB",
        Needs::SwapFootprints => "No unlocked footprint is selected on a PCB",
        Needs::PcbSelection => "Nothing is selected on the PCB in view",
        Needs::PadPair => "Select two pads of one unlocked component on a PCB",
        Needs::ReuseSource => "Nothing is selected on a PCB to save as a reuse block",
        Needs::ReuseAnchor => "Select one unlocked component of the channel to place the block on",
        Needs::ProbeTarget => "Nothing is selected on the sheet or board to cross probe",
        Needs::PcbSync => "No schematic or PCB of an open project is in view",
        Needs::OpenProject => "No project is open",
        Needs::PcbInProject => "No board of an open project is in view, or it is read-only",
        Needs::Library => "No library is open",
        Needs::ProjectSheet => "No sheet of a project is open",
        Needs::TileableWindows => {
            "Tile needs two documents in the main window, or a document in its own window"
        }
        Needs::Tiled => "The documents are not tiled",
    }
}

/// Every need, from the catalog's gates and the planned tools.
fn every_need() -> Vec<Needs> {
    let mut all: Vec<Needs> = Vec::new();
    for id in crate::keymap::all_command_ids() {
        let command = AppCommandId::new(id).expect("catalog id is valid");
        if let Some(need) = super::needs::needs(&command)
            && !all.contains(&need)
        {
            all.push(need);
        }
    }
    all
}

#[test]
fn every_reason_renders_the_english_it_replaced() {
    let needs = every_need();
    assert_eq!(needs.len(), 48, "a need no catalog id reaches: {needs:?}");
    for need in needs {
        assert_eq!(
            Translatable::new(need.reason()).render_en_us(),
            old_reason(need),
            "{need:?}"
        );
    }
}

/// The agent reply and the Messages line of a gate's refusal are the old
/// English, word for word.
#[test]
fn the_agent_reply_and_the_messages_line_keep_their_text() {
    let _ = crate::diagnostics::init_logging();
    let (mut app, _task) = Signex::new();
    let command = AppCommandId::new("zoom_in_at_cursor").expect("id");

    let (outcome, _task) = app.dispatch_command_reporting(&command, CommandArgs::none(), None);
    let reply = crate::app::agent::outcome_verdict(command.as_str(), outcome, None);
    assert_eq!(reply.1.as_deref(), Some("No schematic or PCB is open"));

    let since = crate::diagnostics::newest_entry_id().unwrap_or(0);
    let _ = app.run_menu_command(&command, None);
    let logged: Vec<String> = crate::diagnostics::recent_entries()
        .into_iter()
        .filter(|entry| entry.id > since)
        .map(|entry| entry.message)
        .collect();
    assert!(
        logged
            .iter()
            .any(|line| line == "Menu: zoom_in_at_cursor did nothing: No schematic or PCB is open"),
        "{logged:?}"
    );
    assert!(matches!(
        app.dispatch_command_reporting(&command, CommandArgs::none(), None)
            .0,
        CommandOutcome::Disabled { .. }
    ));
}
