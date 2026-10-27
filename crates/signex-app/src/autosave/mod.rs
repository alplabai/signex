//! Opt-in autosave of dirty schematics, symbols and footprints to a
//! recovery sidecar (#104, #1185).
//!
//! Off by default (Preferences > Editing). When on, a timer subscription
//! asks every [`AUTOSAVE_INTERVAL`] for a recovery copy of each dirty
//! schematic, symbol editor and footprint editor. The copy goes to a [`sidecar`] file; the user's real file is
//! never touched. Writing needs the preference; removing a sidecar does
//! not, so a copy written earlier is still cleaned up after the
//! preference goes off.
//!
//! ADR-0003 checklist for this deferred work:
//!
//! 1. Owed/in-flight state is [`AutosaveState`] in `DocumentState`: per
//!    path, the [`DocStamp`] last written and whether a write is in flight.
//! 2. No boundary needs a flush: a normal save supersedes any in-flight
//!    write and removes the sidecar (see `handlers/autosave.rs`). Quit is
//!    the one boundary that waits, see 7.
//! 3. A sidecar is a serialised snapshot taken in `update`, never an undo
//!    snapshot.
//! 4. A completion removes its own sidecar when the document was saved or
//!    closed meanwhile, so a late write cannot resurrect stale content.
//! 5. A panic or dropped task maps to `Err` in [`AutosaveDone`], is
//!    reported once to the Messages panel, and frees the in-flight slot.
//! 6. Every disk write goes through `atomic_write`.
//! 7. Quit waits for every write and removal still running
//!    ([`AutosaveBarrier`]), and a write that a save, a close or a
//!    discarding quit superseded removes its own result inside the task, so
//!    no completion message has to be processed after the app stops.
//! 8. A sidecar the previous run left behind is reconciled against its
//!    document when the document is opened ([`recovery::reconcile`]): one
//!    byte-identical to the file is removed, a differing one is offered
//!    through [`RecoveryOffer`] whatever its mtime.

pub mod recovery;
pub mod sidecar;

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::Duration;

use iced::Subscription;
use serde::Serialize;

/// How often the autosave timer fires while the preference is on.
pub const AUTOSAVE_INTERVAL: Duration = Duration::from_secs(60);

/// The revision of a document a sidecar was taken at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct DocStamp {
    pub revision: u64,
    pub net_color_revision: u64,
}

impl DocStamp {
    /// The stamp of a document with no revision counter (a symbol or
    /// footprint editor): a hash of the bytes the sidecar would hold, so an
    /// unchanged draft is not rewritten.
    pub fn of_bytes(bytes: &[u8]) -> Self {
        Self {
            revision: sidecar::bytes_hash(bytes),
            net_color_revision: 0,
        }
    }
}

/// Autosave messages. Namespaced under `Message::Autosave`.
#[derive(Debug, Clone, Serialize)]
pub enum AutosaveMsg {
    /// The autosave timer fired.
    Ticked,
    /// A background sidecar write finished.
    Written(AutosaveDone),
    /// A background sidecar removal finished; the error text is what the
    /// Messages panel shows.
    Discarded(AutosaveRemoved),
    /// The off-thread check of a just-opened document's sidecar finished.
    RecoveryChecked(RecoveryChecked),
    /// The user answered the recovery prompt.
    Recovery(RecoveryChoice),
    /// The sidecar chosen for Restore was read and parsed off-thread.
    RestoreLoaded(RestoreLoaded),
    /// The startup sweep of stranded temp files finished.
    StartupSwept(Result<usize, String>),
    /// A recovery copy followed its document to a new path (rename or
    /// project folder move), or failed to.
    Moved(AutosaveMoved),
}

/// The user's answer to the recovery prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum RecoveryChoice {
    /// Load the recovery copy as unsaved edits.
    Restore,
    /// Delete the recovery copy.
    Discard,
    /// Close the prompt and keep the copy for the next open. What Esc does.
    Later,
}

/// Result of checking one document's sidecar.
#[derive(Debug, Clone, Serialize)]
pub struct RecoveryChecked {
    pub path: PathBuf,
    pub result: Result<Option<recovery::Recoverable>, String>,
}

