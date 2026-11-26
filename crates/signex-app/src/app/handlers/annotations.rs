//! The Annotations panel (mockup row 165): reading the project's notebook files off
//! the UI thread, reporting what could not be read in Messages, the filters,
//! and a reference click going to its part or net. The rows the panel draws
//! are built here, in `update`, into `PanelContext::annotations`; see
//! `docs/NOTEBOOK_ANNOTATIONS.md`, "What the panel reads".

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::Task;
use signex_i18n::{I18n, Localized, ids, t};
use signex_net::NetMemberKind;
use signex_notebook::{NOTEBOOK_DIR, NOTEBOOK_EXTENSION, ProblemKind, newest_first};
use signex_types::schematic::{SelectedItem, SelectedKind};

use crate::app::handlers::canvas::{AfterOpen, SheetOpen};
use crate::app::notebook_references;
use crate::app::project_sheets::{self, OpenSheets, ProjectSheetSet};
use crate::app::sheet_hierarchy::build_sheet_hierarchy;
use crate::app::{CaseFold, Message, ProjectId, Signex};
use crate::dock::DockMessage;
use crate::panels::annotations::{
    Annotation, AnnotationKind, AnnotationRow, AnnotationsState, AnnotationsView, FileGroup,
    FileReadError, IsoDate, KindChip, ListStatus, NotebookFile, NotebookScan, PartIndex, PartPlace,
    RefChip, Reference, RowKey, ScanProblem, notebook_badges, record_fields,
};
use crate::panels::{AnnotationsMsg, NetInspectorMsg, PanelKind, PanelMsg};
use crate::schematic_runtime::notebook_badges::{
    NotebookBadgeInk, NotebookBadgeView, NotebookBadges,
};

/// The log target of the panel's Messages lines (Source `SIGNEX-NOTEBOOK`).
const TARGET: &str = "signex::notebook";
const SECONDS_PER_DAY: u64 = 86_400;

/// What a scan reads, captured in `update`.
pub(crate) struct ScanInputs {
    project_dir: PathBuf,
    /// The design to find the referenced parts in; `None` without a sheet.
    design: Option<DesignInputs>,
}

struct DesignInputs {
    open: OpenSheets,
    pages: Vec<PathBuf>,
    root: PathBuf,
    fold: CaseFold,
}

impl Signex {
    /// Keep the scan current: a scan starts when the panel is shown and when
    /// the project in view changes (a project opened, a switch to another),
    /// panel open or not, since the schematic's Notebook badges come from
    /// it and are drawn by default. Runs after every message.
    pub(crate) fn sync_annotations_panel(&mut self) -> Task<Message> {
        let open = self.is_panel_open(PanelKind::Annotations);
        let newly_shown = open && !std::mem::replace(&mut self.ui_state.annotations.shown, true);
        if !open {
            self.ui_state.annotations.shown = false;
        }
        // Compared without building the target: this runs on every message.
        let held = self.ui_state.annotations.target.as_ref();
        let active = self.document_state.active_document_project();
        let moved = held.map(|(id, dir)| (*id, dir.as_path())) != active.map(|p| (p.id, p.dir()));
        // A project being closed is not read for the badges it is about to lose.
        let closing = active.is_some_and(|p| self.ui_state.annotations.closing == Some(p.id));
        if newly_shown || (moved && !closing) {
            let target = self.annotations_target();
            return self.start_annotations_scan(target);
        }
        Task::none()
    }

    /// The project the panel lists: the active document's.
    pub(crate) fn annotations_target(&self) -> Option<(ProjectId, PathBuf)> {
        self.document_state
            .active_document_project()
            .map(|project| (project.id, project.dir().to_path_buf()))
    }

    /// Read the notebook files of `target` on a worker. The last list stays
    /// shown meanwhile unless the project changed.
    pub(crate) fn start_annotations_scan(
        &mut self,
        target: Option<(ProjectId, PathBuf)>,
    ) -> Task<Message> {
        let inputs = target
            .as_ref()
            .map(|(id, dir)| self.capture_annotation_inputs(*id, dir));
        let state = &mut self.ui_state.annotations;
        let same_project = state.target.as_ref().map(|t| t.0) == target.as_ref().map(|t| t.0);
        state.target = target;
        state.generation += 1;
        let generation = state.generation;
        let Some(inputs) = inputs else {
            state.scan = None;
            state.expanded = None;
            state.status = ListStatus::NoProject;
            clear_badges(state);
            self.refresh_annotations_view();
            self.repaint_notebook_badges();
            return Task::none();
        };
        if !same_project || state.scan.is_none() {
            state.scan = None;
            state.expanded = None;
            state.status = ListStatus::Reading;
            // Another project's badges never show on this one's sheets.
            clear_badges(state);
            self.repaint_notebook_badges();
        }
        self.refresh_annotations_view();
        Task::perform(
            async move {
                match tokio::task::spawn_blocking(move || scan_notebooks(&inputs)).await {
                    Ok(result) => result,
                    Err(e) => Err(format!("the notebook read stopped: {e}")),
                }
            },
            move |result| {
                Message::Dock(DockMessage::Panel(PanelMsg::Annotations(
                    AnnotationsMsg::Scanned {
                        generation,
                        result: Arc::new(result),
                    },
                )))
            },
        )
    }

