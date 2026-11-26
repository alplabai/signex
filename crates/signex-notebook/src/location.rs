//! Where a project keeps its Design Notebook: the one rule the Projects
//! tree, the notebook tab, the generator and the command line share.

use std::path::{Path, PathBuf};

use crate::{DEFAULT_NOTEBOOK_FILE, NOTEBOOK_DIR, NOTEBOOK_EXTENSION};

/// The notebook file of the project in `project_dir`:
/// `<project_dir>/notebook/design.snxnbk`. Nothing is read: the file may
/// not exist yet, since a notebook is written only on Save.
pub fn notebook_path(project_dir: &Path) -> PathBuf {
    project_dir.join(NOTEBOOK_DIR).join(DEFAULT_NOTEBOOK_FILE)
}

/// Whether `name` (a file name or a path) names a `.snxnbk` file. The
/// extension is compared without regard to ASCII case, as a file system
/// that ignores case would.
pub fn is_notebook_file(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case(NOTEBOOK_EXTENSION))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_notebook_is_design_snxnbk_in_the_notebook_folder() {
        let path = notebook_path(Path::new("/work/buck-3v3"));
        assert_eq!(
            path,
            Path::new("/work/buck-3v3")
                .join("notebook")
                .join("design.snxnbk")
        );
    }

    #[test]
    fn a_notebook_file_is_named_by_its_extension() {
        for name in [
            "design.snxnbk",
            "notebook/chapters/power.snxnbk",
            "DESIGN.SNXNBK",
        ] {
            assert!(is_notebook_file(name), "{name}");
        }
        for name in [
            "design.typ",
            "design.snxsch",
            "snxnbk",
            "design.snxnbk.bak",
            "",
        ] {
            assert!(!is_notebook_file(name), "{name}");
        }
    }
}
