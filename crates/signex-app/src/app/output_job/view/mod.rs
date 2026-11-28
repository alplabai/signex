//! The Export Job tab's view (mockup row 114): the shared document-tab
//! shell with the toolbar (profile, variant, pages, Run Selected, Run
//! profile or Stop, the outputs chip), then the page: Settings (outputs,
//! the picked output's form, the container and pipeline), Automation, the
//! run Flow, or the Summary with the history. Pure: it reads the draft, the
//! tab's view state and the runs.

mod automation;
mod flow;
mod form;
mod gerber_form;
mod kit;
mod outputs;
mod side;
mod summary;
mod toolbar;

use std::path::{Path, PathBuf};

use iced::widget::{column, container, row, scrollable, stack};
use iced::{Element, Length, Widget as _};
use signex_i18n::{I18n, Localized, ids, t};
use signex_types::output_job::{JobProfile, OutputJob};
use signex_types::theme::ThemeTokens;
use signex_widgets::doc_tab_shell::{DocTabShell, DocTabShellStyle, Footer};
use signex_widgets::text;

use super::pipeline::{JobRun, RunStage};
use super::tab::{JobTabUi, TabPage};
use crate::app::{Message, Signex};
use crate::styles::{Spacing, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The width of the outputs list and of the container column, in body ems.
const SIDE_EMS: f32 = 22.0;

/// What every part of the view reads.
#[derive(Clone, Copy)]
pub(super) struct Look<'a> {
    pub tokens: &'a ThemeTokens,
    pub i18n: &'a I18n,
}

impl Look<'_> {
    pub(super) fn t(&self, id: signex_i18n::MessageId) -> Localized {
        t!(self.i18n, id)
    }
}

/// The tab's job and everything the page shows of it.
pub(super) struct Frame<'a> {
    pub look: Look<'a>,
    pub job: &'a Path,
    pub draft: &'a OutputJob,
    pub ui: &'a JobTabUi,
    /// This session's newest run of the job.
    pub run: Option<&'a JobRun>,
    /// The profile shown, when the job has one.
    pub profile: Option<&'a JobProfile>,
    /// The project's variants, Base first.
    pub variants: Vec<String>,
    /// The shown profile's ticked outputs out of date.
    pub stale: usize,
    pub command_line: Option<String>,
    /// The project's file name and revision, for the naming example.
    pub project_stem: String,
    pub rev: String,
}

impl Frame<'_> {
    pub(super) fn job_path(&self) -> PathBuf {
        self.job.to_path_buf()
    }

    /// Whether the run shown is going now.
    pub(super) fn running(&self) -> bool {
        self.run.is_some_and(JobRun::is_running)
    }
}

impl Signex {
    /// The Export Job tab of `job`.
    pub(crate) fn view_output_job_tab(&self, job: PathBuf) -> Element<'_, Message> {
        let look = Look {
            tokens: &self.document_view.panel_ctx.tokens,
            i18n: &self.ui_state.i18n,
        };
        let ui = self.output_jobs().tabs.get(&job);
        let held = self.document_state.output_jobs.get_key_value(&job);
        let (Some(ui), Some((path, draft))) = (ui, held) else {
            return not_ready(look, ui);
        };
        let profile = self
            .shown_profile_name(path)
            .and_then(|name| draft.profile(&name));
        let stale = profile.map_or(0, |profile| {
            self.output_jobs().auto.stale_count(path, &profile.outputs)
        });
        let variants = self
            .document_state
            .document_project(path)
            .map(|project| {
                let mut all = vec![signex_types::project::BASE_VARIANT.to_string()];
                all.extend(project.data.variant_definitions.iter().cloned());
                all
            })
            .unwrap_or_default();
        let (project_stem, rev) = self
            .output_job_scope(path)
            .map(|scope| (scope.stem, scope.rev))
            .unwrap_or_default();
        let frame = Frame {
            look,
            job: path,
            draft,
            ui,
            run: self.output_jobs().runs.latest(path),
            profile,
            variants,
            stale,
            command_line: self.job_command_line(path),
            project_stem,
            rev,
        };
        tab(frame)
    }
}

