//! The tab's toolbar (mockup row 114, top bar): the profile with its Auto
//! badge and its popover (Save as profile…, Rename…, Duplicate, Delete…,
//! Set as default), the one variant the profile builds, the pages, the
//! outputs chip, Run Selected and Run profile (Stop while a run goes).

use iced::widget::{Column, Space, column, container, mouse_area, row};
use iced::{Alignment, Element, Length, Widget as _};
use signex_i18n::{UserText, ids, t};
use signex_widgets::doc_tab_shell::Toolbar;

use super::{Frame, SP, kit, over};
use crate::app::Message;
use crate::app::output_job::tab::{JobTabMsg, NamingMode, TabPage};
use crate::styles::PillTone;

pub(super) fn view<'a>(frame: &Frame<'a>) -> Toolbar<'a, Message> {
    let look = frame.look;
    let tokens = look.tokens;
    let job = frame.job_path();
    let profile_words = match frame.profile {
        Some(profile) => {
            t!(look.i18n, ids::OUTPUT_JOB_PROFILE_BUTTON, "name" => profile.name.as_str())
        }
        None => look.t(ids::OUTPUT_JOB_PROFILE_NONE),
    };
    let mut bar = Toolbar::new().push(kit::ghost(
        tokens,
        profile_words,
        Some(Message::from(JobTabMsg::ProfileMenuToggled {
            job: job.clone(),
        })),
    ));
    if frame.profile.is_some_and(|p| p.triggers.is_auto()) {
        bar = bar.push(kit::chip(
            tokens,
            PillTone::Accent,
            look.t(ids::OUTPUT_JOB_AUTO),
        ));
    }
    bar = bar.push(variant_choice(frame)).separator();
    for (page, label) in [
        (TabPage::Settings, ids::OUTPUT_JOB_PAGE_SETTINGS),
        (TabPage::Automation, ids::OUTPUT_JOB_PAGE_AUTOMATION),
        (TabPage::Flow, ids::OUTPUT_JOB_PAGE_FLOW),
        (TabPage::Summary, ids::OUTPUT_JOB_PAGE_SUMMARY),
    ] {
        let msg = Message::from(JobTabMsg::PagePicked {
            job: job.clone(),
            page,
        });
        bar = bar.push(match frame.ui.page == page {
            true => kit::primary(tokens, look.t(label), Some(msg)),
            false => kit::ghost(tokens, look.t(label), Some(msg)),
        });
    }
    bar = bar.push_end(outputs_chip(frame));
    let ticked = frame.profile.map_or(0, |p| p.outputs.len());
    let selected = t!(look.i18n, ids::OUTPUT_JOB_RUN_SELECTED, "count" => ticked);
    if frame.running() {
        bar.push_end(kit::primary(
            tokens,
            look.t(ids::OUTPUT_JOB_STOP),
            Some(Message::from(JobTabMsg::StopPressed { job })),
        ))
    } else {
        let can_run = frame.draft.fault().is_none() && !frame.draft.outputs.is_empty();
        bar.push_end(kit::ghost(
            tokens,
            selected,
            can_run.then(|| {
                Message::from(JobTabMsg::RunPressed {
                    job: job.clone(),
                    selected: true,
                })
            }),
        ))
        .push_end(kit::primary(
            tokens,
            look.t(ids::OUTPUT_JOB_RUN_PROFILE),
            can_run.then(|| {
                Message::from(JobTabMsg::RunPressed {
                    job,
                    selected: false,
                })
            }),
        ))
    }
}

/// The one variant the shown profile builds.
fn variant_choice<'a>(frame: &Frame<'a>) -> Element<'a, Message> {
    let look = frame.look;
    let picked = frame
        .profile
        .and_then(|p| p.variant.clone())
        .or_else(|| frame.draft.variant.clone())
        .unwrap_or_else(|| signex_types::project::BASE_VARIANT.to_string());
    let mut line = row![kit::quiet(look, look.t(ids::OUTPUT_JOB_VARIANT))]
        .spacing(SP.xs)
        .align_y(Alignment::Center);
    for variant in &frame.variants {
        let msg = Message::from(JobTabMsg::VariantPicked {
            job: frame.job_path(),
            variant: variant.clone(),
        });
        let words = UserText::new(variant.clone());
        line = line.push(match variant.eq_ignore_ascii_case(&picked) {
            true => kit::primary(look.tokens, words, Some(msg)),
            false => kit::ghost(look.tokens, words, Some(msg)),
        });
    }
    line.boxed()
}

