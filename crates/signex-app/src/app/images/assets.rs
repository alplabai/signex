//! The project's image assets (#1116, mockup row 204): which files are under
//! `assets/images/`, which an image names, how many images use each and
//! which are missing. A scan reads the folder, the named files and the
//! project's closed sheets off the UI thread; the Projects tree, the status
//! bar count, the Properties facts and `list_image_assets` read its report.
//! The rules are `signex_types::image_asset_report`'s.

use std::collections::{BTreeSet, HashMap};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::Task;
use serde_json::json;
use signex_types::image_asset::IMAGE_ASSET_DIR;
use signex_types::image_asset_report::{AssetReport, ImageUse, ScannedAsset, asset_report};
use signex_types::schematic::{SchDrawing, SchematicSheet};

use super::ImagesMsg;
use crate::app::agent::WaitingCommand;
use crate::app::command::CommandOutcome;
use crate::app::project_sheets::{
    OpenSheets, ProjectSheetSet, assemble_project_sheets_quiet_from, open_sheets_snapshot,
    project_pages, resolve_netlist_root_for,
};
use crate::app::state::scope::CaseFold;
use crate::app::{Message, Signex};

/// The last scan of each root (a project folder, or a loose sheet's
/// folder), and the key each root's last scan was started for.
#[derive(Debug, Default)]
pub struct AssetIndex {
    reports: HashMap<PathBuf, Arc<AssetReport>>,
    requested: HashMap<PathBuf, u64>,
    /// Bumped when a copy lands in a project, so the next look scans again.
    generation: u64,
}

impl AssetIndex {
    /// The last report of `root`.
    pub(crate) fn report(&self, root: &Path) -> Option<&AssetReport> {
        self.reports.get(root).map(Arc::as_ref)
    }

    /// Scan again at the next look: the disk changed under the reports.
    pub(crate) fn invalidate(&mut self) {
        self.generation += 1;
    }
}

/// A project's sheets to assemble off the UI thread: the open ones as they
/// are now, the closed ones from disk.
pub(crate) struct ProjectSheetsRead {
    open: OpenSheets,
    pages: Vec<PathBuf>,
    root: PathBuf,
    fold: CaseFold,
}

impl ProjectSheetsRead {
    /// Read the closed sheets and assemble the project. Blocking.
    pub(crate) fn assemble(&self) -> ProjectSheetSet {
        assemble_project_sheets_quiet_from(&self.open, &self.pages, &self.root, self.fold)
    }
}

/// Every placed image of `sheets`.
pub(crate) fn image_uses<'a>(
    sheets: impl IntoIterator<Item = (&'a PathBuf, &'a SchematicSheet)>,
) -> Vec<ImageUse> {
    let mut uses = Vec::new();
    for (path, sheet) in sheets {
        for drawing in &sheet.drawings {
            if let SchDrawing::Image(image) = drawing {
                uses.push(ImageUse {
                    sheet: path.clone(),
                    image: image.uuid,
                    file: image.file.clone(),
                    hash: image.hash.clone(),
                });
            }
        }
    }
    uses
}

/// What one scan reads, owned, for a blocking thread.
pub(crate) struct AssetScan {
    root: PathBuf,
    project: Option<ProjectSheetsRead>,
    /// A loose sheet's own document, when there is no project.
    loose: Option<(PathBuf, SchematicSheet)>,
    pending: Vec<(String, String)>,
}

impl AssetScan {
    /// List the images folder, read every named file and report. Blocking.
    pub(crate) fn run(self) -> AssetReport {
        let sheets: Vec<(PathBuf, SchematicSheet)> = match &self.project {
            Some(read) => read.assemble().sheets.into_iter().collect(),
            None => self.loose.into_iter().collect(),
        };
        let uses = image_uses(sheets.iter().map(|(path, sheet)| (path, sheet)));
        let mut files: BTreeSet<String> = super::read::list_asset_folder(&self.root);
        files.extend(uses.iter().map(|used| used.file.clone()));
        let scanned: Vec<ScannedAsset> = files
            .into_iter()
            .map(|file| ScannedAsset {
                facts: super::read::read_asset_facts(&self.root, &file),
                file,
            })
            .collect();
        asset_report(&scanned, &uses, &self.pending)
    }
}

