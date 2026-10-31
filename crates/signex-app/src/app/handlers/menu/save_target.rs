//! Where a project-output save panel starts and what it suggests (#1766).

use std::path::{Path, PathBuf};

use crate::app::Signex;
use crate::app::state::scope::{CaseFold, path_is_under};

/// The panels a [`SaveTarget`] starts differently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Panel {
    /// An export of the project: its folder.
    Export,
    /// New Project: beside the open project's folder, never inside it.
    NewProject,
    /// Save As: the document's own folder.
    SaveAs,
}

/// What a save panel for a project output opens with. Built in `update` from
/// the model alone; the start folder is decided by [`Self::start_dir`], which
/// reads the disk and so only runs inside the panel's async task.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct SaveTarget {
    project_dir: Option<PathBuf>,
    stem: Option<String>,
    document: Option<PathBuf>,
}

impl SaveTarget {
    /// `project_file` is the `.snxprj` the output belongs to, if any;
    /// `document` the schematic it is made from. Pure: no disk access.
    pub(crate) fn new(project_file: Option<&Path>, document: Option<&Path>) -> Self {
        let project_dir = project_file
            .and_then(Path::parent)
            .filter(|dir| !dir.as_os_str().is_empty())
            .map(Path::to_path_buf);
        let stem = project_file
            .or(document)
            .and_then(Path::file_stem)
            .map(|stem| stem.to_string_lossy().into_owned());
        Self {
            project_dir,
            stem,
            document: document.map(Path::to_path_buf),
        }
    }

    /// The folder `panel` would like to start in, before the disk is asked.
    fn candidate(&self, panel: Panel) -> Option<&Path> {
        match panel {
            Panel::Export => self.project_dir.as_deref(),
            Panel::NewProject => self.project_dir.as_deref().and_then(Path::parent),
            Panel::SaveAs => self.document.as_deref().and_then(Path::parent),
        }
    }

    /// The start folder of `panel`, or `None` to leave it to the OS (no
    /// project, or a folder of the app's own). Reads the disk.
    pub(crate) fn start_dir(&self, panel: Panel) -> Option<PathBuf> {
        self.candidate(panel)
            .and_then(user_folder)
            .map(Path::to_path_buf)
    }

