//! #1252 — a library part attaches to the cursor: Space / X / Y turn it, a
//! click places it as one undo step and the next click places another copy
//! with the next designator, Esc and a right-click drop it.
//!
//! Driven through the real messages against a real temp `.snxlib`.

use std::path::PathBuf;

use signex_app::app::{ContextMenuMsg, EditMsg, Message, Signex, Tool};
use signex_app::canvas::PlacementGhost;
use signex_app::canvas::placement_ghost::ghost_anchor;
use signex_app::dock::DockMessage;
use signex_app::library::LibraryMessage;
use signex_app::panels::PanelMsg;
use signex_app::schematic_runtime::ScreenTransform;
use signex_app::tab_bar::TabMessage;
use signex_app::toolbar::ToolMessage;
use signex_types::schematic::{SelectedItem, SelectedKind};

use crate::library_place_symbol::{
    Fixture, PLACE_AT, app_with_library, arm, click, complete, doc, fixture,
    fixture_with_designator, resolve, run,
};

fn refs(app: &Signex) -> Vec<String> {
    doc(app)
        .symbols
        .iter()
        .map(|s| s.reference.clone())
        .collect()
}

fn armed(f: &Fixture) -> Signex {
    let mut app = app_with_library(f);
    arm(&mut app, f);
    assert!(
        app.interaction_state.placement.is_some(),
        "the part is on the cursor"
    );
    app
}

fn session_symbol(app: &Signex) -> signex_types::schematic::Symbol {
    app.interaction_state
        .placement
        .as_ref()
        .and_then(|s| s.payload.lone_symbol().cloned())
        .expect("a part on the cursor")
}

#[test]
fn ghost_commit_lands_where_the_ghost_was_drawn() {
    let f = fixture();
    let mut app = armed(&f);
    let cursor = (25.0, 37.9);

    click(&mut app, cursor);

    // The grid point the ghost snaps to under this cursor...
    let grid = signex_app::canvas::grid::pitch_mm_exact(app.ui_state.grid_size_mm);
    let anchor = ghost_anchor(cursor, app.ui_state.snap_enabled, grid);
    // ...is where the symbol lands...
    let placed = &doc(&app).symbols[0];
    assert_eq!((placed.position.x, placed.position.y), anchor);
    // ...and the ghost's origin is drawn on that same point.
    let camera = ScreenTransform {
        offset_x: 13.0,
        offset_y: -7.0,
        scale: 3.5,
    };
    let drawn = PlacementGhost::transform_at(&camera, anchor).world_to_screen((0.0, 0.0));
    let landed = camera.world_to_screen((placed.position.x, placed.position.y));
    assert!((drawn.x - landed.x).abs() < 1e-3 && (drawn.y - landed.y).abs() < 1e-3);
}

#[test]
fn three_clicks_place_r1_r2_r3_each_one_undo() {
    let f = fixture_with_designator("R?");
    let mut app = armed(&f);

    for at in [(10.16, 10.16), (30.48, 10.16), (50.8, 10.16)] {
        click(&mut app, at);
    }

    assert_eq!(refs(&app), ["R1", "R2", "R3"]);
    assert!(
        app.interaction_state.placement.is_some(),
        "the session stays armed"
    );
    for expected in [2, 1, 0] {
        let _ = app.update(Message::Edit(EditMsg::Undo));
        assert_eq!(
            doc(&app).symbols.len(),
            expected,
            "one click is one undo step"
        );
    }
    assert!(
        doc(&app).lib_symbols.is_empty(),
        "the first click's undo takes the body too"
    );
}

