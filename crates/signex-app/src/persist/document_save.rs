//! Off-thread, per-file-ordered document writes (#859).
//!
//! "Per file" means per *ledger key* ([`ledger_key`]), not per `PathBuf`, so
//! two spellings of one file on a case-insensitive volume share one ordering
//! slot (#1112). The ledger, the in-flight set and the follow-ups are keyed
//! by it; the `PathBuf` stays the write target.
//!
//! The UI thread serialises the document (a consistent snapshot) and asks
//! [`DocumentSaves::begin`] for a [`DocumentSaveJob`]; the job performs the
//! blocking write elsewhere. Nothing here touches the model: whether the
//! document is still clean afterwards is decided by the caller, from the
//! engine revision recorded with the snapshot.

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};

/// The key a write to `path` is ordered under.
///
/// Deliberately independent of every project and volume-probe state: the key
/// of one spelling must never change between two saves, or the older write
/// could land last under a different key (the project owner and its probed
/// fold both change while a write is in flight). It therefore ALWAYS folds
/// case (full Unicode lowercase) and, on Windows, normalises `\\` to `/`.
/// Over-folding is the data-loss-safe direction (ADR-0003 D6 exception), but
/// only for ordering: two distinct case-twin files on a case-sensitive volume
/// share one slot, and [`DocumentSaveJob::run`] then asks the sink on the
/// worker thread whether a skipped older snapshot names the same file as the
/// newer write. Only true (or unknowable) aliases are `Superseded`; distinct
/// files are both written.
fn ledger_key(path: &Path) -> String {
    let key = path.to_string_lossy().to_lowercase();
    if cfg!(windows) {
        key.replace('\\', "/")
    } else {
        key
    }
}

/// Whether two paths name one file on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SameFile {
    Same,
    Different,
    /// The answer could not be read. Treated as a match when deciding to
    /// skip an older snapshot (nothing wrong can reach disk), but never as
    /// grounds to overwrite another file's ledger record.
    Unknown,
}

/// Where document bytes go. Production is [`AtomicFileSink`]; tests
/// substitute a recorder.
pub trait DocumentSink: Send + Sync {
    fn write(&self, path: &Path, bytes: &[u8]) -> io::Result<()>;

    /// Whether `a` and `b` name one file on disk (same device and inode, or
    /// the platform's file index). The ledger asks only when two different
    /// spellings share a key: aliases of one file must be ordered, distinct
    /// files must both land. A path that does not exist is a different file.
    fn same_file(&self, a: &Path, b: &Path) -> SameFile {
        if !a.exists() || !b.exists() {
            return SameFile::Different;
        }
        match same_file::is_same_file(a, b) {
            Ok(true) => SameFile::Same,
            Ok(false) => SameFile::Different,
            Err(_) => SameFile::Unknown,
        }
    }
}

/// Crash-safe write: temp file, fsync, rename.
pub struct AtomicFileSink;

impl DocumentSink for AtomicFileSink {
    fn write(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        signex_types::atomic_io::atomic_write(path, bytes)
    }
}

/// What a write that did not fail actually did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveOutcome {
    /// This snapshot's bytes are on disk.
    Written,
    /// A newer snapshot of the same path was already written; this one was
    /// skipped, so its bytes are NOT on disk.
    Superseded,
}

/// Outcome of one write, carried back to `update`. `revision` is the engine
/// revision the snapshot was taken at. `save_as_from` is the document's old
/// path when this was a Save As (the re-key happens only on success).
#[derive(Debug, Clone)]
pub struct DocumentSaveDone {
    pub path: PathBuf,
    pub revision: u64,
    /// The sheet's net-colour revision when the snapshot was taken (see
    /// `UiState::net_color_revision`); `0` for documents without colours.
    pub net_color_revision: u64,
    pub save_as_from: Option<PathBuf>,
    pub result: Result<SaveOutcome, String>,
    /// Keeps quit's autosave wait open from the moment the save starts until
    /// this completion is handled (the copy removal it owes is registered
    /// then). It lives in the message, so a completion that is dropped
    /// unhandled releases it instead of hanging the quit. `None` for a
    /// completion not made by a save task.
    pub recovery_hold: Option<crate::autosave::SaveHold>,
}