/// A recovery copy parsed for a Restore, by kind of document.
#[derive(Debug, Clone)]
pub enum RestoredDocument {
    Schematic(Box<signex_types::schematic::SchematicSheet>),
    Symbol(Box<signex_library::SymbolFile>),
    Footprint(Box<signex_library::FootprintFile>),
}

/// Result of reading the sidecar a Restore chose.
#[derive(Debug, Clone)]
pub struct RestoreLoaded {
    pub path: PathBuf,
    pub result: Result<RestoredDocument, String>,
}

impl Serialize for RestoreLoaded {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // The parsed sheet is not part of a message trace.
        let outcome: Result<(), &String> = self.result.as_ref().map(|_| ());
        (&self.path, outcome).serialize(serializer)
    }
}

/// One document with a recovery copy that differs from its file, waiting for an
/// answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecoveryOffer {
    pub path: PathBuf,
    /// How long before the check the copy was last written, in seconds.
    pub age_secs: u64,
}

/// "5 minutes ago" for the recovery prompt.
pub fn describe_age(age_secs: u64) -> String {
    let (count, unit) = match age_secs {
        0..=59 => return "less than a minute ago".to_string(),
        60..=3599 => (age_secs / 60, "minute"),
        3600..=86_399 => (age_secs / 3600, "hour"),
        _ => (age_secs / 86_400, "day"),
    };
    let plural = if count == 1 { "" } else { "s" };
    format!("{count} {unit}{plural} ago")
}

/// Completion of one sidecar removal.
#[derive(Debug, Clone, Serialize)]
pub struct AutosaveRemoved {
    pub path: PathBuf,
    pub result: Result<(), String>,
}

/// One sidecar rename owed after a document moved from `old` to `new`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SidecarMove {
    pub old: PathBuf,
    pub new: PathBuf,
    /// The copy came from an earlier run (offered or "Decide later"): it
    /// may hold edits nothing else has, so it is never dropped, only moved.
    pub earlier_run: bool,
    /// `new` is already held as "removing" by this move (a retry), so no
    /// autosave write lands there before the copy does.
    held: bool,
}

/// Completion of one sidecar move.
#[derive(Debug, Clone, Serialize)]
pub struct AutosaveMoved {
    pub sidecar: SidecarMove,
    pub result: Result<(), String>,
}

/// Completion of one sidecar write.
#[derive(Debug, Clone, Serialize)]
pub struct AutosaveDone {
    pub path: PathBuf,
    pub stamp: DocStamp,
    pub result: Result<(), String>,
}

#[derive(Debug, Default)]
struct Ledger {
    running: Mutex<usize>,
    idle: Condvar,
}

fn relock<G>(mutex: &Mutex<G>) -> MutexGuard<'_, G> {
    // The guarded value is a plain counter; a panicked task cannot leave
    // it half-updated.
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Blocks until every write and removal handed out so far has finished.
#[derive(Debug, Clone)]
pub struct AutosaveBarrier(Arc<Ledger>);