    pub(crate) fn capture_annotation_inputs(&self, id: ProjectId, dir: &Path) -> ScanInputs {
        let ds = &self.document_state;
        let project = ds.projects.iter().find(|p| p.id == id);
        let pages = project_sheets::project_pages(project);
        let root = project
            .and_then(project_sheets::project_root_sheet_path)
            .or_else(|| pages.first().cloned());
        let design = root.map(|root| DesignInputs {
            open: project_sheets::open_sheets_snapshot(ds),
            pages,
            root,
            fold: crate::app::state::project_case::fold_of(project),
        });
        ScanInputs {
            project_dir: dir.to_path_buf(),
            design,
        }
    }

    /// An Annotations panel event.
    pub(crate) fn handle_annotations_message(&mut self, msg: AnnotationsMsg) -> Task<Message> {
        let state = &mut self.ui_state.annotations;
        match msg {
            AnnotationsMsg::Scanned { generation, result } => {
                if generation != state.generation {
                    return Task::none();
                }
                self.apply_annotation_scan(result);
                self.refresh_annotations_view();
                return Task::none();
            }
            AnnotationsMsg::QueryChanged(query) => state.filter.query = query,
            AnnotationsMsg::KindToggled(kind) => {
                if !state.filter.hidden_kinds.remove(&kind) {
                    state.filter.hidden_kinds.insert(kind);
                }
            }
            AnnotationsMsg::PartPicked(part) => state.filter.part = Some(part),
            AnnotationsMsg::PartCleared => state.filter.part = None,
            AnnotationsMsg::StatusPicked(status) => state.filter.status = Some(status),
            AnnotationsMsg::StatusCleared => state.filter.status = None,
            AnnotationsMsg::DatePicked(window) => state.filter.date = window,
            AnnotationsMsg::RowToggled(key) => {
                state.expanded = (state.expanded.as_ref() != Some(&key)).then_some(key);
            }
            AnnotationsMsg::ReferenceClicked(reference) => {
                return self.jump_to_reference(&reference);
            }
            AnnotationsMsg::RefreshRequested => {
                let target = self.annotations_target();
                return self.start_annotations_scan(target);
            }
        }
        self.refresh_annotations_view();
        Task::none()
    }

    /// Keep a scan's answer and say in Messages what it could not read.
    fn apply_annotation_scan(&mut self, result: Arc<Result<NotebookScan, String>>) {
        let result = Arc::try_unwrap(result).unwrap_or_else(|shared| (*shared).clone());
        let state = &mut self.ui_state.annotations;
        match result {
            Ok(scan) => {
                report_scan(&scan, &self.ui_state.i18n);
                state.status = if scan.folder_found && !scan.files.is_empty() {
                    ListStatus::Ready
                } else {
                    ListStatus::NoNotebook
                };
                let still_there = |key: &RowKey| {
                    scan.files.iter().any(|f| {
                        f.path == key.file && f.annotations.iter().any(|a| a.line == key.line)
                    })
                };
                if !state.expanded.as_ref().is_some_and(still_there) {
                    state.expanded = None;
                }
                let badges = notebook_badges(&scan);
                state.badge_marks = Arc::new(NotebookBadges::from_badges(&badges));
                state.badges = Arc::new(badges);
                state.scan = Some(Arc::new(scan));
            }
            Err(why) => {
                let line = t!(
                    self.ui_state.i18n, ids::ANNOTATIONS_READ_FAILED,
                    "file" => NOTEBOOK_DIR, "why" => why.as_str()
                );
                log::warn!(target: TARGET, "{}", line.into_string());
                state.scan = None;
                state.status = ListStatus::Unavailable;
                clear_badges(state);
            }
        }
        self.repaint_notebook_badges();
    }

