//! Mockup row 33: the Libraries location is cut from the start so the
//! file name stays, and each mount state has its pill tone.

use super::view::LOCATION_CHARS;
use super::{LibraryMount, path_tail};
use crate::styles::PillTone;

/// A path that fits is shown whole; a longer one keeps its tail, file name
/// included, behind one `…`, in at most the given number of characters.
#[test]
fn path_tail_keeps_the_file_name_of_a_long_path() {
    assert_eq!(path_tail("./dcdc-buck.snxlib", 40), "./dcdc-buck.snxlib");
    let shared = r"\\nas\eda\shared\team-passives.snxlib";
    assert_eq!(path_tail(shared, shared.chars().count()), shared);
    let cut = path_tail(shared, 24);
    assert_eq!(cut, r"…ed\team-passives.snxlib");
    assert_eq!(cut.chars().count(), 24);
    assert!(cut.starts_with('…'));
    assert!(cut.ends_with("team-passives.snxlib"));
}

/// The cut counts characters, not bytes, so a path with non-ASCII folder
/// names is never split inside a character.
#[test]
fn path_tail_counts_characters() {
    let path = "C:/Kütüphaneler/Ortak/güç.snxlib";
    let cut = path_tail(path, 12);
    assert_eq!(cut, "…/güç.snxlib");
    assert_eq!(cut.chars().count(), 12);
    assert_eq!(path_tail(path, 0), "…");
    let long = format!("{}/x.snxlib", "d".repeat(LOCATION_CHARS * 2));
    assert_eq!(
        path_tail(&long, LOCATION_CHARS).chars().count(),
        LOCATION_CHARS
    );
}

/// Mounted is ok, Missing is an error, read-only and not mounted are
/// neutral.
#[test]
fn each_mount_state_has_its_pill_tone() {
    assert_eq!(LibraryMount::Mounted.tone(), PillTone::Ok);
    assert_eq!(LibraryMount::ReadOnly.tone(), PillTone::Neutral);
    assert_eq!(LibraryMount::Missing.tone(), PillTone::Err);
    assert_eq!(LibraryMount::NotMounted.tone(), PillTone::Neutral);
}

/// #3122: Import library… in a project's Libraries section opens the
/// Import Library dialog with "This project" preselected, meaning this
/// project; an open dialog stays as it is.
#[test]
fn import_library_opens_the_dialog_for_this_project() {
    use crate::app::handlers::menu::export::tests::app_workspace;
    use crate::app::project_tab::{ProjectSection, ProjectTabMsg};
    use crate::app::{Message, ProjectMsg};
    use crate::library::import_dialog::ImportDialogMsg;
    use crate::library::messages::{LibraryImportMsg, LibraryMessage};
    const WINDOW: iced::Size = iced::Size::new(1200.0, 800.0);
    // In memory only: the workspace writes nothing to this folder.
    let dir = std::env::temp_dir().join(format!("signex-prj-import-{}", uuid::Uuid::now_v7()));
    let mut app = app_workspace(&dir.to_string_lossy(), &["a.snxsch"]);
    app.open_project_tab(0);
    let id = app.document_state.projects[0].id;
    drop(app.update(Message::Project(ProjectMsg::Tab(
        ProjectTabMsg::SectionSelected {
            project_id: id,
            section: ProjectSection::Libraries,
        },
    ))));
    let texts = crate::layout_probe::lay_out_texts(app.view_project_tab(id), WINDOW);
    let at = crate::layout_probe::find(&texts, "Import library…")
        .bounds
        .center();
    let click = [
        iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)),
        iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
            iced::mouse::Button::Left,
        )),
    ];
    let messages =
        crate::layout_probe::messages_for_events(app.view_project_tab(id), WINDOW, at, &click);
    assert!(
        messages.iter().any(|m| matches!(
            m,
            Message::Library(LibraryMessage::Import(LibraryImportMsg::Dialog(
                ImportDialogMsg::RequestedForProject(p)
            ))) if *p == id
        )),
        "{messages:?}"
    );
    for message in messages {
        drop(app.update(message));
    }
    let state = app.library.import_dialog.as_deref().expect("open");
    assert_eq!(state.project, Some(id));
    assert!(!state.mount_for_all, "This project is preselected");
    // A second request leaves the open dialog as it is.
    drop(
        app.update(Message::Library(crate::library::import_dialog::msg(
            ImportDialogMsg::Requested,
        ))),
    );
    let state = app.library.import_dialog.as_deref().unwrap();
    assert!(!state.mount_for_all && state.project == Some(id));
}

/// #2994, mockup row 33: at the default tab width a typical library name
/// shows whole in the Name column, while a long location is still cut from
/// its start. Driven through `Signex::update` and the laid-out tab.
#[test]
fn a_typical_name_shows_whole_and_the_location_is_cut() {
    use std::path::PathBuf;

    use iced::Widget as _;
    use signex_i18n::UserText;
    use signex_types::project::{LibraryEntry, LibraryEntryKind};
    use signex_widgets::text;

    use super::super::look::mono_font;
    use super::super::table::TS;
    use crate::app::handlers::menu::export::tests::app_workspace;
    use crate::app::{Message, ProjectMsg, ProjectSection, ProjectTabMsg};

    /// The Project tab's area in the audit's 1440 × 900 window.
    const TAB: iced::Size = iced::Size::new(888.0, 700.0);
    const NAME: &str = "team-passives";
    const STORED: &str = "../../shared/eda/libraries/team-passives.snxlib";
    let dir = std::env::temp_dir().join(format!("signex-prj-libs-{}", uuid::Uuid::now_v7()));
    let mut app = app_workspace(&dir.to_string_lossy(), &["a.snxsch"]);
    app.document_state.projects[0].data.libraries = vec![LibraryEntry {
        path: PathBuf::from(STORED),
        kind: LibraryEntryKind::Shared,
        library_id: None,
        unknown: Default::default(),
    }];
    app.open_project_tab(0);
    let id = app.document_state.projects[0].id;
    let _ = app.update(Message::Project(ProjectMsg::Tab(
        ProjectTabMsg::SectionSelected {
            project_id: id,
            section: ProjectSection::Libraries,
        },
    )));
    let texts = crate::layout_probe::lay_out_texts(app.view_project_tab(id), TAB);
    let name = crate::layout_probe::find(&texts, NAME).bounds;
    let whole = crate::layout_probe::natural::<Message>(
        text(UserText::new(NAME.to_string())).size(TS.body).boxed(),
        NAME,
    );
    assert!(
        name.width + 0.5 >= whole.width,
        "the name is cut: {name:?} vs {whole:?}"
    );
    let location = crate::layout_probe::find(&texts, STORED).bounds;
    let full = crate::layout_probe::natural::<Message>(
        text(UserText::new(STORED.to_string()))
            .size(TS.body)
            .font(mono_font())
            .boxed(),
        STORED,
    );
    assert!(
        location.width + 0.5 < full.width,
        "the location must still be cut from its start: {location:?} vs {full:?}"
    );
    assert!(
        name.x + name.width <= location.x,
        "{name:?} runs into {location:?}"
    );
}