impl AutosaveBarrier {
    pub fn wait(&self) {
        let mut running = relock(&self.0.running);
        while *running > 0 {
            running = self
                .0
                .idle
                .wait(running)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
    }
}

/// Holds [`AutosaveBarrier::wait`] open while alive. Moved into the
/// blocking task, so a task that is dropped unrun also releases it.
#[derive(Debug)]
pub struct InFlight(Arc<Ledger>);

impl Drop for InFlight {
    fn drop(&mut self) {
        let mut running = relock(&self.0.running);
        *running -= 1;
        if *running == 0 {
            self.0.idle.notify_all();
        }
    }
}

/// What autosave owes and has done, per document path.
#[derive(Debug, Clone, Default, Serialize)]
pub struct AutosaveState {
    dir: Option<PathBuf>,
    /// Documents with a sidecar on disk, and the stamp it holds (`None`
    /// for a copy found on disk, whose stamp is unknown).
    written: HashMap<PathBuf, Option<DocStamp>>,
    /// Documents with a write running; the flag is "a save, a close or a
    /// discarding quit landed meanwhile, so the result must not be kept".
    /// Shared with the write task, which removes its own result when set.
    #[serde(skip)]
    in_flight: HashMap<PathBuf, Arc<AtomicBool>>,
    #[serde(skip)]
    ledger: Arc<Ledger>,
    /// Documents whose recovery copy with edits the file lacks awaits Restore / Discard; the
    /// first is the one the prompt shows.
    offers: VecDeque<RecoveryOffer>,
    /// Documents with a removal running (how many). A new write for the
    /// same path waits for them, so a slow removal can never delete a
    /// sidecar that `written` already records.
    removing: HashMap<PathBuf, u32>,
    /// Documents whose earlier-run recovery copy the user chose to keep
    /// ("Decide later"). Autosave writes nothing for them while they stay
    /// open, because the one sidecar per document is that copy.
    deferred: HashSet<PathBuf>,
    /// The last failure shown, so a failing disk is reported once rather
    /// than every tick.
    last_error: Option<String>,
    /// Sidecar renames not started yet: queued by a document move, or
    /// failed and retried on the next tick.
    moves: Vec<SidecarMove>,
}

/// The recovery directory under the per-user config root, if the platform
/// has one.
pub fn default_recovery_dir() -> Option<PathBuf> {
    crate::config_root::config_root().map(|root| root.join("recovery"))
}

impl AutosaveState {
    pub fn new(dir: Option<PathBuf>) -> Self {
        Self {
            dir,
            ..Self::default()
        }
    }

    pub fn dir(&self) -> Option<&Path> {
        self.dir.as_deref()
    }

    /// Point autosave at another recovery directory.
    pub fn set_dir(&mut self, dir: Option<PathBuf>) {
        self.dir = dir;
    }

    /// Does `path` at `stamp` need a new sidecar?
    pub fn needs_write(&self, path: &Path, stamp: DocStamp) -> bool {
        !self.deferred.contains(path)
            && !self.in_flight.contains_key(path)
            && !self.removing.contains_key(path)
            && self.written.get(path) != Some(&Some(stamp))
    }

    /// A barrier over every write and removal handed out so far (quit
    /// waits on it).
    pub fn barrier(&self) -> AutosaveBarrier {
        AutosaveBarrier(Arc::clone(&self.ledger))
    }

    /// Count one more task the barrier must wait for. Call it on the UI
    /// thread when the task is created and move the guard into the task.
    pub fn track_job(&self) -> InFlight {
        *relock(&self.ledger.running) += 1;
        InFlight(Arc::clone(&self.ledger))
    }

    /// The sidecar of `path` is on disk and was not written by this run
    /// (a Restore keeps it): track it so the next save removes it.
    pub fn adopt_on_disk(&mut self, path: &Path) {
        self.written.entry(path.to_path_buf()).or_insert(None);
    }

    /// Queue a document with a recovery copy that differs from its file for the prompt.
    pub fn offer(&mut self, offer: RecoveryOffer) {
        self.deferred.remove(&offer.path);
        if !self.offers.iter().any(|o| o.path == offer.path) {
            self.offers.push_back(offer);
        }
    }

    /// The user kept `path`'s earlier-run copy: no write may replace it
    /// until the document closes or is offered again.
    pub fn defer(&mut self, path: &Path) {
        self.deferred.insert(path.to_path_buf());
    }

    /// Stop protecting the copies of documents for which `is_open` is false.
    pub fn retain_deferred(&mut self, is_open: impl Fn(&Path) -> bool) {
        self.deferred.retain(|p| is_open(p));
    }

    /// The offer the prompt shows now.
    pub fn current_offer(&self) -> Option<&RecoveryOffer> {
        self.offers.front()
    }

    /// The prompt was answered or dismissed: the next offer, if any, shows.
    pub fn pop_offer(&mut self) -> Option<RecoveryOffer> {
        self.offers.pop_front()
    }

    /// Keep only the offers whose document `is_open` (a closed document has
    /// nothing to restore into; its copy stays on disk for the next open).
    pub fn retain_offers(&mut self, is_open: impl Fn(&Path) -> bool) {
        self.offers.retain(|o| is_open(&o.path));
    }

