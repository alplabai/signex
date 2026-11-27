//! The fab tier cards (#2537) and editable built-ins (#2536) in the New
//! Project dialog: the draft a pick and a card stepper make, the override an
//! edit stores, Reset to default, Rename, Duplicate, and the rollback of a
//! failed settings write. Tasks are dropped, so nothing is written.

use signex_types::project::{PcbField, SetupProfile};

use super::{NewProjectMsg, ProfileTarget, ProfileUndo};
use crate::app::{Message, Signex};
use crate::menu_bar::MenuMessage;

fn open() -> Signex {
    let (mut app, _t) = Signex::new();
    app.document_state.setup_profiles.clear();
    let _ = app.update(Message::Menu(MenuMessage::NewProject));
    app
}

fn send(app: &mut Signex, msg: NewProjectMsg) {
    let _ = app.update(Message::from(msg));
}

fn draft(app: &Signex) -> &super::NewProjectState {
    app.ui_state.new_project.as_deref().expect("the dialog")
}

#[test]
fn picking_a_tier_and_stepping_its_layers_sets_the_draft() {
    let mut app = open();
    send(
        &mut app,
        NewProjectMsg::ProfilePicked(SetupProfile::Advanced),
    );
    let setup = &draft(&app).setup;
    assert_eq!(
        (
            setup.profile,
            setup.pcb.copper_layers,
            setup.pcb.track_width_mm
        ),
        (SetupProfile::Advanced, 6, 0.125)
    );
    assert_eq!(draft(&app).drafts.track, "0.125", "the fields follow");

    // The HDI card's stepper picks HDI on 10 layers, rules kept.
    send(
        &mut app,
        NewProjectMsg::CardLayersStepped(SetupProfile::Hdi, 2),
    );
    let setup = &draft(&app).setup;
    assert_eq!(
        (
            setup.profile,
            setup.pcb.copper_layers,
            setup.pcb.clearance_mm
        ),
        (SetupProfile::Hdi, 10, 0.075)
    );
    // The Customize stepper moves the picked card's count, tier kept.
    send(&mut app, NewProjectMsg::LayersStepped(-2));
    let setup = &draft(&app).setup;
    assert_eq!(
        (setup.profile, setup.pcb.copper_layers),
        (SetupProfile::Hdi, 8)
    );
    // The Fab tier field mirrors the card: picking Standard there picks it.
    send(
        &mut app,
        NewProjectMsg::FabPicked(signex_types::project::FabTemplate::Standard),
    );
    let setup = &draft(&app).setup;
    assert_eq!(
        (
            setup.profile,
            setup.pcb.copper_layers,
            setup.pcb.clearance_mm
        ),
        (SetupProfile::Standard, 8, 0.15)
    );
    let _ = app.view_new_project_dialog();
}

#[test]
fn an_edited_built_in_shows_edited_and_reset_restores_the_tier_values() {
    let mut app = open();
    // Hovering shows the pencil; an unedited built-in's opens the editor.
    send(
        &mut app,
        NewProjectMsg::CardHovered {
            card: SetupProfile::Advanced,
            inside: true,
        },
    );
    assert_eq!(draft(&app).hovered_card, Some(SetupProfile::Advanced));
    let advanced = ProfileTarget::BuiltIn(SetupProfile::Advanced);
    send(
        &mut app,
        NewProjectMsg::ProfileEditRequested(advanced.clone()),
    );
    assert_eq!(draft(&app).editing, Some(advanced.clone()));
    assert!(draft(&app).setup.is_builtin_picked(SetupProfile::Advanced));
    send(
        &mut app,
        NewProjectMsg::RuleEdited(PcbField::Clearance, "0.11".into()),
    );
    send(&mut app, NewProjectMsg::ProfileEditSaved);
    assert_eq!(draft(&app).editing, None);
    let users = &app.document_state.setup_profiles;
    assert!(
        SetupProfile::Advanced.is_edited(users),
        "the override is kept"
    );
    let edit = users.iter().find(|p| p.id == "advanced").expect("same id");
    assert!(edit.edited);
    assert_eq!(edit.pcb.clearance_mm, 0.11);
    let _ = app.view_new_project_dialog();

    // Picking another card and back takes the edited values.
    send(
        &mut app,
        NewProjectMsg::ProfilePicked(SetupProfile::Standard),
    );
    send(
        &mut app,
        NewProjectMsg::ProfilePicked(SetupProfile::Advanced),
    );
    assert_eq!(draft(&app).setup.pcb.clearance_mm, 0.11);

    // The edited built-in's pencil opens its menu; Reset to default drops
    // the override and the picked card shows the shipped values again.
    send(&mut app, NewProjectMsg::ProfileMenuToggled(Some(advanced)));
    let _ = app.view_new_project_dialog();
    send(
        &mut app,
        NewProjectMsg::ProfileResetRequested(SetupProfile::Advanced),
    );
    assert!(!SetupProfile::Advanced.is_edited(&app.document_state.setup_profiles));
    assert_eq!(draft(&app).profile_menu, None);
    assert_eq!(draft(&app).setup.pcb.clearance_mm, 0.1);
}