#[test]
fn tab_designator_override_then_increments() {
    let f = fixture_with_designator("R?");
    let mut app = armed(&f);

    // Tab opens the pre-placement form with the session's defaults.
    let _ = app.update(Message::Tool(ToolMessage::PrePlacementTab));
    let form = app
        .document_state
        .panel_ctx
        .pre_placement
        .clone()
        .expect("form");
    assert_eq!(form.designator, "R1");
    assert!(app.interaction_state.active_canvas().placement_paused);

    let _ = app.update(Message::Dock(DockMessage::Panel(
        PanelMsg::SetPrePlacementDesignator("R10".to_owned()),
    )));
    let _ = app.update(Message::Dock(DockMessage::Panel(
        PanelMsg::SetPrePlacementText("4k7".to_owned()),
    )));
    let _ = app.update(Message::Tool(ToolMessage::ResumePlacement));
    click(&mut app, (10.16, 10.16));
    click(&mut app, (30.48, 10.16));

    assert_eq!(refs(&app), ["R10", "R11"]);
    assert!(doc(&app).symbols.iter().all(|s| s.value == "4k7"));
}

#[test]
fn space_x_y_turn_the_ghost_not_the_selection() {
    let f = fixture();
    let mut app = app_with_library(&f);
    crate::library_place_symbol::place(&mut app, &f);
    let placed = doc(&app).symbols[0].clone();
    app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(placed.uuid, SelectedKind::Symbol)];
    arm(&mut app, &f);

    let _ = app.update(Message::Edit(EditMsg::RotateSelectedCounterclockwise));
    assert_eq!(session_symbol(&app).rotation, 90.0);
    let _ = app.update(Message::Edit(EditMsg::MirrorSelectedX));
    assert!(session_symbol(&app).mirror_x);
    let _ = app.update(Message::Edit(EditMsg::MirrorSelectedY));
    assert!(session_symbol(&app).mirror_y);

    // The placed, selected symbol did not move.
    let after = &doc(&app).symbols[0];
    assert_eq!(after.rotation, placed.rotation);
    assert_eq!(
        (after.mirror_x, after.mirror_y),
        (placed.mirror_x, placed.mirror_y)
    );

    // And the click places the turned part.
    click(&mut app, (60.96, 60.96));
    let second = &doc(&app).symbols[1];
    assert_eq!(second.rotation, 90.0);
    assert!(second.mirror_x && second.mirror_y);
}

#[test]
fn esc_and_right_click_drop_the_session() {
    let f = fixture();
    let mut app = armed(&f);
    let epoch = app.interaction_state.placement_epoch;

    // Esc is a tool reset to Select.
    let _ = app.update(Message::Tool(ToolMessage::SelectTool(Tool::Select)));
    assert!(app.interaction_state.placement.is_none());
    assert!(app.interaction_state.placement_epoch > epoch);
    click(&mut app, PLACE_AT);
    assert!(doc(&app).symbols.is_empty(), "nothing is attached any more");

    // Right-click ends it instead of opening the menu.
    arm(&mut app, &f);
    assert!(app.interaction_state.placement.is_some());
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::Show(50.0, 50.0)));
    assert!(app.interaction_state.placement.is_none());
    assert!(app.interaction_state.context_menu.is_none());
    assert_eq!(app.interaction_state.current_tool, Tool::Select);
}

#[test]
fn resolve_completing_after_esc_does_not_arm() {
    let f = fixture();
    let mut app = app_with_library(&f);
    let task = app.update(Message::Library(LibraryMessage::PlaceLibraryComponent {
        library_path: f.snxlib.clone(),
        table: "ICs".into(),
        row_id: signex_library::RowId(f.row_id),
    }));

    // Esc lands while the row is still being read.
    let _ = app.update(Message::Tool(ToolMessage::SelectTool(Tool::Select)));
    for message in run(task) {
        let _ = app.update(message);
    }

    assert!(app.interaction_state.placement.is_none());
    assert!(
        app.document_state.error_notice.is_none(),
        "a cancel is not a failure"
    );
}

fn second_sheet(app: &mut Signex) -> PathBuf {
    let path = PathBuf::from("other-1252.snxsch");
    let sheet = signex_types::schematic::SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new(sheet).expect("engine"),
    );
    app.document_state.tabs.push(signex_app::app::TabInfo {
        title: "other".to_string(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: signex_app::app::TabKind::Schematic,
    });
    path
}

