//! Where a project's reuse blocks live (#2410): one `.snxpcb` file per block
//! in the project folder's `reuse-blocks` folder, named after the block, and
//! in the Model once read or saved. The file functions block: they run off
//! the update thread.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use signex_types::format::SnxPcb;
use signex_types::pcb::PcbBoard;

/// The folder of a project's blocks, under the project folder.
pub(crate) const BLOCK_FOLDER: &str = "reuse-blocks";

/// The extension of a block file: a board file holding the block alone.
const BLOCK_EXTENSION: &str = "snxpcb";

/// The longest block name, in characters.
const MAX_NAME_CHARS: usize = 64;

/// Names a file cannot take on Windows, whatever its extension.
const RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Whether a project's block folder has been read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
enum ReadState {
    #[default]
    NotRead,
    Read,
    /// The folder could not be listed; why.
    Failed(String),
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
}

impl ReuseBlockStore {
    /// Whether the blocks of the project in `folder` are still to be read.
    pub(crate) fn needs_read(&self, folder: &Path) -> bool {
        self.projects
            .get(folder)
            .is_none_or(|p| p.state == ReadState::NotRead)
    }

    /// Why the project's block folder could not be read, if it could not.
    pub(crate) fn read_failure(&self, folder: &Path) -> Option<&str> {
        match &self.projects.get(folder)?.state {
            ReadState::Failed(why) => Some(why),
            _ => None,
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

    /// Take a read of the project's folder in. A block saved while the read
    /// ran is newer than its file, so it stays.
    pub(crate) fn land(&mut self, folder: &Path, read: &BlocksOnDisk) {
        let project = self.projects.entry(folder.to_path_buf()).or_default();
        project.state = match &read.error {
            Some(why) => ReadState::Failed(why.clone()),
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

/// The block name `name` as stored, or why it cannot name a file.
pub(crate) fn checked_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("a reuse block needs a name".to_string());
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(format!(
            "a reuse block name is at most {MAX_NAME_CHARS} characters"
        ));
    }
    let allowed = |c: char| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.');
    if !name.chars().all(allowed) || name.starts_with('.') || name.ends_with('.') {
        return Err(format!(
            "`{name}` cannot name a block: use letters, digits, spaces, `-`, `_` and `.` inside"
        ));
    }
    // A reserved name stays reserved whatever follows its first dot.
    let stem = name.split('.').next().unwrap_or(name).trim_end();
    if RESERVED.iter().any(|r| r.eq_ignore_ascii_case(stem)) {
        return Err(format!("`{name}` is a name the system keeps for itself"));
    }
    Ok(name.to_string())
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
