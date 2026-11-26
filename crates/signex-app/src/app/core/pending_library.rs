//! A New Library request not written to disk yet; `library/commands.rs`
//! registers and materialises it.

use std::path::PathBuf;

/// Captured shape of a New Library request that hasn't been written
/// to disk yet. Lives on `LoadedProject.pending_libraries` between
/// the user clicking "Create Library" in the New Library dialog
/// and the next successful project save (which materialises the
/// `.snxlib` via `create_pending_library`). Closes
/// `feedback_no_disk_writes_without_user_save.md`'s "wait for
/// explicit user save" invariant — modal confirm flips the project
/// dirty bit but leaves disk untouched; only `Ctrl+S` actually
/// writes anything.
#[derive(Debug, Clone)]
pub struct PendingLibrarySpec {
    pub lib_path: PathBuf,
    pub enable_git: bool,
    pub use_lfs: bool,
    /// Tree-display name. Derived from the `.snxlib` stem at register
    /// time so the project tree can show the entry without having to
    /// touch disk.
    pub display_name: String,
}