    /// The Notebook badges the sheet at `path` shows: the last scan's, when
    /// the sheet is of the project that scan read and View ▸ Notebook Badges
    /// is on; none elsewhere.
    pub(crate) fn notebook_badge_view(&self, path: Option<&Path>) -> NotebookBadgeView<'_> {
        let state = &self.ui_state.annotations;
        let project = path
            .and_then(|path| self.document_state.document_project(path))
            .map(|project| project.id);
        let scanned = state.target.as_ref().map(|target| target.0);
        let shown = !state.badges_hidden && project.is_some() && project == scanned;
        NotebookBadgeView {
            badges: shown.then_some(&*state.badge_marks),
            ink: NotebookBadgeInk::from_tokens(
                &self.tokens_for(self.ui_state.preferences_draft_theme),
            ),
        }
    }

    /// The badges changed: Properties ▸ Design notes reads them, and the
    /// selection layer of every sheet, where they are drawn, redraws.
    pub(crate) fn repaint_notebook_badges(&mut self) {
        self.document_state.panel_ctx.notebook_badges =
            Arc::clone(&self.ui_state.annotations.badges);
        self.interaction_state.canvas.clear_overlay_cache();
        for canvas in self.interaction_state.canvases.values_mut() {
            canvas.clear_overlay_cache();
        }
    }

    /// Rebuild the rows the panel draws from the scan and the filters.
    fn refresh_annotations_view(&mut self) {
        let view = build_view(&self.ui_state.annotations, today());
        self.document_state.panel_ctx.annotations = Arc::new(view);
    }

    /// Go to a reference: a part's sheet, zoomed to and selecting the part;
    /// a net through net navigation. What cannot be found is said in
    /// Messages.
    pub(crate) fn jump_to_reference(&mut self, reference: &Reference) -> Task<Message> {
        let i18n = Arc::clone(&self.ui_state.i18n);
        let name = reference.name();
        let Some(part) = reference.component() else {
            return self.jump_to_net(&name);
        };
        let Some(scan) = self.ui_state.annotations.scan.clone() else {
            return Task::none();
        };
        let Some(parts) = scan.parts.as_ref() else {
            let line = t!(i18n, ids::ANNOTATIONS_JUMP_DESIGN_UNREAD, "name" => part);
            crate::diagnostics::log_info(line.into_string());
            return Task::none();
        };
        let Some(place) = parts.get(part) else {
            crate::diagnostics::log_info(
                t!(i18n, ids::ANNOTATIONS_JUMP_GONE, "name" => part).into_string(),
            );
            return Task::none();
        };
        let open = match &place.placement {
            Some(placement) => SheetOpen::at(Some(placement.clone())),
            None => SheetOpen::plain(),
        };
        let item = SelectedItem::new(place.symbol, SelectedKind::Symbol);
        self.open_sheet(
            place.sheet.clone(),
            open.then(AfterOpen::Reveal { item: Some(item) }),
        )
    }

    /// Highlight `name` on every sheet and go to its first member, as the
    /// Net Inspector does; the net index must be of the project in view.
    fn jump_to_net(&mut self, name: &str) -> Task<Message> {
        let i18n = Arc::clone(&self.ui_state.i18n);
        let ds = &self.document_state;
        let active = ds
            .active_path
            .as_deref()
            .and_then(|path| ds.document_project(path))
            .map(|p| p.id);
        let wanted = self.ui_state.annotations.target.as_ref().map(|t| t.0);
        let Some(project) = active.filter(|id| Some(*id) == wanted) else {
            let line = t!(i18n, ids::ANNOTATIONS_JUMP_NET_OPEN_SHEET, "name" => name);
            crate::diagnostics::log_info(line.into_string());
            return Task::none();
        };
        let planned = crate::app::runtime::net_class_styles::plan_projects(ds)
            .into_iter()
            .find(|p| p.project == project);
        let data = planned.and_then(|p| ds.net_nav.index.fresh(project, &p.key));
        let Some(data) = data else {
            let line = t!(i18n, ids::ANNOTATIONS_JUMP_NET_READING, "name" => name);
            crate::diagnostics::log_info(line.into_string());
            return self.warm_net_index_for_pin_rows();
        };
        let index = &data.index;
        let first = (0..index.net_count())
            .find(|&i| index.net(i).is_some_and(|net| net.name == name))
            .and_then(|i| index.steps(i).first().copied().cloned());
        let Some(member) = first else {
            let line = t!(i18n, ids::ANNOTATIONS_JUMP_NET_UNKNOWN, "name" => name);
            crate::diagnostics::log_info(line.into_string());
            return Task::none();
        };
        let pin = match &member.kind {
            NetMemberKind::Pin { pin, .. } => Some(pin.clone()),
            _ => None,
        };
        let anchor = signex_net::NetAnchor {
            sheet: member.sheet.clone(),
            id: member.id,
            occurrence: member.occurrence.clone(),
            pin,
        };
        self.handle_net_inspector_message(NetInspectorMsg::MemberClicked {
            net: name.to_string(),
            anchor,
        })
    }
}

/// Drop the badges of the last scan, so no other project's show.
fn clear_badges(state: &mut AnnotationsState) {
    state.badges = Arc::default();
    state.badge_marks = Arc::default();
    state.hovered_badge = None;
}

/// Today in UTC, the day the date windows count back from (1970-01-01 on a
/// clock set before it).
pub(crate) fn today() -> IsoDate {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    i64::try_from(seconds / SECONDS_PER_DAY)
        .ok()
        .and_then(IsoDate::from_days_since_epoch)
        .unwrap_or(IsoDate::EPOCH)
}

/// Read every notebook file of the project and find the design's parts.
/// Blocking: runs on a worker.
pub(crate) fn scan_notebooks(inputs: &ScanInputs) -> Result<NotebookScan, String> {
    let (scan, texts) = read_notebook_texts(&inputs.project_dir)?;
    let set = inputs.design.as_ref().map(assemble_design);
    let parts = inputs
        .design
        .as_ref()
        .zip(set.as_ref())
        .and_then(|(design, set)| part_index(design, set));
    // Only a design read whole resolves references: a part on a sheet that
    // could not be read would wrongly read as deleted.
    let design = parts
        .as_ref()
        .and(set.as_ref())
        .and_then(|set| notebook_references::read_design(set, &inputs.project_dir))
        .map(Arc::new);
    let references = design
        .as_deref()
        .map(|design| notebook_references::file_references(&scan.files, &texts, design))
        .unwrap_or_default();
    Ok(NotebookScan {
        parts,
        design,
        references,
        ..scan
    })
}

/// Read every `.snxnbk` file under the project's `notebook/` folder, its
/// chapter subfolders included (mockup row 195: one notebook that includes
/// chapter files), sorted by path. No parts are looked up. Blocking: runs
/// on a worker, for the panel and for `list_annotations`.
pub(crate) fn read_notebook_files(project_dir: &Path) -> Result<NotebookScan, String> {
    read_notebook_texts(project_dir).map(|(scan, _texts)| scan)
}

