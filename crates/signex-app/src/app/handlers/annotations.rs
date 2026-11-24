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
    RefChip, Reference, RowKey, ScanProblem, part_flags,
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
            state.flags = Arc::default();
            self.refresh_annotations_view();
            self.repaint_annotation_flags();
            return Task::none();
        };
        if !same_project || state.scan.is_none() {
            state.scan = None;
            state.expanded = None;
            state.status = ListStatus::Reading;
            // Another project's flags never show on this one's sheets.
            state.flags = Arc::default();
            self.repaint_annotation_flags();
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
                state.flags = Arc::new(part_flags(&scan));
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
                state.flags = Arc::default();
            }
        }
        self.repaint_annotation_flags();
    }

    /// The annotation flags the sheet at `path` shows: the last scan's, when
    /// the sheet is of the project that scan read; none elsewhere.
    pub(crate) fn annotation_flag_view(
        &self,
        path: Option<&Path>,
    ) -> crate::schematic_runtime::annotation_flags::AnnotationFlagView<'_> {
        let state = &self.ui_state.annotations;
        let project = path
            .and_then(|path| self.document_state.document_project(path))
            .map(|project| project.id);
        let scanned = state.target.as_ref().map(|target| target.0);
        crate::schematic_runtime::annotation_flags::AnnotationFlagView {
            parts: (project.is_some() && project == scanned).then_some(&*state.flags),
            ink: crate::schematic_runtime::annotation_flags::AnnotationFlagInk::from_tokens(
                &self.tokens_for(self.ui_state.preferences_draft_theme),
            ),
        }
    }

    /// Redraw the selection layer of every sheet, where the flags are drawn.
    fn repaint_annotation_flags(&mut self) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::handlers::editing_commands::net_color_driver::add_sheet_with;
    use crate::app::{CaseFold, LoadedProject};
    use crate::schematic_runtime::annotation_flags::{FlagMark, PartFlags};
    use signex_types::project::{ProjectData, SheetEntry};
    use signex_types::schematic::{Point, SchematicSheet, Symbol};
    use signex_widgets::theme_ext::AnnotationTone;

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
            unlisted: Vec::new(),
            parts,
        }
    }

    fn flags_of(list: &[(&str, AnnotationTone, u32)]) -> Arc<PartFlags> {
        let mut parts = PartFlags::new();
        for &(part, tone, count) in list {
            parts
                .entry(part.to_string())
                .or_default()
                .push(FlagMark { tone, count });
        }
        Arc::new(parts)
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
    fn a_scan_puts_its_flags_on_the_sheets_and_repaints_them() {
        let (mut app, _path, _id) = app_with_u3();
        let generation = app.ui_state.annotations.generation;
        let before = app.interaction_state.canvas.overlay_cache_clears;
        let scan = notebook(
            "#annotation(kind: \"issue\")[#component(\"U3\") runs warm]\n\
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
            app.ui_state.annotations.flags.get("U3"),
            Some(&vec![
                FlagMark {
                    tone: AnnotationTone::Issue,
                    count: 1
                },
                FlagMark {
                    tone: AnnotationTone::Decision,
                    count: 1
                },
            ])
        );
        assert!(
            app.interaction_state.canvas.overlay_cache_clears > before,
            "the selection layer, where the flags are, redraws"
        );
    }

    #[test]
    fn the_flags_show_only_on_the_sheets_of_the_scanned_project() {
        let (mut app, path, id) = app_with_u3();
        app.ui_state.annotations.flags = flags_of(&[("U3", AnnotationTone::Issue, 2)]);
        let shown = app.annotation_flag_view(Some(path.as_path()));
        assert!(shown.has_flags(), "the scanned project's sheet");
        assert!(
            app.annotation_flag_view(Some(Path::new("/elsewhere/b.snxsch")))
                .parts
                .is_none(),
            "a sheet of no project"
        );
        assert!(app.annotation_flag_view(None).parts.is_none());
        let other = app.document_state.mint_project_id();
        app.ui_state.annotations.target = Some((other, PathBuf::from("/w/other")));
        assert!(
            app.annotation_flag_view(Some(path.as_path()))
                .parts
                .is_none(),
            "a scan of another project ({other:?}, not {id:?})"
        );
    }

    #[test]
    fn no_project_in_view_drops_the_flags() {
        let (mut app, _path, _id) = app_with_u3();
        app.ui_state.annotations.flags = flags_of(&[("U3", AnnotationTone::Note, 1)]);
        let _ = app.start_annotations_scan(None);
        assert!(app.ui_state.annotations.flags.is_empty());
    }
}