    /// A removal for `path` was started.
    pub fn begin_remove(&mut self, path: &Path) {
        *self.removing.entry(path.to_path_buf()).or_insert(0) += 1;
    }

    /// A removal for `path` finished; its writes may run again once none
    /// is left.
    pub fn finish_remove(&mut self, path: &Path) {
        Self::release(&mut self.removing, path);
    }

    /// Every document with a sidecar on disk, no longer tracked. For the
    /// quit that discards all unsaved work.
    pub fn take_all_written(&mut self) -> Vec<PathBuf> {
        // A copy whose move is still owed sits under its old key.
        let unmoved: Vec<PathBuf> = self.moves.drain(..).map(|m| m.old).collect();
        self.written
            .drain()
            .map(|(path, _)| path)
            .chain(unmoved)
            .collect()
    }

    /// Mark every write now running as superseded, so its result is
    /// removed when it lands. For the quit that discards all unsaved work.
    pub fn supersede_all_in_flight(&mut self) {
        for superseded in self.in_flight.values() {
            superseded.store(true, Ordering::SeqCst);
        }
    }

    /// A synchronous save of `path` landed: a write running now is stale.
    /// The sidecar already on disk stays tracked, so the next sweep (the
    /// document is no longer dirty) removes it.
    pub fn supersede_in_flight(&mut self, path: &Path) {
        if let Some(superseded) = self.in_flight.get(path) {
            superseded.store(true, Ordering::SeqCst);
        }
    }

    /// A write for `path` starts. The returned flag is shared with the
    /// task: once set, the task removes the sidecar it just wrote.
    pub fn begin(&mut self, path: &Path) -> Arc<AtomicBool> {
        let superseded = Arc::new(AtomicBool::new(false));
        self.in_flight
            .insert(path.to_path_buf(), Arc::clone(&superseded));
        superseded
    }

    /// A write finished. Returns true when it was superseded by a save.
    pub fn finish(&mut self, path: &Path, stamp: DocStamp, ok: bool) -> bool {
        let superseded = self
            .in_flight
            .remove(path)
            .is_some_and(|flag| flag.load(Ordering::SeqCst));
        if ok && !superseded {
            self.written.insert(path.to_path_buf(), Some(stamp));
        }
        superseded
    }

    /// A normal save landed: any write running now is stale. Returns true
    /// when a sidecar is on disk and must be removed.
    pub fn saved(&mut self, path: &Path) -> bool {
        self.supersede_in_flight(path);
        self.written.remove(path).is_some()
    }

    /// The document at `old` now lives at `new`: its bookkeeping follows and
    /// its sidecar is queued to move ([`Self::take_moves`]). Without it a
    /// crash before the next tick leaves the recovery copy under the old
    /// path, where nothing looks for it.
    pub fn rekey(&mut self, old: &Path, new: &Path) {
        // A write running for `old` would land a sidecar nobody tracks.
        self.supersede_in_flight(old);
        let earlier_run = self.deferred.contains(old) || self.offers.iter().any(|o| o.path == old);
        // A move still owed to `old` now goes straight to `new`; the copy
        // is at its source, not at `old`.
        let mut repointed = false;
        for owed in self.moves.iter_mut().filter(|m| m.new == old) {
            owed.new = new.to_path_buf();
            owed.earlier_run |= earlier_run;
            repointed = true;
            if std::mem::take(&mut owed.held) {
                Self::release(&mut self.removing, old);
            }
        }
        if let Some(stamp) = self.written.remove(old) {
            self.written.insert(new.to_path_buf(), stamp);
        }
        if self.deferred.remove(old) {
            self.deferred.insert(new.to_path_buf());
        }
        for offer in self.offers.iter_mut().filter(|o| o.path == old) {
            offer.path = new.to_path_buf();
        }
        if !repointed {
            self.moves.push(SidecarMove {
                old: old.to_path_buf(),
                new: new.to_path_buf(),
                earlier_run,
                held: false,
            });
        }
    }

    fn release(removing: &mut HashMap<PathBuf, u32>, path: &Path) {
        if let Some(count) = removing.get_mut(path) {
            *count -= 1;
            if *count == 0 {
                removing.remove(path);
            }
        }
    }

