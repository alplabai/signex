//! Annotations panel (mockup row 165): every annotation block of the project's Design
//! Notebook files (`notebook/**/*.snxnbk`, chapters included), grouped by
//! file, newest first, filtered by kind, part, status, date and text. A row
//! shows the block's typed id, and expanded its review record (mockup row
//! 198, `fields`). A reference chip goes to its part or net. A pure view
//! over [`AnnotationsView`], which `app/handlers/annotations.rs` builds from
//! the last scan and the filters; the grammar the scan reads is in
//! `docs/NOTEBOOK_ANNOTATIONS.md`.

mod fields;
mod filters;
mod rows;

pub(crate) use fields::record_fields;
pub use fields::{FieldLabel, RowField, RowValue};

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use iced::widget::{Column, Space, column, container, row, scrollable};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, Localized, MessageId, ids, t};
use signex_types::schematic::InstancePath;
use signex_widgets::text;
use signex_widgets::theme_ext::AnnotationTone;

pub use signex_notebook::{
    Annotation, AnnotationKind, DateWindow, Filter, IsoDate, Reference, ScanProblem, Status,
    Verdict,
};

use super::{PanelContext, PanelMsg};
use crate::schematic_runtime::annotation_flags::{FlagMark, PartFlags};
use crate::styles::{ChromeMetrics, Radii, Spacing, TypeScale, ti, tokens::resolve};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;
const METRICS: ChromeMetrics = ChromeMetrics::DEFAULT;
/// The mock-up's 8 px padding and 4 px / 6 px gaps at the 12 px body.
const PAD: f32 = TS.body * 8.0 / 12.0;
const PAD_TIGHT: f32 = TS.body * 4.0 / 12.0;
const TOOL_GAP: f32 = TS.body * 6.0 / 12.0;
/// The empty-state card's padding.
const HINT_PAD: f32 = TS.body * 14.0 / 12.0;

/// One row of the list: the file it is in and the line its block starts on.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RowKey {
    pub file: PathBuf,
    pub line: u32,
}

/// What happened in the Annotations panel, or to its scan.
#[derive(Debug, Clone)]
pub enum AnnotationsMsg {
    /// A scan of the notebook files finished; `generation` names the request.
    Scanned {
        generation: u64,
        result: Arc<Result<NotebookScan, String>>,
    },
    QueryChanged(String),
    /// A kind chip was pressed: its kind is shown or left out.
    KindToggled(AnnotationKind),
    PartPicked(String),
    PartCleared,
    StatusPicked(Status),
    StatusCleared,
    /// A date window was picked; [`DateWindow::Any`] clears it.
    DatePicked(DateWindow),
    /// A row was clicked: it expands, or folds when it was expanded.
    RowToggled(RowKey),
    /// A reference chip or a Jump button was pressed.
    ReferenceClicked(Reference),
    /// The title bar's Refresh: read the notebook files again.
    RefreshRequested,
}

/// What a scan read: each notebook file of the project and where each part
/// of the design is.
#[derive(Debug, Clone, Default)]
pub struct NotebookScan {
    /// `false` when the project has no `notebook/` folder.
    pub folder_found: bool,
    /// Every `.snxnbk` file of the folder, by name.
    pub files: Vec<NotebookFile>,
    /// Entries of the folder that could not be listed: the system's reason
    /// for each, for Messages.
    pub unlisted: Vec<String>,
    /// Every designator of the design and where its part is; `None` when
    /// the design could not be read whole (no part is then called gone).
    pub parts: Option<PartIndex>,
}

/// One notebook file and what was read from it.
#[derive(Debug, Clone)]
pub struct NotebookFile {
    pub path: PathBuf,
    /// The path the panel shows: `notebook/<name>.snxnbk`.
    pub label: String,
    pub annotations: Vec<Annotation>,
    pub problems: Vec<ScanProblem>,
    /// Why the file could not be read; it then has no annotations.
    pub read_error: Option<FileReadError>,
}

