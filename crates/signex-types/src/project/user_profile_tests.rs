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
    parse_project_bytes(std::path::Path::new("P.snxprj"), json.as_bytes(), |_| false)
        .expect("opens")
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
        SetupProfile::Advanced.pcb(8),
        SheetDefaults::default(),
        DesignatorStandard::Ieee315,
    );
    assert_eq!(again, id, "the same name keeps its id");
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].pcb, SetupProfile::Advanced.pcb(8));
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
        profile: Some("advanced".into()),
        ..ProjectDefaults::default()
    };
    let resolved = ProjectSetup::for_project(&named, None, &users);
    assert_eq!(resolved.setup.pcb, SetupProfile::Advanced.pcb(8));
    assert_eq!(
        resolved.setup.pcb.copper_layers, 6,
        "the tier's own layer count"
    );
    assert!(resolved.setup.is_builtin_picked(SetupProfile::Advanced));
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
        &ProjectSetup::from_profile(SetupProfile::Advanced).defaults(),
        None,
        &users,
    );
    let pcb = |t: &str| ProjectSetup::for_command(t, &project, &users).map(|s| s.pcb);
    assert_eq!(pcb("project"), Ok(SetupProfile::Advanced.pcb(8)));
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
    // A built-in token without the edited mark, and Custom, which cannot be edited.
    list.push(serde_json::json!({"id": "advanced", "name": "x", "pcb": users[0].pcb}));
    list.push(
        serde_json::json!({"id": "custom", "name": "x", "pcb": users[0].pcb, "edited": true}),
    );
    list.push(serde_json::json!(7));
    assert_eq!(user_profiles_from_value(Some(&value)), users);
    assert!(user_profiles_from_value(Some(&serde_json::json!("oops"))).is_empty());
    assert!(user_profiles_from_value(None).is_empty());
}

/// #2536: an edited built-in is an override with the tier's own id and the
/// edited mark; it resolves to its values, round-trips through the settings
/// value, and Reset to default drops it.
#[test]
fn an_edited_built_in_resolves_by_its_own_id_and_reset_removes_it() {
    let mut users = Vec::new();
    let id = team(&mut users);
    let mut mine = SetupProfile::Advanced.values(&users);
    mine.clearance_mm = 0.11;
    let change = override_builtin(&mut users, SetupProfile::Advanced, "Advanced", mine.clone())
        .expect("a tier can be edited");
    assert_eq!(change.previous, None);
    assert_eq!(change.profile.id, "advanced");
    assert!(change.profile.edited && change.profile.is_usable());
    assert!(SetupProfile::Advanced.is_edited(&users));
    assert_eq!(SetupProfile::Advanced.values(&users), mine);
    assert!(matches!(
        resolve_profile("advanced", &users),
        Some(ProfileRef::Edited { profile: SetupProfile::Advanced, values }) if values.pcb == mine
    ));
    // Not a saved profile: not listed, not found by name, not overwritten by a save.
    assert_eq!(saved_profiles(&users).count(), 1);
    assert_eq!(find_user_profile("Advanced", &users), None);
    let saved = save_user_profile(
        &mut users,
        "advanced",
        SetupProfile::Hdi.pcb(8),
        SheetDefaults::default(),
        DesignatorStandard::Ieee315,
    );
    assert!(saved.starts_with(USER_PROFILE_ID_PREFIX), "{saved}");
    assert_eq!(SetupProfile::Advanced.values(&users), mine);

    // Editing again replaces the override in place.
    let again = override_builtin(&mut users, SetupProfile::Advanced, "Advanced", mine.clone())
        .expect("edited again");
    assert_eq!(again.previous.map(|p| p.id), Some("advanced".to_string()));
    assert_eq!(users.iter().filter(|p| p.id == "advanced").count(), 1);

    // The settings value keeps it.
    let value = serde_json::to_value(&users).unwrap();
    assert_eq!(user_profiles_from_value(Some(&value)), users);

    // A project naming the tier and storing no rules takes the edited values.
    let named = ProjectDefaults {
        profile: Some("advanced".into()),
        ..ProjectDefaults::default()
    };
    let resolved = ProjectSetup::for_project(&named, None, &users);
    assert_eq!(resolved.setup.pcb, mine);

    // Reset to default drops the override; the shipped values come back.
    let (dropped, _) = reset_builtin(&mut users, SetupProfile::Advanced).expect("dropped");
    assert_eq!(dropped.pcb, mine);
    assert!(!SetupProfile::Advanced.is_edited(&users));
    assert_eq!(
        SetupProfile::Advanced.values(&users),
        SetupProfile::Advanced.pcb(4)
    );
    assert_eq!(reset_builtin(&mut users, SetupProfile::Advanced), None);
    assert!(override_builtin(&mut users, SetupProfile::Custom, "Custom", mine).is_none());
    assert!(users.iter().any(|p| p.id == id));
}

/// #2536: rename keeps the id and refuses a name another saved profile has;
/// duplicate adds a fresh `user-` id under a free name.
#[test]
fn rename_and_duplicate_keep_ids_unique_with_the_user_prefix() {
    let mut users = Vec::new();
    let id = team(&mut users);
    let pcb = users[0].pcb.clone();
    let copy = duplicate_profile(
        &mut users,
        "Team HDI",
        pcb.clone(),
        SheetDefaults::default(),
        DesignatorStandard::Ieee315,
    );
    assert_eq!(copy.name, "Team HDI 2");
    assert_eq!(copy.id, "user-team-hdi-2");
    let third = duplicate_profile(
        &mut users,
        "Team HDI",
        pcb,
        SheetDefaults::default(),
        DesignatorStandard::Ieee315,
    );
    assert_eq!(third.name, "Team HDI 3");
    let ids: std::collections::BTreeSet<_> = users.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids.len(), users.len(), "ids are unique");
    assert!(
        users
            .iter()
            .all(|p| p.id.starts_with(USER_PROFILE_ID_PREFIX))
    );

    let renamed = rename_user_profile(&mut users, &id, " Power 4L ")
        .expect("renamed")
        .expect("found");
    assert_eq!(renamed.profile.id, id, "a rename keeps the id");
    assert_eq!(renamed.profile.name, "Power 4L");
    assert_eq!(
        renamed.previous.map(|p| p.name),
        Some("Team HDI".to_string())
    );
    assert_eq!(
        rename_user_profile(&mut users, &copy.id, "power 4l"),
        Err(ProfileNameProblem::Taken)
    );
    assert_eq!(
        rename_user_profile(&mut users, &copy.id, " "),
        Err(ProfileNameProblem::Empty)
    );
    assert_eq!(rename_user_profile(&mut users, "user-gone", "X"), Ok(None));
    assert_eq!(unique_profile_name("Fresh", &users), "Fresh");
}