    /// Whether autosave still has anything for the document at `path`: a
    /// copy it wrote, one on offer, or one the user kept.
    fn tracks(&self, path: &Path) -> bool {
        self.written.contains_key(path)
            || self.deferred.contains(path)
            || self.offers.iter().any(|o| o.path == path)
    }

    /// A copy of this run for a document saved or closed since: nothing to
    /// recover, so it is removed instead of moved.
    fn is_obsolete(&self, owed: &SidecarMove) -> bool {
        !owed.earlier_run && !self.tracks(&owed.new)
    }

    /// The rename of `failed.old`'s sidecar to `failed.new` failed (a file
    /// lock, say): the copy is still under the old key. It is retried on
    /// the next tick, with `new` held so no write lands there first.
    /// Returns the old key to remove instead when the copy is obsolete.
    pub fn move_failed(&mut self, mut failed: SidecarMove) -> Option<PathBuf> {
        failed.held = true;
        // The document moved on again while this rename ran: go straight
        // to its newest path.
        if let Some(i) = self.moves.iter().position(|m| m.old == failed.new) {
            let onward = self.moves.remove(i);
            Self::release(&mut self.removing, &failed.new);
            failed.new = onward.new;
            failed.earlier_run |= onward.earlier_run;
            failed.held = false;
        }
        if self.is_obsolete(&failed) {
            if failed.held {
                Self::release(&mut self.removing, &failed.new);
            }
            return Some(failed.old);
        }
        self.moves.push(failed);
        None
    }

    /// The renames to start now, each with its new path held, and the old
    /// keys of obsolete copies to remove instead. A rename whose source is
    /// itself the target of a rename still running waits for it.
    pub fn take_moves(&mut self) -> (Vec<SidecarMove>, Vec<PathBuf>) {
        let (mut start, mut obsolete, mut wait) = (Vec::new(), Vec::new(), Vec::new());
        for owed in std::mem::take(&mut self.moves) {
            if self.removing.contains_key(&owed.old) {
                wait.push(owed);
            } else if self.is_obsolete(&owed) {
                if owed.held {
                    Self::release(&mut self.removing, &owed.new);
                }
                obsolete.push(owed.old);
            } else {
                start.push(owed);
            }
        }
        self.moves = wait;
        for owed in &mut start {
            if !std::mem::replace(&mut owed.held, true) {
                self.begin_remove(&owed.new);
            }
        }
        (start, obsolete)
    }

    /// Stop tracking `path`'s sidecar. Returns true if one was on disk.
    pub fn forget(&mut self, path: &Path) -> bool {
        self.written.remove(path).is_some()
    }

    /// Paths with a sidecar for which `is_live` says there is nothing left
    /// to recover (saved, closed or discarded).
    pub fn stale(&self, is_live: impl Fn(&Path) -> bool) -> Vec<PathBuf> {
        self.written
            .keys()
            .filter(|p| !is_live(p))
            .cloned()
            .collect()
    }

    /// Record a failure; true when it differs from the last one shown.
    pub fn should_report(&mut self, error: &str) -> bool {
        if self.last_error.as_deref() == Some(error) {
            return false;
        }
        self.last_error = Some(error.to_string());
        true
    }