/// Why a notebook file could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileReadError {
    /// The system's reason.
    Io(String),
    /// The file is not UTF-8 text.
    NotUtf8,
}

/// Designator to the place of its part.
pub type PartIndex = HashMap<String, PartPlace>;

/// Where a part is: its sheet, its symbol and the sheet placement the
/// designator belongs to (`None`: the symbol's own designator).
#[derive(Debug, Clone, PartialEq)]
pub struct PartPlace {
    pub sheet: PathBuf,
    pub symbol: uuid::Uuid,
    pub placement: Option<InstancePath>,
}

/// Where the panel's list stands.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ListStatus {
    /// No project in view.
    #[default]
    NoProject,
    /// The first scan of this project is running.
    Reading,
    /// The scan failed (the reason is in Messages).
    Unavailable,
    /// The project has no notebook folder or no notebook file.
    NoNotebook,
    Ready,
}

/// The panel's model, kept in `UiState`.
#[derive(Debug, Clone, Default)]
pub struct AnnotationsState {
    pub filter: Filter,
    /// The row shown expanded with its source.
    pub expanded: Option<RowKey>,
    /// The last scan of the project in view.
    pub scan: Option<Arc<NotebookScan>>,
    pub status: ListStatus,
    /// The newest scan request; an older answer is dropped.
    pub generation: u64,
    /// The project (and its folder) the scan is for.
    pub target: Option<(crate::app::ProjectId, PathBuf)>,
    /// The panel was open at the last look, so a later open is a new show.
    pub shown: bool,
    /// The flags the last scan puts on the parts of the design (mockup row 165).
    pub flags: Arc<PartFlags>,
    /// The project whose Close Project is resolving its sheets: a change of
    /// the project in view does not read its notebook meanwhile.
    pub closing: Option<crate::app::ProjectId>,
}

/// The flags of every part the scan's annotations name: per designator, one
/// mark per kind (in kind order) with how many blocks of that kind name the
/// part. A block naming a part twice (`#component("U1")` and
/// `#pin("U1", "3")`) counts once.
pub(crate) fn part_flags(scan: &NotebookScan) -> PartFlags {
    let mut counts: HashMap<&str, [u32; AnnotationKind::ALL.len()]> = HashMap::new();
    for annotation in scan.files.iter().flat_map(|f| &f.annotations) {
        let Some(slot) = AnnotationKind::ALL
            .iter()
            .position(|&k| k == annotation.kind)
        else {
            continue;
        };
        let mut named: Vec<&str> = annotation
            .references
            .iter()
            .filter_map(Reference::component)
            .collect();
        named.sort_unstable();
        named.dedup();
        for part in named {
            counts.entry(part).or_default()[slot] += 1;
        }
    }
    counts
        .into_iter()
        .map(|(part, by_kind)| {
            let marks = AnnotationKind::ALL
                .iter()
                .zip(by_kind)
                .filter(|(_, count)| *count > 0)
                .map(|(&kind, count)| FlagMark {
                    tone: tone(kind),
                    count,
                })
                .collect();
            (part.to_string(), marks)
        })
        .collect()
}

/// A kind chip: its count over every annotation and whether it is shown.
#[derive(Debug, Clone, PartialEq)]
pub struct KindChip {
    pub kind: AnnotationKind,
    pub count: u32,
    pub on: bool,
}

/// A reference as a chip.
#[derive(Debug, Clone, PartialEq)]
pub struct RefChip {
    pub reference: Reference,
    pub label: String,
    /// A part the design no longer has (amber).
    pub gone: bool,
}

/// One annotation as listed.
#[derive(Debug, Clone, PartialEq)]
pub struct AnnotationRow {
    pub key: RowKey,
    pub kind: AnnotationKind,
    /// The typed id (`DEC-002`), shown in mono before the title.
    pub id: Option<String>,
    pub title: String,
    pub snippet: String,
    pub chips: Vec<RefChip>,
    pub status: Option<Status>,
    /// Pass, Warn or Fail of a Calculation or Measurement.
    pub verdict: Option<Verdict>,
    /// The review record an expanded row lists above its source.
    pub fields: Vec<RowField>,
    pub date: Option<IsoDate>,
    pub expanded: bool,
    /// The block as written; shown when expanded.
    pub source: String,
    /// The file's own name, for the expanded caption.
    pub file_name: String,
    pub line: u32,
    /// Where Jump goes: the first part named, else the first net.
    pub jump: Option<Reference>,
}