#[test]
fn a_failed_settings_write_takes_the_override_back() {
    let mut app = open();
    send(
        &mut app,
        NewProjectMsg::ProfileEditRequested(ProfileTarget::BuiltIn(SetupProfile::Hdi)),
    );
    send(
        &mut app,
        NewProjectMsg::RuleEdited(PcbField::TrackWidth, "0.09".into()),
    );
    send(&mut app, NewProjectMsg::ProfileEditSaved);
    assert!(SetupProfile::Hdi.is_edited(&app.document_state.setup_profiles));
    app.setup_profiles_written(
        String::new(),
        ProfileUndo::Saved {
            id: "hdi".into(),
            previous: None,
        },
        Err("disk full".into()),
    );
    assert!(
        !SetupProfile::Hdi.is_edited(&app.document_state.setup_profiles),
        "the in-memory override is taken back"
    );

    // A failed Reset puts the override back where it was.
    send(
        &mut app,
        NewProjectMsg::ProfileEditRequested(ProfileTarget::BuiltIn(SetupProfile::Hdi)),
    );
    send(&mut app, NewProjectMsg::ProfileEditSaved);
    let kept = app.document_state.setup_profiles.clone();
    send(
        &mut app,
        NewProjectMsg::ProfileResetRequested(SetupProfile::Hdi),
    );
    let dropped = kept[0].clone();
    app.setup_profiles_written(
        String::new(),
        ProfileUndo::Removed {
            profile: dropped,
            at: 0,
        },
        Err("disk full".into()),
    );
    assert_eq!(app.document_state.setup_profiles, kept);
}

#[test]
fn rename_duplicate_and_enter_in_the_rename_field() {
    let mut app = open();
    send(&mut app, NewProjectMsg::SaveProfileOpened);
    send(
        &mut app,
        NewProjectMsg::SaveProfileNameEdited("Power".into()),
    );
    send(&mut app, NewProjectMsg::SaveProfileConfirmed);
    let id = app.document_state.setup_profiles[0].id.clone();

    send(
        &mut app,
        NewProjectMsg::ProfileDuplicateRequested {
            source: ProfileTarget::Saved(id.clone()),
            name: "Power copy".into(),
        },
    );
    let copy = app.document_state.setup_profiles[1].clone();
    assert_eq!(copy.name, "Power copy");
    assert_ne!(copy.id, id);
    assert!(
        copy.id
            .starts_with(signex_types::project::USER_PROFILE_ID_PREFIX)
    );

    // Rename to a taken name is refused in the field; Enter confirms, it
    // does not create the project.
    send(
        &mut app,
        NewProjectMsg::ProfileRenameRequested(copy.id.clone()),
    );
    send(&mut app, NewProjectMsg::ProfileRenameEdited("power".into()));
    send(&mut app, NewProjectMsg::SubmitRequested);
    let renaming = draft(&app).renaming.clone().expect("still open");
    assert_eq!(
        renaming.problem,
        Some(signex_types::project::ProfileNameProblem::Taken)
    );
    assert!(draft(&app).creating.is_none(), "Enter did not create");
    send(
        &mut app,
        NewProjectMsg::ProfileRenameEdited("Sensor".into()),
    );
    send(&mut app, NewProjectMsg::SubmitRequested);
    assert_eq!(draft(&app).renaming, None);
    let renamed = &app.document_state.setup_profiles[1];
    assert_eq!(
        (renamed.id.as_str(), renamed.name.as_str()),
        (copy.id.as_str(), "Sensor")
    );
    let _ = app.view_new_project_dialog();
}