/// "Outputs: generating 3/11…", "Outputs: run stopped", "Outputs: 3 out of
/// date" or "Outputs up to date" (mockup row 114, the chip on the tab).
fn outputs_chip<'a>(frame: &Frame<'a>) -> Element<'a, Message> {
    let look = frame.look;
    let (tone, words) = match frame.run {
        Some(run) if run.is_running() => {
            let (done, total) = run.progress();
            (
                PillTone::Accent,
                t!(look.i18n, ids::OUTPUT_JOB_CHIP_GENERATING, "done" => done, "total" => total),
            )
        }
        _ if frame.stale > 0 => (
            PillTone::Warn,
            t!(look.i18n, ids::OUTPUT_JOB_CHIP_STALE, "count" => frame.stale),
        ),
        Some(run) if run.stopped.is_some() => (PillTone::Err, look.t(ids::OUTPUT_JOB_CHIP_STOPPED)),
        _ => (PillTone::Ok, look.t(ids::OUTPUT_JOB_CHIP_UP_TO_DATE)),
    };
    kit::chip(look.tokens, tone, words)
}

/// The profile popover over `body` while it is open, or the name field of
/// Save as profile… / Rename….
pub(super) fn with_menus<'a>(
    frame: &Frame<'a>,
    body: Element<'a, Message>,
) -> Element<'a, Message> {
    let ui = frame.ui;
    if !ui.profile_menu && ui.naming.is_none() {
        return body;
    }
    let look = frame.look;
    let job = frame.job_path();
    let mut items: Column<Element<'a, Message>> =
        column![kit::head(look, ids::OUTPUT_JOB_PROFILES)]
            .spacing(SP.xs)
            .width(Length::Fixed(super::TS.body * 22.0));
    if let Some((mode, typed)) = &ui.naming {
        let title = match mode {
            NamingMode::SaveAs => ids::OUTPUT_JOB_PROFILE_SAVE_AS,
            NamingMode::Rename => ids::OUTPUT_JOB_PROFILE_RENAME,
        };
        let editing = job.clone();
        items = items
            .push(kit::line(look, look.t(title)))
            .push(kit::field(
                look.t(ids::OUTPUT_JOB_PROFILE_NAME),
                typed,
                move |text| {
                    Message::from(JobTabMsg::ProfileNameEdited {
                        job: editing.clone(),
                        text,
                    })
                },
            ))
            .push(
                row![
                    kit::primary(
                        look.tokens,
                        look.t(ids::OUTPUT_JOB_OK),
                        Some(Message::from(JobTabMsg::ProfileNamingConfirmed {
                            job: job.clone()
                        }))
                    ),
                    kit::ghost(
                        look.tokens,
                        look.t(ids::OUTPUT_JOB_CANCEL),
                        Some(Message::from(JobTabMsg::ProfileNamingCancelled {
                            job: job.clone()
                        }))
                    ),
                ]
                .spacing(SP.s),
            );
    } else {
        for profile in &frame.draft.profiles {
            let variant = profile
                .variant
                .clone()
                .unwrap_or_else(|| signex_types::project::BASE_VARIANT.to_string());
            let words = UserText::new(format!("{} · {variant}", profile.name));
            let msg = Message::from(JobTabMsg::ProfilePicked {
                job: job.clone(),
                name: profile.name.clone(),
            });
            items = items.push(kit::ghost(look.tokens, words, Some(msg)));
        }
        let has = frame.profile.is_some();
        let action = |id, msg: JobTabMsg, live: bool| {
            kit::ghost(look.tokens, look.t(id), live.then(|| Message::from(msg)))
        };
        items = items
            .push(action(
                ids::OUTPUT_JOB_PROFILE_SAVE_AS,
                JobTabMsg::ProfileNamingStarted {
                    job: job.clone(),
                    mode: NamingMode::SaveAs,
                },
                true,
            ))
            .push(action(
                ids::OUTPUT_JOB_PROFILE_RENAME,
                JobTabMsg::ProfileNamingStarted {
                    job: job.clone(),
                    mode: NamingMode::Rename,
                },
                has,
            ))
            .push(action(
                ids::OUTPUT_JOB_PROFILE_DUPLICATE,
                JobTabMsg::ProfileDuplicated { job: job.clone() },
                has,
            ))
            .push(action(
                ids::OUTPUT_JOB_PROFILE_DELETE,
                JobTabMsg::ProfileDeleted { job: job.clone() },
                has,
            ))
            .push(action(
                ids::OUTPUT_JOB_PROFILE_DEFAULT,
                JobTabMsg::ProfileMadeDefault { job: job.clone() },
                has,
            ))
            .push(kit::quiet(look, look.t(ids::OUTPUT_JOB_PROFILES_NOTE)));
    }
    let card = container(items)
        .padding(SP.s)
        .style(crate::styles::card(look.tokens));
    // A press off the popover closes it.
    let close = match ui.naming.is_some() {
        true => JobTabMsg::ProfileNamingCancelled { job },
        false => JobTabMsg::ProfileMenuToggled { job },
    };
    let backdrop = mouse_area(Space::new().width(Length::Fill).height(Length::Fill))
        .on_press(Message::from(close));
    let layer = iced::widget::stack![backdrop.boxed(), container(card).padding(SP.s).boxed()]
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed();
    over(body, layer)
}
