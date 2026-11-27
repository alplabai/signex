//! The right column of Settings (mockup row 114): the shown profile's
//! output container (folder, zip, a folder per group, the naming pattern
//! with a live example, a release folder, Clear the folder), its run
//! pipeline (Validate, On errors, On warnings) and the last run.

use iced::widget::{Column, column, scrollable};
use iced::{Element, Length, Widget as _};
use signex_i18n::{UserText, ids, t};
use signex_types::output_job::{JobProfile, OnErrors, OnWarnings, expand_naming};

use super::{Frame, SP, kit};
use crate::app::Message;
use crate::app::output_job::tab::{JobTabMsg, PackagingSwitch};

pub(super) fn view<'a>(frame: &Frame<'a>) -> Element<'a, Message> {
    let look = frame.look;
    let mut side: Column<Element<'a, Message>> = column![].spacing(SP.m).padding(SP.m);
    side = side.push(kit::head(look, ids::OUTPUT_JOB_CONTAINER));
    match frame.profile {
        Some(profile) => {
            side = side
                .push(container(frame, profile))
                .push(pipeline(frame, profile));
        }
        None => {
            side = side
                .push(kit::quiet(
                    look,
                    t!(look.i18n, ids::OUTPUT_JOB_FOLDER_OF_JOB, "folder" => frame.draft.folder.as_str()),
                ))
                .push(kit::quiet(look, look.t(ids::OUTPUT_JOB_NO_PROFILE)));
        }
    }
    side = side.push(kit::head(look, ids::OUTPUT_JOB_LAST_RUN));
    match frame.draft.history.first() {
        Some(record) => {
            side = side.push(kit::quiet(
                look,
                t!(
                    look.i18n,
                    ids::OUTPUT_JOB_LAST_RUN_LINE,
                    "started" => record.started.as_str(),
                    "outputs" => record.outputs,
                    "files" => record.files
                ),
            ));
            for line in record.warning_lines.iter().take(5) {
                side = side.push(kit::quiet(look, UserText::new(line.clone())));
            }
        }
        None => side = side.push(kit::quiet(look, look.t(ids::OUTPUT_JOB_NEVER_RUN))),
    }
    scrollable(side)
        .direction(crate::styles::thin_vertical())
        .height(Length::Fill)
        .boxed()
}

fn switch(frame: &Frame<'_>, which: PackagingSwitch) -> Message {
    Message::from(JobTabMsg::PackagingToggled {
        job: frame.job_path(),
        switch: which,
    })
}

/// Folder, zip, group folders, naming, release folder, Clear the folder.
fn container<'a>(frame: &Frame<'a>, profile: &'a JobProfile) -> Element<'a, Message> {
    let look = frame.look;
    let folder = profile
        .folder
        .clone()
        .unwrap_or_else(|| frame.draft.folder.clone());
    let editing = frame.job_path();
    let naming_job = frame.job_path();
    let pattern = profile.naming.clone().unwrap_or_default();
    let example = expand_naming(
        profile.naming.as_deref().unwrap_or("{project}"),
        &frame.project_stem,
        &frame.rev,
        profile.variant.as_deref().unwrap_or(""),
    );
    column![
        kit::field(look.t(ids::OUTPUT_JOB_FOLDER), &folder, move |text| {
            Message::from(JobTabMsg::FolderEdited {
                job: editing.clone(),
                text,
            })
        }),
        kit::tick(
            profile.zip,
            look.t(ids::OUTPUT_JOB_ZIP),
            Some(switch(frame, PackagingSwitch::Zip))
        ),
        kit::tick(
            profile.group_folders,
            look.t(ids::OUTPUT_JOB_GROUP_FOLDERS),
            Some(switch(frame, PackagingSwitch::GroupFolders))
        ),
        kit::head(look, ids::OUTPUT_JOB_NAMING),
        kit::field(look.t(ids::OUTPUT_JOB_NAMING_HINT), &pattern, move |text| {
            Message::from(JobTabMsg::NamingEdited {
                job: naming_job.clone(),
                text,
            })
        }),
        kit::quiet(
            look,
            t!(look.i18n, ids::OUTPUT_JOB_NAMING_EXAMPLE, "example" => example)
        ),
        kit::tick(
            profile.release_folder,
            look.t(ids::OUTPUT_JOB_RELEASE_FOLDER),
            Some(switch(frame, PackagingSwitch::ReleaseFolder))
        ),
        kit::tick(
            profile.clear_folder,
            look.t(ids::OUTPUT_JOB_CLEAR_FOLDER),
            Some(switch(frame, PackagingSwitch::ClearFolder))
        ),
        kit::quiet(look, look.t(ids::OUTPUT_JOB_CLEAR_FOLDER_NOTE)),
        kit::ghost(
            look.tokens,
            look.t(ids::OUTPUT_JOB_OPEN_FOLDER),
            Some(Message::from(JobTabMsg::OpenFolderPressed {
                job: frame.job_path()
            }))
        ),
    ]
    .spacing(SP.s)
    .into()
}

/// Validate, On errors and On warnings.
fn pipeline<'a>(frame: &Frame<'a>, profile: &'a JobProfile) -> Element<'a, Message> {
    let look = frame.look;
    let job = frame.job_path();
    let errors_job = job.clone();
    let warnings_job = job.clone();
    column![
        kit::head(look, ids::OUTPUT_JOB_PIPELINE),
        kit::tick(
            profile.validate,
            look.t(ids::OUTPUT_JOB_STEP_VALIDATE_LONG),
            Some(switch(frame, PackagingSwitch::Validate))
        ),
        kit::quiet(look, look.t(ids::OUTPUT_JOB_STEP_GENERATE_LONG)),
        kit::quiet(look, look.t(ids::OUTPUT_JOB_STEP_PACKAGE_LONG)),
        kit::quiet(look, look.t(ids::OUTPUT_JOB_STEP_SUMMARY_LONG)),
        kit::quiet(look, look.t(ids::OUTPUT_JOB_ON_ERRORS)),
        kit::choice(
            look.tokens,
            profile.on_errors,
            vec![
                (OnErrors::Stop, look.t(ids::OUTPUT_JOB_POLICY_STOP)),
                (OnErrors::Ask, look.t(ids::OUTPUT_JOB_POLICY_ASK)),
            ],
            move |policy| Message::from(JobTabMsg::OnErrorsPicked {
                job: errors_job.clone(),
                policy,
            }),
        ),
        kit::quiet(look, look.t(ids::OUTPUT_JOB_ON_WARNINGS)),
        kit::choice(
            look.tokens,
            profile.on_warnings,
            vec![
                (
                    OnWarnings::Continue,
                    look.t(ids::OUTPUT_JOB_POLICY_CONTINUE)
                ),
                (OnWarnings::Ask, look.t(ids::OUTPUT_JOB_POLICY_ASK)),
                (OnWarnings::Stop, look.t(ids::OUTPUT_JOB_POLICY_STOP)),
            ],
            move |policy| Message::from(JobTabMsg::OnWarningsPicked {
                job: warnings_job.clone(),
                policy,
            }),
        ),
    ]
    .spacing(SP.s)
    .into()
}
