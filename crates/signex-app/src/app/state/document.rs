//! The open documents and projects: `DocumentState` and the jobs it tracks.

use super::*;
use crate::app::TabInfo;
use crate::dock::DockArea;

pub struct DocumentState {
    /// Document ids a document cannot say itself: project files not loaded,
    /// and the order documents were first seen in (#2172).
    pub document_ids: crate::app::document_ids::DocumentIds,
    /// Counts the changes that can alter an ERC finding: an edit to a sheet
    /// (through the engine, or Annotate, Replace, library update and the like),
    /// a project-file change, and a sheet written behind the engines. Each calls
    /// [`DocumentState::note_sheet_edit`], mostly via `mark_document_dirty`.
    /// ERC compares it with the count its results were read at (#1768).
    pub sheet_edit_epoch: u64,
    /// Symbol editors opened so far; see [`DocumentState::next_symbol_history_base`].
    pub symbol_editors_opened: u64,
    pub dock: DockArea,
    /// Change detection + debounce for persisting `dock` (#692). The
    /// write itself runs as a `Task`; see `handlers/dock/layout_persist.rs`.
    pub dock_layout_sync: crate::dock::DockLayoutSync,
    /// The Components filter + Library Browser queries and the debounce
    /// that persists them (#859); see `handlers/dock/search_prefs_persist.rs`.
    pub search_prefs: crate::persist::SearchPrefsSync,
    /// Off-thread document writes: per-path ordering, in-flight tracking and
    /// the quit barrier (#859); see `handlers/document_files/save_async.rs`.
    pub document_saves: crate::persist::DocumentSaves,
    /// A Save All's settle that ran while rows of its batch were still
    /// writing, by batch id; resumed by the last of those writes (#1728).
    pub parked_save_all_settles:
        std::collections::HashMap<u64, crate::app::contracts::SaveAllSettle>,
    /// Opt-in recovery sidecars of dirty schematics (#104); see `crate::autosave`.
    pub autosave: crate::autosave::AutosaveState,
    /// Disk facts behind the project tree's `(missing)` marks (#111).
    pub project_fs: project_fs::ProjectFsCache,
    /// Idle check of open documents against the disk (#1877).
    pub disk_poll: crate::app::handlers::document_files::DiskPoll,
    /// Off-thread Navigator hierarchy assembly (#1470).
    pub navigator_job: navigator_job::NavigatorJob,
    /// Hierarchies of the other projects undocked windows show (#2021).
    pub window_hierarchies: window_hierarchies::WindowHierarchies,
    /// Cached sheet-path -> owning-project index, rebuilt when its inputs
    /// change (#1470); see `state/ownership_index.rs`.
    pub(crate) ownership: std::cell::RefCell<Option<ownership_index::OwnershipIndex>>,
    /// Projects already told (once per open) that an ERC run fell back to this
    /// machine's preferences because they store no rule set (#1085).
    pub erc_fallback_noted: std::collections::HashSet<ProjectId>,
    /// Projects already told (once per open) that the profile their rules
    /// were seeded from has changed locally (#1085).
    pub profile_drift_noted: std::collections::HashSet<ProjectId>,
    /// Undo / redo of each project's stored rule set edits (#1085).
    pub rules_history: std::collections::HashMap<ProjectId, rules_history::RulesHistory>,
    /// How many undo steps every history keeps (`undo_depth`); see
    /// [`Signex::set_undo_depth`](crate::app::Signex::set_undo_depth).
    pub undo_depth: usize,
    /// Which font families the Messages panel has already said an open
    /// document names but this machine lacks (#510), so each is reported
    /// once per document.
    pub missing_fonts: crate::fonts::document_report::MissingFontReporter,
    /// Engines (`Engine::instance_id`) already told, once per open, that an
    /// edit was refused because the sheet is read-only (#1402).
    pub read_only_refusal_noted: std::collections::HashSet<u64>,
    pub tabs: Vec<TabInfo>,
    pub active_tab: usize,
    /// All live schematic engines keyed by their on-disk path. Every
    /// open schematic tab — whether it's the active one in the main
    /// window or parked in an undocked-tab window — has a live entry
    /// here. `active_path` names which entry `active_engine()` resolves
    /// to; undocked windows look up their own entry via
    /// `engine_for_window`. Save-as rekeys an entry via
    /// `rekey_engine(old, new)`.
    pub engines: std::collections::HashMap<PathBuf, signex_engine::Engine>,
    /// The clock and the file times the computed special strings show.
    pub times: DocumentTimes,
    /// Per-tab state for open `.snxsym` document tabs. Keyed by the
    /// file path stored on `TabInfo.path` for matching
    /// `TabKind::SymbolEditor(path)` tabs. Insert on
    /// `Signex::open_primitive`; drop in
    /// `close_tab_at_index` alongside the engine cleanup.
    pub symbol_editors: std::collections::HashMap<PathBuf, super::SymbolDocument>,
    /// Per-tab state for open `.snxfpt` document tabs. Keyed the same
    /// way as `symbol_editors`.
    pub footprint_editors: std::collections::HashMap<PathBuf, super::FootprintDocument>,
    /// v0.26-E — process-local clipboard for footprint-editor pad
    /// Cut / Copy / Paste. Survives tab switches so a pad copied from
    /// one footprint can be pasted into another. `None` until the
    /// first Cut / Copy. Replaced wholesale on every copy. Keyed by
    /// no path on purpose — Altium parity is single-slot, last-write-
    /// wins.
    pub pad_clipboard: Option<crate::library::editor::footprint::state::EditorPad>,
    /// The path of the schematic the main window is currently editing.
    /// `active_engine()` reads `engines.get(active_path)`. `None` means
    /// no schematic tab is active (e.g. a PCB tab is active, or nothing
    /// is open).
    pub active_path: Option<PathBuf>,
    /// Every loaded project in the workspace. Order = load order. First
    /// project becomes active on load; subsequent opens append.
    pub projects: Vec<LoadedProject>,
    /// Which project is "active" for handlers that operate on the workspace
    /// at large (save-all, the Projects panel). Currently tracks the
    /// most-recently-loaded project plus whichever project contains the
    /// active tab; single source of truth for "where am I focused".
    ///
    /// Sticky by design: focusing a tab with no project of its own leaves it
    /// pointing at the last-loaded project. Anything scoped to the *document
    /// set the user is working in* — export, ERC, annotate, child-sheet
    /// resolution — must use [`DocumentState::active_document_project`]
    /// instead, or it silently operates on another project's sheets (#406).
    pub active_project: Option<ProjectId>,
    /// Project paths (or loose root sheets) whose per-placement-designator
    /// load note was already started, so it appears once per load, not once
    /// per opened sheet.
    pub instance_note_scopes: std::collections::HashSet<std::path::PathBuf>,
    /// A schematic tab just opened: the next `finish_update` starts the
    /// per-placement designator note (a no-op once its scope was noted).
    pub instance_note_due: bool,
    /// Schematic tabs opened since the last `update` finished: each needs
    /// its earlier-run recovery copy checked (#1208). `open_schematic_tab`
    /// queues the path, `update` starts the check.
    pub recovery_check_due: Vec<std::path::PathBuf>,
    /// Dirty documents that no editor draft holds; see `dirty.rs`, the only
    /// reader and writer. Ask `is_document_dirty`, never this field.
    pub(crate) dirty_docs: dirty::DirtyDocuments,
    /// Undo/redo of each project's net-class table (#1042). The table is
    /// project data, so it has its own history beside the per-sheet engines.
    pub net_class_history: std::collections::HashMap<ProjectId, signex_engine::NetClassHistory>,
    /// Project tab state per loaded project (#1202): selected section and
    /// the General history. Outlives the tab, dropped with the project.
    pub project_tabs: std::collections::HashMap<ProjectId, crate::app::ProjectTabState>,
    /// Monotonic counter used to mint `ProjectId` on load. Never reused —
    /// closing a project does not free its id (tabs may hold stale refs,
    /// which we detect by resolving through `projects` and treating None
    /// as "no longer loaded").
    pub next_project_id: u32,
    /// Monotonic counter minting the raw value of [`signex_engine::UndoGroupId`]s for
    /// project-wide undo steps (an annotate run under "One step for the whole
    /// project"). Never reused.
    pub next_undo_group: u64,
    /// Bumped whenever a document is opened, saved or closed, when a project
    /// closes, and when a rename, delete or project-folder move starts and
    /// finishes. A background job that read sheets from disk records it first
    /// and applies its result only if it is unchanged, so it never installs
    /// bytes a later open, save, close, move or delete has already superseded.
    pub documents_epoch: u64,
    /// Designators used on the sheets of the project a part is placed in,
    /// scanned off-thread; see `designator_scan.rs`.
    pub(crate) designator_cache: crate::app::designator_scan::DesignatorCache,
    /// Highest board revision any engine reached; see `state/pcb_revision.rs`.
    pub next_pcb_revision: u64,
    /// Board revision per PCB path (mirror of its engine); a save marks clean only if it matches.
    pub pcb_revisions: std::collections::HashMap<PathBuf, u64>,
    /// The engines of PCB tabs closed while dirty; see `state/pcb_board.rs`.
    pub(crate) parked_pcb_boards: pcb_board::ParkedBoards,
    /// The sheets each undo group touched. Undo/redo of a
    /// grouped entry consults this to find the sheets that must move with it.
    pub undo_groups:
        std::collections::HashMap<signex_engine::UndoGroupId, crate::app::UndoGroupSheets>,
    /// An agent's edit list in progress, and the labels of its undo steps (#2178).
    pub agent_edits: AgentEdits,
    /// The last runs of `check_erc`, for a later run's baseline (#2199).
    pub(crate) check_runs: check_runs::CheckRuns,
    /// Each open project's design reuse blocks, read or saved (#2410).
    pub(crate) reuse_blocks: crate::app::board_reuse::ReuseBlockStore,
    /// The one store of half-typed title-block and paper-edge text (#2027);
    /// every panel context holds a handle to it.
    pub drafts: crate::panels::PanelDrafts,
    pub panel_ctx: crate::panels::PanelContext,
    /// The panel context of each undocked document window, so its dock
    /// panels show and edit that window's sheet (#1966).
    pub window_panel_ctx: std::collections::HashMap<iced::window::Id, crate::panels::PanelContext>,
    /// What each window context was last built from; see `window_ctx_key`.
    pub(crate) window_panel_ctx_keys:
        std::collections::HashMap<iced::window::Id, crate::app::runtime::WindowCtxKey>,
    /// Print-preview overlay state. `Some` while the preview dialog is
    /// open. Doubles as the unified PDF Export modal — `File → Export
    /// PDF` and `File → Print Preview` both populate this field.
    pub preview: Option<PreviewState>,
    /// Page setup (size, orientation, margins, scale) each document last used
    /// in the PDF export modal, restored when it reopens for the same
    /// document. Session-only; never written to disk.
    pub page_setup_memory: PageSetupMemory,
    /// The sheet symbol each child tab was descended through (#1133).
    pub tab_placements: TabPlacements,
    /// Sheet history, net highlight, hover insight and the project net index
    /// they read (#1270, #1271, #1272).
    pub net_nav: net_nav::NetNavState,
    /// Generation counter and in-flight job of the preview renderer.
    pub raster_queue: RasterQueue,
    /// The app's one user-visible error card. `Some` while it is shown;
    /// cleared by `OverlayMsg::DismissErrorNotice`.
    ///
    /// Not export-only, despite the heading it used to hardcode: the
    /// project-close path already raised it for "could not save N
    /// file(s)", so a failed close announced itself to the user as
    /// "Export Failed" (#532). The title now travels with the message.
    pub error_notice: Option<ErrorNotice>,
    /// #431 — pending "Export anyway (incomplete)?" prompt. `Some` while the
    /// netlist-incomplete modal is up, waiting on the user's choice. Set by
    /// `handle_export_netlist_finished` when the derived netlist has a hole;
    /// cleared by both prompt actions (Export anyway / Cancel).
    pub netlist_incomplete_prompt: Option<NetlistIncompletePrompt>,
    /// BOM preview state. `Some` while the BOM Export modal is open.
    /// Mirrors the Print Preview pattern: the user adjusts grouping /
    /// include flags / format / variant in the modal and clicks
    /// Export to drive the file picker with the chosen options.
    pub bom_preview: Option<BomPreviewState>,
    /// Right-dock History panel state — current generation counter,
    /// last loaded entries, the path the load was issued for. Driven
    /// by `Message::HistoryLoaded` and re-targeted on tab switch.
    pub history: crate::panels::history::HistoryPanelState,
    /// The History panel state of each undocked document window, swapped in
    /// with its panel context so the window shows and restores its own file
    /// (#2067).
    pub(crate) window_history:
        std::collections::HashMap<iced::window::Id, crate::panels::history::HistoryPanelState>,
    /// v0.23 — Async git commit work queue. Save handlers push tuples
    /// here; `finish_update` drains them into `Task::perform` calls
    /// that run the actual commit on a tokio `spawn_blocking`. Each
    /// completion routes through `Message::Project(ProjectMsg::GitCommitDone)`.
    pub pending_git_commits: Vec<PendingGitCommit>,
    /// v0.23 — Set of `(project_root, rel_path)` pairs whose commits
    /// are currently queued or in flight. Drives the status bar's
    /// "Saving…" pill — when non-empty the user sees an indicator.
    /// An entry lands here as soon as
    /// [`Signex::commit_save_to_project_git`] enqueues the work and
    /// clears on `Message::Project(ProjectMsg::GitCommitDone)`.
    pub inflight_git_commits: std::collections::HashSet<(PathBuf, PathBuf)>,
    /// The subset of `inflight_git_commits` whose commit reads the working
    /// file when its worker starts instead of holding the bytes it records
    /// (primitive-editor saves, project-file saves, renames). A History
    /// restore of such a file waits, or the commit would record the restored
    /// bytes under its own label; commits that carry their bytes never block it.
    pub unbound_git_commits: std::collections::HashSet<(PathBuf, PathBuf)>,
    /// The newest save sequence queued for a commit, per `(project_root,
    /// rel_path)`; a commit of an older write is dropped.
    pub git_commit_sequences: std::collections::HashMap<(PathBuf, PathBuf), u64>,
    /// #833 — repo roots with a commit currently running. At most one
    /// commit per repo is in flight; the rest wait in
    /// `pending_git_commits` so concurrent tasks never race on the
    /// repo's index lock. Cleared per repo by `GitCommitDone`.
    pub active_git_repos: std::collections::HashSet<PathBuf>,
    /// #1205 — project folders whose auto-commit the guard refused to
    /// initialise; each is announced once, not on every save.
    pub paused_git_projects: std::collections::HashSet<PathBuf>,
    /// #478 review — paths whose async schematic/PCB read+parse
    /// (`open_schematic_file` / `open_pcb_file`) has been spawned but
    /// hasn't completed yet. Without this, firing the same open twice
    /// before the first `Task::perform` resolves races two tabs into
    /// existence aliasing one `engines` entry; closing either then
    /// orphans the other. Inserted right before the `Task::perform` is
    /// spawned; cleared in both the `Ok` and `Err` arms of
    /// `FileMsg::SchematicOpenFinished` / `FileMsg::PcbOpenFinished`.
    pub pending_opens: crate::app::document_read::InFlightReads,
    /// What the directory case probes of loose documents reported, and the
    /// opens and saves waiting on one (#1447).
    pub dir_case: dir_case::DirCases,
    /// #859 — History restores running on a worker, keyed by the document
    /// path. A second restore click for the same document is ignored until
    /// the first completes; closing the project or all documents cancels it.
    pub pending_restores: crate::app::document_read::InFlightReads,
    /// #859 — renames and deletes whose disk work runs on a worker. Saves,
    /// opens, restores and second moves of an overlapping path are refused
    /// until the completion message drops the entry.
    pub pending_path_ops: crate::app::path_ops::PendingPathOps,
    /// New Projects whose folder and files a worker is still writing, by the
    /// chosen `.snxprj`. A quit is held until they land (#2280).
    pub new_project_writes: std::collections::HashSet<PathBuf>,
    /// The Altium import still running, if any. A second import is
    /// refused while one runs: two converters writing the same output
    /// folder would interleave.
    pub altium_import_running: Option<AltiumImportRun>,
    /// Projects whose folder is being renamed on the blocking pool (#1221).
    /// A second rename of the same project is refused until the first
    /// reports back.
    pub project_folder_renames: std::collections::HashSet<ProjectId>,
    /// Where each write running on the blocking pool lands (an export, Add
    /// New Schematic, the target folder of an Add Existing copy, a new
    /// library file), one entry per write. A folder rename is refused while
    /// one lies under the folder, or the late write would land in the old
    /// folder.
    pub project_writes: Vec<PathBuf>,
    /// Project folders a rename moved away this session. A write a dialog
    /// aimed there before the move is refused, or it would recreate the old
    /// folder as an orphan.
    pub moved_project_dirs: Vec<MovedProjectDir>,
    /// Commits asked for while their project folder moved, run once the move
    /// has finished.
    pub deferred_git_commits: Vec<DeferredGitCommit>,
}

