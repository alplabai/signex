//! Rows 146 and 147: Preferences > Annotation and Editing, with the fast grid
//! fields and the pitch labels on the snap preview.

use iced::Point;
use signex_app::annotate_undo::AnnotateUndo;
use signex_app::app::Message;
use signex_app::canvas::grid::FAST_GRIDS_MM;
use signex_app::keymap::AppCommandId;
use signex_app::menu_bar::MenuMessage;
use signex_app::paste_prefs::PasteDesignators;
use signex_app::preferences::{PagesViewMsg, PrefMsg, PrefNav};
use signex_widgets::dimension_label::DimensionLabelMessage as Label;

use super::support::{Prefs, opened};

/// Preferences on the default fast grids, whatever the shared file holds.
fn on_default_grids() -> Prefs {
    let mut prefs = opened();
    prefs.app.ui_state.fast_grids_mm = FAST_GRIDS_MM;
    prefs.app.ui_state.preferences_draft_fast_grids = ["1.27".to_string(), "2.54".to_string()];
    prefs.send(PrefMsg::DraftFastGrid(0, "1.27".to_string()));
    assert!(
        !prefs.is_dirty(),
        "fixture: the fields show the saved grids"
    );
    prefs
}

fn pitch(prefs: &mut Prefs, msg: Label) {
    prefs.send(PrefMsg::PagesView(PagesViewMsg::SnapLabel(msg)));
}

fn field(prefs: &Prefs, index: usize) -> &str {
    &prefs.app.ui_state.preferences_draft_fast_grids[index]
}

fn editing_label(prefs: &Prefs) -> Option<usize> {
    prefs
        .app
        .ui_state
        .preferences_pages
        .pages_view
        .snap_label_editing()
}

// ---- Row 146: Annotation ----

#[test]
fn both_annotate_undo_radios_commit_on_apply() {
    let mut prefs = opened();

    for scope in [AnnotateUndo::PerSheet, AnnotateUndo::Project] {
        prefs.send(PrefMsg::DraftAnnotateUndo(scope));
        prefs.apply();

        assert_eq!(prefs.app.ui_state.annotate_undo, scope);
        assert!(!prefs.is_dirty());
    }
}

#[test]
fn cancel_keeps_the_saved_annotate_undo() {
    let mut prefs = opened();
    let saved = prefs.app.ui_state.annotate_undo;
    let other = match saved {
        AnnotateUndo::Project => AnnotateUndo::PerSheet,
        AnnotateUndo::PerSheet => AnnotateUndo::Project,
    };
    prefs.send(PrefMsg::DraftAnnotateUndo(other));
    assert_eq!(prefs.dirty_pages(), [PrefNav::Annotation]);

    prefs.cancel();

    assert_eq!(prefs.app.ui_state.annotate_undo, saved);
    assert_eq!(prefs.app.ui_state.preferences_draft_annotate_undo, saved);
}

#[test]
fn the_related_row_opens_editing_and_keeps_the_annotation_draft() {
    let mut prefs = opened();
    let saved = prefs.app.ui_state.annotate_undo;
    let other = match saved {
        AnnotateUndo::Project => AnnotateUndo::PerSheet,
        AnnotateUndo::PerSheet => AnnotateUndo::Project,
    };
    prefs.send(PrefMsg::Nav(PrefNav::Annotation));
    prefs.send(PrefMsg::DraftAnnotateUndo(other));

    prefs.send(PrefMsg::Nav(PrefNav::Editing));

    assert_eq!(prefs.app.ui_state.preferences_nav, PrefNav::Editing);
    assert_eq!(prefs.app.ui_state.preferences_draft_annotate_undo, other);
}

// ---- Row 147: the switches ----

#[test]
fn the_editing_switches_wait_for_apply_then_commit() {
    let mut prefs = opened();
    let ui = &prefs.app.ui_state;
    let (wires, drag, hover, autosave) = (
        ui.move_keeps_wires,
        ui.drag_direction_select,
        ui.net_hover_enabled,
        ui.autosave_enabled,
    );

    prefs.send(PrefMsg::DraftMoveKeepsWires(!wires));
    prefs.send(PrefMsg::DraftDragDirectionSelect(!drag));
    prefs.send(PrefMsg::DraftNetHoverEnabled(!hover));
    prefs.send(PrefMsg::DraftAutosaveEnabled(!autosave));

    let ui = &prefs.app.ui_state;
    assert_eq!(
        (
            ui.move_keeps_wires,
            ui.drag_direction_select,
            ui.net_hover_enabled,
            ui.autosave_enabled
        ),
        (wires, drag, hover, autosave),
        "nothing before Apply"
    );
    assert_eq!(prefs.dirty_pages(), [PrefNav::Editing]);

    prefs.apply();

    let ui = &prefs.app.ui_state;
    assert_eq!(
        (
            ui.move_keeps_wires,
            ui.drag_direction_select,
            ui.net_hover_enabled,
            ui.autosave_enabled
        ),
        (!wires, !drag, !hover, !autosave)
    );
    assert!(!prefs.is_dirty());
}

