//! The compile itself, off the update thread: the project's sheet set and
//! graph are assembled exactly as for the stitched netlist the Net Inspector
//! reads (`app::stitched_netlist`, `app::project_sheets`), then
//! `signex_net::compile_design` walks it with the project's channel naming.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use signex_net::{CompiledDesign, SheetKey, SheetMiniature};
use signex_types::project::ChannelNaming;
use signex_types::schematic::SchematicSheet;

use crate::app::project_sheets::{self, OpenSheets};
use crate::app::state::ProjectId;
use crate::app::state::scope::CaseFold;

/// Everything a compile needs, owned so it can leave the UI thread.
pub(super) struct CompileJob {
    pub project: ProjectId,
    pub generation: u64,
    /// The open sheets of the project, as their tabs hold them.
    pub open: OpenSheets,
    pub pages: Vec<PathBuf>,
    pub root: PathBuf,
    pub fold: CaseFold,
    pub project_dir: PathBuf,
    pub naming: ChannelNaming,
}

/// A finished compile.
#[derive(Debug)]
pub struct CompileOutcome {
    pub project: ProjectId,
    pub generation: u64,
    pub result: Result<Compiled, String>,
    /// Unreadable sheets and stitch issues, for Messages.
    pub warnings: Vec<String>,
}

/// A compile that produced a design.
#[derive(Debug)]
pub struct Compiled {
    pub design: CompiledDesign,
    pub paths: HashMap<SheetKey, PathBuf>,
    /// A sheet of the design was not read (`app::unread_sheets`).
    pub has_unread_sheets: bool,
    /// Those sheets.
    pub unread_paths: Vec<PathBuf>,
    /// With a sheet unread: the bare contents links with no file beside
    /// their table, so they can only be legacy links to another folder.
    pub bare_links_missing: Vec<PathBuf>,
    /// The card miniature of each channel row, by row index of `design`.
    pub miniatures: HashMap<usize, Arc<SheetMiniature>>,
    /// The clock when the compile finished, read here off the UI thread.
    pub finished_at: DateTime<Utc>,
}

impl CompileJob {
    /// Read and stitch the project. Blocking.
    pub(super) fn run(self) -> CompileOutcome {
        let set = project_sheets::assemble_project_sheets_from(
            &self.open,
            &self.pages,
            &self.root,
            self.fold,
        );
        let mut warnings: Vec<String> = set
            .unreadable
            .iter()
            .map(|(path, why)| format!("Compile project: '{}' {why}", path.display()))
            .collect();
        let outcome = |result, warnings| CompileOutcome {
            project: self.project,
            generation: self.generation,
            result,
            warnings,
        };
        let Some(root_path) = set.root.clone() else {
            return outcome(Err(root_missing(&self.root)), warnings);
        };
        let base_dir = Some(self.project_dir.as_path());
        let graph =
            project_sheets::project_graph_with(&set.sheets, base_dir, Some(&root_path), self.fold);
        warnings.extend(
            graph
                .issues
                .iter()
                .map(project_sheets::stitch_issue_message),
        );
        let root_key = project_sheets::sheet_key_with(&root_path, base_dir, self.fold);
        if !graph.sheets.contains_key(&root_key) {
            return outcome(Err(root_missing(&root_path)), warnings);
        }
        let roots = project_sheets::project_roots(root_key, &set, &graph, base_dir, self.fold);
        let project = signex_net::ProjectGraph {
            sheets: &graph.sheets,
            resolved: &graph.resolved,
            roots: &roots,
        };
        let design = signex_net::compile_design(&project, &self.naming);
        let unread_paths = crate::app::unread_sheets::unread_sheets(&set);
        let bare_links_missing = if unread_paths.is_empty() {
            Vec::new()
        } else {
            crate::app::unread_sheets::bare_links_missing_on_disk(&self.open, self.fold)
        };
        let miniatures = channel_miniatures(&design, &graph.sheets);
        let compiled = Compiled {
            design,
            paths: graph.paths.clone(),
            has_unread_sheets: !unread_paths.is_empty(),
            unread_paths,
            bare_links_missing,
            miniatures,
            finished_at: Utc::now(),
        };
        outcome(Ok(compiled), warnings)
    }
}

/// The miniature of every channel row of `design` whose sheet draws
/// something.
fn channel_miniatures(
    design: &CompiledDesign,
    sheets: &HashMap<SheetKey, SchematicSheet>,
) -> HashMap<usize, Arc<SheetMiniature>> {
    design
        .sheets
        .iter()
        .enumerate()
        .filter(|(_, row)| row.channel.is_some())
        .filter_map(|(index, row)| {
            let sheet = sheets.get(&row.key)?;
            signex_net::sheet_miniature(sheet, row).map(|m| (index, Arc::new(m)))
        })
        .collect()
}

fn root_missing(root: &std::path::Path) -> String {
    format!(
        "the project's root sheet '{}' did not assemble",
        root.display()
    )
}
