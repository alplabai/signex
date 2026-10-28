//! UI state of the rule-profile screens (#1608): the "Save as profile"
//! dialog, the Preferences profile editor, and the queue that keeps profile
//! file writes in order.

use std::collections::BTreeMap;

use signex_types::project::{ErcRules, RuleProfile};

use crate::rule_profiles::{ListedProfile, ProfileKind, ProfileListing};

/// The open "Save as profile" dialog of the ERC dialog. Nothing is written
/// until Save; a name that exists asks before replacing it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveProfileDialog {
    /// The project whose rules are being saved.
    pub project: crate::app::state::ProjectId,
    pub name: String,
    pub version: String,
    /// Why the last Save was refused, shown under the inputs.
    pub error: Option<String>,
    /// The name exists: waiting for the user to confirm or go back.
    pub confirm_overwrite: bool,
    /// A write is running; the buttons are off until it reports.
    pub busy: bool,
}

impl SaveProfileDialog {
    pub fn new(project: crate::app::state::ProjectId, name: String) -> Self {
        Self {
            project,
            name,
            version: crate::rule_profiles::DEFAULT_VERSION.to_string(),
            error: None,
            confirm_overwrite: false,
            busy: false,
        }
    }
}

/// Preferences ▸ Rule Profiles. Edits apply at once, like the ERC dialog's:
/// each one is written to the profile's file, not held as a draft.
#[derive(Debug, Clone, Default)]
pub struct ProfileManager {
    /// Last listing read from disk, edited in place as the user edits.
    pub listing: ProfileListing,
    /// A listing has been requested at least once since Preferences opened.
    pub loaded: bool,
    /// Name of the profile the rule editor shows.
    pub selected: Option<String>,
    /// Text of the name field used by New / Duplicate / Rename.
    pub name_input: String,
    /// Delete waiting for confirmation, by profile name.
    pub confirm_delete: Option<String>,
    /// Outcome of the last action: `(is_error, sentence)`.
    pub status: Option<(bool, String)>,
}

impl ProfileManager {
    pub fn selected_profile(&self) -> Option<&ListedProfile> {
        let name = self.selected.as_deref()?;
        self.listing
            .profiles
            .iter()
            .find(|l| l.profile.name == name)
    }

    pub fn selected_profile_mut(&mut self) -> Option<&mut ListedProfile> {
        let name = self.selected.as_deref()?;
        self.listing
            .profiles
            .iter_mut()
            .find(|l| l.profile.name == name)
    }

    /// Replace the listing with a fresh read, keeping the selection when the
    /// profile is still there (else the first profile).
    pub fn apply_listing(&mut self, listing: ProfileListing) {
        self.listing = listing;
        self.loaded = true;
        let keep = self
            .selected
            .as_deref()
            .is_some_and(|name| self.listing.profiles.iter().any(|l| l.profile.name == name));
        if !keep {
            self.selected = self
                .listing
                .profiles
                .first()
                .map(|l| l.profile.name.clone());
        }
        if self
            .confirm_delete
            .as_deref()
            .is_some_and(|name| !self.listing.profiles.iter().any(|l| l.profile.name == name))
        {
            self.confirm_delete = None;
        }
    }

    /// Whether `name` is the machine's own rule set.
    pub fn is_user_default(&self, name: &str) -> bool {
        self.listing
            .profiles
            .iter()
            .any(|l| l.kind == ProfileKind::UserDefault && l.profile.name == name)
    }
}

/// One profile file write waiting its turn.
#[derive(Debug, Clone)]
pub enum QueuedWrite {
    /// The machine's rule set (`User Default`), keeping what the file holds
    /// that this build does not know.
    UserDefault(ErcRules),
    /// An edited library profile, written to its own file.
    Profile(RuleProfile),
}

/// Profile file writes, one at a time and in the order the user made them.
///
/// Two edits in quick succession would otherwise run on two worker threads in
/// either order and the older snapshot could land last. A newer snapshot of
/// the same profile replaces an older one still waiting.
#[derive(Debug, Default)]
pub struct ProfileWriteQueue {
    running: Option<QueuedWrite>,
    waiting: BTreeMap<String, QueuedWrite>,
}

impl ProfileWriteQueue {
    /// Queue `write` for the profile `key`. Returns the write to start now
    /// when nothing is running, else keeps it for [`Self::finished`].
    pub fn submit(&mut self, key: &str, write: QueuedWrite) -> Option<QueuedWrite> {
        if self.running.is_some() {
            self.waiting.insert(key.to_lowercase(), write);
            None
        } else {
            self.running = Some(write.clone());
            Some(write)
        }
    }

    /// The running write ended; returns the next one to start, if any.
    pub fn finished(&mut self) -> Option<QueuedWrite> {
        self.running = None;
        let (_, next) = self.waiting.pop_first()?;
        self.running = Some(next.clone());
        Some(next)
    }

    /// The write that has been handed to a worker and has not reported yet.
    pub fn running(&self) -> Option<&QueuedWrite> {
        self.running.as_ref()
    }

    pub fn is_idle(&self) -> bool {
        self.running.is_none() && self.waiting.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn erc(rule: &str) -> ErcRules {
        let mut erc = ErcRules::default();
        erc.severity.insert(rule.into(), "error".into());
        erc
    }

    #[test]
    fn writes_run_one_at_a_time_and_a_newer_snapshot_replaces_a_waiting_one() {
        let mut queue = ProfileWriteQueue::default();
        assert!(
            queue
                .submit("User Default", QueuedWrite::UserDefault(erc("a")))
                .is_some()
        );
        assert!(
            queue
                .submit("User Default", QueuedWrite::UserDefault(erc("b")))
                .is_none()
        );
        assert!(
            queue
                .submit("user default", QueuedWrite::UserDefault(erc("c")))
                .is_none()
        );
        assert!(
            queue
                .submit(
                    "Other",
                    QueuedWrite::Profile(RuleProfile::new("Other", "1", erc("x")))
                )
                .is_none()
        );

        let Some(QueuedWrite::Profile(next)) = queue.finished() else {
            panic!("the waiting writes start in key order")
        };
        assert_eq!(next.name, "Other");
        let Some(QueuedWrite::UserDefault(next)) = queue.finished() else {
            panic!("the machine rule set is next")
        };
        assert!(
            next.severity.contains_key("c"),
            "only the newest snapshot survives"
        );
        assert!(queue.finished().is_none());
        assert!(queue.is_idle());
    }
}