/// A project folder a rename moved from `old` to `new`.
#[derive(Debug, Clone)]
pub struct MovedProjectDir {
    pub old: PathBuf,
    pub new: PathBuf,
    pub fold: CaseFold,
}

/// A project commit asked for while the project folder moved; the paths are
/// as they were named then.
#[derive(Debug, Clone)]
pub struct DeferredGitCommit {
    pub project: ProjectId,
    pub file_path: PathBuf,
    pub message: String,
    pub replace_message: bool,
    pub removed_path: Option<PathBuf>,
    /// Whether the commit may carry the agent edit summary (#2253); a restore
    /// never does.
    pub with_summary: bool,
}

/// An Altium import on the blocking pool (#1123).
#[derive(Debug, Clone)]
pub struct AltiumImportRun {
    /// The picked Altium file.
    pub input: PathBuf,
    /// Set by Cancel; the converter loop stops the child when it sees it.
    pub cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl PartialEq for AltiumImportRun {
    fn eq(&self, other: &Self) -> bool {
        self.input == other.input && std::sync::Arc::ptr_eq(&self.cancel, &other.cancel)
    }
}

/// v0.23 — One queued commit for the async git pipeline. Stays
/// resident in `DocumentState.pending_git_commits` until
/// `finish_update` drains it into a `Task::perform`.
#[derive(Debug, Clone)]
pub struct PendingGitCommit {
    pub project_root: PathBuf,
    pub rel_path: PathBuf,
    /// A project-relative path the same commit removes, so a move reads as
    /// a rename in the history (#1219).
    pub removed_rel_path: Option<PathBuf>,
    pub message: String,
    /// The exact bytes to commit for `rel_path`: what a save wrote or a
    /// restore read back. `None` commits the working file as it is when the
    /// worker runs (callers that hold no copy of what they wrote).
    pub content: Option<crate::persist::SharedBytes>,
    /// The project's "Summarise agent changes in the commit message" as it was
    /// when the commit was queued (#2253).
    pub summarise_agent_edits: bool,
}

impl PendingGitCommit {
    /// Whether the worker reads the working file when it runs: it holds no
    /// bytes, or it is a rename commit, which stages the file from disk and
    /// ignores any bytes.
    pub fn reads_working_file(&self) -> bool {
        self.content.is_none() || self.removed_rel_path.is_some()
    }
}

/// The bytes a commit records and the order of the write they came from.
/// Saves of one file can report completion out of order; a commit of an
/// older write must never replace, or queue behind, a newer one's.
#[derive(Debug, Clone)]
pub struct CommitBytes {
    pub bytes: crate::persist::SharedBytes,
    /// The write's position in its document-save ledger; `0` is exempt from
    /// ordering (a restore).
    pub sequence: u64,
}

impl DocumentState {
    /// Mint a fresh `ProjectId` and bump the counter. Never reuses ids.
    pub fn mint_project_id(&mut self) -> ProjectId {
        ProjectId::mint(&mut self.next_project_id)
    }

