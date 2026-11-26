//! Compiles as you type (mockup row 195): each edit asks for a compile; one
//! runs at a time off the UI thread and an edit made meanwhile asks for one
//! more when it lands, so the preview never falls behind by more than one
//! compile. An error keeps the last good compile on screen under its
//! banner instead of a blank page. The same job reads what the margin lane
//! shows (mockup row 199): the measurement counts, the stale attachments
//! and the open timeline, against the part values of the open sheets.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use iced::Task;
use iced::widget::image;
use signex_notebook::attachment::DesignValues;
use signex_notebook::measurement::Timeline;
use signex_notebook::report::{AttachmentState, Summary, attachments_report, timeline_report};
use signex_notebook::{
    CompileOptions, CompiledNotebook, IsoDate, NotebookSources, NotebookStatus, PageColours,
    PageImage, Reference, compile,
};

use super::{NotebookMsg, NotebookTab};
use crate::app::state::ProjectId;
use crate::app::{Message, Signex};

/// Pixels per typographic point of the preview page: sharp at the zoom a
/// page fits the pane in; the image widget scales it to the pane.
pub(super) const PREVIEW_PIXELS_PER_PT: f64 = 2.0;
/// Pixels per point of a page thumbnail in the Pages panel.
pub(super) const THUMBNAIL_PIXELS_PER_PT: f64 = 0.25;

/// What the margin lane shows of one compile (mockup row 199).
#[derive(Debug, Default)]
pub struct Lane {
    /// The status strip's counts.
    pub summary: Summary,
    /// Every attachment with its stale mark.
    pub attachments: Vec<AttachmentState>,
    /// The timeline of the target the lane shows, if one is open.
    pub timeline: Option<Timeline>,
}

/// What one compile gave.
#[derive(Debug)]
pub struct CompileOutcome {
    /// The text generation it read.
    pub generation: u64,
    pub status: NotebookStatus,
    pub notebook: Option<Arc<CompiledNotebook>>,
    pub elapsed_ms: u64,
    /// The wall clock when it finished, `14:02:18`.
    pub finished_at: String,
    /// The preview page, drawn in the theme's page colours.
    pub page: Option<(usize, Arc<PageImage>)>,
    /// Every page small, for the Pages panel; empty when it was not shown.
    pub thumbnails: Vec<Arc<PageImage>>,
    pub lane: Lane,
}

/// A page ready for the image widget.
#[derive(Debug, Clone)]
pub struct ShownPage {
    pub page: usize,
    pub handle: image::Handle,
}

/// The compiles of one tab.
#[derive(Debug, Default)]
pub struct CompileState {
    /// The text generation a running compile read.
    pub running: Option<u64>,
    /// An edit landed while a compile ran: compile again when it lands.
    pub owed: bool,
    /// The newest compile, which may have failed.
    pub last: Option<Arc<CompileOutcome>>,
    /// The newest compile that gave pages.
    pub last_good: Option<Arc<CompileOutcome>>,
    /// The preview page of the last good compile.
    pub shown: Option<ShownPage>,
    /// The thumbnails of the last good compile.
    pub thumbnails: Vec<image::Handle>,
}

impl CompileState {
    /// True when the newest compile failed.
    pub fn failed(&self) -> bool {
        self.last
            .as_ref()
            .is_some_and(|outcome| !outcome.status.compiled)
    }

    /// What the margin lane shows: of the newest compile.
    pub fn lane(&self) -> Option<&Lane> {
        self.last.as_ref().map(|outcome| &outcome.lane)
    }
}

/// What a compile job reads, gathered in `update`.
struct CompileJob {
    generation: u64,
    sources: NotebookSources,
    /// The notebook folder, for the attachments the pages print.
    folder: PathBuf,
    values: DesignValues,
    timeline: Option<Reference>,
    page: usize,
    colours: PageColours,
    thumbnails: bool,
}

