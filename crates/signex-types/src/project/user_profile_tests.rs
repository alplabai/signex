//! User setup profiles and the project's default-profile id (#2232,
//! 2026-11-12).

use super::*;
use crate::designator::DesignatorStandard;

fn team(users: &mut Vec<UserProfile>) -> String {
    let mut pcb = SetupProfile::Hdi.pcb(8);
    pcb.copper_layers = 10;
    pcb.clearance_mm = 0.09;
    let sheet = SheetDefaults {
        paper_size: "A2".into(),
        show_title_block: false,
    };
    save_user_profile(
        users,
        "Team HDI",
        pcb,
        sheet,
        DesignatorStandard::Iec81346_2,
    )
}

fn opened(json: &str) -> ProjectData {
    parse_project_bytes(std::path::Path::new("P.snxprj"), json.as_bytes()).expect("opens")
}

#[test]
fn a_saved_profile_gets_a_user_id_and_the_same_name_updates_it() {
    let mut users = Vec::new();
    let id = team(&mut users);
    assert_eq!(id, "user-team-hdi");
    assert_eq!(new_profile_id("Team HDI", &users), "user-team-hdi-2");
    let again = save_user_profile(
        &mut users,
        " team hdi ",
        SetupProfile::Hobby.pcb(8),
        SheetDefaults::default(),
        DesignatorStandard::Ieee315,
    );
    assert_eq!(again, id, "the same name keeps its id");
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].pcb, SetupProfile::Hobby.pcb(8));
    assert_eq!(check_profile_name("  "), Err(ProfileNameProblem::Empty));
    assert_eq!(
        check_profile_name(&"x".repeat(MAX_PROFILE_NAME_CHARS + 1)),
        Err(ProfileNameProblem::TooLong)
    );
    assert_eq!(check_profile_name(" Mine "), Ok("Mine"));
}

#[test]
fn an_id_resolves_to_a_built_in_a_user_profile_or_nothing() {
    let mut users = Vec::new();
    let id = team(&mut users);
    assert_eq!(
        resolve_profile("hdi", &users),
        Some(ProfileRef::BuiltIn(SetupProfile::Hdi))
    );
    assert!(matches!(resolve_profile(&id, &users), Some(ProfileRef::User(p)) if p.id == id));
    assert_eq!(resolve_profile("user-gone", &users), None);
    assert_eq!(
        find_user_profile("TEAM hdi", &users).map(|p| p.id.as_str()),
        Some(id.as_str())
    );
}

#[test]
fn a_project_resolves_its_profile_built_in_user_or_missing() {
    let mut users = Vec::new();
    let id = team(&mut users);

    // Built in, no stored values: the profile's rules.
    let named = ProjectDefaults {
        profile: Some("hobby".into()),
        ..ProjectDefaults::default()
    };
    let resolved = ProjectSetup::for_project(&named, None, &users);
    assert_eq!(resolved.setup.pcb, SetupProfile::Hobby.pcb(8));
    assert!(resolved.setup.is_builtin_picked(SetupProfile::Hobby));
    assert_eq!(resolved.missing_profile, None);

    // A user profile, no stored values: its rules, sheet and standard.
    let named = ProjectDefaults {
        profile: Some(id.clone()),
        ..ProjectDefaults::default()
    };
    let resolved = ProjectSetup::for_project(&named, None, &users);
    assert_eq!(resolved.setup.pcb, users[0].pcb);
    assert_eq!(resolved.setup.sheet.paper_size, "A2");
    assert_eq!(resolved.setup.designators, DesignatorStandard::Iec81346_2);
    assert_eq!(resolved.setup.profile_id(), id);

    // Stored values win over the named profile; the id picks the card.
    let mut stored = ProjectSetup::from_profile(SetupProfile::Standard).defaults();
    stored.profile = Some(id.clone());
    let resolved = ProjectSetup::for_project(&stored, None, &users);
    assert_eq!(resolved.setup.pcb, SetupProfile::Standard.pcb(8));
    assert_eq!(resolved.setup.user_profile.as_deref(), Some(id.as_str()));

    // A user profile that is gone: the stored values and the id to report.
    let resolved = ProjectSetup::for_project(&stored, None, &[]);
    assert_eq!(resolved.setup.pcb, SetupProfile::Standard.pcb(8));
    assert_eq!(resolved.setup.user_profile, None);
    assert_eq!(resolved.missing_profile.as_deref(), Some(id.as_str()));

    // Gone and nothing stored: Standard.
    let resolved = ProjectSetup::for_project(&named, None, &[]);
    assert_eq!(resolved.setup.pcb, SetupProfile::Standard.pcb(8));
    assert_eq!(resolved.missing_profile.as_deref(), Some(id.as_str()));
}

#[test]
fn a_command_profile_token_names_the_project_a_built_in_or_a_saved_profile() {
    let mut users = Vec::new();
    let id = team(&mut users);
    let project = ProjectSetup::for_project(
        &ProjectSetup::from_profile(SetupProfile::Hobby).defaults(),
        None,
        &users,
    );
    let pcb = |t: &str| ProjectSetup::for_command(t, &project, &users).map(|s| s.pcb);
    assert_eq!(pcb("project"), Ok(SetupProfile::Hobby.pcb(8)));
    assert_eq!(pcb("hdi"), Ok(SetupProfile::Hdi.pcb(8)));
    assert_eq!(pcb(&id), Ok(users[0].pcb.clone()));
    assert_eq!(pcb("Team HDI"), Ok(users[0].pcb.clone()));
    assert!(pcb("nope").is_err());
}

#[test]
fn the_default_profile_id_is_absent_on_old_files_and_round_trips_when_set() {
    const BASE: &str = r#""name":"P","dir":"","schematic_root":null,"pcb_file":null"#;
    // A file from before the id: loads with none and re-saves without it.
    let old = opened(&format!(
        r#"{{{BASE},"project_defaults":{{"sheet":{{"paper_size":"A4","show_title_block":true}}}}}}"#
    ));
    assert_eq!(old.project_defaults.profile, None);
    let bytes = encode_project(&old).unwrap();
    let back: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(back["project_defaults"].get("profile").is_none());

    // Present: read and written back.
    let named = opened(&format!(
        r#"{{{BASE},"project_defaults":{{"profile":"user-team"}}}}"#
    ));
    assert_eq!(named.project_defaults.profile.as_deref(), Some("user-team"));
    assert!(!named.has_unreadable_project_defaults());
    let bytes = encode_project(&named).unwrap();
    let back: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(back["project_defaults"]["profile"], "user-team");

    // A project with no defaults at all writes no section.
    let bare = opened(&format!("{{{BASE}}}"));
    let bytes = encode_project(&bare).unwrap();
    let back: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(back.get("project_defaults").is_none());
}

#[test]
fn settings_keep_only_usable_profiles_once_each() {
    let mut users = Vec::new();
    team(&mut users);
    let mut value = serde_json::to_value(&users).unwrap();
    let list = value.as_array_mut().unwrap();
    list.push(list[0].clone());
    list.push(serde_json::json!({"id": "hobby", "name": "x", "pcb": users[0].pcb}));
    list.push(serde_json::json!(7));
    assert_eq!(user_profiles_from_value(Some(&value)), users);
    assert!(user_profiles_from_value(Some(&serde_json::json!("oops"))).is_empty());
    assert!(user_profiles_from_value(None).is_empty());
}