    pub fn project_by_id(&self, id: ProjectId) -> Option<&LoadedProject> {
        self.projects.iter().find(|p| p.id == id)
    }

    pub fn project_by_id_mut(&mut self, id: ProjectId) -> Option<&mut LoadedProject> {
        self.projects.iter_mut().find(|p| p.id == id)
    }

    /// Resolve the project that contains a file at this path. Used for
    /// per-tab project scoping (tabs store a path, we resolve to the
    /// project that parented them at load time).
    pub fn project_for_path(&self, path: &std::path::Path) -> Option<&LoadedProject> {
        let dir = path.parent()?;
        self.projects.iter().find(|p| p.path.parent() == Some(dir))
    }

    /// Convenience: currently-active project. Returns `None` when the
    /// workspace is empty or no project has been made active yet.
    pub fn active_loaded_project(&self) -> Option<&LoadedProject> {
        self.active_project.and_then(|id| self.project_by_id(id))
    }

    /// Project that owns the *active document* — the scope for export, ERC,
    /// annotate and hierarchical child-sheet resolution.
    ///
    /// Deliberately not `active_loaded_project()`: `active_project` is a
    /// sticky workspace-wide pointer that survives focusing a tab with no
    /// project of its own, so scoping off it makes an export of a loose
    /// schematic emit another project's sheets (#406). Resolution rules and
    /// the hierarchy walk live in [`scope::project_owning_sheet`].
    ///
    /// - active schematic owned by a project → that project;
    /// - active schematic owned by none (loose) → `None`, i.e. operate on
    ///   this one document;
    /// - no active schematic at all (a PCB / symbol / footprint tab is
    ///   focused, or nothing is open) → the sticky pointer, which is what
    ///   "the workspace at large" means when there is no document to scope by.
    pub fn active_document_project(&self) -> Option<&LoadedProject> {
        let Some(path) = self.active_path.as_ref() else {
            return self.active_loaded_project();
        };
        self.document_project(path)
    }