/// The newest snapshot written to one file under a ledger key, and the exact
/// path it went to. A key holds one entry per distinct file: a different
/// spelling may name a different file, and an older write to that file must
/// not hide a newer write to this one.
struct Newest {
    sequence: u64,
    path: PathBuf,
}

#[derive(Default)]
struct Ledger {
    /// Per ledger key: the newest snapshot written to each distinct file.
    /// The inner mutex is held for the whole write, so writes to one file
    /// are serialised and "is a newer snapshot already on disk?" is atomic
    /// with the write itself.
    newest_written: Mutex<HashMap<String, Arc<Mutex<Vec<Newest>>>>>,
    /// Jobs handed out and not yet finished (or dropped unrun).
    in_flight: Mutex<usize>,
    idle: Condvar,
}

fn relock<G>(mutex: &Mutex<G>) -> MutexGuard<'_, G> {
    // The guarded values are plain counters and maps; a panicked writer
    // cannot leave them half-updated.
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Blocks until every job handed out so far has finished its write.
#[derive(Clone)]
pub struct SaveBarrier(Arc<Ledger>);

impl SaveBarrier {
    pub fn wait(&self) {
        let mut in_flight = relock(&self.0.in_flight);
        while *in_flight > 0 {
            in_flight = self
                .0
                .idle
                .wait(in_flight)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
    }
}

/// Holds [`SaveBarrier::wait`] open while alive: one write that was handed
/// out and has not finished (or was dropped unrun).
pub struct InFlight(Arc<Ledger>);

impl Drop for InFlight {
    fn drop(&mut self) {
        let mut in_flight = relock(&self.0.in_flight);
        *in_flight -= 1;
        if *in_flight == 0 {
            self.0.idle.notify_all();
        }
    }
}

/// One blocking write. Run it off the UI thread.
pub struct DocumentSaveJob {
    sink: Arc<dyn DocumentSink>,
    path: PathBuf,
    key: String,
    bytes: Vec<u8>,
    sequence: u64,
    ledger: Arc<Ledger>,
    _in_flight: InFlight,
}

impl DocumentSaveJob {
    /// Write the snapshot unless a newer one for the same file already
    /// reached the sink: that one holds more recent content, so this one
    /// reports [`SaveOutcome::Superseded`] and can never clobber it.
    pub fn run(self) -> Result<SaveOutcome, String> {
        let slot = Arc::clone(
            relock(&self.ledger.newest_written)
                .entry(self.key.clone())
                .or_default(),
        );
        let mut files = relock(&slot);
        let matches: Vec<SameFile> = files
            .iter()
            .map(|newest| self.relation_to(&newest.path))
            .collect();
        // Unknown counts as a match here: skipping is the side that cannot
        // overwrite anything.
        let newest_match = files
            .iter()
            .zip(&matches)
            .filter(|(_, relation)| **relation != SameFile::Different)
            .map(|(newest, _)| newest.sequence)
            .max();
        if newest_match.is_some_and(|newest| self.sequence <= newest) {
            return Ok(SaveOutcome::Superseded);
        }
        self.sink
            .write(&self.path, &self.bytes)
            .map_err(|error| format!("write {}: {error}", self.path.display()))?;
        // Only a confirmed match is this file's record; an Unknown one may
        // belong to another file and must keep its sequence.
        match matches
            .iter()
            .position(|relation| *relation == SameFile::Same)
        {
            Some(index) => {
                files[index].sequence = self.sequence;
                files[index].path = self.path.clone();
            }
            None => files.push(Newest {
                sequence: self.sequence,
                path: self.path.clone(),
            }),
        }
        Ok(SaveOutcome::Written)
    }