#[test]
fn cancel_puts_the_editing_switches_back() {
    let mut prefs = opened();
    let wires = prefs.app.ui_state.move_keeps_wires;
    let agents = prefs.app.ui_state.agent_connections_enabled;
    prefs.send(PrefMsg::DraftMoveKeepsWires(!wires));
    prefs.send(PrefMsg::DraftAgentConnectionsEnabled(!agents));
    assert_eq!(prefs.dirty_pages(), [PrefNav::Editing]);

    prefs.cancel();

    let ui = &prefs.app.ui_state;
    assert_eq!(ui.move_keeps_wires, wires);
    assert_eq!(ui.preferences_draft_move_keeps_wires, wires);
    assert_eq!(ui.agent_connections_enabled, agents);
    assert_eq!(ui.preferences_draft_agent_connections_enabled, agents);
}

#[test]
fn every_designators_on_paste_choice_commits_on_apply() {
    let mut prefs = opened();

    for mode in [
        PasteDesignators::Reset,
        PasteDesignators::Keep,
        PasteDesignators::KeepIfFree,
    ] {
        prefs.send(PrefMsg::DraftPasteDesignators(mode));
        prefs.apply();

        assert_eq!(prefs.app.ui_state.paste_designators, mode);
    }
}

// ---- Row 147: Increment step ----

#[test]
fn the_increment_stepper_skips_zero_in_both_directions() {
    let mut prefs = opened();
    let step = |prefs: &Prefs| prefs.app.ui_state.preferences_draft_increment_step;
    prefs.send(PrefMsg::DraftIncrementStep(1));

    prefs.send(PrefMsg::PagesView(PagesViewMsg::IncrementStepNudged(-1)));
    assert_eq!(step(&prefs), -1);

    prefs.send(PrefMsg::PagesView(PagesViewMsg::IncrementStepNudged(1)));
    assert_eq!(step(&prefs), 1);
}

#[test]
fn the_increment_step_keeps_to_minus_100_through_100() {
    let mut prefs = opened();
    let step = |prefs: &Prefs| prefs.app.ui_state.preferences_draft_increment_step;

    prefs.send(PrefMsg::DraftIncrementStep(100));
    prefs.send(PrefMsg::PagesView(PagesViewMsg::IncrementStepNudged(1)));
    assert_eq!(step(&prefs), 100);

    prefs.send(PrefMsg::DraftIncrementStep(-100));
    prefs.send(PrefMsg::PagesView(PagesViewMsg::IncrementStepNudged(-1)));
    assert_eq!(step(&prefs), -100);
}

#[test]
fn the_increment_step_commits_on_apply_and_cancel_restores_it() {
    let mut prefs = opened();
    let saved = prefs.app.ui_state.increment_step;
    let wanted = if saved == 7 { 8 } else { 7 };

    prefs.send(PrefMsg::DraftIncrementStep(wanted));
    assert_eq!(prefs.app.ui_state.increment_step, saved);
    prefs.apply();
    assert_eq!(prefs.app.ui_state.increment_step, wanted);

    prefs.send(PrefMsg::DraftIncrementStep(saved));
    prefs.cancel();
    assert_eq!(prefs.app.ui_state.increment_step, wanted);
}

// ---- Row 147: the fast grids ----

#[test]
fn a_fast_grid_set_here_is_what_the_fast_grid_command_snaps_to() {
    let mut prefs = on_default_grids();
    prefs.send(PrefMsg::DraftFastGrid(0, "0.635".to_string()));
    assert_eq!(prefs.app.ui_state.fast_grids_mm, FAST_GRIDS_MM);

    prefs.apply();

    assert_eq!(prefs.app.ui_state.fast_grids_mm[0], 0.635);
    let id = AppCommandId::new("fast_grid_1").expect("a catalog command");
    let _ = prefs.app.update(Message::Menu(MenuMessage::Command(id)));
    assert_eq!(prefs.app.ui_state.grid_size_mm, 0.635);
}

#[test]
fn a_fast_grid_below_the_minimum_shows_its_reason_on_the_row_while_typing() {
    let mut prefs = on_default_grids();

    prefs.send(PrefMsg::DraftFastGrid(1, "0.0000001".to_string()));

    assert_eq!(
        prefs.app.ui_state.fast_grid_field_error(1).as_deref(),
        Some("0.0000001 mm is finer than the 1 nm (0.000001 mm) minimum")
    );
    assert_eq!(prefs.app.ui_state.fast_grid_field_error(0), None);
}

