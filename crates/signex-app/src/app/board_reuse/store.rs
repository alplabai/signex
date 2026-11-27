//! Where a project's reuse blocks live (#2410): one `.snxpcb` file per block
//! in the project folder's `reuse-blocks` folder, named after the block, and
//! in the Model once read or saved. The file functions block: they run off
//! the update thread.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use signex_types::format::SnxPcb;
use signex_types::pcb::PcbBoard;
use signex_types::reuse::{MAX_NAME_CHARS, NameRefusal};

use super::SavedBlock;

/// A block file write: what was saved and the file's text.
pub(crate) type BlockWrite = (SavedBlock, String);

/// The folder of a project's blocks, under the project folder.
pub(crate) const BLOCK_FOLDER: &str = "reuse-blocks";

/// The extension of a block file: a board file holding the block alone.
const BLOCK_EXTENSION: &str = "snxpcb";

/// Whether a project's block folder has been read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
enum ReadState {
    #[default]
    NotRead,
    Read,
    /// The folder could not be listed: why, and whether a request was
    /// already refused for it. A refused failure is read again on the next
    /// request; one no request has met yet is not, so the request that
    /// waited for the read does not start another.
    Failed {
        why: String,
        answered: bool,
    },
}

/// One project's blocks, by name.
#[derive(Debug, Clone, Default)]
struct ProjectBlocks {
    state: ReadState,
    blocks: BTreeMap<String, PcbBoard>,
}

/// Every open project's reuse blocks, keyed by the project folder.
#[derive(Debug, Clone, Default)]
pub struct ReuseBlockStore {
    projects: HashMap<PathBuf, ProjectBlocks>,
    /// Block files being written, each with the newest save that waits for
    /// that write to land: one file is never written twice at once, so the
    /// last save is what the file holds.
    writes: HashMap<PathBuf, Option<BlockWrite>>,
}

impl ReuseBlockStore {
    /// Whether the blocks of the project in `folder` are still to be read.
    pub(crate) fn needs_read(&self, folder: &Path) -> bool {
        self.projects
            .get(folder)
            .is_none_or(|p| p.state == ReadState::NotRead)
    }

    /// Why the project's block folder could not be read, if it could not.
    #[cfg(test)]
    pub(crate) fn read_failure(&self, folder: &Path) -> Option<&str> {
        match &self.projects.get(folder)?.state {
            ReadState::Failed { why, .. } => Some(why),
            _ => None,
        }
    }

    /// Why the folder could not be read, for a request refused for it: the
    /// next request reads it again ([`Self::retry_answered_failure`]).
    pub(crate) fn answer_read_failure(&mut self, folder: &Path) -> Option<String> {
        match &mut self.projects.get_mut(folder)?.state {
            ReadState::Failed { why, answered } => {
                *answered = true;
                Some(why.clone())
            }
            _ => None,
        }
    }

    /// A request starts: a failed read a request was already refused for is
    /// to be read again. The blocks kept stay.
    pub(crate) fn retry_answered_failure(&mut self, folder: &Path) {
        if let Some(project) = self.projects.get_mut(folder)
            && matches!(project.state, ReadState::Failed { answered: true, .. })
        {
            project.state = ReadState::NotRead;
        }
    }

    /// A board menu opens: any failed read is to be read again.
    pub(crate) fn retry_failed_read(&mut self, folder: &Path) {
        if let Some(project) = self.projects.get_mut(folder)
            && matches!(project.state, ReadState::Failed { .. })
        {
            project.state = ReadState::NotRead;
        }
    }

    /// The names of the project's blocks, sorted.
    pub(crate) fn names(&self, folder: &Path) -> Vec<String> {
        self.projects
            .get(folder)
            .map(|p| p.blocks.keys().cloned().collect())
            .unwrap_or_default()
    }

    pub(crate) fn block(&self, folder: &Path, name: &str) -> Option<&PcbBoard> {
        self.projects.get(folder)?.blocks.get(name)
    }

