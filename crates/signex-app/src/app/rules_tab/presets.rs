//! Presets in the app (#2398): a generic manufacturing tier merges its
//! minimums into the rules; a rule profile (`.snxrules`, from the profile
//! library) replaces them, as "Update from profile" does. Either is shown
//! as a diff (added, changed and removed rules) before it is applied as
//! one undo step. Behaviour: `docs/DRC_RULES.md`, "Presets".

use iced::Task;
use serde_json::{Value, json};
use signex_drc::{DrcRuleSection, WrittenRuleEdit, adopt_profile_drc, diff_drc_against_profile};
use signex_drc_dsl::table::presets::{
    PresetMode, RowChange, RowChangeKind, diff_rows, preset_result, tier_from_id, tier_id,
    tier_text,
};
use signex_types::project::{DrcRuleSet, FabTemplate, RuleProfile};

use super::PresetPreview;
use super::commands::{Ran, answered, refused, text};
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::state::ProjectId;
use crate::app::{Message, Signex};

/// Why a profile preset cannot be read yet: the library is read off the
/// update loop.
pub(crate) const PROFILES_NOT_READ: &str =
    "the rule profiles are not read yet; they are being read, ask again";

/// A preset named by id.
enum Preset {
    Tier(FabTemplate),
    Profile(RuleProfile),
}

/// The id a tier is picked by.
pub(crate) fn tier_preset(tier: FabTemplate) -> String {
    format!("tier:{}", tier_id(tier))
}

/// The id a profile is picked by.
pub(crate) fn profile_preset(name: &str) -> String {
    format!("profile:{name}")
}

impl Signex {
    /// The rule profiles that hold DRC rules, as read last; `None` while
    /// the library was never read.
    pub(crate) fn preset_profiles(&self) -> Option<Vec<&RuleProfile>> {
        let manager = &self.ui_state.profile_manager;
        manager.loaded.then(|| {
            manager
                .listing
                .profiles
                .iter()
                .map(|listed| &listed.profile)
                .filter(|profile| !profile.drc.is_empty())
                .collect()
        })
    }

    /// `preset`: `tier:<id>` or `profile:<name>`; a bare tier id or
    /// profile name works too.
    fn find_preset(&self, preset: &str) -> Result<Preset, String> {
        let preset = preset.trim();
        let (tier, profile) = match preset.split_once(':') {
            Some(("tier", id)) => (Some(id), None),
            Some(("profile", name)) => (None, Some(name)),
            _ => (Some(preset), Some(preset)),
        };
        if let Some(tier) = tier.and_then(tier_from_id) {
            return Ok(Preset::Tier(tier));
        }
        let Some(name) = profile else {
            return Err(unknown(preset));
        };
        let profiles = self.preset_profiles().ok_or(PROFILES_NOT_READ)?;
        profiles
            .into_iter()
            .find(|p| p.name == name)
            .cloned()
            .map(Preset::Profile)
            .ok_or_else(|| unknown(preset))
    }

    /// What applying `preset` to `id`'s rules changes.
    pub(crate) fn preset_preview(
        &self,
        id: ProjectId,
        preset: &str,
    ) -> Result<PresetPreview, String> {
        let base = self
            .rules_drc_of(id)
            .ok_or("the project is no longer open")?;
        let current = base.dsl.clone().unwrap_or_default();
        let (key, label, mut result, other) = match self.find_preset(preset)? {
            Preset::Tier(tier) => {
                let text = preset_result(&current, &tier_text(tier), PresetMode::Merge)
                    .map_err(|e| e.to_string())?;
                let mut result = base.clone();
                result.dsl = (!text.trim().is_empty()).then_some(text);
                let id = tier_id(tier).to_string();
                (tier_preset(tier), id, result, Vec::new())
            }
            Preset::Profile(profile) => {
                let other: Vec<String> = diff_drc_against_profile(&base, &profile.drc)
                    .iter()
                    .filter(|c| c.section != DrcRuleSection::RuleText)
                    .map(|c| c.describe())
                    .collect();
                let result = adopt_profile_drc(&base, &profile.drc);
                (profile_preset(&profile.name), profile.name, result, other)
            }
        };
        let changes = diff_rows(&current, result.dsl.as_deref().unwrap_or_default());
        if matches!(self.find_preset(preset)?, Preset::Tier(_)) {
            // A rule a tier adds or replaces must not keep a stale entry.
            for change in &changes {
                signex_drc::follow_written_rule_edit(
                    &mut result,
                    WrittenRuleEdit::Dropped(&change.name),
                );
            }
        }
        Ok(PresetPreview {
            preset: key,
            label,
            changes,
            other,
            base,
            result,
        })
    }