impl CompileJob {
    /// Compile, draw the preview page and, when asked, the thumbnails; read
    /// the margin lane's counts, attachments and timeline.
    fn run(mut self) -> CompileOutcome {
        let started = Instant::now();
        for path in self.sources.missing_attachments() {
            if let Ok(bytes) = std::fs::read(self.folder.join(&path)) {
                let _held = self.sources.insert(&path, bytes);
            }
        }
        self.sources.set_design_values(self.values.clone());
        let today = today();
        // No cell results: every Python cell shows as not run (mockup row
        // 200); cells run on export.
        let options = CompileOptions {
            today,
            ..CompileOptions::default()
        };
        let compilation = compile(&self.sources, &options);
        let status = NotebookStatus::of(&self.sources, &compilation);
        let notebook = compilation.notebook.map(Arc::new);
        let page = notebook.as_ref().and_then(|notebook| {
            let index = self.page.min(notebook.page_count().saturating_sub(1));
            let drawn = notebook.render_page(index, PREVIEW_PIXELS_PER_PT)?;
            Some((index, Arc::new(self.colours.apply(&drawn))))
        });
        let thumbnails = match (&notebook, self.thumbnails) {
            (Some(notebook), true) => (0..notebook.page_count())
                .filter_map(|index| notebook.render_page(index, THUMBNAIL_PIXELS_PER_PT))
                .map(|drawn| Arc::new(self.colours.apply(&drawn)))
                .collect(),
            _ => Vec::new(),
        };
        let report = attachments_report(&self.sources);
        let lane = Lane {
            summary: report.summary,
            attachments: report.attachments,
            timeline: self
                .timeline
                .as_ref()
                .map(|reference| timeline_report(&self.sources, reference, None).timeline),
        };
        CompileOutcome {
            generation: self.generation,
            status,
            notebook,
            elapsed_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            finished_at: chrono::Local::now().format("%H:%M:%S").to_string(),
            page,
            thumbnails,
            lane,
        }
    }
}

/// Today, for `datetime.today()` in the preview and a new lab notebook.
pub(super) fn today() -> Option<IsoDate> {
    use chrono::Datelike as _;
    let now = chrono::Local::now().date_naive();
    IsoDate::new(
        now.year(),
        u8::try_from(now.month()).ok()?,
        u8::try_from(now.day()).ok()?,
    )
}

/// The page colours of the preview under `tokens`: the theme's paper and
/// text (decided as drawn: the page follows the theme, the PDF is white).
pub(super) fn page_colours(tokens: &signex_types::theme::ThemeTokens) -> PageColours {
    PageColours {
        paper: [tokens.paper.r, tokens.paper.g, tokens.paper.b],
        ink: [tokens.text.r, tokens.text.g, tokens.text.b],
    }
}

/// An image the image widget shows.
pub(super) fn handle(image: &PageImage) -> image::Handle {
    image::Handle::from_rgba(image.width, image.height, image.rgba.clone())
}

impl Signex {
    /// The part values of `id`'s open sheets, with their unsaved edits: what
    /// an attachment's stale mark is judged against in the tab, and the
    /// values the Python cells of an export read.
    pub(crate) fn notebook_design_values(&self, id: ProjectId) -> DesignValues {
        let docs = &self.document_state;
        let sheets = docs
            .engines
            .iter()
            .filter(|(path, _)| {
                docs.document_project(path)
                    .is_some_and(|project| project.id == id)
            })
            .map(|(_, engine)| engine.document());
        crate::app::notebook_measurements::values_of(sheets)
    }