    /// How this job's path relates to `other`, a spelling an earlier snapshot
    /// went to. The same spelling is always the same file; otherwise ask the
    /// sink (case-twins on a case-sensitive volume are two files, on a
    /// case-insensitive one they are one).
    fn relation_to(&self, other: &Path) -> SameFile {
        if other == self.path {
            SameFile::Same
        } else {
            self.sink.same_file(&self.path, other)
        }
    }
}

/// UI-thread bookkeeping for document writes.
pub struct DocumentSaves {
    sink: Arc<dyn DocumentSink>,
    ledger: Arc<Ledger>,
    next_sequence: u64,
    /// Writes begun whose completion message has not been handled yet.
    saving: HashMap<String, usize>,
    /// Why the last write to a path failed, until a later one succeeds.
    /// Save All reads it to name the reason per file.
    failures: HashMap<PathBuf, String>,
    /// Saves asked for while their file's write was running, per ledger
    /// key: one follow-up per asking spelling (a sibling spelling may be a
    /// different file with its own editor), run when the write completes.
    follow_ups: HashMap<String, Vec<(PathBuf, u64)>>,
    /// Spellings whose snapshot was skipped as superseded, per ledger key,
    /// until the last write under the key reports back.
    superseded: HashMap<String, Vec<PathBuf>>,
    /// Last generation handed out by [`Self::mint_generation`].
    generation: u64,
}

impl DocumentSaves {
    pub fn new(sink: Arc<dyn DocumentSink>) -> Self {
        Self {
            sink,
            ledger: Arc::default(),
            next_sequence: 0,
            saving: HashMap::new(),
            failures: HashMap::new(),
            follow_ups: HashMap::new(),
            superseded: HashMap::new(),
            generation: 0,
        }
    }

    /// Hand out a write of `bytes` to `path`. Later snapshots of one path
    /// always win over earlier ones, whatever order the jobs get scheduled.
    pub fn begin(&mut self, path: PathBuf, bytes: Vec<u8>) -> DocumentSaveJob {
        self.next_sequence += 1;
        let in_flight = self.begin_work(path.clone());
        DocumentSaveJob {
            sink: Arc::clone(&self.sink),
            key: ledger_key(&path),
            path,
            bytes,
            sequence: self.next_sequence,
            ledger: Arc::clone(&self.ledger),
            _in_flight: in_flight,
        }
    }

    /// Register a write the caller performs itself (a library container
    /// save): the quit barrier waits for the returned guard to drop, and
    /// [`Self::is_saving`] holds until [`Self::finish`].
    pub fn begin_work(&mut self, path: PathBuf) -> InFlight {
        *self.saving.entry(ledger_key(&path)).or_default() += 1;
        *relock(&self.ledger.in_flight) += 1;
        InFlight(Arc::clone(&self.ledger))
    }

    /// Remember why the write to `path` failed.
    pub fn record_failure(&mut self, path: PathBuf, reason: String) {
        self.failures.insert(path, reason);
    }

    /// The write to `path` landed: forget an earlier failure.
    pub fn clear_failure(&mut self, path: &Path) {
        self.failures.remove(path);
    }

    pub fn failure(&self, path: &Path) -> Option<&str> {
        self.failures.get(path).map(String::as_str)
    }

    /// A fresh, never-repeated stamp. A container save stamps the editor
    /// with one; its completion only touches an editor still wearing it.
    pub fn mint_generation(&mut self) -> u64 {
        self.generation += 1;
        self.generation
    }

    /// Ask for one more save of `path` once its running write completes.
    /// `generation` is the save generation of the editor that asked, so the
    /// follow-up is dropped if another editor has replaced it by then.
    /// Repeats by the same editor coalesce; `false` when one was already
    /// queued for that editor.
    pub fn queue_follow_up(&mut self, path: PathBuf, generation: u64) -> bool {
        let asked = self.follow_ups.entry(ledger_key(&path)).or_default();
        match asked.iter_mut().find(|(asking, _)| *asking == path) {
            Some((_, queued)) if *queued == generation => false,
            Some((_, queued)) => {
                *queued = generation;
                true
            }
            None => {
                asked.push((path, generation));
                true
            }
        }
    }

    /// Take every follow-up queued under `path`'s key: each asking spelling
    /// with the generation of the editor that asked, oldest ask first.
    pub fn take_follow_ups(&mut self, path: &Path) -> Vec<(PathBuf, u64)> {
        self.follow_ups
            .remove(&ledger_key(path))
            .unwrap_or_default()
    }

