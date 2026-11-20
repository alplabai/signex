//! #2232: the Project setup section edits the defaults a project was created
//! with, as a normal project edit: it dirties the `.snxprj`, writes nothing
//! until Save, and undoes.

use signex_types::project::{PcbField, SetupProfile};

use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{Message, NewProjectMsg, ProjectMsg, ProjectSection, ProjectTabMsg, Signex};

fn app() -> Signex {
    let dir = std::env::temp_dir().join(format!("signex-prj-setup-{}", uuid::Uuid::now_v7()));
    let mut app = app_workspace(&dir.to_string_lossy(), &["root.snxsch"]);
    app.open_project_tab(0);
    let project_id = app.document_state.projects[0].id;
    let _ = app.update(Message::Project(ProjectMsg::Tab(
        ProjectTabMsg::SectionSelected {
            project_id,
            section: ProjectSection::Setup,
        },
    )));
    app
}

fn edit(app: &mut Signex, msg: NewProjectMsg) {
    let project_id = app.document_state.projects[0].id;
    let _ = app.update(Message::Project(ProjectMsg::Tab(
        ProjectTabMsg::SetupEdited { project_id, msg },
    )));
}

fn project_dirty(app: &Signex) -> bool {
    let path = &app.document_state.projects[0].path;
    app.document_state.is_document_dirty(path)
}

#[test]
fn a_profile_and_a_clearance_are_stored_as_a_project_edit() {
    let mut app = app();
    let project_path = app.document_state.projects[0].path.clone();
    let on_disk = std::fs::read(&project_path).ok();
    assert!(!project_dirty(&app));

    edit(
        &mut app,
        NewProjectMsg::ProfilePicked(SetupProfile::Advanced),
    );
    edit(
        &mut app,
        NewProjectMsg::RuleEdited(PcbField::Clearance, "0.12".into()),
    );

    let stored = &app.document_state.projects[0].data.project_defaults;
    assert_eq!(stored.pcb.as_ref().map(|p| p.copper_layers), Some(6));
    assert_eq!(stored.pcb.as_ref().map(|p| p.clearance_mm), Some(0.12));
    assert!(project_dirty(&app), "the .snxprj is dirty");
    assert_eq!(
        std::fs::read(&project_path).ok(),
        on_disk,
        "nothing is written before Save"
    );
}

#[test]
fn undo_and_a_bad_value_leave_the_stored_defaults_unchanged() {
    let mut app = app();
    edit(&mut app, NewProjectMsg::ProfilePicked(SetupProfile::Hdi));
    let after_pick = app.document_state.projects[0].data.project_defaults.clone();
    assert_eq!(after_pick.pcb.as_ref().map(|p| p.copper_layers), Some(8));

    // A value that does not parse stays a draft with its reason.
    edit(
        &mut app,
        NewProjectMsg::RuleEdited(PcbField::Clearance, "abc".into()),
    );
    assert_eq!(
        app.document_state.projects[0].data.project_defaults,
        after_pick
    );

    // Undo takes the section back to what the project stored before.
    let id = app.document_state.projects[0].id;
    assert!(app.step_project_tab_history(id, true));
    assert!(
        app.document_state.projects[0]
            .data
            .project_defaults
            .is_empty()
    );
    assert!(!project_dirty(&app), "undo returned to the saved state");
}

#[test]
fn the_section_builds_from_the_stored_values() {
    let app = app();
    let id = app.document_state.projects[0].id;
    let _ = app.view_project_tab(id);
}

#[test]
fn each_pick_is_its_own_undo_step() {
    let mut app = app();
    edit(
        &mut app,
        NewProjectMsg::ProfilePicked(SetupProfile::Advanced),
    );
    edit(&mut app, NewProjectMsg::ProfilePicked(SetupProfile::Hdi));
    let id = app.document_state.projects[0].id;
    assert!(app.step_project_tab_history(id, true));
    let pcb = app.document_state.projects[0]
        .data
        .project_defaults
        .pcb
        .clone();
    assert_eq!(
        pcb.map(|p| p.copper_layers),
        Some(6),
        "Ctrl+Z returns Advanced"
    );
}

#[test]
fn the_selected_card_and_the_designator_store_only_what_changed() {
    let mut app = app();
    // Standard is shown selected for a project with no defaults.
    edit(
        &mut app,
        NewProjectMsg::ProfilePicked(SetupProfile::Standard),
    );
    assert!(
        app.document_state.projects[0]
            .data
            .project_defaults
            .is_empty()
    );
    assert!(
        !project_dirty(&app),
        "clicking the selected card changes nothing"
    );

    edit(
        &mut app,
        NewProjectMsg::DesignatorsPicked(signex_types::designator::DesignatorStandard::Iec81346_2),
    );
    let data = &app.document_state.projects[0].data;
    assert!(
        data.project_defaults.is_empty(),
        "no unchosen defaults stored"
    );
    assert_eq!(
        data.designator_standard(),
        Some(signex_types::designator::DesignatorStandard::Iec81346_2)
    );
    let id = app.document_state.projects[0].id;
    assert!(app.step_project_tab_history(id, true));
    assert_eq!(
        app.document_state.projects[0].data.designator_standard(),
        None
    );
    assert!(!app.step_project_tab_history(id, true), "it was one step");
}

