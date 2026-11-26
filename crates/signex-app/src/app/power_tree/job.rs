//! The run behind `query_power_tree` (#1313): the project's sheets are
//! stitched off the UI thread as the BOM's are (open sheets from memory,
//! closed ones from disk, within the headless read limits in a headless run
//! or for an agent), then `signex_net::power_tree` builds the tree and the
//! budget per sheet for the variant asked for.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use serde_json::Value;
use signex_net::SheetKey;
use signex_net::power_tree::{ListedRail, power_topology, power_tree, sheet_budgets};
use signex_types::schematic::Symbol;

use super::answer::{AnswerContext, power_tree_answer};
use crate::app::Signex;
use crate::app::core::block_diagram_panes::PowerTreeRead;
use crate::app::project_sheets::{
    OpenSheets, assemble_project_sheets_guarded_from, open_sheets_snapshot, project_graph_quiet,
    project_pages, project_root_sheet_path, project_roots, sheet_key_with, stitch_issue_message,
};
use crate::app::saved_channel_naming::SavedChannelNaming;
use crate::app::state::LoadedProject;
use crate::app::state::scope::CaseFold;

/// Everything the run needs, captured from memory so it can leave the UI
/// thread.
pub(crate) struct PowerTreeJob {
    open: OpenSheets,
    pages: Vec<PathBuf>,
    root: Option<PathBuf>,
    dir: PathBuf,
    fold: CaseFold,
    naming: SavedChannelNaming,
    /// The variant whose fitted parts count; `None` is Base.
    variant: Option<String>,
    /// The project's own rails. Empty until the project's rail list is read
    /// here (#2943); a rail then takes the voltage its net name spells.
    listed: BTreeMap<String, ListedRail>,
    /// The folder closed sheets must be read from, in a headless run or for
    /// an agent.
    confine: Option<PathBuf>,
    project_file: PathBuf,
}

impl Signex {
    /// The power tree run of `project` for `variant` (`None` is Base).
    pub(crate) fn power_tree_job(
        &self,
        project: &LoadedProject,
        variant: Option<String>,
    ) -> PowerTreeJob {
        let confined = self.headless_answers().is_some() || self.agent.serving;
        PowerTreeJob {
            open: open_sheets_snapshot(&self.document_state),
            pages: project_pages(Some(project)),
            root: project_root_sheet_path(project),
            dir: project.dir().to_path_buf(),
            fold: crate::app::state::project_case::fold_of(Some(project)),
            naming: SavedChannelNaming::of(&project.data),
            variant,
            listed: BTreeMap::new(),
            confine: confined.then(|| project.dir().to_path_buf()),
            project_file: project.path.clone(),
        }
    }
}

impl PowerTreeJob {
    /// Stitch the project and build its power tree. Blocking. Refused when
    /// the project has no root sheet, the root cannot be read or the
    /// channel naming cannot be read for a design that repeats sheets.
    pub(crate) fn run(self) -> Result<Value, String> {
        let built = self.build()?;
        let budgets = sheet_budgets(&built.read.topology, &built.read.tree);
        let context = AnswerContext {
            project: &self.project_file,
            dir: &self.dir,
            variant: self.variant.as_deref(),
            paths: &built.paths,
            warnings: built.read.warnings.clone(),
        };
        Ok(power_tree_answer(
            &built.read.topology,
            &built.read.tree,
            &budgets,
            context,
        ))
    }

    /// The same run, for the block diagram tab's Power tree view.
    pub(crate) fn read(self) -> Result<PowerTreeRead, String> {
        self.build().map(|built| built.read)
    }

    fn build(&self) -> Result<Built, String> {
        let Some(root) = self.root.clone() else {
            return Err("the project has no root sheet".to_string());
        };
        let confine = self.confine.as_deref();
        let guard = |path: &Path| {
            confine.map_or(Ok(()), |dir| {
                crate::app::document_ids::confined_read(dir, path)
                    .map_err(|why| format!("is not read: {why}"))
            })
        };
        let set =
            assemble_project_sheets_guarded_from(&self.open, &self.pages, &root, self.fold, &guard);
        let mut warnings: Vec<String> = set
            .unreadable
            .iter()
            .map(|(path, why)| format!("Power tree: sheet {} {why}", path.display()))
            .collect();
        let dir = Some(self.dir.as_path());
        let graph = project_graph_quiet(&set.sheets, dir, Some(root.as_path()), set.fold);
        warnings.extend(
            graph
                .issues
                .iter()
                .map(|issue| format!("Power tree: {}", stitch_issue_message(issue))),
        );
        let root_key = sheet_key_with(&root, dir, set.fold);
        if !graph.sheets.contains_key(&root_key) {
            return Err(format!(
                "the root sheet {} could not be read",
                root.display()
            ));
        }
        let roots = project_roots(root_key, &set, &graph, dir, set.fold);
        let walk = signex_net::ProjectGraph {
            sheets: &graph.sheets,
            resolved: &graph.resolved,
            roots: &roots,
        };
        let naming = self
            .naming
            .for_design(&walk)
            .map_err(|refused| refused.to_string())?;
        let variant = self.variant.as_deref();
        let fitted = |units: &[&Symbol]| signex_output::part_is_fitted(units, variant);
        let listed: BTreeSet<String> = self.listed.keys().cloned().collect();
        let topology = power_topology(&walk, naming, &listed, &fitted);
        let tree = power_tree(&topology, &self.listed);
        Ok(Built {
            read: PowerTreeRead {
                variant: self.variant.clone(),
                topology,
                tree,
                warnings,
            },
            paths: graph.paths,
        })
    }
}

/// A run's tree and the file of each sheet, which the answer names.
struct Built {
    read: PowerTreeRead,
    paths: HashMap<SheetKey, PathBuf>,
}
