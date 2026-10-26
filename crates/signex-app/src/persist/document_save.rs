//! Off-thread, per-file-ordered document writes (#859).
//!
//! "Per file" means per *ledger key* ([`ledger_key`]), not per `PathBuf`, so
//! two spellings of one file on a case-insensitive volume share one ordering
//! slot (#1112). The `PathBuf` stays the write target.
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
/// share one slot, and [`DocumentSaveJob::run`] then checks on the worker
/// thread whether a skipped older snapshot names the same file as the newer
/// write. Only true aliases are `Superseded`; distinct files are both written.
pub fn ledger_key(path: &Path) -> String {
    let key = path.to_string_lossy().to_lowercase();
    if cfg!(windows) {
        key.replace('\\', "/")
    } else {
        key
    }
}

/// Where document bytes go. Production is [`AtomicFileSink`]; tests
/// substitute a recorder.
pub trait DocumentSink: Send + Sync {
    fn write(&self, path: &Path, bytes: &[u8]) -> io::Result<()>;

    /// Whether `a` and `b` name one file on disk (same device and inode, or
    /// the platform's file index). The ledger asks only when two different
    /// spellings share a key: aliases of one file must be ordered, distinct
    /// files must both land. A path that does not exist is a different file;
    /// an error reading either is answered "same", the side that cannot
    /// overwrite anything.
    fn is_same_file(&self, a: &Path, b: &Path) -> bool {
        if !a.exists() || !b.exists() {
            return false;
        }
        same_file::is_same_file(a, b).unwrap_or(true)
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
    /// The ledger key the write was begun under (see [`DocumentSaves::begin`]);
    /// carried so `finish` releases the same key even if the owning project
    /// changed while the write ran.
    pub ledger_key: String,
    pub revision: u64,
    /// The sheet's net-colour revision when the snapshot was taken (see
    /// `UiState::net_color_revision`); `0` for documents without colours.
    pub net_color_revision: u64,
    pub save_as_from: Option<PathBuf>,
    pub result: Result<SaveOutcome, String>,
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
    /// Per ledger key: sequence number of the newest snapshot written.
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

/// One blocking write. Run it off the UI thread.
pub struct DocumentSaveJob {
    sink: Arc<dyn DocumentSink>,
    path: PathBuf,
    key: String,
    bytes: Vec<u8>,
    sequence: u64,
    ledger: Arc<Ledger>,
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
        let own = files
            .iter()
            .position(|newest| self.names_same_file(&newest.path));
        if own.is_some_and(|index| self.sequence <= files[index].sequence) {
            return Ok(SaveOutcome::Superseded);
        }
        self.sink
            .write(&self.path, &self.bytes)
            .map_err(|error| format!("write {}: {error}", self.path.display()))?;
        match own {
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
}

impl DocumentSaveJob {
    /// Whether this job's path names the same file as `other`, a spelling an
    /// earlier snapshot went to. The same spelling always is; otherwise ask
    /// the sink (case-twins on a case-sensitive volume are two files, on a
    /// case-insensitive one they are one).
    fn names_same_file(&self, other: &Path) -> bool {
        other == self.path || self.sink.is_same_file(&self.path, other)
    }
}

impl Drop for DocumentSaveJob {
    fn drop(&mut self) {
        let mut in_flight = relock(&self.ledger.in_flight);
        *in_flight -= 1;
        if *in_flight == 0 {
            self.ledger.idle.notify_all();
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
    /// Spellings whose snapshot was skipped as superseded, per ledger key,
    /// until the last write under the key reports back.
    superseded: HashMap<String, Vec<PathBuf>>,
}

impl DocumentSaves {
    pub fn new(sink: Arc<dyn DocumentSink>) -> Self {
        Self {
            sink,
            ledger: Arc::default(),
            next_sequence: 0,
            saving: HashMap::new(),
            superseded: HashMap::new(),
        }
    }

    /// Hand out a write of `bytes` to `path`, ordered under `key` (the
    /// [`ledger_key`] of `path`). Later snapshots of one key always win over earlier
    /// ones, whatever order the jobs get scheduled and whichever spelling of
    /// the file they name.
    pub fn begin(&mut self, key: String, path: PathBuf, bytes: Vec<u8>) -> DocumentSaveJob {
        self.next_sequence += 1;
        *self.saving.entry(key.clone()).or_default() += 1;
        *relock(&self.ledger.in_flight) += 1;
        DocumentSaveJob {
            sink: Arc::clone(&self.sink),
            path,
            key,
            bytes,
            sequence: self.next_sequence,
            ledger: Arc::clone(&self.ledger),
        }
    }

    /// The completion message for a write ordered under `key` was handled.
    pub fn finish(&mut self, key: &str) {
        if let Some(count) = self.saving.get_mut(key) {
            *count -= 1;
            if *count == 0 {
                self.saving.remove(key);
            }
        }
    }

    /// Remember that `path`'s snapshot was skipped because a newer write to
    /// the same file landed first.
    pub fn defer_superseded(&mut self, key: &str, path: PathBuf) {
        self.superseded
            .entry(key.to_owned())
            .or_default()
            .push(path);
    }

    /// The spellings recorded by [`Self::defer_superseded`] under `key`.
    pub fn take_superseded(&mut self, key: &str) -> Vec<PathBuf> {
        self.superseded.remove(key).unwrap_or_default()
    }

    /// Whether a write ordered under `key` has not been reported back yet.
    pub fn is_saving(&self, key: &str) -> bool {
        self.saving.contains_key(key)
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

    /// `.1` set: case-twin spellings are distinct files (case-sensitive
    /// volume); unset: they are one file (case-insensitive volume).
    #[derive(Default)]
    struct Recorder(Mutex<Vec<Vec<u8>>>, bool);

    impl DocumentSink for Recorder {
        fn write(&self, _path: &Path, bytes: &[u8]) -> io::Result<()> {
            relock(&self.0).push(bytes.to_vec());
            Ok(())
        }

        fn is_same_file(&self, _a: &Path, _b: &Path) -> bool {
            !self.1
        }
    }

    #[test]
    fn an_older_snapshot_scheduled_last_never_overwrites_a_newer_one() {
        let recorder = Arc::new(Recorder::default());
        let mut saves = DocumentSaves::new(recorder.clone());
        let older = saves.begin("a".into(), "a.snxsch".into(), b"old".to_vec());
        let newer = saves.begin("a".into(), "a.snxsch".into(), b"new".to_vec());
        assert_eq!(newer.run().unwrap(), SaveOutcome::Written);
        assert_eq!(older.run().unwrap(), SaveOutcome::Superseded);
        assert_eq!(*relock(&recorder.0), vec![b"new".to_vec()]);
    }

    #[test]
    fn two_spellings_under_one_key_are_one_file_to_the_ledger() {
        let recorder = Arc::new(Recorder::default());
        let mut saves = DocumentSaves::new(recorder.clone());
        let older = saves.begin("lib".into(), "Lib.snxsch".into(), b"old".to_vec());
        let newer = saves.begin("lib".into(), "lib.snxsch".into(), b"new".to_vec());
        assert!(saves.is_saving("lib"));
        assert_eq!(newer.run().unwrap(), SaveOutcome::Written);
        assert_eq!(older.run().unwrap(), SaveOutcome::Superseded);
        assert_eq!(*relock(&recorder.0), vec![b"new".to_vec()]);
        saves.finish("lib");
        assert!(saves.is_saving("lib"));
        saves.finish("lib");
        assert!(!saves.is_saving("lib"));
    }

    #[test]
    fn distinct_case_twin_files_under_one_key_are_both_written() {
        // A case-sensitive volume: the older snapshot must land, not be
        // skipped, or Quit / Save All would lose it silently.
        let recorder = Arc::new(Recorder(Mutex::default(), true));
        let mut saves = DocumentSaves::new(recorder.clone());
        let lib_upper = saves.begin(
            "lib".into(),
            "/nonexistent/Lib.snxsch".into(),
            b"upper".to_vec(),
        );
        let lib_lower = saves.begin(
            "lib".into(),
            "/nonexistent/lib.snxsch".into(),
            b"lower".to_vec(),
        );
        assert_eq!(lib_lower.run().unwrap(), SaveOutcome::Written);
        assert_eq!(lib_upper.run().unwrap(), SaveOutcome::Written);
        assert_eq!(
            *relock(&recorder.0),
            vec![b"lower".to_vec(), b"upper".to_vec()]
        );
    }

    /// Records (path, bytes); two paths are one file only if they are equal.
    #[derive(Default)]
    struct PathRecorder(Mutex<Vec<(PathBuf, Vec<u8>)>>);

    impl DocumentSink for PathRecorder {
        fn write(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
            relock(&self.0).push((path.to_path_buf(), bytes.to_vec()));
            Ok(())
        }

        fn is_same_file(&self, a: &Path, b: &Path) -> bool {
            a == b
        }
    }

    #[test]
    fn an_older_write_to_another_file_does_not_hide_a_newer_write_to_this_one() {
        // Distinct twins X=Lib, Y=lib under one key; the pool runs 3, 2, 1.
        let recorder = Arc::new(PathRecorder::default());
        let mut saves = DocumentSaves::new(recorder.clone());
        let x = PathBuf::from("/w/Lib.snxsch");
        let y = PathBuf::from("/w/lib.snxsch");
        let x1 = saves.begin("lib".into(), x.clone(), b"x1".to_vec());
        let x2 = saves.begin("lib".into(), x.clone(), b"x2".to_vec());
        let y3 = saves.begin("lib".into(), y.clone(), b"y3".to_vec());
        assert_eq!(y3.run().unwrap(), SaveOutcome::Written);
        assert_eq!(x2.run().unwrap(), SaveOutcome::Written);
        assert_eq!(x1.run().unwrap(), SaveOutcome::Superseded);
        let writes = relock(&recorder.0);
        let last_x = writes.iter().rev().find(|(path, _)| *path == x);
        assert_eq!(last_x.map(|(_, bytes)| bytes.as_slice()), Some(&b"x2"[..]));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn real_case_twin_files_on_a_case_sensitive_volume_both_land() {
        let dir = tempfile::tempdir().unwrap();
        let (upper, lower) = (dir.path().join("Lib.snxsch"), dir.path().join("lib.snxsch"));
        std::fs::write(&upper, b"x").unwrap();
        std::fs::write(&lower, b"x").unwrap();
        let mut saves = DocumentSaves::default();
        let older = saves.begin("lib".into(), upper.clone(), b"upper".to_vec());
        let newer = saves.begin("lib".into(), lower.clone(), b"lower".to_vec());
        assert_eq!(newer.run().unwrap(), SaveOutcome::Written);
        assert_eq!(older.run().unwrap(), SaveOutcome::Written);
        assert_eq!(std::fs::read(&upper).unwrap(), b"upper");
        assert_eq!(std::fs::read(&lower).unwrap(), b"lower");
    }

    #[test]
    fn snapshots_in_order_both_land_and_other_paths_are_independent() {
        let recorder = Arc::new(Recorder::default());
        let mut saves = DocumentSaves::new(recorder.clone());
        let first = saves.begin("a".into(), "a.snxsch".into(), b"1".to_vec());
        let other = saves.begin("b".into(), "b.snxsch".into(), b"x".to_vec());
        let second = saves.begin("a".into(), "a.snxsch".into(), b"2".to_vec());
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
        let job = saves.begin("a".into(), "a.snxsch".into(), Vec::new());
        job.run().unwrap();
        assert!(saves.is_saving("a"));
        saves.finish("a");
        assert!(!saves.is_saving("a"));
    }

    #[test]
    fn the_barrier_waits_for_an_unfinished_job() {
        let mut saves = DocumentSaves::new(Arc::new(Recorder::default()));
        let job = saves.begin("a".into(), "a.snxsch".into(), Vec::new());
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
