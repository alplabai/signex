//! Opt-in autosave of dirty schematics, PCBs, symbols and footprints to a
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
//!    through [`RecoveryOffer`] whatever its mtime, with a warning when the
//!    copy is older than the file. From open until that check resolves, and
//!    while its offer is unanswered, no write may replace the copy.

mod keys;
pub mod pending_move;
pub mod recovery;
pub mod sidecar;

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::Duration;

use iced::Subscription;
use serde::Serialize;

/// How long quit waits for autosave work before exiting anyway.
pub const QUIT_WAIT_LIMIT: Duration = Duration::from_secs(10);

/// Failed renames of one recovery copy after which autosave may write a
/// fresh copy to the new key instead of waiting for the rename (#1433).
pub const FAILED_MOVES_BEFORE_FRESH_COPY: u32 = 2;

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
    StartupSwept(Result<recovery::SweepReport, String>),
    /// A recovery copy followed its document to a new path (rename or
    /// project folder move), or failed to.
    Moved(AutosaveMoved),
    /// A dropped owed move's journal entry was cleared off-thread.
    JournalCleared,
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
    Pcb(Box<signex_types::pcb::PcbBoard>),
}

/// Result of reading the sidecar a Restore chose.
#[derive(Debug, Clone)]
pub struct RestoreLoaded {
    pub path: PathBuf,
    /// The offer the user accepted, so a refused Restore can put it back.
    pub offer: RecoveryOffer,
    pub result: Result<RestoredDocument, recovery::LoadError>,
}

impl Serialize for RestoreLoaded {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // The parsed sheet is not part of a message trace.
        let outcome: Result<(), &recovery::LoadError> = self.result.as_ref().map(|_| ());
        (&self.path, &self.offer, outcome).serialize(serializer)
    }
}

/// One document with a recovery copy that differs from its file, waiting for an
/// answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecoveryOffer {
    pub path: PathBuf,
    /// How long before the check the copy was last written, in seconds.
    pub age_secs: u64,
    /// The copy is older than the saved file (see
    /// [`recovery::Recoverable::older_than_file`]).
    pub older_than_file: bool,
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
#[derive(Debug, Clone, Serialize)]
pub struct SidecarMove {
    pub old: PathBuf,
    pub new: PathBuf,
    /// The copy is one this run wrote. Only such a copy is ever dropped
    /// (its document saved or closed since); any other may be an earlier
    /// run's crash copy and is always moved.
    pub this_run: bool,
    /// `new` is already held as "removing" by this move, so no autosave
    /// write lands there before the copy does.
    held: bool,
    /// Renames that failed so far. At [`FAILED_MOVES_BEFORE_FRESH_COPY`] the
    /// hold on `new` is given up so a fresh copy can be written there.
    #[serde(skip)]
    attempts: u32,
    /// The hold on `new` was given up after repeated failures: autosave
    /// writes the current content to `new` while this move stays owed, only
    /// to bring the old copy over (newest wins) or remove it.
    #[serde(skip)]
    released: bool,
    /// Every target this move was journalled or repointed to, so dropping it
    /// clears only an entry that still names one of them.
    #[serde(skip)]
    targets: Vec<PathBuf>,
    /// Set by a discarding quit while the rename runs: the task removes the
    /// copy it just moved.
    #[serde(skip)]
    discard: Arc<AtomicBool>,
}

