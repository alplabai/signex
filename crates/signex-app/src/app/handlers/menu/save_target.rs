//! Where a project-output save panel starts and what it suggests (#1766).

use std::path::{Path, PathBuf};

use crate::app::Signex;

/// Folder and file stem a save panel for a project output opens with.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct SaveTarget {
    dir: Option<PathBuf>,
    stem: Option<String>,
    document: Option<PathBuf>,
}

impl SaveTarget {
    /// `project_file` is the `.snxprj` the output belongs to, if any;
    /// `document` the schematic it is made from. Without a project the panel
    /// keeps the OS's own start folder.
    pub(crate) fn new(project_file: Option<&Path>, document: Option<&Path>) -> Self {
        let dir = project_file
            .and_then(Path::parent)
            .and_then(user_folder)
            .map(Path::to_path_buf);
        let stem = project_file
            .or(document)
            .and_then(Path::file_stem)
            .map(|stem| stem.to_string_lossy().into_owned());
        Self {
            dir,
            stem,
            document: document.map(Path::to_path_buf),
        }
    }

    /// The start folder, or `None` to leave it to the OS.
    pub(crate) fn dir(&self) -> Option<&Path> {
        self.dir.as_deref()
    }

    /// Where New Project starts: the folder that holds the open project's
    /// folder, so the new project lands beside it, never inside it.
    pub(crate) fn new_project_dir(&self) -> Option<&Path> {
        self.dir().and_then(Path::parent).and_then(user_folder)
    }

    /// The project or document name, when one is known.
    pub(crate) fn stem(&self) -> Option<&str> {
        self.stem.as_deref()
    }

    /// Where Save As starts: the document's own folder.
    pub(crate) fn save_as_dir(&self) -> Option<&Path> {
        self.document
            .as_deref()
            .and_then(Path::parent)
            .and_then(user_folder)
    }

    /// The document's current file name, offered by Save As.
    pub(crate) fn save_as_file_name(&self) -> Option<String> {
        let name = self.document.as_deref()?.file_name()?;
        Some(name.to_string_lossy().into_owned())
    }

    /// `<name><suffix>.<ext>`, the name being the project or document name;
    /// with neither known, `<fallback>.<ext>` and no suffix.
    pub(crate) fn file_name(&self, fallback: &str, suffix: &str, ext: &str) -> String {
        match self.stem.as_deref() {
            Some(stem) => format!("{stem}{suffix}.{ext}"),
            None => format!("{fallback}.{ext}"),
        }
    }

    /// A save panel titled `title`, started in the project folder when known.
    pub(crate) fn dialog(&self, title: &str) -> rfd::AsyncFileDialog {
        Self::dialog_in(title, self.dir())
    }

    /// A Save As panel, started in the document's own folder.
    pub(crate) fn save_as_dialog(&self, title: &str) -> rfd::AsyncFileDialog {
        Self::dialog_in(title, self.save_as_dir())
    }

    /// A New Project panel, started beside the open project's folder.
    pub(crate) fn new_project_dialog(&self, title: &str) -> rfd::AsyncFileDialog {
        Self::dialog_in(title, self.new_project_dir())
    }

    fn dialog_in(title: &str, dir: Option<&Path>) -> rfd::AsyncFileDialog {
        let dialog = rfd::AsyncFileDialog::new().set_title(title);
        match dir {
            Some(dir) => dialog.set_directory(dir),
            None => dialog,
        }
    }
}

/// `dir` as a start folder, unless it is empty or lies in the app's own
/// config folder (the bundled Sample Project lives there): such a folder is
/// not the user's, so the OS picks the start folder instead.
fn user_folder(dir: &Path) -> Option<&Path> {
    if dir.as_os_str().is_empty() {
        return None;
    }
    let in_config = crate::config_root::config_root().is_some_and(|root| dir.starts_with(root));
    (!in_config).then_some(dir)
}

impl Signex {
    /// The save target for an output made from `document` (the active
    /// document when `None`).
    pub(crate) fn save_target(&self, document: Option<&Path>) -> SaveTarget {
        let document = document.or(self.document_state.active_path.as_deref());
        let project = match document {
            Some(path) => self.document_state.document_project(path),
            None => self.document_state.active_document_project(),
        };
        SaveTarget::new(project.map(|p| p.path.as_path()), document)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn a_project_output_panel_starts_in_the_project_folder() {
        let target = SaveTarget::new(
            Some(&p("/work/amp/amp.snxprj")),
            Some(&p("/work/amp/a.snxsch")),
        );
        assert_eq!(target.dir(), Some(Path::new("/work/amp")));
    }

    #[test]
    fn new_project_starts_beside_the_open_project_not_inside_it() {
        let target = SaveTarget::new(Some(&p("/work/amp/amp.snxprj")), None);
        assert_eq!(target.new_project_dir(), Some(Path::new("/work")));
        assert_eq!(SaveTarget::default().new_project_dir(), None);
    }

    #[test]
    fn without_a_project_the_os_keeps_its_start_folder() {
        let target = SaveTarget::new(None, Some(&p("/tmp/loose.snxsch")));
        assert_eq!(target.dir(), None);
    }

    #[test]
    fn a_project_file_with_no_folder_gives_no_start_folder() {
        let target = SaveTarget::new(Some(&p("amp.snxprj")), None);
        assert_eq!(target.dir(), None);
    }

    #[test]
    fn the_suggested_name_is_the_project_name() {
        let target = SaveTarget::new(
            Some(&p("/work/amp/amp.snxprj")),
            Some(&p("/work/amp/a.snxsch")),
        );
        assert_eq!(target.file_name("schematic", "", "pdf"), "amp.pdf");
        assert_eq!(
            target.file_name("erc-report", "-erc-report", "html"),
            "amp-erc-report.html"
        );
    }

    #[test]
    fn a_loose_document_suggests_its_own_name() {
        let target = SaveTarget::new(None, Some(&p("/tmp/loose.snxsch")));
        assert_eq!(target.file_name("schematic", "", "pdf"), "loose.pdf");
    }

    #[test]
    fn nothing_known_falls_back_to_the_generic_name() {
        assert_eq!(
            SaveTarget::default().file_name("schematic", "", "pdf"),
            "schematic.pdf"
        );
    }

    fn config_project() -> PathBuf {
        crate::config_root::config_root()
            .expect("config root")
            .join("samples/linear-regulator/linear-regulator.snxprj")
    }

    #[test]
    fn a_project_under_the_config_folder_leaves_every_panel_to_the_os() {
        let project = config_project();
        let document = project.with_file_name("top.snxsch");
        let target = SaveTarget::new(Some(&project), Some(&document));
        assert_eq!(target.dir(), None);
        assert_eq!(target.new_project_dir(), None);
        assert_eq!(target.save_as_dir(), None);
        assert_eq!(target.save_as_file_name().as_deref(), Some("top.snxsch"));
    }

    #[test]
    fn save_as_starts_in_the_documents_own_folder_with_its_name() {
        let target = SaveTarget::new(
            Some(&p("/work/amp/amp.snxprj")),
            Some(&p("/work/amp/sheets/psu.snxsch")),
        );
        assert_eq!(target.save_as_dir(), Some(Path::new("/work/amp/sheets")));
        assert_eq!(target.save_as_file_name().as_deref(), Some("psu.snxsch"));
    }

    #[test]
    fn save_as_of_a_loose_document_starts_in_its_folder_too() {
        let target = SaveTarget::new(None, Some(&p("/tmp/loose.snxsch")));
        assert_eq!(target.save_as_dir(), Some(Path::new("/tmp")));
    }
}
