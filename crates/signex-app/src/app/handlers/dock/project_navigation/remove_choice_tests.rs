//! The Remove modal's radio cards (#2262): which card is selected, what the
//! footer button says, that clicking a card removes nothing, and that Enter
//! does nothing there (Close Project's Enter is tested here too).

use std::path::Path;

use signex_i18n::{I18n, ids};

use crate::app::state::{ModalId, WindowKind};
use crate::app::view::dialogs::remove::content::{RemoveNote, RemoveOffer, footer_action};
use crate::app::{Message, ProjectCloseChoice, ProjectMsg, RemoveChoice, RemoveDialogState};
use crate::app::{RemoveMsg, Signex};
use crate::styles::DialogButtonKind;

const EXCLUDE: RemoveChoice = RemoveChoice::ExcludeFromProject;
const DELETE: RemoveChoice = RemoveChoice::DeleteFile;

fn offer(exclude: bool, delete: bool) -> RemoveOffer {
    RemoveOffer { exclude, delete }
}

fn quiet_app() -> Signex {
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app
}

fn dialog(target: &Path, outside: bool) -> RemoveDialogState {
    RemoveDialogState {
        target_path: target.to_path_buf(),
        tree_path: vec![0],
        project_id: None,
        display_name: target
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        is_outside_project: outside,
        unsaved_prompt: false,
        is_directory_package: false,
        choice: None,
    }
}

fn enter() -> iced::keyboard::Event {
    iced::keyboard::Event::KeyPressed {
        key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter),
        modified_key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter),
        physical_key: iced::keyboard::key::Physical::Unidentified(
            iced::keyboard::key::NativeCode::Unidentified,
        ),
        location: iced::keyboard::Location::Standard,
        modifiers: iced::keyboard::Modifiers::empty(),
        text: None,
        repeat: false,
    }
}

#[test]
fn exclude_is_selected_until_another_card_is_picked() {
    let both = offer(true, true);
    assert_eq!(both.selected(None), Some(EXCLUDE));
    assert_eq!(both.selected(Some(DELETE)), Some(DELETE));
    assert_eq!(both.selected(Some(EXCLUDE)), Some(EXCLUDE));
}

#[test]
fn a_lone_exclude_is_selected_but_delete_is_never_selected_unpicked() {
    assert_eq!(offer(true, false).selected(None), Some(EXCLUDE));
    assert_eq!(offer(true, false).selected(Some(DELETE)), Some(EXCLUDE));
    assert_eq!(offer(false, true).selected(None), None);
    assert_eq!(offer(false, true).selected(Some(EXCLUDE)), None);
    assert_eq!(offer(false, true).selected(Some(DELETE)), Some(DELETE));
    assert_eq!(offer(false, false).selected(Some(DELETE)), None);
}

/// A dialog that opens with both cards selects Exclude. When Exclude stops
/// being offered while it is open (the tree is rebuilt and the sheet is no
/// longer a page), the footer offers no Delete until the user picks it, so
/// a click aimed at Exclude can never delete the file.
#[test]
fn exclude_withdrawn_after_open_leaves_no_footer_action_until_delete_is_picked() {
    let mut st = dialog(Path::new("p.snxsch"), false);
    let at_open = offer(true, true);
    assert_eq!(at_open.selected(st.choice), Some(EXCLUDE));
    assert_eq!(
        at_open.selected(st.choice).map(footer_action),
        Some(footer_action(EXCLUDE))
    );

    let flipped = offer(false, true);
    assert_eq!(
        flipped.selected(st.choice),
        None,
        "no Delete without a pick"
    );

    let mut app = quiet_app();
    app.ui_state.remove_dialog = Some(st.clone());
    let _ = app.update(Message::Remove(RemoveMsg::ChoicePicked(DELETE)));
    st = app
        .ui_state
        .remove_dialog
        .clone()
        .expect("the dialog stays open");
    assert_eq!(flipped.selected(st.choice), Some(DELETE));
    assert_eq!(
        flipped.selected(st.choice).map(footer_action),
        Some(footer_action(DELETE))
    );
}

#[test]
fn the_footer_button_follows_the_selected_card() {
    let en = I18n::en_us_ref();
    let (label, kind) = footer_action(EXCLUDE);
    assert_eq!(kind, DialogButtonKind::Primary);
    assert_eq!(en.message_plain(label, &[]), "Exclude");
    let (label, kind) = footer_action(DELETE);
    assert_eq!(kind, DialogButtonKind::DangerSolid);
    assert_eq!(en.message_plain(label, &[]), "Delete file");
}

/// A sheet nobody else places: Exclude first, then Delete, Exclude selected,
/// each card saying what it does.
#[test]
fn a_sheet_offers_exclude_then_delete_with_exclude_selected() {
    let app = quiet_app();
    let en = I18n::en_us_ref();
    let mut st = dialog(Path::new("p.snxsch"), false);
    let content = app.remove_content(&st, en);
    let cards: Vec<_> = content.cards.iter().map(|c| c.choice).collect();
    assert_eq!(cards, [EXCLUDE, DELETE]);
    assert_eq!(content.selected, Some(EXCLUDE));
    assert_eq!(
        content.footer_action(),
        Some((ids::DIALOG_REMOVE_ACTION_EXCLUDE, DialogButtonKind::Primary))
    );
    let said: Vec<String> = content
        .cards
        .iter()
        .map(|c| c.description.to_string())
        .collect();
    assert_eq!(
        said,
        [
            "The file stays on disk, but the project no longer lists it.",
            "The file is removed from the project and permanently deleted.",
        ]
    );
    assert!(content.notes.is_empty());

    st.choice = Some(DELETE);
    let content = app.remove_content(&st, en);
    assert_eq!(content.selected, Some(DELETE));
    assert_eq!(
        content.footer_action(),
        Some((
            ids::DIALOG_REMOVE_ACTION_DELETE,
            DialogButtonKind::DangerSolid
        ))
    );
}