    /// Drop the follow-up queued by exactly `path` (a synchronous save
    /// covers it); a sibling spelling's follow-up stays queued.
    pub fn discard_follow_up(&mut self, path: &Path) {
        let key = ledger_key(path);
        if let Some(asked) = self.follow_ups.get_mut(&key) {
            asked.retain(|(asking, _)| asking != path);
            if asked.is_empty() {
                self.follow_ups.remove(&key);
            }
        }
    }

    /// Drop the failure and the queued follow-up of every path `gone`
    /// matches: its document was discarded, closed or renamed.
    pub fn forget_where(&mut self, gone: impl Fn(&Path) -> bool) {
        self.failures.retain(|path, _| !gone(path));
        for asked in self.follow_ups.values_mut() {
            asked.retain(|(path, _)| !gone(path));
        }
        self.follow_ups.retain(|_, asked| !asked.is_empty());
    }

    /// The completion message for a write to `path` was handled.
    pub fn finish(&mut self, path: &Path) {
        let key = ledger_key(path);
        if let Some(count) = self.saving.get_mut(&key) {
            *count -= 1;
            if *count == 0 {
                self.saving.remove(&key);
            }
        }
    }

    /// Whether any document write, to any path, has not been reported back
    /// yet. A folder rename refuses to start while one runs.
    pub fn is_any_saving(&self) -> bool {
        !self.saving.is_empty()
    }

    /// Whether a write to `path`, under any spelling of it, has not been
    /// reported back yet.
    pub fn is_saving(&self, path: &Path) -> bool {
        self.saving.contains_key(&ledger_key(path))
    }

    /// Remember that `path`'s snapshot was skipped because a newer write to
    /// the same file landed first.
    pub fn defer_superseded(&mut self, path: PathBuf) {
        self.superseded
            .entry(ledger_key(&path))
            .or_default()
            .push(path);
    }

    /// The spellings recorded by [`Self::defer_superseded`] under `path`'s
    /// key, once no write under it is still owed; empty before that.
    pub fn take_superseded(&mut self, path: &Path) -> Vec<PathBuf> {
        let key = ledger_key(path);
        if self.saving.contains_key(&key) {
            return Vec::new();
        }
        self.superseded.remove(&key).unwrap_or_default()
    }

    pub fn barrier(&self) -> SaveBarrier {
        SaveBarrier(Arc::clone(&self.ledger))
    }
}

impl Default for DocumentSaves {
    fn default() -> Self {
        Self::new(Arc::new(AtomicFileSink))
    }
}

impl std::fmt::Debug for DocumentSaves {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DocumentSaves")
            .field("saving", &self.saving.len())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Case-twin spellings are one file (a case-insensitive volume).
    #[derive(Default)]
    struct Recorder(Mutex<Vec<Vec<u8>>>);

    impl DocumentSink for Recorder {
        fn write(&self, _path: &Path, bytes: &[u8]) -> io::Result<()> {
            relock(&self.0).push(bytes.to_vec());
            Ok(())
        }

        fn same_file(&self, _a: &Path, _b: &Path) -> SameFile {
            SameFile::Same
        }
    }

    /// Records (path, bytes); `relation` answers `same_file` for two paths
    /// that are not spelled identically.
    struct PathRecorder {
        writes: Mutex<Vec<(PathBuf, Vec<u8>)>>,
        relation: Mutex<SameFile>,
    }

    impl PathRecorder {
        fn new(relation: SameFile) -> Arc<Self> {
            Arc::new(Self {
                writes: Mutex::default(),
                relation: Mutex::new(relation),
            })
        }

        fn answer(&self, relation: SameFile) {
            *relock(&self.relation) = relation;
        }

        fn last_bytes_of(&self, path: &Path) -> Option<Vec<u8>> {
            let writes = relock(&self.writes);
            let last = writes.iter().rev().find(|(written, _)| written == path);
            last.map(|(_, bytes)| bytes.clone())
        }
    }

    impl DocumentSink for PathRecorder {
        fn write(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
            relock(&self.writes).push((path.to_path_buf(), bytes.to_vec()));
            Ok(())
        }

        fn same_file(&self, _a: &Path, _b: &Path) -> SameFile {
            *relock(&self.relation)
        }
    }