    /// The designator standard of the project that owns `sheet` (#1416).
    pub fn designator_standard_for(
        &self,
        sheet: &std::path::Path,
    ) -> Option<signex_types::designator::DesignatorStandard> {
        self.document_project(sheet)
            .and_then(|p| p.data.designator_standard())
    }

    /// The resolved schematic defaults (#2026) of the project that owns
    /// `sheet`; the built-in table for a loose sheet.
    pub fn schematic_defaults_for(
        &self,
        sheet: &std::path::Path,
    ) -> signex_types::schematic::SchematicDefaults {
        self.document_project(sheet)
            .map_or(signex_types::schematic::SchematicDefaults::BUILT_IN, |p| {
                p.data.schematic_defaults.resolve()
            })
    }

    /// [`Self::schematic_defaults_for`] the active document, or the sheet an
    /// agent's edit list runs on.
    pub fn active_schematic_defaults(&self) -> signex_types::schematic::SchematicDefaults {
        self.edit_path()
            .map(|path| self.schematic_defaults_for(path))
            .unwrap_or_default()
    }

    /// A pre-placement form for the active document, starting from its
    /// project's text defaults (#2026).
    pub fn new_pre_placement(
        &self,
        tool_name: String,
        kind: crate::panels::PrePlacementKind,
        label_text: String,
        cursor_mm: (f64, f64),
    ) -> crate::panels::PrePlacementData {
        let mut data = crate::panels::PrePlacementData::new(
            tool_name,
            kind,
            label_text,
            cursor_mm.0,
            cursor_mm.1,
        );
        let defaults = self.active_schematic_defaults();
        data.font_size_pt = defaults.placed_text_pt();
        data.shape_fill = defaults.shape_fill;
        if let Some(path) = self.active_path.as_deref() {
            data.font = self.schematic_default_font_for(path);
        }
        data
    }

