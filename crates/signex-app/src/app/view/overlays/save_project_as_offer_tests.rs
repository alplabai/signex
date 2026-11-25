//! #2908 — the Save Failed card of a read-only project's refused save offers
//! Save Project As… for that project, and the dialog it opens paints.

use super::*;
use crate::app::SaveProjectAsMsg;
use crate::layout_probe::{lay_out_texts, messages_on_click};

const WINDOW: iced::Size = iced::Size::new(1400.0, 900.0);

fn guarded(mut layers: Vec<Element<'_, Message>>) -> Element<'_, Message> {
    assert_eq!(layers.len(), 1, "one layer: backdrop and card together");
    super::scroll_guard(layers.pop().expect("one layer"))
}

/// The workspace project, read as `snxprj/1.2`, after the Save of its
/// unsaved settings was refused. A Save of a project with nothing unsaved
/// writes nothing and is never refused, so the settings are marked unsaved.
fn refused_app() -> (Signex, crate::app::ProjectId) {
    let mut app = crate::app::handlers::menu::export::tests::app_workspace("/ro", &[]);
    app.ui_state.first_run_tour_open = false;
    app.ui_state.window_size = (WINDOW.width, WINDOW.height);
    let project = &mut app.document_state.projects[0];
    project.data.unknown.newer_minor_source = Some("snxprj/1.2".into());
    let (id, path) = (project.id, project.path.clone());
    app.document_state.mark_document_dirty(&path);
    let Err(error) = app.begin_project_save(id) else {
        panic!("a read-only project's save is refused");
    };
    app.report_save_failure("Failed to save project file", &error);
    (app, id)
}

#[test]
fn the_read_only_refusal_card_offers_save_project_as_for_its_project() {
    let (app, id) = refused_app();
    let messages = messages_on_click(
        guarded(app.error_notice_overlay()),
        WINDOW,
        "Save Project As…",
    );
    assert!(
        matches!(
            messages.as_slice(),
            [Message::Project(crate::app::ProjectMsg::SaveAs(
                SaveProjectAsMsg::Requested { project: Some(project) }
            ))] if *project == id
        ),
        "{messages:?}"
    );
}

#[test]
fn the_dialog_paints_the_copy_path_and_the_read_only_note() {
    let (mut app, id) = refused_app();
    let _ = app.update(Message::from(SaveProjectAsMsg::Requested {
        project: Some(id),
    }));
    let card = app.save_project_as_overlay().expect("the dialog paints");
    let texts = lay_out_texts(card, WINDOW);
    let plain: Vec<String> = texts
        .iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect();
    assert!(plain.iter().any(|t| t == "Save copy"), "{plain:#?}");
    assert!(
        plain
            .iter()
            .any(|t| t.contains("'ProjectA' was written by a newer") && t.contains("snxprj/1.2")),
        "{plain:#?}"
    );
    assert!(
        plain.iter().any(|t| t.ends_with("ProjectA copy.snxprj")),
        "the path the copy lands on: {plain:#?}"
    );
}

/// A Save that found keys the copy cannot keep: the dialog names them, and
/// its primary button agrees to go without them.
#[test]
fn the_dialog_names_the_keys_the_copy_cannot_keep_and_asks_before_writing() {
    let (mut app, id) = refused_app();
    let _ = app.update(Message::from(SaveProjectAsMsg::Requested {
        project: Some(id),
    }));
    let dialog = app.ui_state.save_project_as.as_deref_mut().expect("dialog");
    dialog.target = crate::app::new_project::TargetStatus::Clear;
    dialog.not_kept = Some(vec!["fab_outputs.laser_marking".into()]);
    let texts = lay_out_texts(app.save_project_as_overlay().expect("paints"), WINDOW);
    let plain: Vec<String> = texts
        .iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect();
    assert!(
        plain
            .iter()
            .any(|t| t.contains("fab_outputs.laser_marking")
                && t.contains("Nothing has been written")),
        "{plain:#?}"
    );
    assert!(!plain.iter().any(|t| t == "Save copy"), "{plain:#?}");
    let messages = messages_on_click(
        app.save_project_as_overlay().expect("paints"),
        WINDOW,
        "Save copy without them",
    );
    assert!(
        matches!(
            messages.as_slice(),
            [Message::Project(crate::app::ProjectMsg::SaveAs(
                SaveProjectAsMsg::KeysNotKeptAccepted
            ))]
        ),
        "{messages:?}"
    );
}