/// [`read_notebook_files`] and the text of each file, `None` where it
/// could not be read.
fn read_notebook_texts(project_dir: &Path) -> Result<(NotebookScan, Vec<Option<String>>), String> {
    let folder = project_dir.join(NOTEBOOK_DIR);
    let entries = match std::fs::read_dir(&folder) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok((NotebookScan::default(), Vec::new()));
        }
        Err(e) => return Err(e.to_string()),
    };
    // An entry that cannot be listed is said in Messages, never dropped.
    let (mut paths, mut unlisted) = (Vec::new(), Vec::new());
    let mut pending = vec![entries];
    while let Some(entries) = pending.pop() {
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    unlisted.push(e.to_string());
                    continue;
                }
            };
            let path = entry.path();
            // The entry's own type: a link to a folder is not followed.
            let is_dir = entry.file_type().is_ok_and(|kind| kind.is_dir());
            // The generated design report copies the notebook's decisions:
            // its blocks are not read again, so none counts twice (#3021).
            if is_dir && path == folder.join(signex_notebook::generate::GENERATED_DIR) {
                continue;
            }
            if is_dir {
                match std::fs::read_dir(&path) {
                    Ok(inner) => pending.push(inner),
                    Err(e) => unlisted.push(format!("{}: {e}", path.display())),
                }
                continue;
            }
            let notebook = path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case(NOTEBOOK_EXTENSION));
            if notebook && path.is_file() {
                paths.push(path);
            }
        }
    }
    paths.sort();
    let (files, texts): (Vec<NotebookFile>, Vec<Option<String>>) = paths
        .into_iter()
        .map(|path| read_notebook(&folder, path))
        .unzip();
    let scan = NotebookScan {
        folder_found: true,
        files,
        unlisted,
        ..NotebookScan::default()
    };
    Ok((scan, texts))
}

fn read_notebook(folder: &Path, path: PathBuf) -> (NotebookFile, Option<String>) {
    let inside: Vec<String> = path
        .strip_prefix(folder)
        .unwrap_or(&path)
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect();
    let label = format!("{NOTEBOOK_DIR}/{}", inside.join("/"));
    let text = std::fs::read(&path)
        .map_err(|e| FileReadError::Io(e.to_string()))
        .and_then(|bytes| String::from_utf8(bytes).map_err(|_| FileReadError::NotUtf8));
    let (annotations, problems, read_error, text) = match text {
        Ok(text) => {
            let (annotations, problems) = signex_notebook::scan(&text);
            (annotations, problems, None, Some(text))
        }
        Err(why) => (Vec::new(), Vec::new(), Some(why), None),
    };
    let file = NotebookFile {
        path,
        label,
        annotations,
        problems,
        read_error,
    };
    (file, text)
}

/// The project's sheets, open ones with their unsaved edits.
fn assemble_design(design: &DesignInputs) -> ProjectSheetSet {
    project_sheets::assemble_project_sheets_quiet_from(
        &design.open,
        &design.pages,
        &design.root,
        design.fold,
    )
}

/// Every designator of the design and where its part is. `None` when a sheet
/// could not be read or is missing: a part there would wrongly read as gone.
fn part_index(design: &DesignInputs, set: &ProjectSheetSet) -> Option<PartIndex> {
    if !set.unreadable.is_empty() || !set.absent.is_empty() {
        return None;
    }
    let hierarchy = build_sheet_hierarchy(
        &set.sheets,
        &set.unreadable,
        &design.root,
        &set.pages_outside_the_hierarchy,
        design.fold,
    );
    let mut paths: Vec<&PathBuf> = set.sheets.keys().collect();
    paths.sort();
    let mut parts = PartIndex::new();
    for path in paths {
        let placements = hierarchy.designator_scope(path).placements;
        for symbol in set.sheets[path]
            .symbols
            .iter()
            .filter(|s| !s.reference.is_empty())
        {
            let place = |placement| PartPlace {
                sheet: path.clone(),
                symbol: symbol.uuid,
                placement,
            };
            for placement in &placements {
                let designator = symbol.designator_in(placement).to_string();
                parts
                    .entry(designator)
                    .or_insert_with(|| place(Some(placement.clone())));
            }
            parts
                .entry(symbol.reference.clone())
                .or_insert_with(|| place(None));
        }
    }
    Some(parts)
}

/// One Messages line per file that could not be read and per problem.
fn report_scan(scan: &NotebookScan, i18n: &I18n) {
    for why in &scan.unlisted {
        let line = t!(
            i18n, ids::ANNOTATIONS_READ_FAILED,
            "file" => NOTEBOOK_DIR, "why" => why.as_str()
        );
        log::warn!(target: TARGET, "{}", line.into_string());
    }
    for file in &scan.files {
        if let Some(why) = &file.read_error {
            let why = match why {
                FileReadError::Io(reason) => signex_i18n::Arg::from(reason.as_str()),
                FileReadError::NotUtf8 => {
                    signex_i18n::Translatable::new(ids::ANNOTATIONS_NOT_UTF8).into()
                }
            };
            let line = t!(
                i18n, ids::ANNOTATIONS_READ_FAILED,
                "file" => file.label.as_str(), "why" => why
            );
            log::warn!(target: TARGET, "{}", line.into_string());
        }
        for problem in &file.problems {
            let line = problem_text(i18n, &file.label, problem);
            log::warn!(target: TARGET, "{}", line.into_string());
        }
    }
}