    /// The font a label or note placed on the active document stores: the
    /// project's default font when it names one, else `None` (#2026).
    pub fn placed_font_family(&self) -> Option<String> {
        self.edit_path()
            .and_then(|path| self.document_project(path))
            .and_then(|p| p.data.schematic_defaults.placed_font_family())
    }

    /// The font new text on `sheet` is placed in (#2026).
    pub fn schematic_default_font_for(&self, sheet: &std::path::Path) -> String {
        self.document_project(sheet).map_or_else(
            || signex_types::schematic::SCHEMATIC_DEFAULT_FONT.to_string(),
            |p| p.data.schematic_defaults.font_family().to_string(),
        )
    }
    /// Every open engine that belongs to the same project as the active
    /// document, the active one included. A loose active document yields only
    /// itself. Used where a name must be unique across the whole hierarchy
    /// (a project-wide default such as a new child sheet's `sheetN`).
    pub fn engines_in_active_document_project(&self) -> Vec<&signex_engine::Engine> {
        let Some(project) = self.active_document_project() else {
            return self.active_engine().into_iter().collect();
        };
        scope::engines_in_project(
            &self.projects,
            &self.ownership_index(),
            &self.engines,
            project.id,
        )
    }

    /// [`engines_in_active_document_project`](Self::engines_in_active_document_project)
    /// with each engine's sheet path, so a reference can be resolved against
    /// the folder of the sheet that carries it.
    pub(crate) fn engine_entries_in_active_document_project(
        &self,
    ) -> Vec<(&PathBuf, &signex_engine::Engine)> {
        let Some(project) = self.active_document_project() else {
            return self
                .active_path
                .as_ref()
                .and_then(|path| self.engines.get_key_value(path))
                .into_iter()
                .collect();
        };
        scope::engine_entries_in_project(
            &self.projects,
            &self.ownership_index(),
            &self.engines,
            project.id,
        )
    }