#[test]
fn apply_refuses_a_bad_fast_grid_keeps_the_text_and_commits_nothing_else() {
    let mut prefs = on_default_grids();
    let wires = prefs.app.ui_state.move_keeps_wires;
    prefs.send(PrefMsg::DraftMoveKeepsWires(!wires));
    prefs.send(PrefMsg::DraftFastGrid(0, "0.635".to_string()));
    prefs.send(PrefMsg::DraftFastGrid(1, "abc".to_string()));

    prefs.apply();

    assert_eq!(field(&prefs, 1), "abc", "the typed text stays");
    assert_eq!(prefs.app.ui_state.fast_grids_mm, FAST_GRIDS_MM);
    assert_eq!(prefs.app.ui_state.move_keeps_wires, wires);
    assert!(prefs.is_dirty());
    let status = &prefs.app.ui_state.preferences_fast_grid_status;
    assert!(
        status.contains("not saved") && status.contains("Fast grid 2"),
        "{status}"
    );
}

#[test]
fn ok_with_a_bad_fast_grid_keeps_the_dialog_open() {
    let mut prefs = on_default_grids();
    prefs.send(PrefMsg::DraftFastGrid(1, "0".to_string()));

    prefs.ok();

    assert!(prefs.is_open());
    assert_eq!(field(&prefs, 1), "0");
    assert_eq!(prefs.app.ui_state.fast_grids_mm, FAST_GRIDS_MM);
}

#[test]
fn fixing_a_refused_fast_grid_clears_the_error_and_apply_goes_through() {
    let mut prefs = on_default_grids();
    prefs.send(PrefMsg::DraftFastGrid(1, "abc".to_string()));
    prefs.apply();

    prefs.send(PrefMsg::DraftFastGrid(1, "5.08".to_string()));
    assert_eq!(prefs.app.ui_state.fast_grid_field_error(1), None);
    prefs.apply();

    assert_eq!(prefs.app.ui_state.fast_grids_mm[1], 5.08);
    assert!(prefs.app.ui_state.preferences_fast_grid_status.is_empty());
    assert!(!prefs.is_dirty());
}

#[test]
fn cancel_puts_the_fast_grid_fields_back() {
    let mut prefs = on_default_grids();
    prefs.send(PrefMsg::DraftFastGrid(0, "abc".to_string()));

    prefs.cancel();

    assert_eq!(field(&prefs, 0), "1.27");
    assert_eq!(prefs.app.ui_state.fast_grids_mm, FAST_GRIDS_MM);
    assert!(!prefs.is_dirty());
}

// ---- Row 147: the pitch labels on the snap preview ----

#[test]
fn a_pitch_typed_on_the_drawing_fills_its_field_as_a_draft() {
    let mut prefs = on_default_grids();

    pitch(&mut prefs, Label::LabelPressed(0));
    pitch(&mut prefs, Label::DraftEdited("0.635".to_string()));
    pitch(&mut prefs, Label::DraftSubmitted);

    assert_eq!(field(&prefs, 0), "0.635");
    assert_eq!(editing_label(&prefs), None);
    assert!(prefs.is_dirty());
    assert_eq!(prefs.app.ui_state.fast_grids_mm, FAST_GRIDS_MM);
}

#[test]
fn tab_on_a_pitch_commits_it_and_opens_the_other() {
    let mut prefs = on_default_grids();

    pitch(&mut prefs, Label::LabelPressed(0));
    pitch(&mut prefs, Label::DraftEdited("0.5".to_string()));
    pitch(&mut prefs, Label::TabPressed { backward: false });

    assert_eq!(field(&prefs, 0), "0.5");
    assert_eq!(editing_label(&prefs), Some(1));
}

#[test]
fn esc_on_a_pitch_keeps_the_field_as_it_was() {
    let mut prefs = on_default_grids();

    pitch(&mut prefs, Label::LabelPressed(1));
    pitch(&mut prefs, Label::DraftEdited("9".to_string()));
    pitch(&mut prefs, Label::EscapePressed);

    assert_eq!(field(&prefs, 1), "2.54");
    assert_eq!(editing_label(&prefs), None);
    assert!(!prefs.is_dirty());
}

#[test]
fn dragging_a_pitch_handle_moves_its_field() {
    let mut prefs = on_default_grids();

    pitch(&mut prefs, Label::HandlePressed(0));
    pitch(&mut prefs, Label::PointerMoved(Point::new(100.0, 30.0)));
    pitch(&mut prefs, Label::PointerMoved(Point::new(124.0, 30.0)));
    pitch(&mut prefs, Label::PointerReleased);

    assert_eq!(field(&prefs, 0), "2.27", "24 px on the drawing is 1 mm");
    assert_eq!(field(&prefs, 1), "2.54");
}

#[test]
fn a_pitch_that_is_not_a_number_is_refused_and_its_field_stays_open() {
    let mut prefs = on_default_grids();

    pitch(&mut prefs, Label::LabelPressed(0));
    pitch(&mut prefs, Label::DraftEdited("abc".to_string()));
    pitch(&mut prefs, Label::DraftSubmitted);

    assert_eq!(field(&prefs, 0), "1.27", "the value did not change");
    assert_eq!(
        editing_label(&prefs),
        Some(0),
        "the refused text stays in its open field"
    );
}
