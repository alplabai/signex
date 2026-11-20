//! Presets in the app (#2398): a generic manufacturing tier merges its
//! minimums into the rules; a rule profile (`.snxrules`, from the profile
//! library) replaces them, as "Update from profile" does. Either is shown
//! as a diff (added, changed and removed rules) before it is applied as
//! one undo step. Behaviour: `docs/DRC_RULES.md`, "Presets".

use serde_json::{Value, json};
use signex_drc::{DrcRuleSection, adopt_profile_drc, diff_drc_against_profile};
use signex_drc_dsl::table::presets::{
    RowChange, RowChangeKind, diff_rows, tier_from_id, tier_id, with_tier,
};
use signex_types::project::{DrcRuleSet, FabTemplate, ProjectRules, RuleProfile};

use super::PresetPreview;
use super::commands::{Ran, answered, refused, text};
use crate::app::Signex;
use crate::app::command::CommandArgs;
use crate::app::state::ProjectId;

/// Why a profile preset cannot be read yet: the library is read off the
/// update loop.
pub(crate) const PROFILES_NOT_READ: &str =
    "the rule profiles are not read yet; they are being read, ask again";

/// A preset named by id.
enum Preset {
    Tier(FabTemplate),
    Profile(Box<RuleProfile>),
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
            .map(|p| Preset::Profile(Box::new(p)))
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
        let (key, label, result, other, fab) = match self.find_preset(preset)? {
            Preset::Tier(tier) => {
                let result = with_tier(&base, tier).map_err(|e| e.to_string())?;
                let id = tier_id(tier).to_string();
                (tier_preset(tier), id, result, Vec::new(), Some(tier.info()))
            }
            Preset::Profile(profile) => {
                let other: Vec<String> = diff_drc_against_profile(&base, &profile.drc)
                    .iter()
                    .filter(|c| c.section != DrcRuleSection::RuleText)
                    .map(|c| c.describe())
                    .collect();
                let result = adopt_profile_drc(&base, &profile.drc);
                let fab = profile.fab.clone();
                (
                    profile_preset(&profile.name),
                    profile.name,
                    result,
                    other,
                    fab,
                )
            }
        };
        let changes = diff_rows(&current, result.dsl.as_deref().unwrap_or_default());
        Ok(PresetPreview {
            preset: key,
            label,
            changes,
            other,
            fab,
            base,
            result,
        })
    }

    /// `id`'s rules with `tier`'s `fab_*` rules merged in (#2311), for a
    /// tier picked in project setup. A project that stores no rules starts
    /// from this machine's, as any rule edit does.
    pub(crate) fn rules_with_tier(
        &self,
        id: ProjectId,
        tier: FabTemplate,
    ) -> Result<ProjectRules, String> {
        let project = self
            .document_state
            .project_by_id(id)
            .ok_or("the project is no longer open")?;
        if project.data.has_unreadable_rules() {
            return Err(
                "this project holds a rule set from a newer Signex, which storing rules \
                 would replace"
                    .into(),
            );
        }
        let mut rules = project
            .data
            .rules
            .clone()
            .unwrap_or_else(|| self.seeded_project_rules());
        rules.drc = with_tier(&rules.drc, tier).map_err(|e| e.to_string())?;
        Ok(rules)
    }

    /// A tier picked for a new board of `id`: its `fab_*` rules merged into
    /// the project's as one rule-history step, nothing written until Save.
    /// A refusal is said in Messages; the board is made either way.
    pub(crate) fn seed_tier_rules(&mut self, id: ProjectId, tier: FabTemplate) {
        let seeded = self
            .rules_with_tier(id, tier)
            .and_then(|rules| self.commit_drc_section(id, |drc| *drc = rules.drc));
        match seeded {
            Ok(()) => self.set_project_router_tool(id, tier.capabilities().router_tool_diameter),
            Err(why) => crate::diagnostics::log_warning(format!(
                "New PCB: the {} fab tier rules were not added: {why}",
                tier.token()
            )),
        }
    }

    /// Apply `preview` as one undo step, unless the rules moved since it
    /// was computed. A tier, or a fab profile naming a router tool, also
    /// sets the project's router tool diameter (#179d); a profile naming
    /// none leaves it as it is.
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
        self.commit_drc_section(id, |drc| *drc = result)?;
        if let Some(diameter) = preview
            .fab
            .as_ref()
            .and_then(|fab| fab.router_tool_diameter)
        {
            self.set_project_router_tool(id, diameter);
        }
        Ok(())
    }

    /// Store `diameter` mm as `id`'s router tool and show it in an open
    /// board sketch. Memory only; the project saves with the next save.
    fn set_project_router_tool(&mut self, id: ProjectId, diameter: f64) {
        let ds = &mut self.document_state;
        let Some(project) = ds.project_by_id_mut(id) else {
            return;
        };
        if project.data.router_tool_diameter() == Some(diameter) {
            return;
        }
        project.data.set_router_tool_diameter(Some(diameter));
        ds.sync_project_history_dirty(id, false);
        self.sync_board_sketch_router();
        self.refresh_panel_ctx();
    }

    /// `rules_preset_diff`: what applying `preset` changes. A profile
    /// preset before the library was read is refused while it is read.
    pub(crate) fn rules_preset_diff(&mut self, args: &CommandArgs) -> Ran {
        let id = match self.rules_project_named(args) {
            Ok(id) => id,
            Err(reason) => return refused(reason),
        };
        let Some(preset) = text(args, "preset") else {
            return refused("needs the operand `preset`");
        };
        match self.preset_preview(id, preset) {
            Ok(preview) => answered(preview_json(&preview)),
            // The library is read now; the answer is to ask again.
            Err(reason) if reason == PROFILES_NOT_READ => {
                (refused(reason).0, Self::list_profiles_task())
            }
            Err(reason) => refused(reason),
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
        "fab": preview.fab,
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
            return (refused(reason).0, Signex::list_profiles_task());
        }
        Err(reason) => return refused(reason),
    };
    match app.apply_preset(id, &preview) {
        Ok(()) => answered(preview_json(&preview)),
        Err(reason) => refused(reason),
    }
}
