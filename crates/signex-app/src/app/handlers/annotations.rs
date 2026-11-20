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
use crate::app::project_sheets::{self, OpenSheets, ProjectSheetSet};
use crate::app::sheet_hierarchy::build_sheet_hierarchy;
use crate::app::{CaseFold, Message, ProjectId, Signex};
use crate::dock::DockMessage;
use crate::panels::annotations::{
    Annotation, AnnotationKind, AnnotationRow, AnnotationsState, AnnotationsView, FileGroup,
    FileReadError, IsoDate, KindChip, ListStatus, NotebookFile, NotebookScan, PartIndex, PartPlace,
    RefChip, Reference, RowKey, ScanProblem,
};
use crate::panels::{AnnotationsMsg, NetInspectorMsg, PanelKind, PanelMsg};

/// The log target of the panel's Messages lines (Source `SIGNEX-NOTEBOOK`).
const TARGET: &str = "signex::notebook";
const SECONDS_PER_DAY: u64 = 86_400;

/// What a scan reads, captured in `update`.
struct ScanInputs {
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
    /// Keep the panel's scan current: a scan starts when the panel is shown
    /// and when the project in view changes (a project opened). Runs after
    /// every message; does nothing while the panel is closed.
    pub(crate) fn sync_annotations_panel(&mut self) -> Task<Message> {
        if !self.is_panel_open(PanelKind::Annotations) {
            self.ui_state.annotations.shown = false;
            return Task::none();
        }
        let newly_shown = !std::mem::replace(&mut self.ui_state.annotations.shown, true);
        // Compared without building the target: this runs on every message.
        let held = self.ui_state.annotations.target.as_ref();
        let active = self.document_state.active_document_project();
        let moved = held.map(|(id, dir)| (*id, dir.as_path())) != active.map(|p| (p.id, p.dir()));
        if newly_shown || moved {
            let target = self.annotations_target();
            return self.start_annotations_scan(target);
        }
        Task::none()
    }

    /// The project the panel lists: the active document's.
    fn annotations_target(&self) -> Option<(ProjectId, PathBuf)> {
        self.document_state
            .active_document_project()
            .map(|project| (project.id, project.dir().to_path_buf()))
    }

    /// Read the notebook files of `target` on a worker. The last list stays
    /// shown meanwhile unless the project changed.
    fn start_annotations_scan(&mut self, target: Option<(ProjectId, PathBuf)>) -> Task<Message> {
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
            self.refresh_annotations_view();
            return Task::none();
        };
        if !same_project || state.scan.is_none() {
            state.scan = None;
            state.expanded = None;
            state.status = ListStatus::Reading;
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

    fn capture_annotation_inputs(&self, id: ProjectId, dir: &Path) -> ScanInputs {
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
            }
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
    fn jump_to_reference(&mut self, reference: &Reference) -> Task<Message> {
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
fn scan_notebooks(inputs: &ScanInputs) -> Result<NotebookScan, String> {
    let folder = inputs.project_dir.join(NOTEBOOK_DIR);
    let entries = match std::fs::read_dir(&folder) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(NotebookScan::default());
        }
        Err(e) => return Err(e.to_string()),
    };
    // An entry that cannot be listed is said in Messages, never dropped.
    let (mut paths, mut unlisted) = (Vec::new(), Vec::new());
    for entry in entries {
        match entry {
            Ok(entry) => {
                let path = entry.path();
                let notebook = path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case(NOTEBOOK_EXTENSION));
                if notebook && path.is_file() {
                    paths.push(path);
                }
            }
            Err(e) => unlisted.push(e.to_string()),
        }
    }
    paths.sort();
    let files = paths.into_iter().map(read_notebook).collect();
    Ok(NotebookScan {
        folder_found: true,
        files,
        unlisted,
        parts: inputs.design.as_ref().and_then(part_index),
    })
}

fn read_notebook(path: PathBuf) -> NotebookFile {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let label = format!("{NOTEBOOK_DIR}/{name}");
    let text = std::fs::read(&path)
        .map_err(|e| FileReadError::Io(e.to_string()))
        .and_then(|bytes| String::from_utf8(bytes).map_err(|_| FileReadError::NotUtf8));
    let (annotations, problems, read_error) = match text {
        Ok(text) => {
            let (annotations, problems) = signex_notebook::scan(&text);
            (annotations, problems, None)
        }
        Err(why) => (Vec::new(), Vec::new(), Some(why)),
    };
    NotebookFile {
        path,
        label,
        annotations,
        problems,
        read_error,
    }
}

/// Every designator of the design and where its part is. `None` when a sheet
/// could not be read or is missing: a part there would wrongly read as gone.
fn part_index(design: &DesignInputs) -> Option<PartIndex> {
    let set: ProjectSheetSet = project_sheets::assemble_project_sheets_quiet_from(
        &design.open,
        &design.pages,
        &design.root,
        design.fold,
    );
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
            .map(|a| row_of(file, a, scan.parts.as_ref(), state.expanded.as_ref()))
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

fn row_of(
    file: &NotebookFile,
    annotation: &Annotation,
    parts: Option<&PartIndex>,
    expanded: Option<&RowKey>,
) -> AnnotationRow {
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
        title: annotation.title.clone(),
        snippet: annotation.snippet.clone(),
        chips,
        status: annotation.status,
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
