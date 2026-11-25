//! #925 — a footprint grid step typed as `nan` / `inf` is refused out
//! loud. Both inputs already kept the stored step, but they dropped the
//! value without a word, so the edit looked like a dead Enter key. Every
//! refusal has to reach the Messages panel.

use signex_app::app::{FootprintDocument, GridPropertiesMsg, Message, Signex, TabInfo, TabKind};
use signex_app::dock::DockMessage;
use signex_app::panels::PanelMsg;
use signex_library::{Footprint, FootprintFile};
use std::path::PathBuf;

const NON_FINITE: &str = "inf";

fn footprint_app() -> (Signex, PathBuf) {
    let _ = signex_app::diagnostics::init_logging();
    let (mut app, _boot) = Signex::new();
    let path = PathBuf::from("grid-step-925.snxfpt");
    let file = FootprintFile::from_footprint(Footprint::empty("A"));
    app.document_state
        .footprint_editors
        .insert(path.clone(), FootprintDocument::new(path.clone(), file));
    app.document_state.tabs.push(TabInfo {
        title: "grid".into(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    (app, path)
}

fn grid_step(app: &Signex, path: &PathBuf) -> f64 {
    app.document_state.footprint_editors[path]
        .state
        .snap_options
        .grid_step_mm
}

fn refusal_logged(app: &Signex) -> bool {
    app.document_state
        .panel_ctx
        .diagnostics
        .iter()
        .any(|entry| entry.message.contains("refused") && entry.message.contains(NON_FINITE))
}

/// #2792: Snap Distance and Axis Snap Range take 0.001 to 100 mm. Anything
/// else changes nothing and stays in the box; a good value clears it.
#[test]
fn a_snap_reach_outside_its_range_is_refused_with_its_text_kept() {
    use signex_app::library::editor::footprint::SnapOptions;
    type Msg = fn(String) -> PanelMsg;
    type Stored = fn(&SnapOptions) -> f64;
    let fields: [(Msg, &str, Stored); 2] = [
        (
            PanelMsg::FpEditorSetSnapDistance,
            SnapOptions::DISTANCE_BUFFER,
            |o| o.snap_distance_mm,
        ),
        (
            PanelMsg::FpEditorSetAxisSnapRange,
            SnapOptions::AXIS_RANGE_BUFFER,
            |o| o.axis_snap_range_mm,
        ),
    ];
    let (mut app, path) = footprint_app();
    for (msg, buffer, stored) in fields {
        let state = |app: &Signex| app.document_state.footprint_editors[&path].state.clone();
        let before = stored(&state(&app).snap_options);
        for bad in ["0", "-1", "100.5", "abc"] {
            let _ = app.update(Message::Dock(DockMessage::Panel(msg(bad.into()))));
            assert_eq!(stored(&state(&app).snap_options), before, "{bad}");
            assert_eq!(
                state(&app).numeric_buffers.get(buffer).map(String::as_str),
                Some(bad)
            );
        }
        let _ = app.update(Message::Dock(DockMessage::Panel(msg("100".into()))));
        assert_eq!(stored(&state(&app).snap_options), 100.0);
        assert!(!state(&app).numeric_buffers.contains_key(buffer));
    }
}

#[test]
fn a_non_finite_grid_step_in_the_properties_panel_is_refused_out_loud() {
    let (mut app, path) = footprint_app();
    let before = grid_step(&app, &path);

    let _ = app.update(Message::Dock(DockMessage::Panel(
        PanelMsg::FpEditorSetSnapGridStep(NON_FINITE.into()),
    )));

    assert_eq!(grid_step(&app, &path), before, "the stored step is kept");
    assert!(
        refusal_logged(&app),
        "the refusal must reach the Messages panel; got {:?}",
        app.document_state.panel_ctx.diagnostics
    );
}

#[test]
fn a_non_finite_grid_step_in_grid_properties_is_refused_out_loud() {
    let (mut app, path) = footprint_app();
    let before = grid_step(&app, &path);

    let _ = app.update(Message::GridProperties(GridPropertiesMsg::Open));
    let _ = app.update(Message::GridProperties(GridPropertiesMsg::SetStep(
        NON_FINITE.into(),
    )));
    let _ = app.update(Message::GridProperties(GridPropertiesMsg::Apply));

    assert_eq!(grid_step(&app, &path), before, "the stored step is kept");
    assert!(
        refusal_logged(&app),
        "the refusal must reach the Messages panel; got {:?}",
        app.document_state.panel_ctx.diagnostics
    );
}

#[test]
fn a_valid_grid_step_still_applies() {
    let (mut app, path) = footprint_app();

    let _ = app.update(Message::Dock(DockMessage::Panel(
        PanelMsg::FpEditorSetSnapGridStep("0.25".into()),
    )));

    assert_eq!(grid_step(&app, &path), 0.25);
}

fn apply_typed_step(app: &mut Signex, text: &str) {
    let _ = app.update(Message::GridProperties(GridPropertiesMsg::Open));
    let _ = app.update(Message::GridProperties(GridPropertiesMsg::SetStep(
        text.into(),
    )));
    let _ = app.update(Message::GridProperties(GridPropertiesMsg::Apply));
}

fn refusal_with(app: &Signex, needle: &str) -> bool {
    app.document_state
        .panel_ctx
        .diagnostics
        .iter()
        .any(|entry| entry.message.contains("Grid step refused") && entry.message.contains(needle))
}

/// #972 — zero, negative and non-numeric steps are refused through
/// Apply: value kept, dialog still open, reason in the Messages panel.
#[test]
fn grid_properties_apply_refuses_bad_steps_and_keeps_the_dialog_open() {
    for (text, needle) in [("0", "above zero"), ("-0.5", "above zero"), ("nan", "nan")] {
        let (mut app, path) = footprint_app();
        let before = grid_step(&app, &path);
        apply_typed_step(&mut app, text);

        assert_eq!(grid_step(&app, &path), before, "{text}: stored step kept");
        assert!(
            app.ui_state.grid_properties.is_some(),
            "{text}: dialog must stay open"
        );
        assert!(
            refusal_with(&app, needle),
            "{text}: refusal with reason must reach the Messages panel; got {:?}",
            app.document_state.panel_ctx.diagnostics
        );
    }
}

#[test]
fn grid_properties_apply_accepts_a_valid_step_and_closes() {
    let (mut app, path) = footprint_app();
    apply_typed_step(&mut app, "0.25");

    assert_eq!(grid_step(&app, &path), 0.25);
    assert!(app.ui_state.grid_properties.is_none());
}