#[test]
fn a_pick_is_stored_while_a_rule_field_holds_a_bad_value() {
    let mut app = app();
    edit(
        &mut app,
        NewProjectMsg::RuleEdited(PcbField::Clearance, "abc".into()),
    );
    edit(&mut app, NewProjectMsg::SheetSizePicked("A2".into()));
    let sheet = app.document_state.projects[0]
        .data
        .project_defaults
        .sheet
        .clone();
    assert_eq!(sheet.map(|s| s.paper_size), Some("A2".to_string()));
}

#[test]
fn rules_the_loader_would_refuse_are_not_stored() {
    let mut app = app();
    edit(
        &mut app,
        NewProjectMsg::ProfilePicked(SetupProfile::Advanced),
    );
    let good = app.document_state.projects[0]
        .data
        .project_defaults
        .pcb
        .clone();
    // Each value is in range alone; together the drill (0.2) is not smaller
    // than the via.
    edit(
        &mut app,
        NewProjectMsg::RuleEdited(PcbField::ViaSize, "0.2".into()),
    );
    assert_eq!(
        app.document_state.projects[0].data.project_defaults.pcb, good,
        "the last good rules stay stored"
    );
    let id = app.document_state.projects[0].id;
    let draft = app.document_state.project_tabs[&id]
        .setup_draft
        .as_ref()
        .unwrap();
    assert!(
        draft.drafts.problem(PcbField::ViaDrill).is_some(),
        "the inline reason shows"
    );
}

#[test]
fn an_unreadable_section_is_kept_and_the_section_still_builds() {
    let mut app = app();
    let path = app.document_state.projects[0].path.clone();
    let bytes =
        signex_types::project::encode_project(&app.document_state.projects[0].data).unwrap();
    let mut json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    json["project_defaults"] = serde_json::json!({"pcb": {"copper_layers": 300}});
    let reread =
        signex_types::project::parse_project_bytes(&path, json.to_string().as_bytes()).unwrap();
    app.document_state.projects[0].data = reread;
    assert!(
        app.document_state.projects[0]
            .data
            .has_unreadable_project_defaults()
    );
    let id = app.document_state.projects[0].id;
    let _ = app.view_project_tab(id);
}

fn fab_rule_names(app: &Signex) -> Vec<String> {
    let dsl = app.document_state.projects[0]
        .data
        .rules
        .as_ref()
        .and_then(|r| r.drc.dsl.clone())
        .unwrap_or_default();
    signex_drc_dsl::table::table(&dsl)
        .rows
        .iter()
        .map(|r| r.name.clone())
        .collect()
}

fn rule_text(app: &Signex) -> String {
    app.document_state.projects[0]
        .data
        .rules
        .as_ref()
        .and_then(|r| r.drc.dsl.clone())
        .unwrap_or_default()
}

/// #2311: a tier pick writes the tier's `fab_*` rules beside the user's as
/// one undo step that also takes back the defaults it filled; nothing is
/// written before Save.
#[test]
fn a_fab_tier_pick_seeds_its_rules_as_one_undo_step() {
    use signex_types::project::{DrcRuleSet, FabTemplate, ProjectRules};
    let mut app = app();
    let id = app.document_state.projects[0].id;
    let project_path = app.document_state.projects[0].path.clone();
    let on_disk = std::fs::read(&project_path).ok();
    let mine = "rule mine on clearance require clearance >= 0.3mm then error\n";
    app.document_state.projects[0].data.rules = Some(ProjectRules {
        drc: DrcRuleSet {
            dsl: Some(mine.into()),
            ..DrcRuleSet::default()
        },
        ..ProjectRules::default()
    });
    let before_rules = app.document_state.projects[0].data.rules.clone();
    let before_defaults = app.document_state.projects[0].data.project_defaults.clone();

    edit(&mut app, NewProjectMsg::FabPicked(FabTemplate::Standard));
    let names = fab_rule_names(&app);
    assert_eq!(
        names,
        [
            "mine",
            "fab_clearance",
            "fab_track",
            "fab_via",
            "fab_hole",
            "fab_edge",
            "fab_mask",
            "fab_silk"
        ]
    );
    assert!(rule_text(&app).contains("rule fab_track on track require width >= 0.15mm"));
    assert!(project_dirty(&app));
    assert_eq!(std::fs::read(&project_path).ok(), on_disk, "no write");

    // Re-choosing replaces only the fab rules: the user's stays as typed.
    edit(&mut app, NewProjectMsg::FabPicked(FabTemplate::Hdi));
    let text = rule_text(&app);
    assert!(text.starts_with(mine), "{text}");
    assert!(text.contains("rule fab_track on track require width >= 0.075mm"));
    assert_eq!(fab_rule_names(&app).len(), 8, "no rule doubled");

    // Each pick is one step: two undos restore rules and defaults alike.
    assert!(app.step_project_tab_history(id, true));
    assert!(rule_text(&app).contains("width >= 0.15mm"));
    assert!(app.step_project_tab_history(id, true));
    assert_eq!(app.document_state.projects[0].data.rules, before_rules);
    assert_eq!(
        app.document_state.projects[0].data.project_defaults,
        before_defaults
    );
    assert!(app.step_project_tab_history(id, false));
    assert!(rule_text(&app).contains("width >= 0.15mm"));
}