/// The Messages line of one problem: the file, the line, what was skipped.
pub(crate) fn problem_text(i18n: &I18n, file: &str, problem: &ScanProblem) -> Localized {
    let line = problem.line.to_string();
    let at = |id: signex_i18n::MessageId| t!(i18n, id, "file" => file, "line" => line.as_str());
    match &problem.kind {
        ProblemKind::UnclosedFrontMatter => at(ids::ANNOTATIONS_PROBLEM_UNCLOSED_FRONT_MATTER),
        ProblemKind::MissingKind => at(ids::ANNOTATIONS_PROBLEM_MISSING_KIND),
        ProblemKind::MissingBody => at(ids::ANNOTATIONS_PROBLEM_MISSING_BODY),
        ProblemKind::UnbalancedBlock => at(ids::ANNOTATIONS_PROBLEM_UNBALANCED_BLOCK),
        ProblemKind::UnknownKind { kind } => t!(
            i18n, ids::ANNOTATIONS_PROBLEM_UNKNOWN_KIND,
            "file" => file, "line" => line.as_str(), "kind" => kind.as_str()
        ),
        ProblemKind::UnknownStatus { kind, status } => t!(
            i18n, ids::ANNOTATIONS_PROBLEM_UNKNOWN_STATUS,
            "file" => file, "line" => line.as_str(),
            "kind" => kind.token(), "status" => status.as_str()
        ),
        ProblemKind::BadDate { date } => t!(
            i18n, ids::ANNOTATIONS_PROBLEM_BAD_DATE,
            "file" => file, "line" => line.as_str(), "date" => date.as_str()
        ),
        ProblemKind::UnknownArgument { argument } => t!(
            i18n, ids::ANNOTATIONS_PROBLEM_UNKNOWN_ARGUMENT,
            "file" => file, "line" => line.as_str(), "argument" => argument.as_str()
        ),
        ProblemKind::NotText { argument } => t!(
            i18n, ids::ANNOTATIONS_PROBLEM_NOT_TEXT,
            "file" => file, "line" => line.as_str(), "argument" => argument.as_str()
        ),
        ProblemKind::Unclosed { opener } => t!(
            i18n, ids::ANNOTATIONS_PROBLEM_UNCLOSED,
            "file" => file, "line" => line.as_str(), "opener" => opener_text(*opener)
        ),
        ProblemKind::BadReference { reference } => t!(
            i18n, ids::ANNOTATIONS_PROBLEM_BAD_REFERENCE,
            "file" => file, "line" => line.as_str(), "reference" => reference.as_str()
        ),
        ProblemKind::BadId { kind, id } => t!(
            i18n, ids::ANNOTATIONS_PROBLEM_BAD_ID,
            "file" => file, "line" => line.as_str(), "id" => id.as_str(),
            "kind" => kind.function(), "prefix" => kind.id_prefix()
        ),
        ProblemKind::DuplicateId { id } => t!(
            i18n, ids::ANNOTATIONS_PROBLEM_DUPLICATE_ID,
            "file" => file, "line" => line.as_str(), "id" => id.as_str()
        ),
        ProblemKind::BadField { argument, value } => t!(
            i18n, ids::ANNOTATIONS_PROBLEM_BAD_FIELD,
            "file" => file, "line" => line.as_str(),
            "argument" => argument.as_str(), "value" => value.as_str()
        ),
    }
}

/// How an opener reads in a Messages line: a block comment as `/*`.
fn opener_text(opener: char) -> String {
    match opener {
        '/' => "/*".to_string(),
        other => other.to_string(),
    }
}

/// The rows the panel draws: the scan under the filters, each file's rows
/// newest first, with the kind counts and the filter choices.
pub(crate) fn build_view(state: &AnnotationsState, today: IsoDate) -> AnnotationsView {
    let filter = &state.filter;
    let mut view = AnnotationsView {
        status: state.status,
        query: filter.query.clone(),
        part: filter.part.clone(),
        status_filter: filter.status,
        date: filter.date,
        ..AnnotationsView::default()
    };
    let Some(scan) = state.scan.as_deref() else {
        return view;
    };
    let mut counts = [0u32; AnnotationKind::ALL.len()];
    let (mut parts, mut statuses) = (BTreeSet::new(), BTreeSet::new());
    for annotation in scan.files.iter().flat_map(|f| &f.annotations) {
        view.total += 1;
        counts[kind_index(annotation.kind)] += 1;
        parts.extend(
            annotation
                .references
                .iter()
                .filter_map(Reference::component),
        );
        statuses.extend(annotation.status);
    }
    view.files = scan.files.len();
    view.parts = parts.into_iter().map(str::to_string).collect();
    view.statuses = statuses.into_iter().collect();
    view.kinds = AnnotationKind::ALL
        .iter()
        .map(|&kind| KindChip {
            kind,
            count: counts[kind_index(kind)],
            on: !filter.hidden_kinds.contains(&kind),
        })
        .collect();
    // Blocks cite each other across files: a chapter's Issue `from:` a
    // Measurement of another.
    let everything: Vec<Annotation> = scan
        .files
        .iter()
        .flat_map(|f| f.annotations.iter().cloned())
        .collect();
    let links = signex_notebook::links(&everything);
    for file in &scan.files {
        let mut shown: Vec<&Annotation> = file
            .annotations
            .iter()
            .filter(|a| filter.shows(a, today))
            .collect();
        if shown.is_empty() {
            continue;
        }
        shown.sort_by(|a, b| newest_first(a, b));
        let rows: Vec<AnnotationRow> = shown
            .into_iter()
            .map(|a| {
                let place = RowPlace {
                    file,
                    parts: scan.parts.as_ref(),
                    expanded: state.expanded.as_ref(),
                    links: &links,
                };
                row_of(&place, a)
            })
            .collect();
        view.shown += rows.len();
        view.groups.push(FileGroup {
            label: file.label.clone(),
            rows,
        });
    }
    view
}