    /// Keep `block` as the project's block `name`, over one of that name.
    pub(crate) fn put(&mut self, folder: &Path, name: &str, block: PcbBoard) {
        self.projects
            .entry(folder.to_path_buf())
            .or_default()
            .blocks
            .insert(name.to_string(), block);
    }

    /// The write to start for `write`: itself, or none while its file is
    /// being written, when it waits for that write to land instead. A later
    /// save of the file replaces a waiting one; its text is older.
    pub(crate) fn begin_write(&mut self, write: BlockWrite) -> Option<BlockWrite> {
        match self.writes.get_mut(&write.0.file) {
            Some(waiting) => {
                *waiting = Some(write);
                None
            }
            None => {
                self.writes.insert(write.0.file.clone(), None);
                Some(write)
            }
        }
    }

    /// The write of `file` landed: the save that waited for it, to write
    /// now, if one did.
    pub(crate) fn end_write(&mut self, file: &Path) -> Option<BlockWrite> {
        let next = self.writes.get_mut(file)?.take();
        if next.is_none() {
            self.writes.remove(file);
        }
        next
    }

    /// The write [`Self::end_write`] handed out is dropped, not started:
    /// the file is free again.
    pub(crate) fn drop_write(&mut self, file: &Path) {
        self.writes.remove(file);
    }

    /// Whether a write of `file` is running.
    #[cfg(test)]
    pub(crate) fn is_writing(&self, file: &Path) -> bool {
        self.writes.contains_key(file)
    }

    /// Take a read of the project's folder in. A block saved while the read
    /// ran is newer than its file, so it stays.
    pub(crate) fn land(&mut self, folder: &Path, read: &BlocksOnDisk) {
        let project = self.projects.entry(folder.to_path_buf()).or_default();
        project.state = match &read.error {
            Some(why) => ReadState::Failed {
                why: why.clone(),
                answered: false,
            },
            None => ReadState::Read,
        };
        for (name, block) in &read.blocks {
            project
                .blocks
                .entry(name.clone())
                .or_insert_with(|| block.clone());
        }
    }
}

/// What a read of a project's block folder found.
#[derive(Debug, Default)]
pub struct BlocksOnDisk {
    pub(crate) blocks: Vec<(String, PcbBoard)>,
    /// Block files that could not be read, each with why.
    pub(crate) failures: Vec<String>,
    /// Why the folder could not be listed.
    pub(crate) error: Option<String>,
}

/// The block name `name` as stored, or why it cannot name a file: the one
/// rule of `signex_types::reuse`, in a block's words.
pub(crate) fn checked_name(name: &str) -> Result<String, String> {
    signex_types::reuse::checked_name(name).map_err(|refusal| {
        let name = name.trim();
        match refusal {
            NameRefusal::Empty => "a reuse block needs a name".to_string(),
            NameRefusal::TooLong => {
                format!("a reuse block name is at most {MAX_NAME_CHARS} characters")
            }
            NameRefusal::Characters => format!(
                "`{name}` cannot name a block: use letters, digits, spaces, `-`, `_` and `.` inside"
            ),
            NameRefusal::Reserved => format!("`{name}` is a name the system keeps for itself"),
        }
    })
}

/// The file of block `name` of the project in `folder`.
pub(crate) fn block_file(folder: &Path, name: &str) -> PathBuf {
    folder
        .join(BLOCK_FOLDER)
        .join(format!("{name}.{BLOCK_EXTENSION}"))
}

/// Every block file in the project's block folder. Blocking.
pub(crate) fn read_blocks(folder: &Path) -> BlocksOnDisk {
    let dir = folder.join(BLOCK_FOLDER);
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return BlocksOnDisk::default();
        }
        Err(error) => {
            return BlocksOnDisk {
                error: Some(format!("{}: {error}", dir.display())),
                ..BlocksOnDisk::default()
            };
        }
    };
    let mut read = BlocksOnDisk::default();
    for entry in entries {
        let path = match entry {
            Ok(entry) => entry.path(),
            Err(error) => {
                read.failures.push(format!("{}: {error}", dir.display()));
                continue;
            }
        };
        if path.extension().and_then(|e| e.to_str()) != Some(BLOCK_EXTENSION) {
            continue;
        }
        let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let parsed = std::fs::read_to_string(&path)
            .map_err(|error| error.to_string())
            .and_then(|text| SnxPcb::parse(&text).map_err(|error| error.to_string()));
        match parsed {
            Ok(file) => read.blocks.push((name.to_string(), file.board)),
            Err(why) => read.failures.push(format!("{}: {why}", path.display())),
        }
    }
    read.blocks.sort_by(|a, b| a.0.cmp(&b.0));
    read
}