/// #2311: re-picking the tier already applied brings back an edited fab
/// rule, as one step.
#[test]
fn re_picking_the_same_tier_restores_an_edited_fab_rule() {
    use signex_types::project::FabTemplate;
    let mut app = app();
    edit(&mut app, NewProjectMsg::FabPicked(FabTemplate::Advanced));
    let seeded = rule_text(&app);
    let edited = seeded.replace("width >= 0.1mm", "width >= 0.2mm");
    app.document_state.projects[0]
        .data
        .rules
        .as_mut()
        .expect("rules")
        .drc
        .dsl = Some(edited);
    edit(&mut app, NewProjectMsg::FabPicked(FabTemplate::Advanced));
    assert_eq!(rule_text(&app), seeded);
}

fn setup_draft_open(app: &Signex) -> bool {
    let id = app.document_state.projects[0].id;
    app.document_state
        .project_tabs
        .get(&id)
        .is_some_and(|tab| tab.setup_draft.is_some())
}

/// #2536: Edit profile... in the Project tab edits the profile only. Opening
/// it, typing in a field and Cancel leave the project as it was: same
/// defaults, not dirty, and the section shows the project's values again.
#[test]
fn a_cancelled_profile_edit_leaves_the_project_unchanged() {
    use crate::app::new_project::ProfileTarget;
    let mut app = app();
    let before = app.document_state.projects[0].data.project_defaults.clone();
    for target in [
        ProfileTarget::BuiltIn(SetupProfile::Advanced),
        ProfileTarget::Saved(saved_profile(&mut app)),
    ] {
        edit(
            &mut app,
            NewProjectMsg::ProfileEditRequested(target.clone()),
        );
        edit(
            &mut app,
            NewProjectMsg::RuleEdited(PcbField::Clearance, "0.12".into()),
        );
        assert_eq!(
            app.document_state.projects[0].data.project_defaults, before,
            "{target:?}: opening and typing store nothing"
        );
        edit(&mut app, NewProjectMsg::ProfileEditCancelled);
        assert_eq!(
            app.document_state.projects[0].data.project_defaults, before,
            "{target:?}"
        );
        assert!(!project_dirty(&app), "{target:?}: the project is not dirty");
        assert!(
            !setup_draft_open(&app),
            "{target:?}: the project's values show"
        );
    }
}

/// #2536: Save of a profile edit changes the profile, not the project.
#[test]
fn a_saved_profile_edit_changes_the_profile_only() {
    use crate::app::new_project::ProfileTarget;
    let mut app = app();
    let before = app.document_state.projects[0].data.project_defaults.clone();
    edit(
        &mut app,
        NewProjectMsg::ProfileEditRequested(ProfileTarget::BuiltIn(SetupProfile::Advanced)),
    );
    edit(
        &mut app,
        NewProjectMsg::RuleEdited(PcbField::Clearance, "0.12".into()),
    );
    edit(&mut app, NewProjectMsg::ProfileEditSaved);
    assert!(
        app.ui_state
            .setup_profiles
            .iter()
            .any(|p| p.edited && p.id == "advanced" && p.pcb.clearance_mm == 0.12),
        "the built-in's override holds the edit"
    );
    assert_eq!(app.document_state.projects[0].data.project_defaults, before);
    assert!(!project_dirty(&app), "the project is not dirty");
    assert!(!setup_draft_open(&app), "the project's values show again");
}

fn saved_profile(app: &mut Signex) -> String {
    let setup = signex_types::project::ProjectSetup::default();
    signex_types::project::save_user_profile(
        &mut app.ui_state.setup_profiles,
        "Mine",
        setup.pcb,
        setup.sheet,
        setup.designators,
    )
}