    /// The project or document name, when one is known.
    pub(crate) fn stem(&self) -> Option<&str> {
        self.stem.as_deref()
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
    pub(crate) async fn dialog(self, title: &str) -> rfd::AsyncFileDialog {
        self.panel(title, Panel::Export).await
    }

    /// A Save As panel, started in the document's own folder.
    pub(crate) async fn save_as_dialog(self, title: &str) -> rfd::AsyncFileDialog {
        self.panel(title, Panel::SaveAs).await
    }

    /// A New Project panel, started beside the open project's folder.
    pub(crate) async fn new_project_dialog(self, title: &str) -> rfd::AsyncFileDialog {
        self.panel(title, Panel::NewProject).await
    }

    /// The start folder is resolved on a blocking thread: a stale network
    /// share would otherwise stall the task's executor for its timeout.
    async fn panel(self, title: &str, panel: Panel) -> rfd::AsyncFileDialog {
        let dir = tokio::task::spawn_blocking(move || self.start_dir(panel))
            .await
            .unwrap_or_else(|e| {
                crate::diagnostics::log_warning(format!(
                    "Save panel: its start folder could not be resolved: {e}"
                ));
                None
            });
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
    let root = crate::config_root::config_root().map(resolved);
    user_folder_under(dir, root.as_deref())
}

/// `path` with symlinks resolved: its deepest existing ancestor is
/// canonicalised and the part that does not exist (yet) is kept as given.
fn resolved(path: PathBuf) -> PathBuf {
    let mut missing = Vec::new();
    let mut existing = path.as_path();
    loop {
        if let Ok(real) = existing.canonicalize() {
            return missing.iter().rev().fold(real, |acc, part| acc.join(part));
        }
        match (existing.parent(), existing.file_name()) {
            (Some(parent), Some(name)) => {
                missing.push(name.to_os_string());
                existing = parent;
            }
            _ => return path,
        }
    }
}

/// [`user_folder`] against an already resolved `config_root`. Both sides are
/// compared resolved: a recent-projects entry or an Open panel hands back the
/// resolved spelling (`/private/var/…` for `/var/…`), not the one the config
/// root was built from.
fn user_folder_under<'a>(dir: &'a Path, config_root: Option<&Path>) -> Option<&'a Path> {
    if dir.as_os_str().is_empty() {
        return None;
    }
    let in_config = config_root.is_some_and(|root| {
        path_is_under(&resolved(dir.to_path_buf()), root, CaseFold::host_default())
    });
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
        assert_eq!(
            target.start_dir(Panel::Export).as_deref(),
            Some(Path::new("/work/amp"))
        );
    }

    #[test]
    fn new_project_starts_beside_the_open_project_not_inside_it() {
        let target = SaveTarget::new(Some(&p("/work/amp/amp.snxprj")), None);
        assert_eq!(
            target.start_dir(Panel::NewProject).as_deref(),
            Some(Path::new("/work"))
        );
        assert_eq!(SaveTarget::default().start_dir(Panel::NewProject), None);
    }

    #[test]
    fn without_a_project_the_os_keeps_its_start_folder() {
        let target = SaveTarget::new(None, Some(&p("/tmp/loose.snxsch")));
        assert_eq!(target.start_dir(Panel::Export).as_deref(), None);
    }

    #[test]
    fn a_project_file_with_no_folder_gives_no_start_folder() {
        let target = SaveTarget::new(Some(&p("amp.snxprj")), None);
        assert_eq!(target.start_dir(Panel::Export).as_deref(), None);
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
        assert_eq!(target.start_dir(Panel::Export).as_deref(), None);
        assert_eq!(target.start_dir(Panel::NewProject).as_deref(), None);
        assert_eq!(target.start_dir(Panel::SaveAs).as_deref(), None);
        assert_eq!(target.save_as_file_name().as_deref(), Some("top.snxsch"));
    }

    #[test]
    fn save_as_starts_in_the_documents_own_folder_with_its_name() {
        let target = SaveTarget::new(
            Some(&p("/work/amp/amp.snxprj")),
            Some(&p("/work/amp/sheets/psu.snxsch")),
        );
        assert_eq!(
            target.start_dir(Panel::SaveAs).as_deref(),
            Some(Path::new("/work/amp/sheets"))
        );
        assert_eq!(target.save_as_file_name().as_deref(), Some("psu.snxsch"));
    }

    #[test]
    fn save_as_of_a_loose_document_starts_in_its_folder_too() {
        let target = SaveTarget::new(None, Some(&p("/tmp/loose.snxsch")));
        assert_eq!(
            target.start_dir(Panel::SaveAs).as_deref(),
            Some(Path::new("/tmp"))
        );
    }

    #[test]
    fn the_canonical_spelling_of_the_config_folder_is_still_the_config_folder() {
        let root = crate::config_root::config_root().expect("config root");
        std::fs::create_dir_all(&root).unwrap();
        let canonical = root.canonicalize().unwrap();
        let project = canonical.join("samples/linear-regulator/linear-regulator.snxprj");
        let target = SaveTarget::new(Some(&project), None);
        assert_eq!(target.start_dir(Panel::Export).as_deref(), None);
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_config_folder_is_still_the_config_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let real = tmp.path().join("real");
        std::fs::create_dir_all(real.join("samples/s")).unwrap();
        let link = tmp.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let via_real = real.join("samples/s");
        let via_link = link.join("samples/s");
        assert_eq!(
            user_folder_under(&via_real, Some(&link.canonicalize().unwrap())),
            None
        );
        assert_eq!(
            user_folder_under(&via_link, Some(&real.canonicalize().unwrap())),
            None
        );
        let elsewhere = tmp.path().join("projects");
        assert_eq!(
            user_folder_under(&elsewhere, Some(&link.canonicalize().unwrap())),
            Some(elsewhere.as_path())
        );
    }

    #[test]
    fn building_the_target_defers_the_config_folder_decision_to_the_async_task() {
        let project = config_project();
        let target = SaveTarget::new(Some(&project), None);
        assert_eq!(
            target.candidate(Panel::Export),
            project.parent(),
            "`new` keeps the raw folder; only `start_dir` reads the disk"
        );
    }
}