/// The rows of one notebook file the filters keep.
#[derive(Debug, Clone, PartialEq)]
pub struct FileGroup {
    pub label: String,
    pub rows: Vec<AnnotationRow>,
}

/// What the panel shows.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AnnotationsView {
    pub status: ListStatus,
    pub query: String,
    pub kinds: Vec<KindChip>,
    pub part: Option<String>,
    /// The parts the annotations name, sorted.
    pub parts: Vec<String>,
    pub status_filter: Option<Status>,
    /// The statuses the annotations carry, in their order.
    pub statuses: Vec<Status>,
    pub date: DateWindow,
    pub groups: Vec<FileGroup>,
    /// Rows shown, of every annotation of every file.
    pub shown: usize,
    pub total: usize,
    /// Notebook files read.
    pub files: usize,
}

pub(super) fn msg(m: AnnotationsMsg) -> PanelMsg {
    PanelMsg::Annotations(m)
}

/// The colour key of a kind.
pub(crate) fn tone(kind: AnnotationKind) -> AnnotationTone {
    match kind {
        AnnotationKind::Note => AnnotationTone::Note,
        AnnotationKind::Calculation => AnnotationTone::Calculation,
        AnnotationKind::Measurement => AnnotationTone::Measurement,
        AnnotationKind::Issue => AnnotationTone::Issue,
        AnnotationKind::Decision => AnnotationTone::Decision,
    }
}

pub(crate) fn kind_label(i18n: &I18n, kind: AnnotationKind) -> Localized {
    match kind {
        AnnotationKind::Note => t!(i18n, ids::ANNOTATIONS_KIND_NOTE),
        AnnotationKind::Calculation => t!(i18n, ids::ANNOTATIONS_KIND_CALCULATION),
        AnnotationKind::Measurement => t!(i18n, ids::ANNOTATIONS_KIND_MEASUREMENT),
        AnnotationKind::Issue => t!(i18n, ids::ANNOTATIONS_KIND_ISSUE),
        AnnotationKind::Decision => t!(i18n, ids::ANNOTATIONS_KIND_DECISION),
    }
}

pub(crate) fn status_label(i18n: &I18n, status: Status) -> Localized {
    match status {
        Status::Open => t!(i18n, ids::ANNOTATIONS_STATUS_OPEN),
        Status::Resolved => t!(i18n, ids::ANNOTATIONS_STATUS_RESOLVED),
        Status::Proposed => t!(i18n, ids::ANNOTATIONS_STATUS_PROPOSED),
        Status::Accepted => t!(i18n, ids::ANNOTATIONS_STATUS_ACCEPTED),
        Status::Superseded => t!(i18n, ids::ANNOTATIONS_STATUS_SUPERSEDED),
        Status::Planned => t!(i18n, ids::ANNOTATIONS_STATUS_PLANNED),
        Status::Done => t!(i18n, ids::ANNOTATIONS_STATUS_DONE),
        Status::Current => t!(i18n, ids::ANNOTATIONS_STATUS_CURRENT),
        Status::Stale => t!(i18n, ids::ANNOTATIONS_STATUS_STALE),
    }
}

/// Render the panel.
pub fn view<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let view = &*ctx.annotations;
    match view.status {
        ListStatus::NoProject => note(ctx, ids::ANNOTATIONS_NO_PROJECT),
        ListStatus::Reading => note(ctx, ids::ANNOTATIONS_READING),
        ListStatus::Unavailable => note(ctx, ids::ANNOTATIONS_UNAVAILABLE),
        ListStatus::NoNotebook => empty_card(ctx),
        ListStatus::Ready => column![
            filters::tools(ctx, view),
            body(ctx, view),
            rule(ti(ctx.tokens.border)),
            footer(ctx, view),
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed(),
    }
}