/// Write `text` as the block file `file`, its folder made if missing.
/// Blocking.
pub(crate) fn write_block(file: &Path, text: &str) -> Result<(), String> {
    signex_types::atomic_io::atomic_write(file, text.as_bytes()).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_block_name_is_trimmed_and_must_be_able_to_name_a_file() {
        assert_eq!(checked_name("  amp 1 "), Ok("amp 1".to_string()));
        assert!(checked_name("").is_err());
        assert!(checked_name("../amp").is_err(), "no folder steps");
        assert!(checked_name("amp.").is_err(), "no trailing dot");
        assert!(checked_name(&"x".repeat(MAX_NAME_CHARS + 1)).is_err());
    }

    fn write(file: &str, text: &str) -> BlockWrite {
        let saved = SavedBlock {
            name: "amp".into(),
            file: PathBuf::from(file),
            footprints: 0,
            tracks: 0,
            vias: 0,
        };
        (saved, text.into())
    }

    /// #2451: a second save of one file waits for the first write to land,
    /// a third replaces the second, and the last save is written last.
    #[test]
    fn writes_of_one_file_run_one_at_a_time_and_the_newest_waits() {
        let mut store = ReuseBlockStore::default();
        assert_eq!(store.begin_write(write("a", "1")), Some(write("a", "1")));
        assert_eq!(store.begin_write(write("a", "2")), None);
        assert_eq!(store.begin_write(write("a", "3")), None);
        assert_eq!(
            store.begin_write(write("b", "1")),
            Some(write("b", "1")),
            "another file is written at once"
        );
        assert_eq!(store.end_write(Path::new("a")), Some(write("a", "3")));
        assert_eq!(store.begin_write(write("a", "4")), None, "3 is running");
        assert_eq!(store.end_write(Path::new("a")), Some(write("a", "4")));
        assert_eq!(store.end_write(Path::new("a")), None);
        assert_eq!(store.begin_write(write("a", "5")), Some(write("a", "5")));
    }

    /// #2451: a failed folder listing is read again on the next request
    /// once a request was refused for it, and on any menu open; not by the
    /// request that waited for the failed read itself.
    #[test]
    fn a_failed_read_is_retried_on_the_next_request_or_menu_open() {
        let folder = Path::new("p");
        let failed = BlocksOnDisk {
            error: Some("busy".into()),
            ..BlocksOnDisk::default()
        };
        let mut store = ReuseBlockStore::default();
        store.land(folder, &failed);
        store.retry_answered_failure(folder);
        assert!(!store.needs_read(folder), "the waiting request meets it");
        assert_eq!(store.answer_read_failure(folder).as_deref(), Some("busy"));
        store.retry_answered_failure(folder);
        assert!(store.needs_read(folder), "the next request reads again");

        store.land(folder, &failed);
        store.retry_failed_read(folder);
        assert!(store.needs_read(folder), "a menu open reads again");
        store.land(folder, &BlocksOnDisk::default());
        assert_eq!(store.read_failure(folder), None);
    }

    #[test]
    fn a_reserved_name_is_refused_with_or_without_a_dot_after_it() {
        for name in ["CON", "nul", "com1.amp", "Lpt9 .x"] {
            assert!(
                checked_name(name).is_err_and(|why| why.contains("keeps for itself")),
                "{name}"
            );
        }
        assert_eq!(checked_name("console"), Ok("console".to_string()));
    }
}
