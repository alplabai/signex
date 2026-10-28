//! The rule set a new project starts with (#1608): a copy of the selected
//! default profile, recorded with its provenance so Update from profile and
//! the drift check work from the first save.

use std::path::Path;

use signex_erc::project_rules::profile::{
    BUILTIN_PROFILE_NAME, SOURCE_BUILTIN, builtin_profile, profile_ref,
};
use signex_types::project::{ProjectRules, RuleProfileRef};

use super::{USER_DEFAULT_PROFILE_NAME, resolve_profile};

/// The rules for a new project, plus a sentence for each fallback taken.
///
/// `selected` is the profile the user picked as the default. With none picked,
/// `User Default` is used when the machine has one, so a user who set up their
/// rules before profiles existed keeps getting them; otherwise the shipped
/// profile. A picked profile that cannot be read falls back to the shipped one
/// and says why, so a project is never created without rules by accident.
pub fn rules_for_new_project(
    dir: Option<&Path>,
    selected: Option<&str>,
) -> (ProjectRules, Vec<String>) {
    let mut notes = Vec::new();
    let wanted = selected.map(str::trim).filter(|n| !n.is_empty());
    let mut found = None;
    for name in wanted
        .into_iter()
        .chain((wanted.is_none() && dir.is_some()).then_some(USER_DEFAULT_PROFILE_NAME))
    {
        if name.eq_ignore_ascii_case(BUILTIN_PROFILE_NAME) {
            break;
        }
        let reference = RuleProfileRef {
            name: name.to_string(),
            source: signex_erc::project_rules::profile::SOURCE_USER.to_string(),
            ..Default::default()
        };
        match resolve_profile(dir, &reference) {
            Ok(Some(hit)) => found = Some(hit),
            // No `User Default` yet is normal; a missing picked profile is not.
            Ok(None) if wanted.is_none() => {}
            Ok(None) => notes.push(format!(
                "The default rule profile \"{name}\" is not in the profile library, so the new \
                 project starts from \"{BUILTIN_PROFILE_NAME}\"."
            )),
            Err(why) => notes.push(format!(
                "The default rule profile \"{name}\" could not be read ({why}), so the new \
                 project starts from \"{BUILTIN_PROFILE_NAME}\"."
            )),
        }
    }
    let (profile, source) = found.unwrap_or_else(|| (builtin_profile(), SOURCE_BUILTIN));
    let rules = ProjectRules {
        erc: profile.erc.clone(),
        profile: Some(profile_ref(&profile, source)),
        ..ProjectRules::default()
    };
    (rules, notes)
}