    /// Record that the set of open documents, or what they hold on disk,
    /// changed. See [`DocumentState::documents_epoch`].
    pub fn note_documents_changed(&mut self) {
        self.documents_epoch += 1;
    }

    /// Install `engine` for `path` and move the epoch. Use this for any engine
    /// a background read could otherwise miss. The active-session swap in
    /// `load_gateway` and the bulk `engines.retain` calls bypass it; they act
    /// only on engines that already existed, which a background read never
    /// replaces from disk.
    pub fn insert_engine(
        &mut self,
        path: PathBuf,
        engine: signex_engine::Engine,
    ) -> Option<signex_engine::Engine> {
        self.note_documents_changed();
        self.engines.insert(path, engine)
    }

    /// Drop the engine for `path`; see [`DocumentState::insert_engine`].
    pub fn remove_engine(&mut self, path: &std::path::Path) -> Option<signex_engine::Engine> {
        self.note_documents_changed();
        // The head read before it was open may be older than what it saved.
        self.document_ids.forget_header(path);
        self.engines.remove(path)
    }

    /// A `.snxsch` was written or deleted on disk with no engine behind it
    /// (the unopened-sheet Annotate and Reset Duplicate Designators writes,
    /// a deleted project file). The writers reach it through
    /// `autosave_after_disk_write` or `drop_deleted_documents`; a new
    /// engine-less writer must call one of those or this, so a background read
    /// of that file can tell it is out of date.
    pub fn note_document_written_behind_engines(&mut self) {
        self.note_documents_changed();
        self.note_sheet_edit();
    }