/// `(file, hash)` of every pick a save will copy.
pub(crate) fn pending_image_keys(docs: &crate::app::DocumentState) -> Vec<(String, String)> {
    docs.image_copies
        .iter()
        .map(|copy| (copy.file.clone(), copy.hash.clone()))
        .collect()
}

/// `config` with the image files the sheets of a run name and cannot show
/// (`image_file_missing`). `project_dir` is the run's project folder; a
/// loose sheet's images are relative to its own folder. Blocking: it reads
/// every named file.
pub(crate) fn with_missing_images(
    config: &signex_erc::ErcConfig,
    project_dir: Option<&Path>,
    sheets: &HashMap<PathBuf, SchematicSheet>,
    pending: &[(String, String)],
) -> signex_erc::ErcConfig {
    let mut missing = BTreeSet::new();
    for used in image_uses(sheets.iter()) {
        let waiting = pending
            .iter()
            .any(|(file, hash)| *file == used.file && *hash == used.hash);
        if waiting || missing.contains(&used.file) {
            continue;
        }
        let root = match project_dir {
            Some(dir) => dir.to_path_buf(),
            None => used
                .sheet
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_default(),
        };
        if super::read::read_placed_image(&root, &used.file, &used.hash).is_err() {
            missing.insert(used.file.clone());
        }
    }
    config.clone().with_missing_images(missing)
}

impl Signex {
    /// The project of `sheet` to assemble off the UI thread; `None` for a
    /// loose sheet.
    pub(crate) fn project_read_of(&self, sheet: &Path) -> Option<ProjectSheetsRead> {
        let docs = &self.document_state;
        let project = docs.document_project(sheet)?;
        Some(ProjectSheetsRead {
            open: open_sheets_snapshot(docs),
            pages: project_pages(Some(project)),
            root: resolve_netlist_root_for(docs, sheet),
            fold: crate::app::state::project_case::fold_of(Some(project)),
        })
    }

    /// The scan of the assets `sheet`'s root holds.
    fn asset_scan_of(&self, sheet: &Path) -> AssetScan {
        let root = self.image_root(sheet);
        let project = self.project_read_of(sheet);
        let loose = match project {
            Some(_) => None,
            None => self
                .document_state
                .engines
                .get(sheet)
                .map(|engine| (sheet.to_path_buf(), engine.document().clone())),
        };
        AssetScan {
            root,
            project,
            loose,
            pending: pending_image_keys(&self.document_state),
        }
    }

    /// What a scan of `root` depends on in memory: the images of the open
    /// sheets that resolve to it, the picks waiting for a save, and the
    /// copies landed since.
    fn asset_key(&self, root: &Path) -> u64 {
        let mut uses: Vec<(PathBuf, String, String)> = Vec::new();
        for (path, engine) in &self.document_state.engines {
            if self.image_root(path) != root {
                continue;
            }
            for used in image_uses([(path, engine.document())]) {
                uses.push((used.sheet, used.file, used.hash));
            }
        }
        uses.sort();
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        uses.hash(&mut hasher);
        pending_image_keys(&self.document_state).hash(&mut hasher);
        self.ui_state
            .image_cache
            .assets
            .generation
            .hash(&mut hasher);
        hasher.finish()
    }

    /// Scan the active sheet's assets again when what the scan reads in
    /// memory changed since its last scan. A root none of whose open sheets
    /// names an image, with no pick waiting and no scan yet, is not scanned
    /// here: Place ▸ Graphic… and `list_image_assets` scan it. Cheap when
    /// nothing changed.
    pub(crate) fn request_asset_scan(&mut self) -> Task<Message> {
        self.scan_active_assets(false)
    }

    /// [`Self::request_asset_scan`]; `force` scans even a root with no image.
    pub(crate) fn scan_active_assets(&mut self, force: bool) -> Task<Message> {
        let Some(sheet) = self.document_state.active_path.clone() else {
            return Task::none();
        };
        if !self.document_state.engines.contains_key(&sheet) {
            return Task::none();
        }
        let root = self.image_root(&sheet);
        let known = self
            .ui_state
            .image_cache
            .assets
            .requested
            .contains_key(&root);
        if !force && !known && !self.root_has_images(&root) {
            return Task::none();
        }
        let key = self.asset_key(&root);
        if !force && self.ui_state.image_cache.assets.requested.get(&root) == Some(&key) {
            return Task::none();
        }
        self.ui_state
            .image_cache
            .assets
            .requested
            .insert(root.clone(), key);
        let scan = self.asset_scan_of(&sheet);
        Self::scan_task(scan, root, key, None)
    }