/// The groups in a scrollable list, or why there is none.
fn body<'a>(ctx: &'a PanelContext, view: &'a AnnotationsView) -> Element<'a, PanelMsg> {
    if view.groups.is_empty() {
        let id = if view.total == 0 {
            ids::ANNOTATIONS_NONE_YET
        } else {
            ids::ANNOTATIONS_NO_MATCH
        };
        return container(hint_card(ctx, None, t!(ctx.i18n, id)))
            .width(Length::Fill)
            .height(Length::Fill)
            .boxed();
    }
    let mut list: Column<Element<'a, PanelMsg>> = Column::new().width(Length::Fill);
    for group in &view.groups {
        list = list.push(rows::file_header(ctx, group));
        for item in &group.rows {
            list = list.push(rows::annotation_row(ctx, item));
        }
    }
    scrollable(list)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(super::form_look::scrollbar_style)
        .boxed()
}

/// "N of M annotations" and "K files" on the chrome fill.
fn footer<'a>(ctx: &'a PanelContext, view: &AnnotationsView) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let ink = resolve(&ctx.tokens).ink4;
    let fill = ti(ctx.tokens.statusbar_bg);
    let shown = t!(
        i18n, ids::ANNOTATIONS_FOOTER_SHOWN,
        "shown" => view.shown, "total" => view.total
    );
    let files = t!(i18n, ids::ANNOTATIONS_FOOTER_FILES, "count" => view.files);
    container(
        row![
            text(shown).size(TS.caption).color(ink),
            Space::new().width(Length::Fill).height(Length::Shrink),
            text(files).size(TS.caption).color(ink),
        ]
        .spacing(PAD)
        .align_y(Alignment::Center),
    )
    .height(METRICS.status)
    .padding([0.0, PAD])
    .center_y(METRICS.status)
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(fill)),
        ..container::Style::default()
    })
    .boxed()
}

/// A line of muted text where the list would be.
fn note<'a>(ctx: &'a PanelContext, id: MessageId) -> Element<'a, PanelMsg> {
    container(
        text(t!(ctx.i18n, id))
            .size(TS.caption)
            .color(resolve(&ctx.tokens).ink3),
    )
    .padding([SP.m, SP.l])
    .width(Length::Fill)
    .boxed()
}

/// No notebook yet: the card sits at the top of the panel.
fn empty_card<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    container(hint_card(
        ctx,
        Some(t!(i18n, ids::ANNOTATIONS_EMPTY_TITLE)),
        t!(i18n, ids::ANNOTATIONS_EMPTY_BODY),
    ))
    .width(Length::Fill)
    .boxed()
}

/// The mock-up's hint card: an optional title, centred muted text, a
/// rounded line around it.
fn hint_card<'a>(
    ctx: &'a PanelContext,
    title: Option<Localized>,
    body: Localized,
) -> Element<'a, PanelMsg> {
    let r = resolve(&ctx.tokens);
    let mut lines = Column::new()
        .spacing(PAD_TIGHT)
        .align_x(Alignment::Center)
        .width(Length::Fill);
    if let Some(title) = title {
        lines = lines.push(
            text(title)
                .size(TS.body)
                .font(rows::medium())
                .color(ti(ctx.tokens.text))
                .width(Length::Fill)
                .center(),
        );
    }
    lines = lines.push(
        text(body)
            .size(TS.body)
            .color(r.ink3)
            .width(Length::Fill)
            .center(),
    );
    let line = r.line_strong;
    container(
        container(lines)
            .padding(HINT_PAD)
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                border: Border {
                    width: 1.0,
                    color: line,
                    radius: Radii::GRAPHITE.card.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            }),
    )
    .padding(SP.m)
    .width(Length::Fill)
    .boxed()
}

