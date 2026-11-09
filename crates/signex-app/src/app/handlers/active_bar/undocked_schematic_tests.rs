//! #1910: the Active Bar, its dropdown and the canvas context menu painted
//! in an undocked schematic window act on the sheet that window shows, and
//! paint in that window only.

use std::path::{Path, PathBuf};

use super::super::super::*;
use crate::active_bar::{ActiveBarAction, ActiveBarMenu, ActiveBarMsg};
use crate::app::handlers::menu::export::tests as fx;
use crate::app::state::WindowKind;
use signex_types::schematic::{SelectedItem, SelectedKind};

struct Fixture {
    app: Signex,
    main: iced::window::Id,
    window: iced::window::Id,
    a: PathBuf,
    b: PathBuf,
}

fn tab(path: &Path) -> TabInfo {
    TabInfo {
        title: "t".to_string(),
        path: path.to_path_buf(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    }
}

/// Sheet A undocked in its own window, sheet B active in the main window.
/// Each window's canvas has its sheet's wire selected.
fn fixture() -> Fixture {
    let a = PathBuf::from("/w/ov/a.snxsch");
    let b = PathBuf::from("/w/ov/b.snxsch");
    let mut app = fx::app_workspace("/w/ov", &["a.snxsch", "b.snxsch"]);
    let main = iced::window::Id::unique();
    let window = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(main);
    app.ui_state.windows.insert(
        window,
        WindowKind::UndockedTab {
            path: a.clone(),
            title: "a".to_string(),
        },
    );
    let mut slots = Vec::new();
    for (path, net) in [(&a, "A_NET"), (&b, "B_NET")] {
        let sheet = fx::sheet_with_net("R1", net, &[]);
        let wire = SelectedItem::new(sheet.wires[0].uuid, SelectedKind::Wire);
        let mut slot = crate::canvas::CanvasSlot::new();
        slot.set_render_cache(Some(
            crate::schematic_runtime::SchematicRenderCache::from_sheet(&sheet),
        ));
        slot.selected = vec![wire];
        slots.push(slot);
        fx::open_with(&mut app.document_state, path, sheet);
        app.document_state.tabs.push(tab(path));
    }
    let main_slot = slots.pop().expect("b");
    app.interaction_state.canvas = main_slot;
    app.interaction_state
        .canvases
        .insert(window, slots.pop().expect("a"));
    app.document_state.active_tab = 1;
    app.document_state.active_path = Some(b.clone());
    Fixture {
        app,
        main,
        window,
        a,
        b,
    }
}

fn wire_ends(app: &Signex, path: &Path) -> Vec<(f64, f64, f64, f64)> {
    app.document_state.engines[path]
        .document()
        .wires
        .iter()
        .map(|w| (w.start.x, w.start.y, w.end.x, w.end.y))
        .collect()
}

fn scoped_bar(window: iced::window::Id, inner: ActiveBarMsg) -> Message {
    Message::ActiveBar(ActiveBarMsg::ScopedToWindow(window, Box::new(inner)))
}

fn scoped_menu(window: iced::window::Id, inner: ContextMenuMsg) -> Message {
    Message::ContextMenu(ContextMenuMsg::ScopedToWindow(window, Box::new(inner)))
}

/// What pressing a catalog command row of the context menu sends.
fn command_row(id: &str) -> ContextAction {
    ContextAction::Command {
        id: crate::keymap::AppCommandId::new(id).unwrap(),
        project: None,
    }
}

#[test]
fn an_active_bar_action_from_an_undocked_window_acts_on_its_sheet() {
    let mut f = fixture();
    let (a_before, b_before) = (wire_ends(&f.app, &f.a), wire_ends(&f.app, &f.b));
    let _ = f.app.update(scoped_bar(
        f.window,
        ActiveBarMsg::Action(ActiveBarAction::RotateSelection),
    ));
    assert_ne!(wire_ends(&f.app, &f.a), a_before, "A's wire rotated");
    assert_eq!(wire_ends(&f.app, &f.b), b_before, "B is untouched");
    assert_eq!(
        f.app.document_state.active_path.as_deref(),
        Some(f.b.as_path()),
        "the main window keeps its sheet"
    );
    assert_eq!(f.app.document_state.active_tab, 1);
}

#[test]
fn a_context_menu_action_from_an_undocked_window_acts_on_its_sheet() {
    let mut f = fixture();
    let _ = f
        .app
        .update(scoped_menu(f.window, ContextMenuMsg::Show(5.0, 5.0)));
    let menu = f.app.interaction_state.context_menu.as_ref().expect("menu");
    assert_eq!(menu.window, Some(f.window));
    let _ = f.app.update(scoped_menu(
        f.window,
        ContextMenuMsg::Action(command_row("delete_selection")),
    ));
    assert!(wire_ends(&f.app, &f.a).is_empty(), "A's wire is deleted");
    assert_eq!(wire_ends(&f.app, &f.b).len(), 1, "B keeps its wire");
    assert!(f.app.interaction_state.context_menu.is_none());
}

#[test]
fn an_undocked_windows_context_menu_paints_in_that_window_only() {
    let mut f = fixture();
    let _ = f
        .app
        .update(scoped_menu(f.window, ContextMenuMsg::Show(5.0, 5.0)));
    assert!(!f.app.context_menu_overlay(f.window).is_empty());
    assert!(f.app.context_menu_overlay(f.main).is_empty());
}

#[test]
fn the_main_windows_context_menu_does_not_paint_in_an_undocked_window() {
    let mut f = fixture();
    let _ = f
        .app
        .update(Message::ContextMenu(ContextMenuMsg::Show(5.0, 5.0)));
    assert!(!f.app.context_menu_overlay(f.main).is_empty());
    assert!(f.app.context_menu_overlay(f.window).is_empty());
}

#[test]
fn an_undocked_windows_dropdown_paints_in_that_window_only() {
    let mut f = fixture();
    let _ = f.app.update(scoped_bar(
        f.window,
        ActiveBarMsg::ToggleMenu(ActiveBarMenu::Align),
    ));
    assert!(!f.app.active_bar_menu_overlay(f.window).is_empty());
    assert!(f.app.active_bar_menu_overlay(f.main).is_empty());
}

#[test]
fn the_main_windows_dropdown_does_not_paint_in_an_undocked_window() {
    let mut f = fixture();
    let _ = f.app.update(Message::ActiveBar(ActiveBarMsg::ToggleMenu(
        ActiveBarMenu::Align,
    )));
    assert!(!f.app.active_bar_menu_overlay(f.main).is_empty());
    assert!(f.app.active_bar_menu_overlay(f.window).is_empty());
}

#[test]
fn a_scoped_edit_marks_only_its_sheet_dirty_and_is_undoable_there() {
    let mut f = fixture();
    let _ = f
        .app
        .update(scoped_menu(f.window, ContextMenuMsg::Show(5.0, 5.0)));
    let _ = f.app.update(scoped_menu(
        f.window,
        ContextMenuMsg::Action(command_row("delete_selection")),
    ));
    let dirty = f.app.document_state.dirty_document_paths();
    assert!(dirty.contains(&f.a), "A is dirty");
    assert!(!dirty.contains(&f.b), "B is clean");
    assert!(f.app.document_state.engines[&f.a].can_undo());
    assert!(!f.app.document_state.engines[&f.b].can_undo());
}

#[test]
fn move_selection_by_xy_from_an_undocked_window_moves_its_sheet() {
    let mut f = fixture();
    let (a_before, b_before) = (wire_ends(&f.app, &f.a), wire_ends(&f.app, &f.b));
    let _ = f.app.update(scoped_bar(
        f.window,
        ActiveBarMsg::Action(ActiveBarAction::MoveSelectionXY),
    ));
    assert_eq!(f.app.ui_state.move_selection.sheet.as_ref(), Some(&f.a));
    let _ = f
        .app
        .update(Message::MoveSelection(MoveSelectionMsg::DxChanged(
            "10".into(),
        )));
    // The dialog paints in the main window, so Apply arrives untagged.
    let _ = f
        .app
        .update(Message::MoveSelection(MoveSelectionMsg::Apply));
    assert_ne!(wire_ends(&f.app, &f.a), a_before, "A's wire moved");
    assert_eq!(wire_ends(&f.app, &f.b), b_before, "B is untouched");
    assert!(!f.app.document_state.dirty_document_paths().contains(&f.b));
    assert_eq!(
        f.app.document_state.active_path.as_deref(),
        Some(f.b.as_path())
    );
}

#[test]
fn find_similar_from_an_undocked_window_keeps_working_on_its_sheet() {
    let mut f = fixture();
    let _ = f
        .app
        .update(scoped_menu(f.window, ContextMenuMsg::Show(5.0, 5.0)));
    let _ = f.app.update(scoped_menu(
        f.window,
        ContextMenuMsg::Action(command_row("find_similar_objects")),
    ));
    let state = f.app.ui_state.find_similar.as_ref().expect("dialog opened");
    assert_eq!(state.origin.as_ref(), Some(&f.a));
    let property = state.values.last().expect("a property").property;
    // The dialog paints in the main window, so its messages arrive untagged.
    let _ = f.app.update(Message::Selection(
        crate::app::selection_request::SelectionRequest::FindSimilar(
            crate::find_similar::FindSimilarMsg::PropertySet(property, true),
        ),
    ));
    let state = f.app.ui_state.find_similar.as_ref().expect("still open");
    assert!(state.same.contains(&property));
}

#[test]
fn a_menu_in_the_main_window_closes_an_undocked_windows_dropdown() {
    let mut f = fixture();
    let _ = f.app.update(scoped_bar(
        f.window,
        ActiveBarMsg::ToggleMenu(ActiveBarMenu::Align),
    ));
    let _ = f
        .app
        .update(Message::ContextMenu(ContextMenuMsg::Show(5.0, 5.0)));
    assert!(f.app.interaction_state.active_bar_menu.is_none());
    assert!(f.app.interaction_state.context_menu.is_some());
}

#[test]
fn closing_an_undocked_window_drops_the_overlays_it_opened() {
    let mut f = fixture();
    let _ = f.app.update(scoped_bar(
        f.window,
        ActiveBarMsg::ToggleMenu(ActiveBarMenu::Align),
    ));
    let _ = f
        .app
        .update(Message::Window(WindowMsg::SecondaryWindowClosed(f.window)));
    assert!(f.app.interaction_state.active_bar_menu.is_none());
    let mut f = fixture();
    let _ = f
        .app
        .update(scoped_menu(f.window, ContextMenuMsg::Show(5.0, 5.0)));
    let _ = f
        .app
        .update(Message::Window(WindowMsg::SecondaryWindowClosed(f.window)));
    assert!(f.app.interaction_state.context_menu.is_none());
}

#[test]
fn reattaching_an_undocked_window_drops_the_overlays_it_opened() {
    let mut f = fixture();
    let _ = f.app.update(scoped_bar(
        f.window,
        ActiveBarMsg::ToggleMenu(ActiveBarMenu::Align),
    ));
    let _ = f
        .app
        .update(Message::Window(WindowMsg::ReattachTab(f.window)));
    assert!(f.app.interaction_state.active_bar_menu.is_none());
    let mut f = fixture();
    let _ = f
        .app
        .update(scoped_menu(f.window, ContextMenuMsg::Show(5.0, 5.0)));
    let _ = f
        .app
        .update(Message::Window(WindowMsg::ReattachTab(f.window)));
    assert!(f.app.interaction_state.context_menu.is_none());
}

#[test]
fn a_right_press_on_an_undocked_windows_active_bar_belongs_to_that_window() {
    use crate::app::view::overlay_id::scope_canvas_message;
    let mut f = fixture();
    // The canvas publishes the right-press as a plain ActiveBar message (#1509).
    let pressed = scope_canvas_message(
        f.window,
        Message::ActiveBar(ActiveBarMsg::ToggleMenu(ActiveBarMenu::Align)),
    );
    let _ = f.app.update(pressed);
    assert_eq!(
        f.app.interaction_state.active_bar_menu_window,
        Some(f.window)
    );
    assert!(!f.app.active_bar_menu_overlay(f.window).is_empty());
    assert!(f.app.active_bar_menu_overlay(f.main).is_empty());
    // A dropdown entry arrives tagged by the window that paints it and acts
    // on that window's sheet (Rotate stands in: Align needs two items).
    let (a_before, b_before) = (wire_ends(&f.app, &f.a), wire_ends(&f.app, &f.b));
    let _ = f.app.update(scoped_bar(
        f.window,
        ActiveBarMsg::Action(ActiveBarAction::RotateSelection),
    ));
    assert_ne!(wire_ends(&f.app, &f.a), a_before, "A's wire rotated");
    assert_eq!(wire_ends(&f.app, &f.b), b_before, "B is untouched");
}

#[test]
fn submenu_hover_from_an_undocked_window_is_handled_without_the_document() {
    let mut f = fixture();
    let _ = f
        .app
        .update(scoped_menu(f.window, ContextMenuMsg::Show(5.0, 5.0)));
    let _ = f.app.update(scoped_menu(
        f.window,
        ContextMenuMsg::SubmenuHover(ContextSubmenu::Place),
    ));
    assert_eq!(
        f.app.interaction_state.submenu_launcher_hovered,
        Some(ContextSubmenu::Place)
    );
    let _ = f
        .app
        .update(scoped_menu(f.window, ContextMenuMsg::SubmenuEnterPanel));
    assert!(f.app.interaction_state.submenu_panel_hovered);
    let _ = f
        .app
        .update(scoped_menu(f.window, ContextMenuMsg::SubmenuLeavePanel));
    assert!(!f.app.interaction_state.submenu_panel_hovered);
}

#[test]
fn an_undocked_windows_size_places_its_overlays() {
    let mut f = fixture();
    f.app.ui_state.window_size = (1400.0, 900.0);
    let _ = f.app.update(Message::Window(WindowMsg::WindowResizedFor(
        f.window, 600.0, 500.0,
    )));
    assert_eq!(f.app.ui_state.size_of_window(f.window), (600.0, 500.0));
    assert_eq!(f.app.ui_state.size_of_window(f.main), (1400.0, 900.0));
    assert_eq!(f.app.ui_state.window_size, (1400.0, 900.0));
    let _ = f
        .app
        .update(Message::Window(WindowMsg::ReattachTab(f.window)));
    assert_eq!(f.app.ui_state.size_of_window(f.window), (1400.0, 900.0));
}

#[test]
fn smart_paste_from_an_undocked_windows_menu_lands_on_its_sheet() {
    let mut f = fixture();
    let _ = f
        .app
        .update(scoped_menu(f.window, ContextMenuMsg::Show(5.0, 5.0)));
    let _ = f.app.update(scoped_menu(
        f.window,
        ContextMenuMsg::Action(command_row("copy")),
    ));
    let (a_before, b_before) = (wire_ends(&f.app, &f.a), wire_ends(&f.app, &f.b));
    // The clipboard read's follow-up comes back untagged, with the main
    // window on sheet B.
    let request = PasteRequest {
        intent: PasteIntent::SmartPaste,
        epoch: f.app.interaction_state.placement_epoch,
        path: Some(f.a.clone()),
        dialog: 0,
    };
    let _ = f.app.update(Message::Edit(EditMsg::PasteTextRead {
        request,
        text: None,
    }));
    assert_eq!(wire_ends(&f.app, &f.a).len(), a_before.len() + 1);
    assert_eq!(wire_ends(&f.app, &f.b), b_before, "B is untouched");
    assert_eq!(
        f.app.document_state.active_path.as_deref(),
        Some(f.b.as_path())
    );
}

#[test]
fn a_paste_follow_up_for_a_window_without_a_canvas_is_cancelled_not_applied() {
    let mut f = fixture();
    let _ = f
        .app
        .update(scoped_menu(f.window, ContextMenuMsg::Show(5.0, 5.0)));
    let _ = f.app.update(scoped_menu(
        f.window,
        ContextMenuMsg::Action(command_row("copy")),
    ));
    f.app.interaction_state.canvases.remove(&f.window);
    let (a_before, b_before) = (wire_ends(&f.app, &f.a), wire_ends(&f.app, &f.b));
    let request = PasteRequest {
        intent: PasteIntent::SmartPaste,
        epoch: f.app.interaction_state.placement_epoch,
        path: Some(f.a.clone()),
        dialog: 0,
    };
    {
        use crate::ignore::IgnoreResult;
        crate::diagnostics::init_logging()
            .ignore("another test in this binary already installed the same logger");
    }
    // Only entries newer than this one count, so older lines cannot satisfy it.
    let newest = crate::diagnostics::newest_entry_id();
    let _ = f.app.update(Message::Edit(EditMsg::PasteTextRead {
        request,
        text: None,
    }));
    assert_eq!(wire_ends(&f.app, &f.a), a_before);
    assert_eq!(wire_ends(&f.app, &f.b), b_before);
    let said = crate::diagnostics::recent_entries()
        .iter()
        .filter(|e| e.id > newest.unwrap_or(0) && e.message.contains("Paste cancelled"))
        .count();
    // `>=`: other tests log the same text when threads share the ring.
    assert!(said >= 1, "the cancel never reached the Messages panel");
}

#[test]
fn a_paste_numbering_read_for_an_undocked_window_is_kept_for_its_sheet() {
    let mut f = fixture();
    let request = PasteRequest {
        intent: PasteIntent::Paste,
        epoch: f.app.interaction_state.placement_epoch,
        path: Some(f.a.clone()),
        dialog: 0,
    };
    let _ = f.app.update(Message::Edit(EditMsg::PasteCountersSeeded {
        request,
        seed: std::sync::Arc::new(Default::default()),
    }));
    let kept = f.app.interaction_state.paste_counters.as_ref();
    assert_eq!(kept.map(|c| &c.target), Some(&f.a));
}

#[test]
fn change_component_confirmed_for_an_undocked_windows_sheet_changes_that_sheet() {
    use crate::library::messages::{
        LibraryMessage, PlacedPartMsg, PlacedPartTarget, ResolvedPlacement,
    };
    use crate::library::state::{ChangePreview, PickerMode};
    use signex_engine::ComponentReplacement;

    let mut f = fixture();
    let doc = f.app.document_state.engines[&f.a].document().clone();
    let symbol = doc
        .symbols
        .iter()
        .find(|s| !s.is_power)
        .expect("a part")
        .clone();
    let mut lib_symbol = doc.body_of(&symbol).expect("body").clone();
    lib_symbol.id = "L:NEW".to_string();
    let resolved = ResolvedPlacement {
        lib_id: "L:NEW".to_string(),
        lib_symbol,
        library_id: uuid::Uuid::from_u128(7),
        row_id: uuid::Uuid::from_u128(8),
        library_version: "1".to_string(),
        stamp: signex_types::schematic::RowStamp::new(uuid::Uuid::from_u128(8)),
        internal_pn: "NEW-1".to_string(),
        value: "NEWVALUE".to_string(),
        is_power: false,
        prefix: "R".to_string(),
        class: "generic".to_string(),
        prefix_override: None,
        footprint: String::new(),
        datasheet: String::new(),
        fields: Vec::new(),
    };
    let target = PlacedPartTarget {
        sheet: f.a.clone(),
        engine_instance: f.app.document_state.engines[&f.a].instance_id(),
        symbol_id: symbol.uuid,
    };
    let replacement = ComponentReplacement {
        lib_symbol: resolved.lib_symbol.clone(),
        library_id: Some(resolved.library_id),
        row_id: Some(resolved.row_id),
        library_version: resolved.library_version.clone(),
        stamp: Some(resolved.stamp.clone()),
        value: resolved.value.clone(),
        footprint: String::new(),
        datasheet: String::new(),
        fields: Vec::new(),
    };
    let preview = signex_engine::change_component_preview(&doc, symbol.uuid, &replacement);
    f.app.library.picker = Some(crate::library::PickerState {
        mode: PickerMode::ChangePreview(Box::new(ChangePreview {
            target,
            reference: symbol.reference.clone(),
            resolved,
            preview,
        })),
        ..Default::default()
    });
    let values = |app: &Signex, path: &Path| {
        app.document_state.engines[path]
            .document()
            .symbols
            .iter()
            .map(|s| s.value.clone())
            .collect::<Vec<_>>()
    };
    let b_before = values(&f.app, &f.b);
    let _ = f.app.update(Message::Library(LibraryMessage::PlacedPart(
        PlacedPartMsg::ChangeConfirmed,
    )));
    assert!(
        values(&f.app, &f.a).iter().any(|v| v == "NEWVALUE"),
        "A's part changed"
    );
    assert_eq!(values(&f.app, &f.b), b_before, "B is untouched");
    assert!(f.app.library.picker.is_none());
    assert_eq!(
        f.app.document_state.active_path.as_deref(),
        Some(f.b.as_path())
    );
}

#[test]
fn the_view_tags_overlay_and_canvas_messages_with_their_window() {
    use crate::app::view::overlay_id::{scope_canvas_message, scope_overlay_message};
    let window = iced::window::Id::unique();
    let bar = scope_overlay_message(window, Message::ActiveBar(ActiveBarMsg::CloseMenus));
    assert!(matches!(
        bar,
        Message::ActiveBar(ActiveBarMsg::ScopedToWindow(w, _)) if w == window
    ));
    let menu = scope_overlay_message(window, Message::ContextMenu(ContextMenuMsg::Close));
    assert!(matches!(
        menu,
        Message::ContextMenu(ContextMenuMsg::ScopedToWindow(w, _)) if w == window
    ));
    let passed = scope_overlay_message(window, Message::MoveSelection(MoveSelectionMsg::Close));
    assert!(matches!(passed, Message::MoveSelection(_)));
    let show = scope_canvas_message(window, Message::ContextMenu(ContextMenuMsg::Show(1.0, 2.0)));
    assert!(matches!(
        show,
        Message::ContextMenu(ContextMenuMsg::ScopedToWindow(w, _)) if w == window
    ));
    let bar_press = scope_canvas_message(
        window,
        Message::ActiveBar(ActiveBarMsg::ToggleMenu(ActiveBarMenu::Align)),
    );
    assert!(matches!(
        bar_press,
        Message::ActiveBar(ActiveBarMsg::ScopedToWindow(w, _)) if w == window
    ));
    let other = scope_canvas_message(window, Message::ContextMenu(ContextMenuMsg::Close));
    assert!(matches!(other, Message::ContextMenu(ContextMenuMsg::Close)));
}

#[test]
fn a_newly_undocked_window_has_its_opening_size_before_any_resize_event() {
    let mut f = fixture();
    f.app.ui_state.window_size = (1000.0, 700.0);
    f.app.ui_state.windows.remove(&f.window);
    f.app.ui_state.forget_undocked_window_size(f.window);
    let _ = f.app.update(Message::Window(WindowMsg::UndockTab(1)));
    let opened = f
        .app
        .ui_state
        .windows
        .iter()
        .find_map(|(id, kind)| {
            matches!(kind, WindowKind::UndockedTab { path, .. } if *path == f.b).then_some(*id)
        })
        .expect("the tab undocked");
    assert_eq!(f.app.ui_state.size_of_window(opened), (1400.0, 900.0));
}

#[test]
fn a_right_press_in_an_undocked_window_hits_the_button_drawn_for_its_width() {
    use iced::widget::canvas::Program as _;
    use iced::{Rectangle, mouse};
    let mut f = fixture();
    f.app.ui_state.window_size = (1400.0, 900.0);
    f.app
        .ui_state
        .set_undocked_window_size(f.window, (600.0, 500.0));
    let tool = crate::app::Tool::Select;
    let slot = crate::canvas::CanvasSlot::new();
    let mut prefs = f.app.canvas_view_prefs(f.window);
    prefs.current_tool = tool;
    assert_eq!(prefs.window_width, 600.0);
    let view = crate::canvas::SchematicCanvas::new(&slot, prefs);
    let bounds = Rectangle {
        x: 0.0,
        y: 60.0,
        width: 600.0,
        height: 600.0,
    };
    for (menu, centre) in crate::active_bar::drawn_trigger_centres(tool, 600.0) {
        let event = iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right));
        let at = iced::Point::new(centre, bounds.y + 20.0);
        let action = view
            .update(
                &mut crate::canvas::CanvasState::default(),
                &event,
                bounds,
                mouse::Cursor::Available(at),
            )
            .expect("an action");
        let (message, _, _) = action.into_inner();
        assert!(
            matches!(
                message,
                Some(Message::ActiveBar(ActiveBarMsg::ToggleMenu(m))) if m == menu
            ),
            "drawn {menu:?} button centre x={centre} resolved to {message:?}"
        );
    }
}