    pub fn clear_error(&mut self) {
        self.last_error = None;
    }
}

/// The timer; empty while autosave is off.
pub fn subscription(enabled: bool) -> Subscription<AutosaveMsg> {
    if enabled {
        iced::time::every(AUTOSAVE_INTERVAL).map(|_| AutosaveMsg::Ticked)
    } else {
        Subscription::none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_barrier_waits_for_a_running_job_and_a_dropped_job_releases_it() {
        let state = AutosaveState::default();
        let job = state.track_job();
        let barrier = state.barrier();
        let waiter = std::thread::spawn(move || barrier.wait());
        std::thread::sleep(Duration::from_millis(50));
        assert!(!waiter.is_finished(), "wait must block while a job runs");
        drop(job);
        waiter.join().unwrap();
    }

    #[test]
    fn a_stamp_unknown_copy_is_rewritten_on_the_next_tick_and_removed_on_save() {
        let mut state = AutosaveState::default();
        let path = Path::new("a.snxsch");
        let stamp = DocStamp {
            revision: 3,
            net_color_revision: 0,
        };
        state.adopt_on_disk(path);
        assert!(state.needs_write(path, stamp));
        assert!(state.saved(path), "an adopted copy is removed by a save");
    }

    #[test]
    fn ages_read_as_the_largest_whole_unit() {
        assert_eq!(describe_age(5), "less than a minute ago");
        assert_eq!(describe_age(60), "1 minute ago");
        assert_eq!(describe_age(125), "2 minutes ago");
        assert_eq!(describe_age(3600), "1 hour ago");
        assert_eq!(describe_age(7300), "2 hours ago");
        assert_eq!(describe_age(172_800), "2 days ago");
    }

    #[test]
    fn a_deferred_copy_blocks_writes_until_the_document_closes() {
        let mut state = AutosaveState::default();
        let path = Path::new("a.snxsch");
        let stamp = DocStamp {
            revision: 1,
            net_color_revision: 0,
        };
        assert!(state.needs_write(path, stamp));
        state.defer(path);
        assert!(!state.needs_write(path, stamp));
        state.retain_deferred(|_| true);
        assert!(!state.needs_write(path, stamp));
        state.retain_deferred(|_| false);
        assert!(state.needs_write(path, stamp));
    }

    fn tracked_at(path: &Path) -> AutosaveState {
        let mut state = AutosaveState::default();
        let stamp = DocStamp::of_bytes(b"x");
        state.begin(path);
        state.finish(path, stamp, true);
        state
    }

    fn started(state: &mut AutosaveState) -> Vec<(PathBuf, PathBuf)> {
        let (start, obsolete) = state.take_moves();
        assert!(obsolete.is_empty());
        start.into_iter().map(|m| (m.old, m.new)).collect()
    }

    #[test]
    fn a_failed_move_retried_after_a_second_rename_goes_to_the_newest_path() {
        let (a, b, c) = (Path::new("a"), Path::new("b"), Path::new("c"));
        let mut state = tracked_at(a);
        state.rekey(a, b);
        let (mut start, _) = state.take_moves();
        let running = start.remove(0);
        assert!(state.move_failed(running).is_none(), "retried");
        state.rekey(b, c);
        assert_eq!(started(&mut state), [(a.into(), c.into())]);
        assert!(!state.removing.contains_key(b), "b is no longer held");
    }

    #[test]
    fn a_second_rename_while_the_first_move_runs_waits_then_chains() {
        let (a, b, c) = (Path::new("a"), Path::new("b"), Path::new("c"));
        let mut state = tracked_at(a);
        state.rekey(a, b);
        let (mut start, _) = state.take_moves();
        let running = start.remove(0);
        state.rekey(b, c);
        assert!(started(&mut state).is_empty(), "b -> c waits for a -> b");
        assert!(state.move_failed(running).is_none());
        assert_eq!(started(&mut state), [(a.into(), c.into())]);
    }

    #[test]
    fn a_failed_move_of_a_saved_document_removes_the_old_copy() {
        let (a, b) = (Path::new("a"), Path::new("b"));
        let mut state = tracked_at(a);
        state.rekey(a, b);
        let (mut start, _) = state.take_moves();
        state.saved(b);
        assert_eq!(state.move_failed(start.remove(0)), Some(a.into()));
        assert!(state.removing.is_empty(), "the hold on b is released");
    }

    #[test]
    fn offers_queue_once_per_document_in_order() {
        let mut state = AutosaveState::default();
        let offer = |name: &str| RecoveryOffer {
            path: PathBuf::from(name),
            age_secs: 5,
        };
        state.offer(offer("a"));
        state.offer(offer("a"));
        state.offer(offer("b"));
        assert_eq!(state.pop_offer(), Some(offer("a")));
        assert_eq!(state.current_offer(), Some(&offer("b")));
        state.retain_offers(|p| p != Path::new("b"));
        assert!(state.current_offer().is_none());
    }
}
