//! The run banner (mockup row 193b): the Export Job run the files came from
//! (profile, variant, time), Back to run summary and a close; or, when the
//! run's files moved since, where they were.

use iced::widget::{Space, container, row};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{ids, t};
use signex_widgets::{button, text};

use super::{Look, SP, TS};
use crate::app::gerber_viewer::{GerberViewerMsg, RunOrigin};
use crate::styles::ti;

pub(super) fn view<'a>(
    look: Look<'a>,
    origin: &'a RunOrigin,
    missing: bool,
) -> Element<'a, GerberViewerMsg> {
    let accent = ti(look.tokens.accent);
    let line = match missing {
        true => t!(
            look.i18n,
            ids::OUTPUT_JOB_VIEWER_FILES_MISSING,
            "folder" => origin.folder.display().to_string()
        ),
        false => t!(
            look.i18n,
            ids::OUTPUT_JOB_VIEWER_FROM_RUN,
            "profile" => origin.profile.clone().unwrap_or_default(),
            "variant" => origin.variant.as_str(),
            "started" => origin.started.as_str()
        ),
    };
    let back = button(text(look.t(ids::OUTPUT_JOB_VIEWER_BACK)).size(TS.dense))
        .padding([SP.xs, SP.s])
        .style(crate::styles::ghost_button(look.tokens))
        .on_press(GerberViewerMsg::BackToRunPressed);
    let close = button(text(look.t(ids::OUTPUT_JOB_VIEWER_CLOSE)).size(TS.dense))
        .padding([SP.xs, SP.s])
        .style(crate::styles::ghost_button(look.tokens))
        .on_press(GerberViewerMsg::RunBannerClosed);
    let content = row![
        text(line)
            .size(TS.dense)
            .color(ti(look.tokens.text))
            .boxed(),
        Space::new().width(Length::Fill).boxed(),
        back.boxed(),
        close.boxed(),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center);
    container(content)
        .width(Length::Fill)
        .padding([SP.xs, SP.m])
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(iced::Color { a: 0.12, ..accent })),
            border: Border {
                color: accent,
                width: 1.0,
                ..Border::default()
            },
            ..Default::default()
        })
        .boxed()
}
