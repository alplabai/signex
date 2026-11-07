//! ERC / DRC summary card at the foot of the Graphite left dock. It shows
//! only counts the app really has: ERC findings from the last run, and a
//! dash for DRC, which has no engine yet.

use iced::widget::{column, container, row, text};
use iced::{Element, Length, Widget as _};

use super::types::DockMessage;
use crate::panels::{ErcSeverityLite, PanelContext};
use crate::styles::tokens::resolve;
use crate::styles::{PillTone, Radii, TypeScale, pill, ti};

const TS: TypeScale = TypeScale::DEFAULT;

/// Error and warning counts of the ERC findings; `None` when there are none
/// to count (no run yet, or a clean run: the card is then left out).
pub(super) fn erc_counts(ctx: &PanelContext) -> Option<(usize, usize)> {
    if ctx.erc_diagnostics.is_empty() {
        return None;
    }
    let count = |severity| {
        ctx.erc_diagnostics
            .iter()
            .filter(|d| d.severity == severity)
            .count()
    };
    Some((
        count(ErcSeverityLite::Error),
        count(ErcSeverityLite::Warning),
    ))
}

fn counts_pill<'a>(ctx: &PanelContext, label: String, tone: PillTone) -> Element<'a, DockMessage> {
    container(text(label).size(TS.caption))
        .padding([1.0, 7.0])
        .style(pill(&ctx.tokens, tone))
        .boxed()
}

/// One line: a mono caption and its pills.
fn summary_line<'a>(
    ctx: &PanelContext,
    name: &'static str,
    pills: Vec<Element<'a, DockMessage>>,
) -> Element<'a, DockMessage> {
    let mut line = row![
        text(name)
            .size(TS.caption)
            .color(resolve(&ctx.tokens).ink3)
            .width(Length::Fixed(36.0))
            .boxed(),
    ]
    .spacing(6)
    .align_y(iced::Alignment::Center);
    for p in pills {
        line = line.push(p);
    }
    line.boxed()
}

/// The card, or `None` when no ERC counts exist.
pub(super) fn erc_drc_card<'a>(ctx: &PanelContext) -> Option<Element<'a, DockMessage>> {
    let (errors, warnings) = erc_counts(ctx)?;
    let r = resolve(&ctx.tokens);
    let erc = summary_line(
        ctx,
        "ERC",
        vec![
            counts_pill(ctx, format!("{errors} errors"), PillTone::Err),
            counts_pill(ctx, format!("{warnings} warnings"), PillTone::Warn),
        ],
    );
    // No DRC engine yet: a dash, never a made-up count.
    let drc = summary_line(
        ctx,
        "DRC",
        vec![text("\u{2014}").size(TS.caption).color(r.ink4).boxed()],
    );
    let (raised, line) = (r.raised, ti(ctx.tokens.border));
    Some(
        container(column![erc, drc].spacing(6).width(Length::Fill))
            .padding([8.0, 10.0])
            .width(Length::Fill)
            .style(move |_: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(raised)),
                border: iced::Border {
                    width: 1.0,
                    color: line,
                    radius: Radii::GRAPHITE.card.into(),
                    ..iced::Border::default()
                },
                ..container::Style::default()
            })
            .boxed(),
    )
}
