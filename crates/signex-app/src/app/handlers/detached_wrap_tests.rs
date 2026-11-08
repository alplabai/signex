//! A fixed-height dialog window grows with the text it shows: user data (a
//! library name, a part number, a table, a component label) that wraps must
//! not push the footer out of the window (docs/DETACHED_MODALS.md).

use std::path::PathBuf;

use signex_types::theme::ThemeId;

use crate::app::state::{ModalId, Signex, WindowKind};
use crate::layout_probe::lay_out_texts;

const LONG: &str = "Acme precision passives, connectors and electromechanical parts \
                    production library for the northern region";

fn app_on(theme_id: ThemeId) -> Signex {
    let (mut app, _task) = Signex::new();
    let main = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(main);
    app.ui_state.main_window_opened = true;
    app.ui_state.first_run_tour_open = false;
    app.ui_state.theme_id = theme_id;
    app.document_state.panel_ctx.tokens = app.tokens_for(theme_id);
    app
}

/// Every text of the dialog's window is inside it, the footer button labels
/// included, and the footer is there at all.
fn assert_footer_inside(app: &Signex, id: iced::window::Id, modal: ModalId, what: &str) {
    let size = app.dialog_window_size_of(
        modal,
        app.ui_state
            .library_modal_owners
            .get(&id)
            .map(|p| p.as_path()),
        app.dialog_window_is_borderless(id),
    );
    let texts = lay_out_texts(app.view(id), size);
    assert!(
        texts.iter().any(|t| t.text == "Cancel"),
        "{what}: no Cancel label"
    );
    let bottom = texts
        .iter()
        .map(|t| t.bounds.y + t.bounds.height)
        .fold(0.0_f32, f32::max);
    assert!(
        bottom + 4.0 <= size.height,
        "{what}: content reaches {bottom}, window is {}",
        size.height
    );
}

fn window_of(app: &Signex, modal: ModalId) -> iced::window::Id {
    app.ui_state
        .windows
        .iter()
        .find_map(|(id, kind)| {
            matches!(kind, WindowKind::DetachedModal(m) if *m == modal).then_some(*id)
        })
        .expect("the dialog's window")
}

#[test]
fn a_long_library_name_keeps_the_close_prompt_footer_in_its_window() {
    for &theme_id in ThemeId::BUILTINS {
        let mut app = app_on(theme_id);
        app.library.close_library_confirm = Some(crate::library::state::CloseLibraryConfirmState {
            library_path: "l.snxlib".into(),
            library_name: LONG.repeat(2),
            dirty_editors: Vec::new(),
            dirty_paths: vec!["l.snxlib".into()],
        });
        let _ = app.sync_detached_modals();
        let id = window_of(&app, ModalId::LibraryCloseConfirm);
        assert_footer_inside(
            &app,
            id,
            ModalId::LibraryCloseConfirm,
            &format!("{theme_id:?}"),
        );
    }
}

#[test]
fn a_long_part_number_and_table_keep_the_delete_confirm_footer_in_its_window() {
    for &theme_id in ThemeId::BUILTINS {
        let mut app = app_on(theme_id);
        let path = PathBuf::from("/libs/a.snxlib");
        let mut browser = crate::library::state::LibraryBrowserState::new(path.clone());
        browser.delete_confirm = Some(crate::library::state::DeleteConfirmState {
            table: LONG.into(),
            row_id: signex_library::RowId::new(),
            internal_pn: LONG.into(),
        });
        app.library.library_browsers.insert(path.clone(), browser);
        let _ = app.sync_detached_modals();
        let id = app
            .browser_modal_window(ModalId::LibraryDeleteConfirm, &path)
            .expect("a window bound to the browser");
        assert_footer_inside(
            &app,
            id,
            ModalId::LibraryDeleteConfirm,
            &format!("{theme_id:?}"),
        );
    }
}

#[test]
fn a_short_name_leaves_the_close_and_delete_windows_at_their_base_height() {
    let mut app = app_on(ThemeId::GraphiteLight);
    let base = |app: &Signex, modal| app.modal_window_size_of(modal, None).height;
    let before = base(&app, ModalId::LibraryCloseConfirm);
    app.library.close_library_confirm = Some(crate::library::state::CloseLibraryConfirmState {
        library_path: "l.snxlib".into(),
        library_name: "l".into(),
        dirty_editors: Vec::new(),
        dirty_paths: vec!["l.snxlib".into()],
    });
    assert_eq!(base(&app, ModalId::LibraryCloseConfirm), before);
}

#[test]
fn a_long_component_label_keeps_the_copy_to_library_footer_in_its_window() {
    for &theme_id in ThemeId::BUILTINS {
        let mut app = app_on(theme_id);
        app.library.stock_copy = Some(crate::library::stock_copy::StockCopyState {
            source: "/stock/symbols/R.snxsym".into(),
            kind: crate::library::stock_copy::StockPrimitiveKind::Symbol,
            label: LONG.repeat(2),
            previous_unavailable: None,
            error: None,
        });
        let _ = app.sync_detached_modals();
        let id = window_of(&app, ModalId::LibraryStockCopy);
        assert_footer_inside(
            &app,
            id,
            ModalId::LibraryStockCopy,
            &format!("{theme_id:?}"),
        );
    }
}