    /// Compile `id`'s notebook as typed, or note that one is owed when a
    /// compile is running.
    pub(super) fn request_notebook_compile(&mut self, id: ProjectId) -> Task<Message> {
        let colours = page_colours(&self.document_state.panel_ctx.tokens);
        let values = self.notebook_design_values(id);
        let Some((doc, tab)) = self.notebook_parts_mut(id) else {
            return Task::none();
        };
        let Some(text) = &tab.text else {
            return Task::none();
        };
        if tab.compile.running.is_some() {
            tab.compile.owed = true;
            return Task::none();
        }
        let job = CompileJob {
            generation: text.generation,
            sources: doc.compile_sources(),
            folder: doc
                .path
                .parent()
                .map_or_else(PathBuf::new, std::path::Path::to_path_buf),
            values,
            timeline: tab.timeline.as_ref().map(|open| open.shown.clone()),
            page: tab.page,
            colours,
            thumbnails: tab.panel == super::SidePanel::Pages,
        };
        tab.compile.running = Some(job.generation);
        tab.compile.owed = false;
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || job.run())
                    .await
                    .map_err(|error| error.to_string())
            },
            move |result| match result {
                Ok(outcome) => Message::from(NotebookMsg::Compiled {
                    project_id: id,
                    outcome: Arc::new(outcome),
                }),
                Err(error) => {
                    crate::diagnostics::log_warning(format!(
                        "[notebook] the compile did not finish: {error}"
                    ));
                    Message::from(NotebookMsg::Compiled {
                        project_id: id,
                        outcome: Arc::new(CompileOutcome::failed_to_run(error)),
                    })
                }
            },
        )
    }

    /// A compile landed: keep it, show its page when it gave pages, and run
    /// the compile an edit owed meanwhile. A compile that is not the one
    /// running (the tab was reopened, or a later one started) is dropped.
    pub(super) fn notebook_compiled(
        &mut self,
        id: ProjectId,
        outcome: Arc<CompileOutcome>,
    ) -> Task<Message> {
        let Some(tab) = self.ui_state.notebook_tabs.get_mut(&id) else {
            return Task::none();
        };
        if tab.compile.running != Some(outcome.generation) {
            return Task::none();
        }
        land(tab, outcome);
        if tab.compile.owed {
            return self.request_notebook_compile(id);
        }
        Task::none()
    }

    /// Draw page `page` of the last good compile for the preview.
    pub(super) fn draw_notebook_page(&mut self, id: ProjectId, page: usize) -> Task<Message> {
        let colours = page_colours(&self.document_state.panel_ctx.tokens);
        let Some(tab) = self.ui_state.notebook_tabs.get_mut(&id) else {
            return Task::none();
        };
        let Some(good) = tab.compile.last_good.clone() else {
            return Task::none();
        };
        let Some(notebook) = good.notebook.clone() else {
            return Task::none();
        };
        let page = page.min(notebook.page_count().saturating_sub(1));
        tab.page = page;
        let generation = good.generation;
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    notebook
                        .render_page(page, PREVIEW_PIXELS_PER_PT)
                        .map(|drawn| colours.apply(&drawn))
                })
                .await
                .ok()
                .flatten()
            },
            move |drawn| match drawn {
                Some(image) => Message::from(NotebookMsg::PageDrawn {
                    project_id: id,
                    generation,
                    page,
                    image: Arc::new(image),
                }),
                None => Message::noop(),
            },
        )
    }

    /// A page drawn after a page change: shown when it is still of the last
    /// good compile and still the page asked for.
    pub(super) fn notebook_page_drawn(
        &mut self,
        id: ProjectId,
        generation: u64,
        page: usize,
        image: &PageImage,
    ) {
        let Some(tab) = self.ui_state.notebook_tabs.get_mut(&id) else {
            return;
        };
        let current = tab
            .compile
            .last_good
            .as_ref()
            .is_some_and(|good| good.generation == generation);
        if current && tab.page == page {
            tab.compile.shown = Some(ShownPage {
                page,
                handle: handle(image),
            });
        }
    }
}

/// Keep `outcome` on `tab`: always as the newest compile, and as the last
/// good one when it gave pages.
pub(super) fn land(tab: &mut NotebookTab, outcome: Arc<CompileOutcome>) {
    tab.compile.running = None;
    if outcome.status.compiled {
        if let Some((page, image)) = &outcome.page {
            tab.page = *page;
            tab.compile.shown = Some(ShownPage {
                page: *page,
                handle: handle(image),
            });
        }
        if !outcome.thumbnails.is_empty() {
            tab.compile.thumbnails = outcome
                .thumbnails
                .iter()
                .map(|thumb| handle(thumb.as_ref()))
                .collect();
        }
        tab.compile.last_good = Some(outcome.clone());
    }
    tab.compile.last = Some(outcome);
}

impl CompileOutcome {
    /// The outcome of a compile whose worker died: one error, no pages.
    fn failed_to_run(error: String) -> Self {
        let sources = NotebookSources::new(super::open::MAIN_FILE, Vec::new());
        let compilation = signex_notebook::Compilation {
            notebook: None,
            diagnostics: vec![signex_notebook::Diagnostic {
                severity: signex_notebook::DiagnosticSeverity::Error,
                file: String::new(),
                line: 0,
                column: 0,
                message: format!("the compile did not finish: {error}"),
                hints: Vec::new(),
            }],
        };
        CompileOutcome {
            generation: 0,
            status: NotebookStatus::of(&sources, &compilation),
            notebook: None,
            elapsed_ms: 0,
            finished_at: chrono::Local::now().format("%H:%M:%S").to_string(),
            page: None,
            thumbnails: Vec::new(),
            lane: Lane::default(),
        }
    }
}