impl SidecarMove {
    /// A discarding quit arrived while the rename ran.
    pub fn is_discarded(&self) -> bool {
        self.discard.load(Ordering::SeqCst)
    }
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
    /// Wait for every job, at most `limit`. False when jobs were still
    /// running at the limit.
    pub fn wait_timeout(&self, limit: Duration) -> bool {
        let running = relock(&self.0.running);
        let (running, _) = self
            .0
            .idle
            .wait_timeout_while(running, limit, |running| *running > 0)
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *running == 0
    }

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

/// A save's claim on the quit barrier; see
/// [`AutosaveState::hold_for_save`]. Clones share one claim.
#[derive(Debug, Clone)]
pub struct SaveHold(#[expect(dead_code, reason = "held only for its Drop")] Arc<InFlight>);

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
    /// Writes still running for a document renamed meanwhile, old path to
    /// new. The write lands under the old key and is recorded under the new
    /// one; its move waits for it.
    forwarded: HashMap<PathBuf, PathBuf>,
    /// Sidecar renames running now.
    running_moves: Vec<SidecarMove>,
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
    /// Documents just opened whose recovery check has not resolved (how
    /// many). Until it does an earlier run's copy may exist unseen, so no
    /// write may replace it.
    #[serde(skip)]
    checking: HashMap<PathBuf, u32>,
    /// The last failure shown, so a failing disk is reported once rather
    /// than every tick.
    last_error: Option<String>,
    /// Sidecar renames not started yet: queued by a document move, or
    /// failed and retried on the next tick.
    moves: Vec<SidecarMove>,
    /// Sources whose owed move was dropped (the document came back): their
    /// journal entries are cleared by the next flush.
    #[serde(skip)]
    journal_clears: Vec<(PathBuf, Vec<PathBuf>)>,
    /// Documents whose recovery copy a Restore is reading right now: the
    /// path the read was issued for, and where the document lives now.
    restoring: HashMap<PathBuf, PathBuf>,
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
            && !self.checking.contains_key(path)
            && !self.offers.iter().any(|offer| offer.path == path)
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

    /// A recovery check for `path` was started (the document just opened).
    pub fn begin_check(&mut self, path: &Path) {
        *self.checking.entry(path.to_path_buf()).or_insert(0) += 1;
    }

    /// A recovery check for `path` resolved, with an offer, without one, or
    /// with an error.
    pub fn finish_check(&mut self, path: &Path) {
        if let Some(count) = self.checking.get_mut(path) {
            *count -= 1;
            if *count == 0 {
                self.checking.remove(path);
            }
        }
    }

    /// The user kept `path`'s earlier-run copy: no write may replace it
    /// until the document closes or is offered again.
    pub fn defer(&mut self, path: &Path) {
        self.deferred.insert(path.to_path_buf());
    }

    /// `path`'s copy was loaded into the document: it is this session's
    /// own again, so autosave may write for it.
    pub fn allow_writes(&mut self, path: &Path) {
        self.deferred.remove(path);
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

    /// A document save is starting: the hold keeps the barrier closed until
    /// the hold is dropped, which the save's completion message does after
    /// its copy removal was registered.
    pub fn hold_for_save(&self) -> SaveHold {
        SaveHold(Arc::new(self.track_job()))
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
        // A copy of this run whose move is still owed sits under its old
        // key. Any other copy's move stays owed: the quit runs it
        // ([`Self::take_moves`]), since it is not this quit's to discard.
        let (this_run, others): (Vec<_>, Vec<_>) = std::mem::take(&mut self.moves)
            .into_iter()
            .partition(|m| m.this_run);
        self.moves = others;
        let unmoved: Vec<PathBuf> = this_run
            .into_iter()
            .map(|m| {
                if m.held {
                    Self::release(&mut self.removing, &m.new);
                }
                m.old
            })
            .collect();
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
        // A copy of this run being renamed lands after the removals start;
        // its task removes it.
        for running in self.running_moves.iter().filter(|m| m.this_run) {
            running.discard.store(true, Ordering::SeqCst);
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
        let key = self
            .forwarded
            .remove(path)
            .unwrap_or_else(|| path.to_path_buf());
        let superseded = self
            .in_flight
            .remove(path)
            .is_some_and(|flag| flag.load(Ordering::SeqCst));
        if ok && !superseded {
            self.written.insert(key, Some(stamp));
        }
        superseded
    }

    /// Whether the write running for `path` belongs to a document renamed
    /// since: its copy is owed a move and must not be removed under `path`.
    pub fn is_forwarded(&self, path: &Path) -> bool {
        self.forwarded.contains_key(path)
    }

    /// A normal save landed: any write running now is stale. Returns true
    /// when a sidecar is on disk and must be removed.
    pub fn saved(&mut self, path: &Path) -> bool {
        self.supersede_in_flight(path);
        // A rename of this run's copy into `path` that is running now would
        // land after the removal of `path`'s copy and bring back an outdated
        // one; its task removes what it moved.
        for running in self
            .running_moves
            .iter()
            .filter(|m| m.this_run && m.new == path)
        {
            running.discard.store(true, Ordering::SeqCst);
        }
        self.written.remove(path).is_some()
    }

    /// The document at `old` now lives at `new`: its bookkeeping follows and
    /// its sidecar is queued to move ([`Self::take_moves`]). Without it a
    /// crash before the next tick leaves the recovery copy under the old
    /// path, where nothing looks for it.
    pub fn rekey(&mut self, old: &Path, new: &Path) {
        // A write running for `old` lands there; it is recorded under `new`
        // and its copy moves once it has landed ([`Self::take_moves`]).
        for target in self.forwarded.values_mut().filter(|p| p.as_path() == old) {
            *target = new.to_path_buf();
        }
        if self.in_flight.contains_key(old) {
            self.forwarded.insert(old.to_path_buf(), new.to_path_buf());
        }
        let mut restoring = false;
        for current in self.restoring.values_mut().filter(|p| p.as_path() == old) {
            *current = new.to_path_buf();
            restoring = true;
        }
        // Decided positively: only a copy this run wrote may later be
        // dropped. A copy of a sheet that is not open, or one offered, kept
        // or being restored, is an earlier run's and always moves.
        let this_run = matches!(self.written.get(old), Some(Some(_)))
            && !restoring
            && !self.deferred.contains(old)
            && !self.offers.iter().any(|o| o.path == old);
        // A Restore reading the copy left the offer; until it reports, the
        // copy is kept like "Decide later" so nothing writes over it.
        if restoring {
            self.deferred.insert(new.to_path_buf());
        }
        // A move still owed to `old` now goes straight to `new`; the copy
        // is at its source, not at `old`.
        let mut repointed = false;
        for owed in self
            .moves
            .iter_mut()
            .filter(|m| m.new == old && !m.released)
        {
            owed.new = new.to_path_buf();
            owed.targets.push(new.to_path_buf());
            owed.this_run &= this_run;
            repointed = true;
            if owed.held {
                Self::release(&mut self.removing, old);
                *self.removing.entry(new.to_path_buf()).or_insert(0) += 1;
            }
        }
        // Renamed back to where the copy sits: nothing left to move.
        let (removing, journal_clears) = (&mut self.removing, &mut self.journal_clears);
        self.moves.retain(|m| {
            let back_home = m.old == m.new;
            if back_home && m.held {
                Self::release(removing, &m.new);
            }
            if back_home {
                journal_clears.push((m.old.clone(), m.targets.clone()));
            }
            !back_home
        });
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
            // Held from now on, so no fresh copy lands at `new` and is then
            // overwritten by the older one this move brings.
            self.begin_remove(new);
            self.moves.push(SidecarMove {
                old: old.to_path_buf(),
                new: new.to_path_buf(),
                this_run,
                held: true,
                attempts: 0,
                released: false,
                targets: vec![new.to_path_buf()],
                discard: Arc::default(),
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
        owed.this_run && !self.tracks(&owed.new)
    }

    /// A fresh copy of `path` landed while a released move of this run's
    /// older copy into `path` is still owed (and not running): the older
    /// copy holds nothing the fresh one lacks. Returns its old key to
    /// remove, once the fresh copy is safely on disk.
    pub fn take_obsolete_released(&mut self, path: &Path) -> Option<PathBuf> {
        let i = self.moves.iter().position(|m| {
            m.released
                && m.this_run
                && m.new == path
                && !self
                    .running_moves
                    .iter()
                    .any(|r| r.old == m.old && r.new == m.new)
        })?;
        Some(self.moves.remove(i).old)
    }

    /// The sources whose journal entries must be cleared now.
    pub fn take_journal_clears(&mut self) -> Vec<(PathBuf, Vec<PathBuf>)> {
        std::mem::take(&mut self.journal_clears)
    }

    /// Whether a sidecar move is owed or running: the quit and the timer
    /// keep running them while autosave itself is off.
    pub fn has_owed_moves(&self) -> bool {
        !self.moves.is_empty()
    }

    /// The rename `moved` finished, either way: it no longer runs.
    fn end_running(&mut self, moved: &SidecarMove) {
        if let Some(i) = self
            .running_moves
            .iter()
            .position(|m| m.old == moved.old && m.new == moved.new)
        {
            self.running_moves.remove(i);
        }
    }

    /// The rename `moved` landed: release its hold on the new key.
    pub fn move_landed(&mut self, moved: &SidecarMove) {
        self.end_running(moved);
        if moved.held {
            Self::release(&mut self.removing, &moved.new);
        }
    }

    /// The rename of `failed.old`'s sidecar to `failed.new` failed (a file
    /// lock, say): the copy is still under the old key. It is retried on
    /// the next tick, with `new` held so no write lands there first.
    /// Returns the old key to remove instead when the copy is obsolete.
    pub fn move_failed(&mut self, mut failed: SidecarMove) -> Option<PathBuf> {
        self.end_running(&failed);
        failed.attempts += 1;
        failed.released |= failed.attempts >= FAILED_MOVES_BEFORE_FRESH_COPY;
        // A move that keeps failing must not freeze recovery: give up the
        // hold on `new` so the next write puts the current content there.
        // The old copy stays owed a move, which brings it over only if it is
        // newer, and is never deleted.
        let was_held = failed.held;
        failed.held = !failed.released;
        if failed.released && was_held {
            Self::release(&mut self.removing, &failed.new);
        }
        // The document moved on again while this rename ran: go straight
        // to its newest path. (A released move's fresh copy sits at `new`;
        // the onward move carries that one.)
        if let Some(i) = self
            .moves
            .iter()
            .position(|m| m.old == failed.new && !failed.released)
        {
            let onward = self.moves.remove(i);
            Self::release(&mut self.removing, &failed.new);
            failed.targets.extend(onward.targets);
            failed.new = onward.new;
            failed.this_run &= onward.this_run;
            failed.held = onward.held;
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
        for mut owed in std::mem::take(&mut self.moves) {
            // The copy is still being written or moved into `old`.
            if self.removing.contains_key(&owed.old) || self.in_flight.contains_key(&owed.old) {
                if !owed.released && !std::mem::replace(&mut owed.held, true) {
                    self.begin_remove(&owed.new);
                }
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
            if !owed.released && !std::mem::replace(&mut owed.held, true) {
                self.begin_remove(&owed.new);
            }
        }
        self.running_moves.extend(start.iter().cloned());
        (start, obsolete)
    }

    /// A Restore started reading `path`'s copy.
    pub fn begin_restore(&mut self, path: &Path) {
        self.restoring
            .insert(path.to_path_buf(), path.to_path_buf());
    }

    /// The Restore issued for `path` reported back. Returns where the
    /// document lives now: a rename meanwhile moved it.
    pub fn finish_restore(&mut self, path: &Path) -> PathBuf {
        self.restoring
            .remove(path)
            .unwrap_or_else(|| path.to_path_buf())
    }

    /// Paths whose recovery check has not reported yet.
    pub fn checking_paths(&self) -> impl Iterator<Item = &PathBuf> {
        self.checking.keys()
    }

    /// Whether `path`'s sidecar is not where it belongs yet: a rename into
    /// it is owed or running (or a removal of it runs). Reading it now
    /// would miss the copy.
    pub fn move_owed(&self, path: &Path) -> bool {
        self.removing.contains_key(path) || self.moves.iter().any(|m| m.new == path)
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

/// The timer; empty while autosave is off and no sidecar move is owed (a
/// rename's move is retried even then, so the copy reaches its new key).
pub fn subscription(enabled: bool, moves_owed: bool) -> Subscription<AutosaveMsg> {
    if enabled || moves_owed {
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
    fn a_bounded_wait_gives_up_while_a_job_runs_and_returns_once_it_ends() {
        let state = AutosaveState::default();
        let job = state.track_job();
        let barrier = state.barrier();
        assert!(!barrier.wait_timeout(Duration::from_millis(20)));
        drop(job);
        assert!(barrier.wait_timeout(Duration::from_millis(20)));
    }

    #[test]
    fn a_dropped_save_hold_releases_the_barrier() {
        let state = AutosaveState::default();
        let hold = state.hold_for_save();
        let barrier = state.barrier();
        assert!(!barrier.wait_timeout(Duration::ZERO));
        drop(hold);
        assert!(barrier.wait_timeout(Duration::ZERO));
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

    fn failed_once_more(state: &mut AutosaveState) {
        let (mut start, obsolete) = state.take_moves();
        assert!(obsolete.is_empty());
        assert!(state.move_failed(start.remove(0)).is_none(), "kept owed");
    }

    #[test]
    fn a_failed_move_releases_the_new_key_after_repeated_failures() {
        let (a, b) = (Path::new("a"), Path::new("b"));
        let newer = DocStamp::of_bytes(b"newer");
        let mut state = tracked_at(a);
        state.rekey(a, b);
        failed_once_more(&mut state);
        assert!(!state.needs_write(b, newer), "held after one failure");
        failed_once_more(&mut state);
        assert!(state.needs_write(b, newer), "a fresh copy may go there now");
        assert!(state.has_owed_moves(), "the old copy is still owed");
    }

    #[test]
    fn the_old_copy_is_removed_only_after_the_fresh_copy_lands() {
        let (a, b) = (Path::new("a"), Path::new("b"));
        let mut state = tracked_at(a);
        state.rekey(a, b);
        failed_once_more(&mut state);
        failed_once_more(&mut state);
        // The move is running: its own outcome settles the old copy.
        let (start, _) = state.take_moves();
        assert_eq!(state.take_obsolete_released(b), None);
        state.move_failed(start[0].clone());
        // The fresh copy landed (the write's completion calls this).
        state.begin(b);
        state.finish(b, DocStamp::of_bytes(b"newer"), true);
        assert_eq!(state.take_obsolete_released(b), Some(a.into()));
        assert!(!state.has_owed_moves());
    }

    #[test]
    fn an_earlier_run_copy_is_kept_when_a_fresh_copy_lands() {
        let (a, b) = (Path::new("a"), Path::new("b"));
        let mut state = AutosaveState::default();
        state.defer(a);
        state.rekey(a, b);
        failed_once_more(&mut state);
        failed_once_more(&mut state);
        assert_eq!(state.take_obsolete_released(b), None);
        assert!(state.has_owed_moves());
    }

    #[test]
    fn a_landed_released_move_does_not_release_a_hold_it_never_had() {
        let (a, b) = (Path::new("a"), Path::new("b"));
        let mut state = tracked_at(a);
        state.rekey(a, b);
        failed_once_more(&mut state);
        failed_once_more(&mut state);
        state.begin_remove(b); // another removal of `b` is running
        let (start, _) = state.take_moves();
        state.move_landed(&start[0]);
        assert!(state.removing.contains_key(b), "the other hold stays");
    }

    #[test]
    fn saving_sets_a_running_move_of_this_run_to_discard_what_it_moved() {
        let (a, b) = (Path::new("a"), Path::new("b"));
        let mut state = tracked_at(a);
        state.rekey(a, b);
        let (start, _) = state.take_moves();
        assert!(!start[0].is_discarded());
        state.saved(b);
        assert!(start[0].is_discarded());
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
    fn a_write_running_at_a_rename_lands_then_its_copy_moves() {
        let (a, b) = (Path::new("a"), Path::new("b"));
        let mut state = AutosaveState::default();
        let stamp = DocStamp::of_bytes(b"x");
        state.begin(a);
        state.rekey(a, b);
        assert!(state.is_forwarded(a));
        assert!(
            started(&mut state).is_empty(),
            "the move waits for the write"
        );
        assert!(!state.finish(a, stamp, true), "not superseded");
        assert!(
            matches!(state.written.get(b), Some(Some(_))),
            "recorded under b"
        );
        assert_eq!(started(&mut state), [(a.into(), b.into())]);
    }

    #[test]
    fn a_waiting_move_starts_when_the_one_before_it_lands() {
        let (a, b, c) = (Path::new("a"), Path::new("b"), Path::new("c"));
        let mut state = tracked_at(a);
        state.rekey(a, b);
        let (mut start, _) = state.take_moves();
        let running = start.remove(0);
        state.rekey(b, c);
        assert!(state.has_owed_moves());
        assert!(started(&mut state).is_empty());
        state.move_landed(&running);
        assert_eq!(started(&mut state), [(b.into(), c.into())]);
    }

    #[test]
    fn renaming_back_while_a_move_is_owed_drops_it_and_its_hold() {
        let (a, b) = (Path::new("a"), Path::new("b"));
        let mut state = tracked_at(a);
        state.rekey(a, b);
        let (mut start, _) = state.take_moves();
        assert!(state.move_failed(start.remove(0)).is_none(), "retried");
        state.rekey(b, a);
        assert!(!state.has_owed_moves(), "nothing left to move");
        assert!(state.removing.is_empty(), "no hold left behind");
        assert!(!state.move_owed(a));
    }

    #[test]
    fn an_untracked_copy_is_moved_not_dropped() {
        let (a, b) = (Path::new("a"), Path::new("b"));
        let mut state = AutosaveState::default();
        state.rekey(a, b);
        let (start, obsolete) = state.take_moves();
        assert!(
            obsolete.is_empty(),
            "an earlier run's copy is never dropped"
        );
        assert_eq!(start.len(), 1);
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
        assert!(
            !state.needs_write(c, DocStamp::of_bytes(b"fresh")),
            "c is held while its move waits, so no fresh copy is overwritten"
        );
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
    fn a_copy_being_restored_is_kept_through_a_rename() {
        let (a, b) = (Path::new("a"), Path::new("b"));
        let mut state = AutosaveState::default();
        state.begin_restore(a);
        state.rekey(a, b);
        let (start, obsolete) = state.take_moves();
        assert!(obsolete.is_empty(), "not removed as obsolete");
        assert_eq!(start.len(), 1);
        assert_eq!(
            state.finish_restore(a),
            b,
            "the read reports where it lives now"
        );
        assert!(state.deferred.contains(b), "kept at the new path");
    }

    #[test]
    fn offers_queue_once_per_document_in_order() {
        let mut state = AutosaveState::default();
        let offer = |name: &str| RecoveryOffer {
            path: PathBuf::from(name),
            age_secs: 5,
            older_than_file: false,
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
