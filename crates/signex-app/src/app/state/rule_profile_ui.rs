//! UI state of the rule-profile screens (#1608): the "Save as profile"
//! dialog, the Preferences profile editor, and the queue that keeps profile
//! file writes in order.

use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::sync::{Arc, Mutex};

use signex_types::project::{ErcRules, RuleProfile};

use crate::rule_profiles::{ListedProfile, ProfileKind, ProfileListing, RuleEdit};

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
    /// Reset to the shipped rules waiting for confirmation, by profile name.
    pub confirm_reset: Option<String>,
    /// A create / duplicate / rename / delete is running. Edits wait: they
    /// would be applied to a file that is about to move.
    pub busy: bool,
    /// A listing arrived (or was wanted) while writes were pending; read the
    /// library again once they finish, so the screen never shows a state
    /// older than what was written.
    pub relist_pending: bool,
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

    /// Show what a finished write left in the file, for the profile it names.
    pub fn replace_profile(&mut self, profile: RuleProfile) {
        if let Some(entry) = self
            .listing
            .profiles
            .iter_mut()
            .find(|l| l.profile.name == profile.name && l.kind != ProfileKind::Builtin)
        {
            entry.profile = profile;
        }
    }

    /// Forget the last session's questions and result line.
    pub fn clear_transient(&mut self) {
        self.confirm_delete = None;
        self.confirm_reset = None;
        self.status = None;
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
    /// The machine's rule set (`User Default`): the full snapshot of the
    /// in-memory rules, keeping what the file holds that this build does not
    /// know.
    UserDefault(ErcRules),
    /// One change to a library profile, applied to the file as it is when the
    /// write runs.
    ProfileEdit { name: String, edit: RuleEdit },
}

impl QueuedWrite {
    /// Do the file IO. Returns the profile the file now holds, for a
    /// library-profile edit.
    pub fn execute(&self, dir: &Path) -> Result<Option<RuleProfile>, String> {
        match self {
            QueuedWrite::UserDefault(erc) => {
                crate::rule_profiles::write_user_default(dir, erc.clone()).map(|()| None)
            }
            QueuedWrite::ProfileEdit { name, edit } => {
                crate::rule_profiles::edit_profile_in_place(dir, name, *edit).map(Some)
            }
        }
    }
}

/// The newest write sequence number each profile's file has seen, behind the
/// lock every profile write holds while it runs. It lets the quit path (and a
/// New Project that must read the library) run the writes still queued
/// without any of them being applied twice or out of order.
#[derive(Debug, Default)]
pub struct WriteGate(Mutex<HashMap<String, u64>>);

/// A write handed out by the queue, to be run on a worker.
#[derive(Debug, Clone)]
pub struct Dispatch {
    pub key: String,
    pub seq: u64,
    pub write: QueuedWrite,
    gate: Arc<WriteGate>,
}

impl Dispatch {
    /// Run the write unless it (or a later one for the same profile) already
    /// ran. `Ok(None)` for a write that was skipped.
    pub fn run(&self, dir: &Path) -> Result<Option<RuleProfile>, String> {
        let mut applied = self
            .gate
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let newest = applied.entry(self.key.clone()).or_insert(0);
        if self.seq <= *newest {
            return Ok(None);
        }
        *newest = self.seq;
        self.write.execute(dir)
    }
}

/// Run `writes` in order on the calling (blocking) thread; the first error is
/// returned after all have been tried.
pub fn run_all(dir: &Path, writes: &[Dispatch]) -> Result<(), String> {
    let mut first_error = None;
    for write in writes {
        if let Err(why) = write.run(dir) {
            first_error.get_or_insert(why);
        }
    }
    first_error.map_or(Ok(()), Err)
}

/// Profile file writes, one at a time and in the order the user made them.
///
/// Two edits in quick succession would otherwise run on two worker threads in
/// either order and the older one could land last.
#[derive(Debug, Default)]
pub struct ProfileWriteQueue {
    running: Option<Dispatch>,
    waiting: VecDeque<Dispatch>,
    next_seq: u64,
    gate: Arc<WriteGate>,
}

impl ProfileWriteQueue {
    /// Queue `write` for the profile `key`. Returns the write to start now
    /// when nothing is running, else keeps it for [`Self::finished`].
    pub fn submit(&mut self, key: &str, write: QueuedWrite) -> Option<Dispatch> {
        self.next_seq += 1;
        let dispatch = Dispatch {
            key: key.to_lowercase(),
            seq: self.next_seq,
            write,
            gate: self.gate.clone(),
        };
        if self.running.is_some() {
            self.waiting.push_back(dispatch);
            None
        } else {
            self.running = Some(dispatch.clone());
            Some(dispatch)
        }
    }

    /// The running write ended; returns the next one to start, if any.
    pub fn finished(&mut self) -> Option<Dispatch> {
        self.running = None;
        let next = self.waiting.pop_front()?;
        self.running = Some(next.clone());
        Some(next)
    }

    /// The write that has been handed to a worker and has not reported yet.
    pub fn running(&self) -> Option<&Dispatch> {
        self.running.as_ref()
    }

    /// The running write and every waiting one, oldest first. Running them
    /// (`run_all`) is safe next to the queue's own workers.
    pub fn pending(&self) -> Vec<Dispatch> {
        self.running
            .iter()
            .chain(self.waiting.iter())
            .cloned()
            .collect()
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
    fn writes_run_one_at_a_time_in_the_order_they_were_made() {
        let mut queue = ProfileWriteQueue::default();
        assert!(
            queue
                .submit("A", QueuedWrite::UserDefault(erc("a")))
                .is_some()
        );
        assert!(
            queue
                .submit("A", QueuedWrite::UserDefault(erc("b")))
                .is_none()
        );
        let edit = RuleEdit::PinCell(1, 1);
        let other = QueuedWrite::ProfileEdit {
            name: "Other".into(),
            edit,
        };
        assert!(queue.submit("Other", other).is_none());
        assert_eq!(queue.pending().len(), 3);

        let second = queue.finished().expect("next");
        assert!(
            matches!(second.write, QueuedWrite::UserDefault(ref e) if e.severity.contains_key("b"))
        );
        let third = queue.finished().expect("next");
        assert!(matches!(third.write, QueuedWrite::ProfileEdit { .. }));
        assert!(queue.finished().is_none());
        assert!(queue.is_idle());
    }

    #[test]
    fn a_dispatch_runs_once_even_if_run_twice() {
        let dir = tempfile::tempdir().unwrap();
        crate::rule_profiles::save_named_profile(
            dir.path(),
            "P",
            "1.0",
            ErcRules::default(),
            false,
        )
        .unwrap();
        let mut queue = ProfileWriteQueue::default();
        let edit = QueuedWrite::ProfileEdit {
            name: "P".into(),
            edit: RuleEdit::PinCell(1, 1),
        };
        let first = queue.submit("P", edit).unwrap();
        // Cycling a cell twice would undo it: a second run must not happen.
        assert!(first.run(dir.path()).unwrap().is_some());
        assert!(first.run(dir.path()).unwrap().is_none(), "skipped");
        let bytes = std::fs::read(dir.path().join("p.snxrules")).unwrap();
        let stored = RuleProfile::from_json_bytes(&bytes).unwrap();
        assert_eq!(stored.erc.pin_matrix.len(), 1);
    }
}
