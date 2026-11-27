//! Mockup row 91: the text view's bar, and what its Constraint sets and
//! Presets may do while the text holds typed text not applied.

use iced::Size;
use iced::widget::text_editor::{Action, Edit};

use super::matrix_tests::{app_with_board, dsl, send};
use super::{RulesPanel, RulesTabMsg, SetFormField, TEXT_UNAPPLIED};
use crate::app::state::ProjectId;
use crate::app::{Message, ProjectMsg, Signex};
use crate::layout_probe::{lay_out_texts, messages_on_click};

fn window() -> Size {
    Size::new(1200.0, 800.0)
}

fn text_view(app: &mut Signex, id: ProjectId, on: bool) {
    send(app, RulesTabMsg::TextViewToggled { project_id: id, on });
}

fn text_view_on(app: &mut Signex, id: ProjectId) {
    text_view(app, id, true);
}

fn type_hash(app: &mut Signex, id: ProjectId) {
    send(
        app,
        RulesTabMsg::TextActed {
            project_id: id,
            action: Action::Edit(Edit::Insert('#')),
        },
    );
}

fn shown(app: &Signex, id: ProjectId) -> Vec<String> {
    lay_out_texts(app.view_rules_tab(id), window())
        .into_iter()
        .map(|t| t.text)
        .collect()
}

fn refused(app: &Signex, id: ProjectId) -> Option<String> {
    app.document_view.project_tabs[&id].rules.refused.clone()
}

fn typed_text_kept(app: &Signex, id: ProjectId) -> bool {
    app.document_view.project_tabs[&id]
        .rules
        .text
        .as_ref()
        .is_some_and(|text| text.edited && text.content.text().starts_with('#'))
}

#[test]
fn the_text_view_bar_names_all_domains_and_keeps_sets_presets_and_run_drc() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_board(&dir);
    // Off first: the message gives the project its tab state.
    text_view(&mut app, id, false);
    assert!(shown(&app, id).iter().any(|t| t == "Physical"));
    text_view_on(&mut app, id);
    let bar = shown(&app, id);
    for label in [
        "All domains",
        "Text view",
        "Table",
        "Text",
        "Constraint sets",
        "Presets",
        "Run DRC",
    ] {
        assert!(bar.iter().any(|t| t == label), "{label}: {bar:?}");
    }
    assert!(!bar.iter().any(|t| t == "Physical"), "{bar:?}");
}

#[test]
fn the_text_view_bar_opens_constraint_sets_and_presets() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_board(&dir);
    text_view_on(&mut app, id);
    let sets = messages_on_click(app.view_rules_tab(id), window(), "Constraint sets");
    assert!(
        matches!(
            sets.as_slice(),
            [Message::Project(ProjectMsg::Rules(
                RulesTabMsg::SetsToggled { .. }
            ))]
        ),
        "{sets:?}"
    );
    let presets = messages_on_click(app.view_rules_tab(id), window(), "Presets");
    assert!(
        matches!(
            presets.as_slice(),
            [Message::Project(ProjectMsg::Rules(
                RulesTabMsg::PanelPicked {
                    panel: Some(RulesPanel::Presets),
                    ..
                }
            ))]
        ),
        "{presets:?}"
    );
    send(&mut app, RulesTabMsg::SetsToggled { project_id: id });
    let open = shown(&app, id);
    assert!(open.iter().any(|t| t == "Create set"), "{open:?}");
    assert!(open.iter().any(|t| t == "All domains"), "{open:?}");
}

#[test]
fn a_set_waits_while_the_text_view_holds_typed_text() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_board(&dir);
    assert!(
        app.create_rule_set(id, "HV_08", "min_clearance=0.8mm", None)
            .is_ok()
    );
    for (field, value) in [
        (SetFormField::Values, "min_clearance=0.9mm"),
        (SetFormField::Scope, "class == \"HV\""),
    ] {
        send(
            &mut app,
            RulesTabMsg::SetFormEdited {
                project_id: id,
                field,
                text: value.into(),
            },
        );
    }
    let stored = dsl(&app);
    text_view_on(&mut app, id);
    type_hash(&mut app, id);
    for msg in [
        RulesTabMsg::SetApplied {
            project_id: id,
            set: "HV_08".into(),
        },
        RulesTabMsg::SetRedefined {
            project_id: id,
            set: "HV_08".into(),
        },
    ] {
        send(&mut app, msg);
        assert_eq!(dsl(&app), stored, "the stored rules wait");
        assert_eq!(refused(&app, id).as_deref(), Some(TEXT_UNAPPLIED));
        assert!(typed_text_kept(&app, id));
    }
    send(&mut app, RulesTabMsg::TextDiscarded { project_id: id });
    send(
        &mut app,
        RulesTabMsg::SetApplied {
            project_id: id,
            set: "HV_08".into(),
        },
    );
    assert!(
        dsl(&app).contains("rule HV_08_HV_clearance"),
        "{}",
        dsl(&app)
    );
}

#[test]
fn a_preset_waits_while_the_text_view_holds_typed_text() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_board(&dir);
    send(
        &mut app,
        RulesTabMsg::PresetPicked {
            project_id: id,
            preset: "tier:standard".into(),
        },
    );
    let stored = dsl(&app);
    text_view_on(&mut app, id);
    type_hash(&mut app, id);
    send(&mut app, RulesTabMsg::PresetApplied { project_id: id });
    assert_eq!(dsl(&app), stored, "the stored rules wait");
    assert_eq!(refused(&app, id).as_deref(), Some(TEXT_UNAPPLIED));
    assert!(typed_text_kept(&app, id));
    assert!(app.document_view.project_tabs[&id].rules.preview.is_some());

    send(&mut app, RulesTabMsg::TextDiscarded { project_id: id });
    send(&mut app, RulesTabMsg::PresetApplied { project_id: id });
    assert!(dsl(&app).contains("rule fab_via"), "{}", dsl(&app));
    assert!(app.document_view.project_tabs[&id].rules.preview.is_none());
}