    /// Whether an open sheet of `root` names an image or a pick waits.
    fn root_has_images(&self, root: &Path) -> bool {
        !self.document_state.image_copies.is_empty()
            || self.document_state.engines.iter().any(|(path, engine)| {
                engine
                    .document()
                    .drawings
                    .iter()
                    .any(|d| matches!(d, SchDrawing::Image(_)))
                    && self.image_root(path) == root
            })
    }

    fn scan_task(
        scan: AssetScan,
        root: PathBuf,
        key: u64,
        waiting: Option<(WaitingCommand, PathBuf)>,
    ) -> Task<Message> {
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || scan.run())
                    .await
                    .map_err(|error| error.to_string())
            },
            move |report| {
                ImagesMsg::AssetsScanned {
                    root,
                    key,
                    report: report.map(Arc::new),
                    waiting: waiting.map(Box::new),
                }
                .into()
            },
        )
    }

    /// `list_image_assets document=<sheet>`: scan the assets of the sheet's
    /// project (or folder) and answer once the scan lands.
    pub(crate) fn list_image_assets(
        &mut self,
        command: &crate::keymap::AppCommandId,
        args: &crate::app::command::CommandArgs,
        sheet: PathBuf,
    ) -> (CommandOutcome, Task<Message>) {
        let root = self.image_root(&sheet);
        let scan = self.asset_scan_of(&sheet);
        let key = self.asset_key(&root);
        let waiting = self.wait_for_command(command, args);
        let ticket = waiting.ticket;
        let task = Self::scan_task(scan, root, key, Some((waiting, sheet)));
        (CommandOutcome::Deferred { ticket }, task)
    }

    /// A scan landed: keep its report, mark the Projects tree, and answer
    /// the `list_image_assets` that waited for it. A scan an older key
    /// started is dropped unless a caller waits for it.
    pub(super) fn assets_scanned(
        &mut self,
        root: PathBuf,
        key: u64,
        report: Result<Arc<AssetReport>, String>,
        waiting: Option<Box<(WaitingCommand, PathBuf)>>,
    ) -> Task<Message> {
        let current = self.ui_state.image_cache.assets.requested.get(&root) == Some(&key);
        if let Ok(report) = &report
            && (current || waiting.is_some())
        {
            self.ui_state
                .image_cache
                .assets
                .reports
                .insert(root.clone(), Arc::clone(report));
            self.mark_project_assets(&root, report);
        }
        let Some(waiting) = waiting else {
            return Task::none();
        };
        let (waiting, sheet) = *waiting;
        self.finish_waiting_command(waiting, &sheet, move |_| match report {
            Ok(report) => CommandOutcome::Answered(json!({
                "root": root,
                "folder": IMAGE_ASSET_DIR,
                "assets": report.entries,
                "missing": report.missing_count(),
            })),
            Err(why) => CommandOutcome::disabled(format!("the assets could not be read: {why}")),
        })
    }

    /// Put `report` on the Projects tree rows of the projects in `root`.
    fn mark_project_assets(&mut self, root: &Path, report: &AssetReport) {
        let rows: Vec<crate::panels::AssetRow> = report
            .entries
            .iter()
            .filter_map(|entry| {
                let name = entry
                    .file
                    .strip_prefix(IMAGE_ASSET_DIR)?
                    .strip_prefix('/')?;
                Some(crate::panels::AssetRow {
                    name: name.to_string(),
                    file: entry.file.clone(),
                    uses: entry.uses,
                    missing: entry.missing,
                    pending: entry.pending,
                })
            })
            .collect();
        let ids: Vec<String> = self
            .document_state
            .projects
            .iter()
            .filter(|project| project.dir() == root)
            .map(|project| project.id.to_string())
            .collect();
        let view = &mut self.document_state.panel_ctx.projects_view;
        for id in ids {
            view.assets.insert(id, rows.clone());
        }
    }

    /// How many files the active sheet's images name and cannot show, as
    /// the last scan of its root found.
    pub(crate) fn missing_asset_count(&self) -> usize {
        let Some(sheet) = self.document_state.active_path.as_deref() else {
            return 0;
        };
        let root = self.image_root(sheet);
        self.ui_state
            .image_cache
            .assets
            .report(&root)
            .map_or(0, AssetReport::missing_count)
    }
}
