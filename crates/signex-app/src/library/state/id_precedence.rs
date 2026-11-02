//! Which of two libraries sharing a `library_id` stays mounted (#1908).
//!
//! The set holds one library per id, so a copied library folder has to give
//! way. Precedence: the bundled stock, then project libraries, then
//! installed, then global. Equal rank keeps the one mounted first. The
//! result does not depend on which mount finishes first.

use std::path::{Path, PathBuf};

use signex_library::LibraryError;

use super::{ComponentsMountSource, LibraryState};

/// A library kept out of the mounted set because another one shares its
/// `library_id`; shown in the Components panel until dismissed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdClashNotice {
    /// The library that stays mounted.
    pub kept: PathBuf,
    /// The library that was refused or unmounted.
    pub set_aside: PathBuf,
    /// True when `set_aside` was mounted and was replaced by `kept`.
    pub replaced: bool,
}

impl IdClashNotice {
    pub fn message(&self) -> String {
        let verb = if self.replaced {
            "was closed in favour of"
        } else {
            "was not opened because of"
        };
        format!(
            "{} {verb} {}: both carry the same library id. The project library wins over an \
             installed one, and an installed one over a global one.",
            self.set_aside.display(),
            self.kept.display()
        )
    }
}

/// Lower is stronger.
fn rank(source: ComponentsMountSource) -> u8 {
    match source {
        ComponentsMountSource::Stock => 0,
        ComponentsMountSource::Project => 1,
        ComponentsMountSource::Installed => 2,
        ComponentsMountSource::Global => 3,
    }
}

impl LibraryState {
    /// Settle a shared `library_id` before the library at `root` is
    /// mounted. `Ok` when the id is free, or when the library mounted under
    /// it was closed because `root` outranks it; the caller then mounts
    /// `root`. `Err(Conflict)` when `root` must not mount. Either way a
    /// clash leaves an [`IdClashNotice`].
    pub(crate) fn resolve_id_clash(
        &mut self,
        root: &Path,
        library_id: uuid::Uuid,
        project_paths: &[PathBuf],
    ) -> Result<(), LibraryError> {
        let Some(other) = self
            .open_libraries
            .iter()
            .find(|lib| lib.library_id == library_id && lib.root != root)
            .map(|lib| lib.root.clone())
        else {
            return Ok(());
        };
        let new_rank = rank(self.mount_source_for(root, project_paths));
        let other_rank = rank(self.mount_source_for(&other, project_paths));
        let outranks = new_rank < other_rank;
        let keep_other = |this: &mut Self, why: &str| {
            this.push_id_clash_notice(IdClashNotice {
                kept: other.clone(),
                set_aside: root.to_path_buf(),
                replaced: false,
            });
            Err(LibraryError::Conflict(format!(
                "{} has the same library id ({library_id}) as {}, which is already open{why}; \
                 it is probably a copy. Close the other one first, or give the copy its own id",
                root.display(),
                other.display()
            )))
        };
        if !outranks {
            return keep_other(self, "");
        }
        if !self.dirty_editors_for_library(&other).is_empty() {
            return keep_other(self, " and has unsaved edits");
        }
        self.close_library(&other);
        if self.library_at(&other).is_some() {
            return keep_other(self, " and is still being written");
        }
        self.push_id_clash_notice(IdClashNotice {
            kept: root.to_path_buf(),
            set_aside: other,
            replaced: true,
        });
        Ok(())
    }

    fn push_id_clash_notice(&mut self, notice: IdClashNotice) {
        crate::diagnostics::log_warning(notice.message());
        if !self.id_clash_notices.contains(&notice) {
            self.id_clash_notices.push(notice);
        }
    }
}