fn kind_index(kind: AnnotationKind) -> usize {
    AnnotationKind::ALL
        .iter()
        .position(|&k| k == kind)
        .unwrap_or_default()
}

/// What a row is built against: its file, the design's parts, the expanded
/// row and every citation of the scan.
struct RowPlace<'a> {
    file: &'a NotebookFile,
    parts: Option<&'a PartIndex>,
    expanded: Option<&'a RowKey>,
    links: &'a [signex_notebook::Link],
}

fn row_of(place: &RowPlace<'_>, annotation: &Annotation) -> AnnotationRow {
    let (file, parts, expanded) = (place.file, place.parts, place.expanded);
    let cited: Vec<_> = annotation.id.as_ref().map_or_else(Vec::new, |id| {
        place
            .links
            .iter()
            .filter(|link| &link.to == id)
            .map(|link| (link.citation, link.by.clone()))
            .collect()
    });
    let key = RowKey {
        file: file.path.clone(),
        line: annotation.line,
    };
    let chips = annotation
        .references
        .iter()
        .map(|reference| RefChip {
            reference: reference.clone(),
            label: reference.name(),
            gone: reference
                .component()
                .zip(parts)
                .is_some_and(|(part, parts)| !parts.contains_key(part)),
        })
        .collect();
    let jump = annotation
        .references
        .iter()
        .find(|r| r.component().is_some())
        .or_else(|| annotation.references.first())
        .cloned();
    AnnotationRow {
        expanded: expanded == Some(&key),
        key,
        kind: annotation.kind,
        id: annotation.id.as_ref().map(ToString::to_string),
        title: annotation.title.clone(),
        snippet: annotation.snippet.clone(),
        chips,
        status: annotation.status,
        verdict: annotation.record.verdict(),
        fields: record_fields(annotation, &cited),
        date: annotation.date,
        source: annotation.source.clone(),
        file_name: file
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        line: annotation.line,
        jump,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::handlers::editing_commands::net_color_driver::add_sheet_with;
    use crate::app::{CaseFold, LoadedProject};
    use crate::schematic_runtime::notebook_badges::BadgeMark;
    use signex_notebook::BadgeTone;
    use signex_types::project::{ProjectData, SheetEntry};
    use signex_types::schematic::{Point, SchematicSheet, Symbol};

    const DIR: &str = "/w/flags";
    const U3: u128 = 0x0001_6503;

    fn project(id: ProjectId) -> LoadedProject {
        LoadedProject {
            id,
            path: PathBuf::from(DIR).join("p.snxprj"),
            data: ProjectData {
                document_id: None,
                description: None,
                name: "p".into(),
                dir: DIR.into(),
                schematic_root: Some("a.snxsch".into()),
                pcb_file: None,
                sheets: vec![SheetEntry {
                    name: "a.snxsch".into(),
                    filename: "a.snxsch".into(),
                    symbols_count: 0,
                    wires_count: 0,
                    labels_count: 0,
                    unknown: Default::default(),
                }],
                variant_definitions: Vec::new(),
                active_variant: None,
                variant_settings: Default::default(),
                libraries: Vec::new(),
                enable_git: false,
                net_classes: Vec::new(),
                rules: None,
                annotation: None,
                schematic_defaults: Default::default(),
                project_defaults: Default::default(),
                agent_journal: Default::default(),
                schematic_design: Default::default(),
                fab_outputs: None,
                router_tool_diameter: None,
                requirements: Default::default(),
                parameters: Default::default(),
                unknown: Default::default(),
            },
            pending_libraries: Default::default(),
            case_fold: CaseFold::from_probe(false),
            case_probed: false,
            case_decided: false,
        }
    }

    /// The project's sheet `a.snxsch`, open and active, holding U3 at
    /// (50, 40); the panel's scan is of this project.
    fn app_with_u3() -> (Signex, PathBuf, ProjectId) {
        let (mut app, _boot) = Signex::new();
        let id = app.document_state.mint_project_id();
        app.document_state.projects.push(project(id));
        let path = PathBuf::from(DIR).join("a.snxsch");
        let sheet = SchematicSheet {
            uuid: uuid::Uuid::now_v7(),
            paper_size: "A4".into(),
            symbols: vec![Symbol {
                uuid: uuid::Uuid::from_u128(U3),
                lib_id: "Device:R".into(),
                reference: "U3".into(),
                position: Point::new(50.0, 40.0),
                ..Default::default()
            }],
            ..Default::default()
        };
        add_sheet_with(&mut app, &path, sheet);
        app.ui_state.annotations.target = Some((id, PathBuf::from(DIR)));
        (app, path, id)
    }

    fn settle(app: &mut Signex, msg: AnnotationsMsg) {
        let task = app.handle_annotations_message(msg);
        let mut driver = crate::task_driver::TaskDriver::new();
        let mut queue = std::collections::VecDeque::from([task]);
        let mut steps = 0;
        while let Some(task) = queue.pop_front() {
            for message in driver.outputs(task).expect("the task chain settled") {
                steps += 1;
                assert!(steps < 1_000, "the task chain settles");
                if !matches!(message, Message::DockLayout(_)) {
                    queue.push_back(app.update(message));
                }
            }
        }
    }

    fn notebook(text: &str, parts: Option<PartIndex>) -> NotebookScan {
        let (annotations, problems) = signex_notebook::scan(text);
        NotebookScan {
            folder_found: true,
            files: vec![NotebookFile {
                path: PathBuf::from(DIR).join("notebook/bringup.snxnbk"),
                label: "notebook/bringup.snxnbk".into(),
                annotations,
                problems,
                read_error: None,
            }],
            parts,
            ..NotebookScan::default()
        }
    }

    fn badges_of(list: &[(&str, BadgeTone, u32)]) -> Arc<NotebookBadges> {
        let mut badges = NotebookBadges::default();
        for &(part, tone, count) in list {
            badges
                .parts
                .insert(part.to_string(), BadgeMark { tone, count });
        }
        Arc::new(badges)
    }

    /// Mockup row 165: clicking U3 opens its sheet, zooms to it and selects it.
    #[test]
    fn a_part_chip_zooms_to_the_part_and_selects_it() {
        let (mut app, path, _id) = app_with_u3();
        let mut parts = PartIndex::new();
        parts.insert(
            "U3".into(),
            PartPlace {
                sheet: path.clone(),
                symbol: uuid::Uuid::from_u128(U3),
                placement: None,
            },
        );
        app.ui_state.annotations.scan = Some(Arc::new(notebook("", Some(parts))));
        app.interaction_state.canvas.pending_fit.set(None);
        settle(
            &mut app,
            AnnotationsMsg::ReferenceClicked(Reference::Component("U3".into())),
        );
        assert_eq!(
            app.document_state.active_path.as_deref(),
            Some(path.as_path())
        );
        assert_eq!(
            app.interaction_state.active_canvas().selected,
            vec![SelectedItem::new(
                uuid::Uuid::from_u128(U3),
                SelectedKind::Symbol
            )]
        );
        let fit = app
            .interaction_state
            .canvas
            .pending_fit
            .get()
            .expect("the view zooms to the part");
        assert_eq!(fit.center(), iced::Point::new(50.0, 40.0));
        assert!(
            fit.width <= 16.0 && fit.height <= 16.0,
            "a close zoom: {fit:?}"
        );
    }

    #[test]
    fn a_scan_puts_its_badges_on_the_sheets_and_repaints_them() {
        let (mut app, _path, _id) = app_with_u3();
        let generation = app.ui_state.annotations.generation;
        let before = app.interaction_state.canvas.overlay_cache_clears;
        let scan = notebook(
            "#annotation(kind: \"issue\", status: \"open\")[#component(\"U3\") runs warm]\n\
             #annotation(kind: \"decision\")[Use the LDO for #component(\"U3\")]\n",
            None,
        );
        settle(
            &mut app,
            AnnotationsMsg::Scanned {
                generation,
                result: Arc::new(Ok(scan)),
            },
        );
        assert_eq!(
            app.ui_state.annotations.badge_marks.parts.get("U3"),
            Some(&BadgeMark {
                tone: BadgeTone::OpenIssue,
                count: 2
            }),
            "one badge, coloured by the worst status (mockup row 197)"
        );
        assert_eq!(app.ui_state.annotations.badges.len(), 1);
        assert!(
            app.interaction_state.canvas.overlay_cache_clears > before,
            "the selection layer, where the badges are, redraws"
        );
    }

    #[test]
    fn the_badges_show_only_on_the_sheets_of_the_scanned_project() {
        let (mut app, path, id) = app_with_u3();
        app.ui_state.annotations.badge_marks = badges_of(&[("U3", BadgeTone::OpenIssue, 2)]);
        let shown = app.notebook_badge_view(Some(path.as_path()));
        assert!(shown.has_badges(), "the scanned project's sheet");
        assert!(
            app.notebook_badge_view(Some(Path::new("/elsewhere/b.snxsch")))
                .badges
                .is_none(),
            "a sheet of no project"
        );
        assert!(app.notebook_badge_view(None).badges.is_none());
        let other = app.document_state.mint_project_id();
        app.ui_state.annotations.target = Some((other, PathBuf::from("/w/other")));
        assert!(
            app.notebook_badge_view(Some(path.as_path()))
                .badges
                .is_none(),
            "a scan of another project ({other:?}, not {id:?})"
        );
    }

    /// View ▸ Notebook Badges (B) hides them; they are on by default.
    #[test]
    fn the_badges_hide_while_switched_off() {
        let (mut app, path, _id) = app_with_u3();
        app.ui_state.annotations.badge_marks = badges_of(&[("U3", BadgeTone::Notes, 1)]);
        assert!(!app.ui_state.annotations.badges_hidden, "on by default");
        app.ui_state.annotations.badges_hidden = true;
        assert!(
            app.notebook_badge_view(Some(path.as_path()))
                .badges
                .is_none()
        );
    }

    #[test]
    fn no_project_in_view_drops_the_badges() {
        let (mut app, _path, _id) = app_with_u3();
        app.ui_state.annotations.badge_marks = badges_of(&[("U3", BadgeTone::Notes, 1)]);
        let _ = app.start_annotations_scan(None);
        assert!(app.ui_state.annotations.badge_marks.is_empty());
        assert!(app.ui_state.annotations.badges.is_empty());
    }

    /// The badges are drawn by default (mockup row 197), so the project in
    /// view is read with the Annotations panel closed too.
    #[test]
    fn the_badges_show_with_the_annotations_panel_closed() {
        let (mut app, path, id) = app_with_u3();
        assert!(!app.is_panel_open(PanelKind::Annotations));
        app.document_state.active_path = Some(path.clone());
        app.ui_state.annotations.target = None;
        let before = app.ui_state.annotations.generation;
        let _ = app.sync_annotations_panel();
        let state = &app.ui_state.annotations;
        assert_eq!(state.generation, before + 1, "a read started");
        assert_eq!(state.target.as_ref().map(|t| t.0), Some(id));
        let generation = state.generation;
        settle(
            &mut app,
            AnnotationsMsg::Scanned {
                generation,
                result: Arc::new(Ok(notebook(
                    "#annotation(kind: \"issue\")[#component(\"U3\") runs warm]\n",
                    None,
                ))),
            },
        );
        assert!(!app.is_panel_open(PanelKind::Annotations));
        assert!(app.notebook_badge_view(Some(path.as_path())).has_badges());
    }

    #[test]
    fn a_closed_panel_reads_again_only_when_the_project_in_view_changes() {
        let (mut app, path, _id) = app_with_u3();
        assert!(!app.is_panel_open(PanelKind::Annotations));
        app.document_state.active_path = Some(path);
        app.ui_state.annotations.target = None;
        let _ = app.sync_annotations_panel();
        let read = app.ui_state.annotations.generation;
        let _ = app.sync_annotations_panel();
        assert_eq!(app.ui_state.annotations.generation, read, "nothing moved");
        let other = app.document_state.mint_project_id();
        app.ui_state.annotations.target = Some((other, PathBuf::from("/w/other")));
        let _ = app.sync_annotations_panel();
        assert_eq!(
            app.ui_state.annotations.generation,
            read + 1,
            "back on this project after another one's read"
        );
    }

    /// Close Project resolving its sheets starts no notebook read of the
    /// project it closes; once the close resolves, a project still open is
    /// read as before.
    #[test]
    fn a_project_being_closed_is_not_read() {
        let (mut app, path, id) = app_with_u3();
        assert!(!app.is_panel_open(PanelKind::Annotations));
        app.document_state.active_path = Some(path);
        app.ui_state.annotations.target = None;
        app.ui_state.annotations.closing = Some(id);
        let before = app.ui_state.annotations.generation;
        let _ = app.sync_annotations_panel();
        assert_eq!(app.ui_state.annotations.generation, before, "no read");
        app.ui_state.annotations.closing = None;
        let _ = app.sync_annotations_panel();
        assert_eq!(app.ui_state.annotations.generation, before + 1);
    }

    /// Mockup row 195: one notebook includes chapter files, so a chapter in
    /// a subfolder of `notebook/` is read too, labelled by its path there;
    /// other files are not.
    #[test]
    fn the_chapters_in_subfolders_of_the_notebook_are_read() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join(NOTEBOOK_DIR);
        std::fs::create_dir_all(folder.join("chapters/deep")).unwrap();
        std::fs::write(folder.join("design.snxnbk"), "#note(id: \"NOTE-001\")[A]\n").unwrap();
        let chapter = "#issue(id: \"ISS-001\")[B]\n";
        std::fs::write(folder.join("chapters/power.snxnbk"), chapter).unwrap();
        std::fs::write(folder.join("chapters/deep/x.snxnbk"), "Plain.\n").unwrap();
        std::fs::write(folder.join("chapters/scope.csv"), "t,v\n").unwrap();
        let scan = read_notebook_files(dir.path()).expect("read");
        assert!(scan.folder_found);
        let labels: Vec<&str> = scan.files.iter().map(|f| f.label.as_str()).collect();
        assert_eq!(
            labels,
            [
                "notebook/chapters/deep/x.snxnbk",
                "notebook/chapters/power.snxnbk",
                "notebook/design.snxnbk",
            ]
        );
        let id = scan.files[1].annotations[0].id.as_ref();
        assert_eq!(id.map(ToString::to_string).as_deref(), Some("ISS-001"));
        assert!(scan.unlisted.is_empty());
        assert!(scan.parts.is_none(), "no parts are looked up");
    }

    /// #3021: the generated design report copies the decisions; its blocks
    /// are not read, so `list_annotations` and the badges count each once.
    #[test]
    fn the_generated_reports_copies_are_not_read_again() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join(NOTEBOOK_DIR);
        std::fs::create_dir_all(folder.join("generated")).unwrap();
        let decision = "#decision(id: \"DEC-001\", target: component(\"R7\"))[A]\n";
        std::fs::write(folder.join("design.snxnbk"), decision).unwrap();
        std::fs::write(folder.join("generated/design-report.snxnbk"), decision).unwrap();
        let scan = read_notebook_files(dir.path()).expect("read");
        let labels: Vec<&str> = scan.files.iter().map(|f| f.label.as_str()).collect();
        assert_eq!(labels, ["notebook/design.snxnbk"]);
    }

    #[test]
    fn a_project_without_a_notebook_folder_reads_as_none() {
        let dir = tempfile::tempdir().unwrap();
        let scan = read_notebook_files(dir.path()).expect("read");
        assert!(!scan.folder_found);
        assert!(scan.files.is_empty());
    }
}