    /// Apply `preview` as one undo step, unless the rules moved since it
    /// was computed.
    pub(crate) fn apply_preset(
        &mut self,
        id: ProjectId,
        preview: &PresetPreview,
    ) -> Result<(), String> {
        let now = self
            .rules_drc_of(id)
            .ok_or("the project is no longer open")?;
        if now != preview.base {
            return Err("the rules changed since the preview; pick the preset again".into());
        }
        let result: DrcRuleSet = preview.result.clone();
        self.commit_drc_section(id, |drc| *drc = result)
    }

    /// `rules_preset_diff`: what applying `preset` changes. A profile
    /// preset before the library was read is refused while it is read.
    pub(crate) fn rules_preset_diff(
        &mut self,
        args: &CommandArgs,
    ) -> (CommandOutcome, Task<Message>) {
        let outcome = |result: Result<Value, String>| match result {
            Ok(value) => CommandOutcome::Answered(value),
            Err(reason) => CommandOutcome::disabled(reason),
        };
        let id = match self.rules_project_named(args) {
            Ok(id) => id,
            Err(reason) => return (outcome(Err(reason)), Task::none()),
        };
        let Some(preset) = text(args, "preset") else {
            return (
                outcome(Err("needs the operand `preset`".into())),
                Task::none(),
            );
        };
        match self.preset_preview(id, preset) {
            Ok(preview) => (outcome(Ok(preview_json(&preview))), Task::none()),
            Err(reason) if reason == PROFILES_NOT_READ => {
                (outcome(Err(reason)), Self::list_profiles_task())
            }
            Err(reason) => (outcome(Err(reason)), Task::none()),
        }
    }
}

fn unknown(preset: &str) -> String {
    let tiers: Vec<String> = FabTemplate::ALL.into_iter().map(tier_preset).collect();
    format!(
        "no preset is called '{preset}' ({} or profile:<name>)",
        tiers.join(", ")
    )
}

fn change_json(change: &RowChange) -> Value {
    json!({
        "rule": change.name,
        "change": change.kind.as_str(),
        "from": change.from,
        "to": change.to,
    })
}

/// A preview as `rules_preset_diff` and `rules_preset_apply` answer it.
pub(crate) fn preview_json(preview: &PresetPreview) -> Value {
    let count = |kind: RowChangeKind| preview.changes.iter().filter(|c| c.kind == kind).count();
    json!({
        "preset": preview.preset,
        "label": preview.label,
        "mode": if preview.preset.starts_with("tier:") { "merge" } else { "replace" },
        "added": count(RowChangeKind::Added),
        "changed": count(RowChangeKind::Changed),
        "removed": count(RowChangeKind::Removed),
        "changes": preview.changes.iter().map(change_json).collect::<Vec<_>>(),
        "other": preview.other,
    })
}

/// `rules_preset_apply {document, preset}`: the diff `rules_preset_diff`
/// shows, applied as one undo step.
pub(crate) fn rules_preset_apply(app: &mut Signex, args: &CommandArgs) -> Ran {
    let id = match app.rules_project_named(args) {
        Ok(id) => id,
        Err(reason) => return refused(reason),
    };
    let Some(preset) = text(args, "preset") else {
        return refused("needs the operand `preset`");
    };
    let preview = match app.preset_preview(id, preset) {
        Ok(preview) => preview,
        Err(reason) if reason == PROFILES_NOT_READ => {
            return (
                CommandOutcome::disabled(reason),
                Signex::list_profiles_task(),
            );
        }
        Err(reason) => return refused(reason),
    };
    match app.apply_preset(id, &preview) {
        Ok(()) => answered(preview_json(&preview)),
        Err(reason) => refused(reason),
    }
}
