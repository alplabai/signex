//! Automation (mockup row 114, "Automation"): the shown profile's triggers
//! (run on save with its files and wait, keep outputs up to date, before
//! closing the project, the release or git tag pattern), the command line
//! for CI with Copy, and the recent auto runs from the history.

use iced::Element;
use iced::widget::{Column, column};
use signex_i18n::{ids, t};
use signex_types::output_job::BeforeClose;

use super::{Frame, kit, page};
use crate::app::Message;
use crate::app::output_job::tab::{JobTabMsg, TabPage, TriggerEdit};

pub(super) fn view<'a>(frame: &Frame<'a>) -> Element<'a, Message> {
    let look = frame.look;
    let job = frame.job_path();
    let back = kit::ghost(
        look.tokens,
        look.t(ids::OUTPUT_JOB_BACK_TO_SETTINGS),
        Some(Message::from(JobTabMsg::PagePicked {
            job: job.clone(),
            page: TabPage::Settings,
        })),
    );
    let Some(profile) = frame.profile else {
        return page(vec![
            back,
            kit::line(look, look.t(ids::OUTPUT_JOB_NO_PROFILE)),
        ]);
    };
    let edit_job = job.clone();
    let edit = move |change: TriggerEdit| {
        Message::from(JobTabMsg::TriggerEdited {
            job: edit_job.clone(),
            edit: change,
        })
    };
    let triggers = &profile.triggers;
    let sources_job = job.clone();
    let wait_job = job.clone();
    let tag_job = job.clone();
    let on_save = column![
        kit::tick(
            triggers.on_save,
            look.t(ids::OUTPUT_JOB_TRIGGER_ON_SAVE),
            Some(edit(TriggerEdit::OnSaveToggled))
        ),
        kit::quiet(look, look.t(ids::OUTPUT_JOB_TRIGGER_ON_SAVE_NOTE)),
        kit::field(
            look.t(ids::OUTPUT_JOB_TRIGGER_SOURCES),
            &triggers.sources.join(", "),
            move |text| Message::from(JobTabMsg::TriggerEdited {
                job: sources_job.clone(),
                edit: TriggerEdit::SourcesEdited(text),
            })
        ),
        kit::quiet(look, look.t(ids::OUTPUT_JOB_TRIGGER_SOURCES_NOTE)),
        kit::field(
            look.t(ids::OUTPUT_JOB_TRIGGER_WAIT),
            &triggers.wait_seconds.to_string(),
            move |text| Message::from(JobTabMsg::TriggerEdited {
                job: wait_job.clone(),
                edit: TriggerEdit::WaitEdited(text),
            })
        ),
        kit::quiet(
            look,
            t!(look.i18n, ids::OUTPUT_JOB_TRIGGER_WAIT_NOTE, "seconds" => triggers.wait_seconds)
        ),
    ]
    .spacing(super::SP.s);
    let keep = column![
        kit::tick(
            triggers.keep_up_to_date,
            look.t(ids::OUTPUT_JOB_TRIGGER_KEEP),
            Some(edit(TriggerEdit::KeepUpToDateToggled))
        ),
        kit::quiet(look, look.t(ids::OUTPUT_JOB_TRIGGER_KEEP_NOTE)),
    ]
    .spacing(super::SP.s);
    let close = column![
        kit::line(look, look.t(ids::OUTPUT_JOB_TRIGGER_BEFORE_CLOSE)),
        kit::choice(
            look.tokens,
            triggers.before_close,
            vec![
                (BeforeClose::Off, look.t(ids::OUTPUT_JOB_TRIGGER_OFF)),
                (BeforeClose::Ask, look.t(ids::OUTPUT_JOB_POLICY_ASK)),
                (BeforeClose::Always, look.t(ids::OUTPUT_JOB_TRIGGER_ALWAYS)),
            ],
            move |when| edit(TriggerEdit::BeforeClosePicked(when)),
        ),
        kit::quiet(look, look.t(ids::OUTPUT_JOB_TRIGGER_BEFORE_CLOSE_NOTE)),
    ]
    .spacing(super::SP.s);
    let tag = column![
        kit::line(look, look.t(ids::OUTPUT_JOB_TRIGGER_TAG)),
        kit::field(
            look.t(ids::OUTPUT_JOB_TRIGGER_TAG_HINT),
            triggers.on_tag.as_deref().unwrap_or(""),
            move |text| Message::from(JobTabMsg::TriggerEdited {
                job: tag_job.clone(),
                edit: TriggerEdit::TagEdited(text),
            })
        ),
        kit::quiet(look, look.t(ids::OUTPUT_JOB_TRIGGER_TAG_NOTE)),
    ]
    .spacing(super::SP.s);
    let mut parts = vec![
        back,
        kit::head(look, ids::OUTPUT_JOB_PAGE_AUTOMATION),
        kit::quiet(look, look.t(ids::OUTPUT_JOB_AUTOMATION_NOTE)),
        kit::card(look.tokens, on_save.into()),
        kit::card(look.tokens, keep.into()),
        kit::card(look.tokens, close.into()),
        kit::card(look.tokens, tag.into()),
    ];
    parts.push(command_card(frame));
    parts.push(recent_auto_runs(frame));
    page(parts)
}

/// The command line for CI, with Copy.
fn command_card<'a>(frame: &Frame<'a>) -> Element<'a, Message> {
    let look = frame.look;
    let line = frame.command_line.clone().unwrap_or_default();
    let col = column![
        kit::head(look, ids::OUTPUT_JOB_COMMAND_LINE),
        kit::user(look, &line),
        kit::ghost(
            look.tokens,
            look.t(ids::OUTPUT_JOB_COPY),
            Some(Message::from(JobTabMsg::CopyCommandPressed {
                job: frame.job_path()
            }))
        ),
        kit::quiet(look, look.t(ids::OUTPUT_JOB_COMMAND_LINE_NOTE)),
    ]
    .spacing(super::SP.s);
    kit::card(look.tokens, col.into())
}

/// The auto runs of the history, newest first.
fn recent_auto_runs<'a>(frame: &Frame<'a>) -> Element<'a, Message> {
    let look = frame.look;
    let mut col: Column<Element<'a, Message>> =
        column![kit::head(look, ids::OUTPUT_JOB_RECENT_AUTO)].spacing(super::SP.xs);
    let mut any = false;
    for record in frame
        .draft
        .history
        .iter()
        .filter(|r| r.trigger.is_auto())
        .take(5)
    {
        any = true;
        let trigger = look.t(super::summary::trigger_label(record.trigger));
        col = col.push(kit::quiet(
            look,
            t!(
                look.i18n,
                ids::OUTPUT_JOB_RECENT_AUTO_LINE,
                "started" => record.started.as_str(),
                "trigger" => trigger.into_string(),
                "profile" => record.profile.clone().unwrap_or_default(),
                "outputs" => record.outputs
            ),
        ));
    }
    if !any {
        col = col.push(kit::quiet(look, look.t(ids::OUTPUT_JOB_RECENT_AUTO_NONE)));
    }
    kit::card(look.tokens, col.into())
}
