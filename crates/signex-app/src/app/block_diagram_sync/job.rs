//! The generate and regenerate run itself, off the update thread: the
//! project's sheets are assembled and stitched as for a compile
//! (`app::design_read`), `signex_net::derive_block_diagram` says what the
//! hierarchy holds, and `signex_types::block_diagram` makes or merges the
//! diagram and writes it atomically.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use signex_types::block_diagram::{
    BlockDiagram, DerivedDiagram, LinkState, SNXBLK_FORMAT_V1, generate_block_diagram,
    read_block_diagram, regenerate_block_diagram, write_block_diagram,
};
use signex_types::document_id::DocumentId;
use signex_types::object_id::UuidV7Ids;
use signex_types::project::ChannelNaming;

use crate::app::design_read::DesignGraph;
use crate::app::project_sheets::{self, OpenSheets};
use crate::app::state::scope::CaseFold;

/// Which of the two commands runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SyncMode {
    /// `generate_block_diagram`: a new diagram; an existing file is replaced
    /// only with `overwrite`.
    Generate { overwrite: bool },
    /// `regenerate_block_diagram`: the changes to the saved diagram, written
    /// only with `apply`.
    Regenerate { apply: bool },
}

/// Everything a run needs, owned so it can leave the UI thread.
pub(crate) struct SyncJob {
    pub(crate) mode: SyncMode,
    /// The `.snxblk` file.
    pub(crate) target: PathBuf,
    pub(crate) project_dir: PathBuf,
    /// The open sheets of the project, as their tabs hold them.
    pub(crate) open: OpenSheets,
    pub(crate) pages: Vec<PathBuf>,
    pub(crate) root: PathBuf,
    pub(crate) fold: CaseFold,
    pub(crate) naming: ChannelNaming,
}

impl SyncJob {
    /// Read the hierarchy, then generate or regenerate. Blocking. The answer
    /// is the command's JSON result; `Err` says why nothing was written.
    pub(crate) fn run(self) -> Result<Value, String> {
        let derived = self.derive()?;
        match self.mode {
            SyncMode::Generate { overwrite } => self.generate(&derived, overwrite),
            SyncMode::Regenerate { apply } => self.regenerate(&derived, apply),
        }
    }

    fn derive(&self) -> Result<DerivedDiagram, String> {
        let set = project_sheets::assemble_project_sheets_from(
            &self.open,
            &self.pages,
            &self.root,
            self.fold,
        );
        if let Some((path, why)) = set.unreadable.first() {
            return Err(format!(
                "the sheet {} could not be read: {why}",
                path.display()
            ));
        }
        let design = DesignGraph::read(&set, &self.project_dir)?;
        let paths: HashMap<_, _> = design
            .graph
            .paths
            .iter()
            .map(|(key, path)| (key.clone(), project_relative(path, &self.project_dir)))
            .collect();
        Ok(signex_net::derive_block_diagram(
            &design.project(),
            &self.naming,
            &paths,
        ))
    }

    fn generate(&self, derived: &DerivedDiagram, overwrite: bool) -> Result<Value, String> {
        if !overwrite && self.target.exists() {
            return Err(format!(
                "{} exists; regenerate it to keep its layout, or pass overwrite=true to replace it",
                self.target.display()
            ));
        }
        let diagram = generate_block_diagram(derived, &mut UuidV7Ids, DocumentId::new())
            .map_err(|e| e.to_string())?;
        write_block_diagram(&self.target, &diagram, SNXBLK_FORMAT_V1)
            .map_err(|e| format!("{} was not written: {e}", self.target.display()))?;
        Ok(json!({
            "path": self.target.display().to_string(),
            "written": true,
            "summary": summary(&diagram),
            "diagram": diagram,
        }))
    }

    fn regenerate(&self, derived: &DerivedDiagram, apply: bool) -> Result<Value, String> {
        let read = read_block_diagram(&self.target)
            .map_err(|e| format!("{} could not be read: {e}", self.target.display()))?;
        if apply && read.is_read_only() {
            return Err(format!(
                "{} was written by a newer version ({}) and is read-only here",
                self.target.display(),
                read.token
            ));
        }
        let regenerated = regenerate_block_diagram(&read.diagram, derived, &mut UuidV7Ids)
            .map_err(|e| e.to_string())?;
        let write = apply && !regenerated.changes.is_empty();
        if write {
            write_block_diagram(&self.target, &regenerated.diagram, &read.token)
                .map_err(|e| format!("{} was not written: {e}", self.target.display()))?;
        }
        Ok(json!({
            "path": self.target.display().to_string(),
            "applied": apply,
            "written": write,
            "changes": regenerated.changes,
            "summary": summary(&regenerated.diagram),
            "diagram": regenerated.diagram,
        }))
    }
}

/// The counts a person reads first.
fn summary(diagram: &BlockDiagram) -> Value {
    let unlinked = diagram
        .blocks
        .iter()
        .filter(|b| {
            b.sheet
                .as_ref()
                .is_some_and(|l| l.state == LinkState::Unlinked)
        })
        .count();
    json!({
        "blocks": diagram.blocks.len(),
        "connections": diagram.connections.len(),
        "unlinked": unlinked,
    })
}

/// `path` relative to `dir` with `/` between its parts; the whole path,
/// `/`-separated, when it is not under `dir`.
fn project_relative(path: &Path, dir: &Path) -> String {
    match path.strip_prefix(dir) {
        Ok(relative) => relative
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/"),
        Err(_) => path.to_string_lossy().replace('\\', "/"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sheet_path_is_written_relative_with_forward_slashes() {
        let dir = Path::new("/p/demo");
        assert_eq!(
            project_relative(&dir.join("power").join("buck.snxsch"), dir),
            "power/buck.snxsch"
        );
        assert_eq!(
            project_relative(Path::new("/x/y.snxsch"), dir),
            "/x/y.snxsch"
        );
    }
}