/// A shared or outside library is never deleted (#139): its one card,
/// Exclude, is shown already selected, whatever was picked before; its path
/// is named in a note.
#[test]
fn a_library_outside_the_project_offers_only_exclude_already_selected() {
    let app = quiet_app();
    let mut st = dialog(Path::new("/shared/Stock.snxlib"), true);
    st.choice = Some(DELETE);
    let content = app.remove_content(&st, I18n::en_us_ref());
    let cards: Vec<_> = content.cards.iter().map(|c| c.choice).collect();
    assert_eq!(cards, [EXCLUDE]);
    assert_eq!(content.selected, Some(EXCLUDE));
    assert_eq!(
        content.cards[0].description.to_string(),
        "Library is shared or lives outside this project's folder. It will be excluded \
         from the project and is not deleted from disk."
    );
    assert!(
        matches!(&content.notes[..], [RemoveNote::Library { path, .. }] if path.ends_with("Stock.snxlib")),
        "{:?}",
        content.notes
    );
}

/// In a project folder that is a Git repository the Exclude card says the
/// file stays in Git too; until the probe has seen `.git`, the plain wording.
#[test]
fn the_exclude_card_names_git_only_when_the_project_folder_has_it() {
    let mut app = crate::app::handlers::menu::export::tests::app_workspace("/w", &["top.snxsch"]);
    let st = dialog(Path::new("/w/top.snxsch"), false);
    let exclude = |app: &Signex| {
        app.remove_content(&st, I18n::en_us_ref())
            .cards
            .iter()
            .find(|c| c.choice == EXCLUDE)
            .map(|c| c.description.to_string())
    };
    assert_eq!(
        exclude(&app).as_deref(),
        Some("The file stays on disk, but the project no longer lists it.")
    );
    let marker = crate::app::state::project_fs::git_marker(app.document_state.projects[0].dir());
    app.document_state.project_fs.facts.set_exists(marker, true);
    assert_eq!(
        exclude(&app).as_deref(),
        Some("The file stays on disk and in Git, but the project no longer lists it.")
    );
}

/// Clicking a card only selects it: the dialog stays open, nothing is
/// deleted or excluded, and the message does not touch the disk.
#[test]
fn picking_a_card_selects_it_and_removes_nothing() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let file = tmp.path().join("p.snxsch");
    std::fs::write(&file, "").expect("write");
    let mut app = quiet_app();
    app.ui_state.remove_dialog = Some(dialog(&file, false));

    let _ = app.update(Message::Remove(RemoveMsg::ChoicePicked(DELETE)));

    let open = app
        .ui_state
        .remove_dialog
        .as_ref()
        .expect("the dialog stays open");
    assert_eq!(open.choice, Some(DELETE));
    assert!(file.exists(), "nothing is deleted");
    assert!(app.document_view.error_notice.is_none());
    assert!(!crate::app::state::project_fs::message_touches_disk(
        &Message::Remove(RemoveMsg::ChoicePicked(DELETE))
    ));
}

/// Enter does nothing in the Remove modal, as before the radio cards: only
/// the footer button removes or deletes.
#[test]
fn enter_in_the_remove_dialog_does_nothing() {
    let mut app = quiet_app();
    let main = app.ui_state.main_window_id.expect("main window");
    app.ui_state.remove_dialog = Some(dialog(Path::new("p.snxsch"), false));
    assert!(app.route_key_for_test(main, &enter()).is_none());

    app.ui_state.remove_dialog.as_mut().expect("open").choice = Some(DELETE);
    assert!(app.route_key_for_test(main, &enter()).is_none());

    app.ui_state
        .remove_dialog
        .as_mut()
        .expect("open")
        .unsaved_prompt = true;
    assert!(
        app.route_key_for_test(main, &enter()).is_none(),
        "the unsaved-edits stage has no Enter"
    );
}

/// In the Remove dialog's own window Enter does nothing either.
#[test]
fn enter_in_the_detached_remove_window_does_nothing() {
    let mut app = quiet_app();
    app.ui_state.remove_dialog = Some(dialog(Path::new("p.snxsch"), false));
    let window = iced::window::Id::unique();
    app.ui_state
        .windows
        .insert(window, WindowKind::DetachedModal(ModalId::RemoveDialog));
    assert!(app.route_key_for_test(window, &enter()).is_none());
}

/// Enter in the Close Project confirm saves all (#2262).
#[test]
fn enter_in_the_close_project_confirm_saves_all() {
    let mut app = quiet_app();
    let main = app.ui_state.main_window_id.expect("main window");
    let project_id = app.document_state.mint_project_id();
    app.ui_state.project_close_confirm = Some(Box::new(crate::app::ProjectCloseConfirmState {
        project_id,
        project_name: "P".into(),
        dirty_paths: vec!["/p/a.snxsch".into()],
        dirty_rows: Vec::new(),
        kept_paths: Vec::new(),
        sheet_refs: Default::default(),
    }));
    assert!(matches!(
        app.route_key_for_test(main, &enter()),
        Some(Message::Project(ProjectMsg::CloseConfirm(
            ProjectCloseChoice::SaveAll
        )))
    ));
}
