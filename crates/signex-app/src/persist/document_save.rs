//! Off-thread, per-path-ordered document writes (#859).
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

/// Where document bytes go. Production is [`AtomicFileSink`]; tests
/// substitute a recorder.
pub trait DocumentSink: Send + Sync {
    fn write(&self, path: &Path, bytes: &[u8]) -> io::Result<()>;
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
    pub save_as_from: Option<PathBuf>,
    pub result: Result<SaveOutcome, String>,
}

#[derive(Default)]
struct Ledger {
    /// Per target path: sequence number of the newest snapshot written.
    /// The inner mutex is held for the whole write, so writes to one path
    /// are serialised and "is a newer snapshot already on disk?" is atomic
    /// with the write itself.
    newest_written: Mutex<HashMap<PathBuf, Arc<Mutex<u64>>>>,
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
    bytes: Vec<u8>,
    sequence: u64,
    ledger: Arc<Ledger>,
}

impl DocumentSaveJob {
    /// Write the snapshot unless a newer one for the same path already
    /// reached the sink: that one holds more recent content, so this one
    /// reports [`SaveOutcome::Superseded`] and can never clobber it.
    pub fn run(self) -> Result<SaveOutcome, String> {
        let slot = Arc::clone(
            relock(&self.ledger.newest_written)
                .entry(self.path.clone())
                .or_default(),
        );
        let mut newest = relock(&slot);
        if self.sequence <= *newest {
            return Ok(SaveOutcome::Superseded);
        }
        self.sink
            .write(&self.path, &self.bytes)
            .map_err(|error| format!("write {}: {error}", self.path.display()))?;
        *newest = self.sequence;
        Ok(SaveOutcome::Written)
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
    saving: HashMap<PathBuf, usize>,
}

impl DocumentSaves {
    pub fn new(sink: Arc<dyn DocumentSink>) -> Self {
        Self {
            sink,
            ledger: Arc::default(),
            next_sequence: 0,
            saving: HashMap::new(),
        }
    }

    /// Hand out a write of `bytes` to `path`. Later snapshots of one path
    /// always win over earlier ones, whatever order the jobs get scheduled.
    pub fn begin(&mut self, path: PathBuf, bytes: Vec<u8>) -> DocumentSaveJob {
        self.next_sequence += 1;
        *self.saving.entry(path.clone()).or_default() += 1;
        *relock(&self.ledger.in_flight) += 1;
        DocumentSaveJob {
            sink: Arc::clone(&self.sink),
            path,
            bytes,
            sequence: self.next_sequence,
            ledger: Arc::clone(&self.ledger),
        }
    }

    /// The completion message for a write to `path` was handled.
    pub fn finish(&mut self, path: &Path) {
        if let Some(count) = self.saving.get_mut(path) {
            *count -= 1;
            if *count == 0 {
                self.saving.remove(path);
            }
        }
    }

    /// Whether a write to `path` has not been reported back yet.
    pub fn is_saving(&self, path: &Path) -> bool {
        self.saving.contains_key(path)
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

    #[derive(Default)]
    struct Recorder(Mutex<Vec<Vec<u8>>>);

    impl DocumentSink for Recorder {
        fn write(&self, _path: &Path, bytes: &[u8]) -> io::Result<()> {
            relock(&self.0).push(bytes.to_vec());
            Ok(())
        }
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