    #[test]
    fn two_spellings_under_one_key_are_one_file_to_the_ledger() {
        let recorder = Arc::new(Recorder::default());
        let mut saves = DocumentSaves::new(recorder.clone());
        let older = saves.begin("Lib.snxsch".into(), b"old".to_vec());
        let newer = saves.begin("lib.snxsch".into(), b"new".to_vec());
        assert!(saves.is_saving(Path::new("LIB.snxsch")));
        assert_eq!(newer.run().unwrap(), SaveOutcome::Written);
        assert_eq!(older.run().unwrap(), SaveOutcome::Superseded);
        assert_eq!(*relock(&recorder.0), vec![b"new".to_vec()]);
        saves.finish(Path::new("Lib.snxsch"));
        assert!(saves.is_saving(Path::new("lib.snxsch")));
        saves.finish(Path::new("lib.snxsch"));
        assert!(!saves.is_saving(Path::new("lib.snxsch")));
    }

    #[test]
    fn distinct_case_twin_files_under_one_key_are_both_written() {
        let recorder = PathRecorder::new(SameFile::Different);
        let mut saves = DocumentSaves::new(recorder.clone());
        let upper = saves.begin("/w/Lib.snxsch".into(), b"upper".to_vec());
        let lower = saves.begin("/w/lib.snxsch".into(), b"lower".to_vec());
        assert_eq!(lower.run().unwrap(), SaveOutcome::Written);
        assert_eq!(upper.run().unwrap(), SaveOutcome::Written);
        assert_eq!(relock(&recorder.writes).len(), 2);
    }

    #[test]
    fn an_older_write_to_another_file_does_not_hide_a_newer_write_to_this_one() {
        let recorder = PathRecorder::new(SameFile::Different);
        let mut saves = DocumentSaves::new(recorder.clone());
        let x = PathBuf::from("/w/Lib.snxsch");
        let x1 = saves.begin(x.clone(), b"x1".to_vec());
        let x2 = saves.begin(x.clone(), b"x2".to_vec());
        let y3 = saves.begin("/w/lib.snxsch".into(), b"y3".to_vec());
        assert_eq!(y3.run().unwrap(), SaveOutcome::Written);
        assert_eq!(x2.run().unwrap(), SaveOutcome::Written);
        assert_eq!(x1.run().unwrap(), SaveOutcome::Superseded);
        assert_eq!(recorder.last_bytes_of(&x), Some(b"x2".to_vec()));
    }

    #[test]
    fn an_unknown_relation_never_overwrites_another_files_record() {
        let recorder = PathRecorder::new(SameFile::Different);
        let mut saves = DocumentSaves::new(recorder.clone());
        let y2 = saves.begin("/w/lib.snxsch".into(), b"y2".to_vec());
        let y3 = saves.begin("/w/lib.snxsch".into(), b"y3".to_vec());
        let x4 = saves.begin("/w/Lib.snxsch".into(), b"x4".to_vec());
        assert_eq!(y3.run().unwrap(), SaveOutcome::Written);
        // X and Y cannot be compared: X is newer, so it is written, but it
        // must not take over Y's record.
        recorder.answer(SameFile::Unknown);
        assert_eq!(x4.run().unwrap(), SaveOutcome::Written);
        // The comparison now works and says they differ; Y's own record
        // (sequence 3) must still stand.
        recorder.answer(SameFile::Different);
        assert_eq!(y2.run().unwrap(), SaveOutcome::Superseded);
    }

