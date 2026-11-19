//! The Problems card of Update PCB (#2533): every blocked row lifted above
//! the list on a red card, one line each (designator, reason pill, the
//! reason in full, its fix button and Cross Probe). The rows also stay in
//! their groups, unticked. Which rows and which fix: `app/update_pcb/table.rs`.

use iced::widget::text::Wrapping;
use iced::widget::{Column, Row, Space, container, row, tooltip as tip};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget as _};
use signex_engine::pcb::eco::{EcoChangeList, EcoRow};
use signex_i18n::{I18n, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{text, tooltip};

use super::change_rows::hint;
use super::{SP, TS, link, texts, up};
use crate::app::update_pcb::table::{ProblemFix, can_cross_probe, object, problems};
use crate::app::{Message, UpdatePcbMsg};
use crate::panels::form_look::ghost_button;
use crate::styles::{PillTone, Radii, pill, pill_colors, ti, tokens};

/// Width of the designator column of a problem line.
const REF_W: f32 = TS.body * 4.0;

/// The card, or nothing when no row is blocked.
pub(super) fn card<'a>(
    list: &EcoChangeList,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Option<Element<'a, Message>> {
    let blocked = problems(list);
    if blocked.is_empty() {
        return None;
    }
    let danger = tokens::danger(tokens);
    let muted = tokens::muted_text(tokens);
    let heading = row![
        text(t!(i18n, ids::CHANGE_ROW_ICON_BLOCKED))
            .size(TS.body)
            .color(danger),
        text(t!(
            i18n,
            ids::UPDATE_PCB_PROBLEMS_HEADING,
            "count" => blocked.len(),
            "parts" => blocked_parts(&blocked)
        ))
        .size(TS.body)
        .color(danger),
        Space::new().width(Length::Fill),
        text(t!(i18n, ids::UPDATE_PCB_PROBLEMS_HINT))
            .size(TS.caption)
            .color(muted),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center);
    let mut col = Column::new().spacing(SP.xs).push(heading.boxed());
    for row in blocked {
        col = col.push(line(row, i18n, tokens));
    }
    let (fill, edge) = (tokens::resolve(tokens).err_soft, danger);
    Some(
        container(col.padding([SP.s, SP.m]))
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(fill)),
                border: Border {
                    width: 1.0,
                    color: edge,
                    radius: Radii::GRAPHITE.card.into(),
                },
                ..container::Style::default()
            })
            .boxed(),
    )
}

/// How many parts the blocked rows hold back: their distinct objects.
fn blocked_parts(rows: &[&EcoRow]) -> usize {
    let mut objects: Vec<String> = rows.iter().map(|r| object(&r.change)).collect();
    objects.sort();
    objects.dedup();
    objects.len()
}

/// One blocked row: designator, reason pill, the reason in full, the fix
/// button and Cross Probe.
fn line<'a>(r: &EcoRow, i18n: &I18n, tokens: &ThemeTokens) -> Element<'a, Message> {
    let Some(why) = r.blocked.as_ref() else {
        return Space::new().boxed();
    };
    let danger = tokens::danger(tokens);
    let reason_ink = pill_colors(tokens, PillTone::Err).1;
    let mut line: Row<Element<'a, Message>> = Row::new()
        .spacing(SP.m)
        .align_y(Alignment::Center)
        .push(
            text(t!(i18n, ids::CHANGE_ROW_ICON_BLOCKED))
                .size(TS.body)
                .color(danger)
                .boxed(),
        )
        .push(
            text(UserText::new(object(&r.change)))
                .size(TS.body)
                .font(super::super::graphite::mono())
                .color(ti(tokens.text))
                .width(REF_W)
                .boxed(),
        )
        .push(
            container(
                text(texts::problem(i18n, why))
                    .size(TS.caption)
                    .color(reason_ink),
            )
            .padding([0.0, SP.s])
            .style(pill(tokens, PillTone::Err))
            .boxed(),
        )
        .push(
            container(
                text(texts::blocked(i18n, why))
                    .size(TS.caption)
                    .color(tokens::muted_text(tokens))
                    .wrapping(Wrapping::None),
            )
            .width(Length::Fill)
            .clip(true)
            .boxed(),
        );
    match ProblemFix::of(why) {
        ProblemFix::ChooseFootprint => {
            line = line.push(link(
                t!(i18n, ids::UPDATE_PCB_FIX_CHOOSE_FOOTPRINT),
                up(UpdatePcbMsg::ChooseFootprintRequested(r.key.clone())),
            ));
        }
        ProblemFix::Annotate => {
            line = line.push(link(
                t!(i18n, ids::UPDATE_PCB_FIX_ANNOTATE),
                up(UpdatePcbMsg::AnnotateRequested),
            ));
        }
        ProblemFix::None => {}
    }
    if can_cross_probe(&r.key) {
        let probe = signex_widgets::button(
            text(t!(i18n, ids::UPDATE_PCB_FIX_CROSS_PROBE_ICON)).size(TS.body),
        )
        .padding([0.0, SP.xs])
        .on_press(up(UpdatePcbMsg::CrossProbeRequested(r.key.clone())))
        .style(ghost_button);
        line = line.push(
            tooltip(
                probe,
                hint(t!(i18n, ids::UPDATE_PCB_FIX_CROSS_PROBE), tokens),
                tip::Position::Bottom,
            )
            .boxed(),
        );
    }
    line.boxed()
}