#[test]
fn sheet_switch_cancels_session_no_placement_into_other_sheet() {
    let f = fixture();
    let mut app = armed(&f);
    let other = second_sheet(&mut app);
    let window = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(window);

    // A real tab switch ends the session.
    let _ = app.update(Message::Tab {
        window_id: window,
        msg: TabMessage::Select(1),
    });
    assert!(app.interaction_state.placement.is_none());
    assert_eq!(
        app.document_state.active_path.as_deref(),
        Some(other.as_path())
    );
    click(&mut app, PLACE_AT);
    assert!(doc(&app).symbols.is_empty());

    // Even a session that outlived a switch (active path changed behind its
    // back) never places into the sheet that is showing now.
    let first = PathBuf::from("place-364.snxsch");
    app.document_state.active_path = Some(first.clone());
    arm(&mut app, &f);
    app.document_state.active_path = Some(other.clone());
    click(&mut app, PLACE_AT);
    assert!(doc(&app).symbols.is_empty(), "the other sheet got nothing");
    assert!(
        app.document_state.engines[&first]
            .document()
            .symbols
            .is_empty()
    );
}

#[test]
fn session_refused_on_newer_minor_sheet() {
    let f = fixture();
    let mut app = app_with_library(&f);
    let engine = app.document_state.active_engine_mut().expect("engine");
    let mut sheet = engine.document().clone();
    sheet.unknown.newer_minor_source = Some("snxsch/1.9".to_owned());
    engine.set_document(sheet);

    complete(&mut app, &f, "place-364.snxsch", resolve(&f, f.row_id));

    assert!(
        app.interaction_state.placement.is_none(),
        "no session on a read-only sheet"
    );
    let notice = app.document_state.error_notice.as_ref().expect("says why");
    assert!(notice.detail.contains("read-only"), "{}", notice.detail);
    assert!(doc(&app).symbols.is_empty());
}

#[test]
fn a_click_in_an_undocked_window_commits_to_that_windows_sheet() {
    let f = fixture();
    let mut app = app_with_library(&f);
    let first = PathBuf::from("place-364.snxsch");
    let other = second_sheet(&mut app);
    let window = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(iced::window::Id::unique());
    // The undocked window shows the second sheet; the main window shows the
    // first one and the part is armed for the second.
    let _ = app.update(Message::Window(
        signex_app::app::WindowMsg::UndockedTabOpened {
            path: other.clone(),
            id: window,
        },
    ));
    app.document_state.active_path = Some(other.clone());
    complete(&mut app, &f, "other-1252.snxsch", resolve(&f, f.row_id));
    app.document_state.active_path = Some(first.clone());
    assert!(
        app.interaction_state.placement.is_some(),
        "armed for the second sheet"
    );
    assert!(
        app.placement_ghost_for_window(window).is_some(),
        "the undocked window shows the target sheet, so it draws the ghost"
    );
    assert!(
        app.placement_ghost_for_window(app.ui_state.main_window_id.unwrap())
            .is_none(),
        "the main window shows another sheet and draws nothing"
    );

    // A click in the main window (active path = first sheet) is refused.
    click(&mut app, PLACE_AT);
    assert!(
        app.document_state.engines[&first]
            .document()
            .symbols
            .is_empty()
    );
    assert!(
        app.document_state.engines[&other]
            .document()
            .symbols
            .is_empty()
    );

    // The same click in the undocked window lands on ITS sheet, and the
    // active path is restored afterwards.
    let _ = app.update(Message::CanvasEventInWindow {
        window_id: window,
        event: signex_app::canvas::CanvasEvent::Clicked {
            world_x: PLACE_AT.0,
            world_y: PLACE_AT.1,
        },
    });
    assert_eq!(
        app.document_state.engines[&other].document().symbols.len(),
        1
    );
    assert!(
        app.document_state.engines[&first]
            .document()
            .symbols
            .is_empty()
    );
    assert_eq!(
        app.document_state.active_path.as_deref(),
        Some(first.as_path())
    );
}