/// A one-pixel rule across the panel.
pub(super) fn rule<'a>(color: iced::Color) -> Element<'a, PanelMsg> {
    container(Space::new())
        .width(Length::Fill)
        .height(1)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(color)),
            ..container::Style::default()
        })
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout_probe::{find, lay_out_texts, messages_on_click_at, root_size};

    const WINDOW: iced::Size = iced::Size::new(320.0, 240.0);

    fn scan_of(text: &str) -> NotebookScan {
        let (annotations, problems) = signex_notebook::scan(text);
        assert!(problems.is_empty(), "{problems:?}");
        NotebookScan {
            folder_found: true,
            files: vec![NotebookFile {
                path: PathBuf::from("notebook/bringup.snxnbk"),
                label: "notebook/bringup.snxnbk".into(),
                annotations,
                problems,
                read_error: None,
            }],
            unlisted: Vec::new(),
            parts: None,
        }
    }

    #[test]
    fn a_part_gets_one_mark_per_kind_counting_the_blocks_that_name_it() {
        let scan = scan_of(
            "#annotation(kind: \"issue\")[#component(\"U3\") runs warm.]\n\
             #annotation(kind: \"decision\")[Use the LDO for #component(\"U3\").]\n\
             #annotation(kind: \"issue\")[#component(\"C6\") is missing near #component(\"U3\").]\n\
             #annotation(kind: \"note\")[#component(\"U1\") pin #pin(\"U1\", \"3\") on #net(\"VBUS\").]\n",
        );
        let flags = part_flags(&scan);
        let mark = |tone, count| FlagMark { tone, count };
        assert_eq!(
            flags.get("U3"),
            Some(&vec![
                mark(AnnotationTone::Issue, 2),
                mark(AnnotationTone::Decision, 1)
            ]),
            "kinds in kind order, Issue before Decision"
        );
        assert_eq!(flags.get("C6"), Some(&vec![mark(AnnotationTone::Issue, 1)]));
        assert_eq!(
            flags.get("U1"),
            Some(&vec![mark(AnnotationTone::Note, 1)]),
            "a part and its pin in one block count once"
        );
        assert!(!flags.contains_key("VBUS"), "a net carries no flag");
        assert_eq!(flags.len(), 3);
    }

    #[test]
    fn no_annotations_no_flags() {
        assert!(part_flags(&NotebookScan::default()).is_empty());
    }

    fn ready_context() -> PanelContext {
        let (app, _boot) = crate::app::Signex::new();
        let mut ctx = app.document_state.panel_ctx.clone();
        ctx.annotations = Arc::new(AnnotationsView {
            status: ListStatus::Ready,
            date: DateWindow::Last7Days,
            ..AnnotationsView::default()
        });
        ctx
    }

    /// Decision 2026-11-24: New annotation (+) sits at the filter bar's right
    /// end and is greyed without an open notebook, which this build never has.
    #[test]
    fn new_annotation_sits_greyed_at_the_right_end_of_the_filter_bar() {
        let ctx = ready_context();
        let size = root_size(filters::tools(&ctx, &ctx.annotations), WINDOW);
        let texts = lay_out_texts(filters::tools(&ctx, &ctx.annotations), WINDOW);
        let date = find(&texts, "Last 7 days").bounds;
        let plus = iced::Point::new(size.width - PAD - METRICS.row / 2.0, date.center_y());
        assert!(
            date.x + date.width < plus.x - METRICS.row / 2.0,
            "the date field ends before the + ({date:?}, {plus:?})"
        );
        let pressed =
            messages_on_click_at(filters::tools(&ctx, &ctx.annotations), WINDOW, None, plus);
        assert!(pressed.is_empty(), "the + is greyed: {pressed:?}");
    }

    #[test]
    fn the_greyed_new_annotation_says_why() {
        let ctx = ready_context();
        let tip = t!(ctx.i18n, ids::ANNOTATIONS_NEW_NEEDS_NOTEBOOK).into_string();
        assert_eq!(tip, "New annotation · open a notebook first");
    }
}