    #[test]
    fn the_newest_of_several_matching_records_decides() {
        let recorder = PathRecorder::new(SameFile::Different);
        let mut saves = DocumentSaves::new(recorder.clone());
        let x1 = saves.begin("/w/Lib.snxsch".into(), b"x1".to_vec());
        let c2 = saves.begin("/w/LIB.snxsch".into(), b"c2".to_vec());
        let y3 = saves.begin("/w/lib.snxsch".into(), b"y3".to_vec());
        x1.run().unwrap();
        y3.run().unwrap();
        // Both records now match C as Unknown; the first is older (1), the
        // second newer (3): C (2) is behind the newest and must be skipped.
        recorder.answer(SameFile::Unknown);
        assert_eq!(c2.run().unwrap(), SaveOutcome::Superseded);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn real_case_twin_files_on_a_case_sensitive_volume_both_land() {
        let dir = tempfile::tempdir().unwrap();
        let (upper, lower) = (dir.path().join("Lib.snxsch"), dir.path().join("lib.snxsch"));
        std::fs::write(&upper, b"x").unwrap();
        std::fs::write(&lower, b"x").unwrap();
        let mut saves = DocumentSaves::default();
        let older = saves.begin(upper.clone(), b"upper".to_vec());
        let newer = saves.begin(lower.clone(), b"lower".to_vec());
        assert_eq!(newer.run().unwrap(), SaveOutcome::Written);
        assert_eq!(older.run().unwrap(), SaveOutcome::Written);
        assert_eq!(std::fs::read(&upper).unwrap(), b"upper");
        assert_eq!(std::fs::read(&lower).unwrap(), b"lower");
    }

    #[test]
    fn an_older_snapshot_scheduled_last_never_overwrites_a_newer_one() {
        let recorder = Arc::new(Recorder::default());
        let mut saves = DocumentSaves::new(recorder.clone());
        let older = saves.begin("a.snxsch".into(), b"old".to_vec());
        let newer = saves.begin("a.snxsch".into(), b"new".to_vec());
        assert_eq!(newer.run().unwrap(), SaveOutcome::Written);
        assert_eq!(older.run().unwrap(), SaveOutcome::Superseded);
        assert_eq!(*relock(&recorder.0), vec![b"new".to_vec()]);
    }

    #[test]
    fn snapshots_in_order_both_land_and_other_paths_are_independent() {
        let recorder = Arc::new(Recorder::default());
        let mut saves = DocumentSaves::new(recorder.clone());
        let first = saves.begin("a.snxsch".into(), b"1".to_vec());
        let other = saves.begin("b.snxsch".into(), b"x".to_vec());
        let second = saves.begin("a.snxsch".into(), b"2".to_vec());
        other.run().unwrap();
        first.run().unwrap();
        second.run().unwrap();
        assert_eq!(
            *relock(&recorder.0),
            vec![b"x".to_vec(), b"1".to_vec(), b"2".to_vec()]
        );
    }

    #[test]
    fn is_saving_holds_until_the_completion_is_handled() {
        let mut saves = DocumentSaves::new(Arc::new(Recorder::default()));
        let job = saves.begin("a.snxsch".into(), Vec::new());
        job.run().unwrap();
        assert!(saves.is_saving(Path::new("a.snxsch")));
        saves.finish(Path::new("a.snxsch"));
        assert!(!saves.is_saving(Path::new("a.snxsch")));
    }

    #[test]
    fn forgetting_a_path_drops_its_failure_and_follow_up_only() {
        let mut saves = DocumentSaves::new(Arc::new(Recorder::default()));
        saves.record_failure("a.snxsym".into(), "disk full".into());
        saves.record_failure("b.snxsym".into(), "disk full".into());
        assert!(saves.queue_follow_up("a.snxsym".into(), 1));
        assert!(
            !saves.queue_follow_up("a.snxsym".into(), 1),
            "repeats coalesce"
        );
        saves.forget_where(|p| p == Path::new("a.snxsym"));
        assert!(saves.failure(Path::new("a.snxsym")).is_none());
        assert!(saves.take_follow_ups(Path::new("a.snxsym")).is_empty());
        assert_eq!(saves.failure(Path::new("b.snxsym")), Some("disk full"));
    }

    #[test]
    fn the_barrier_waits_for_an_unfinished_job() {
        let mut saves = DocumentSaves::new(Arc::new(Recorder::default()));
        let job = saves.begin("a.snxsch".into(), Vec::new());
        let barrier = saves.barrier();
        let (tx, rx) = std::sync::mpsc::channel();
        let waiter = std::thread::spawn(move || {
            barrier.wait();
            tx.send(()).unwrap();
        });
        assert!(
            rx.recv_timeout(std::time::Duration::from_millis(150))
                .is_err()
        );
        job.run().unwrap();
        rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
        waiter.join().unwrap();
    }
}