    pub fn active_engine(&self) -> Option<&signex_engine::Engine> {
        self.engines.get(self.active_path.as_ref()?)
    }

    pub fn active_engine_mut(&mut self) -> Option<&mut signex_engine::Engine> {
        let path = self.active_path.as_ref()?.clone();
        self.engines.get_mut(&path)
    }

    /// Drop the engine for the active path. Used when closing the
    /// active tab — the tab's engine is gone, and `active_path` follows
    /// to whichever tab becomes active next.
    pub fn clear_active_engine(&mut self) {
        self.note_documents_changed();
        if let Some(path) = self.active_path.as_ref().cloned() {
            self.engines.remove(&path);
        }
        self.active_path = None;
    }

    pub fn has_active_engine(&self) -> bool {
        self.active_engine().is_some()
    }

    /// Per-window engine lookup. Main window → the active tab's engine
    /// (same as `active_engine`). Undocked tab windows → the engine for
    /// the path the window was opened on. All schematic engines live in
    /// `self.engines`, so every window resolves with a single HashMap
    /// lookup.
    pub fn engine_for_window(
        &self,
        window_id: iced::window::Id,
        ui: &UiState,
    ) -> Option<&signex_engine::Engine> {
        self.engines.get(self.sheet_path_for_window(window_id, ui)?)
    }

    /// The sheet a window shows: the active one for the main window, the tab's
    /// own for an undocked one.
    pub fn sheet_path_for_window<'a>(
        &'a self,
        window_id: iced::window::Id,
        ui: &'a UiState,
    ) -> Option<&'a std::path::PathBuf> {
        if ui.main_window_id == Some(window_id) {
            return self.active_path.as_ref();
        }
        match ui.windows.get(&window_id)? {
            WindowKind::UndockedTab { path, .. } => Some(path),
            _ => None,
        }
    }
}