/// The tab before its job is read, or when it could not be.
fn not_ready<'a>(look: Look<'a>, ui: Option<&'a JobTabUi>) -> Element<'a, Message> {
    let words = match ui {
        Some(ui) if !ui.loading && ui.read_error.is_some() => t!(
            look.i18n,
            ids::OUTPUT_JOB_READ_FAILED,
            "why" => ui.read_error.as_deref().unwrap_or_default()
        ),
        _ => look.t(ids::OUTPUT_JOB_READING),
    };
    container(text(words).size(TS.body))
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .boxed()
}

/// The whole tab.
fn tab(frame: Frame<'_>) -> Element<'_, Message> {
    let look = frame.look;
    let body = match frame.ui.page {
        TabPage::Settings => settings(&frame),
        TabPage::Automation => automation::view(&frame),
        TabPage::Flow => flow::view(&frame),
        TabPage::Summary => summary::view(&frame),
    };
    let body = toolbar::with_menus(&frame, body);
    let footer = footer(&frame);
    DocTabShell::new(body, DocTabShellStyle::from_tokens(look.tokens, TS.body))
        .toolbar(toolbar::view(&frame))
        .footer(footer)
        .view()
}

/// Settings: outputs on the left, the picked output's form in the middle,
/// the container and the pipeline on the right.
fn settings<'a>(frame: &Frame<'a>) -> Element<'a, Message> {
    let side = Length::Fixed(TS.body * SIDE_EMS);
    let middle = scrollable(container(form::view(frame)).padding(SP.l))
        .direction(crate::styles::thin_vertical())
        .width(Length::Fill)
        .height(Length::Fill);
    row![
        container(outputs::view(frame))
            .width(side)
            .height(Length::Fill),
        middle,
        container(side::view(frame))
            .width(side)
            .height(Length::Fill),
    ]
    .height(Length::Fill)
    .boxed()
}

/// The status line (mockup row 114, the footer of each state).
fn footer<'a>(frame: &Frame<'a>) -> Footer<'a> {
    let i18n = frame.look.i18n;
    let status = match frame.run {
        Some(run) if run.is_running() => {
            let (done, total) = run.outputs_done();
            let at = match run.stage {
                RunStage::Validating => t!(i18n, ids::OUTPUT_JOB_STEP_VALIDATE).into_string(),
                RunStage::Asking => t!(i18n, ids::OUTPUT_JOB_STATUS_ASKING).into_string(),
                RunStage::Packaging => t!(i18n, ids::OUTPUT_JOB_STEP_PACKAGE).into_string(),
                _ => t!(i18n, ids::OUTPUT_JOB_STEP_GENERATE).into_string(),
            };
            t!(
                i18n,
                ids::OUTPUT_JOB_STATUS_RUNNING,
                "profile" => run.profile.clone().unwrap_or_default(),
                "step" => at,
                "done" => done,
                "total" => total
            )
        }
        Some(run) => match &run.stopped {
            Some(stopped) => t!(
                i18n,
                ids::OUTPUT_JOB_STATUS_STOPPED,
                "at" => stopped.at.as_str()
            ),
            None => t!(
                i18n,
                ids::OUTPUT_JOB_STATUS_COMPLETE,
                "variant" => run.variant.as_str(),
                "outputs" => run.report.outputs.len(),
                "files" => run.report.file_count(),
                "warnings" => run.report.warnings.len(),
                "seconds" => run.seconds
            ),
        },
        None if frame.draft.outputs.is_empty() => t!(i18n, ids::OUTPUT_JOB_STATUS_EMPTY),
        None => t!(i18n, ids::OUTPUT_JOB_STATUS_READY),
    };
    let footer = Footer::new(status);
    match frame.draft.fault() {
        Some(fault) => footer.note(t!(
            i18n,
            ids::OUTPUT_JOB_FAULT,
            "fault" => fault.to_string()
        )),
        None => footer,
    }
}

/// `content` with `layer` over it.
pub(super) fn over<'a>(
    content: Element<'a, Message>,
    layer: Element<'a, Message>,
) -> Element<'a, Message> {
    stack![content, layer]
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed()
}

/// A column page with a scroll.
pub(super) fn page<'a>(parts: Vec<Element<'a, Message>>) -> Element<'a, Message> {
    let mut col = column![].spacing(SP.l).padding(SP.l).width(Length::Fill);
    for part in parts {
        col = col.push(part);
    }
    scrollable(col)
        .direction(crate::styles::thin_vertical())
        .height(Length::Fill)
        .width(Length::Fill)
        .boxed()
}
