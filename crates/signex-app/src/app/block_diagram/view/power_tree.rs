//! The Power tree view (mockup row 171, #1313): one row per rail, its
//! sources (a converter or a source part) on the left, the rail in the
//! middle and its loads on the right, each with a budget bar. A source's
//! bar is the rail's draw against its limit, green under 50 %, amber to
//! 90 %, red above; a load's bar is its share of the rail in one neutral
//! colour, so it never reads as a warning. The side panel lists the rails
//! and shows the picked one. A pure view over the last run.

use iced::widget::{Column, Row, Space, column, container, row, scrollable};
use iced::{Alignment, Background, Border, Color, Element, Length, Theme, Widget as _};
use signex_i18n::{Localized, MessageId, UserText, ids, t};
use signex_net::power_tree::{BudgetLevel, PowerRail, PowerTopology, scenario_summary};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, text};

use super::{Look, SP, TS, caption};
use crate::app::block_diagram::OpenDiagram;
use crate::app::block_diagram::TabEvent;
use crate::app::block_diagram::state::PowerTreeRead;
use crate::styles::tokens::resolve;
use crate::styles::{Radii, ti};

/// A card's width, in body-text ems (mockup: 170 px loads, 210 px sources).
const CARD_EMS: f32 = 14.0;
/// A budget bar's height, in body-text ems (mockup: 6 px).
const BAR_EMS: f32 = 0.45;
/// Bar shares are drawn in thousandths of the bar.
const BAR_STEPS: f64 = 1000.0;

type Items<'a> = Column<Element<'a, TabEvent>>;

/// Amps with two decimals, the mockup's "0.64 A".
fn amps(value: f64) -> String {
    format!("{value:.2}")
}

fn percent(share: f64) -> String {
    format!("{:.0}", share * 100.0)
}

fn note<'a>(look: Look<'a>, line: Localized) -> Element<'a, TabEvent> {
    text(line)
        .size(TS.dense)
        .color(ti(look.tokens.text_secondary))
        .boxed()
}

fn strong<'a>(
    look: Look<'a>,
    label: impl Into<signex_widgets::Label<'a>>,
) -> Element<'a, TabEvent> {
    text(label)
        .size(TS.body)
        .color(ti(look.tokens.text))
        .boxed()
}

/// A bar `share` full (0..=1) in `ink`, on the sunken track.
fn bar<'a>(look: Look<'a>, share: f64, ink: Color) -> Element<'a, TabEvent> {
    let track = resolve(look.tokens).sunken;
    let full = (share.clamp(0.0, 1.0) * BAR_STEPS).round() as u16;
    let fill = |colour: Color| {
        move |_: &Theme| container::Style {
            background: Some(Background::Color(colour)),
            border: Border {
                radius: Radii::GRAPHITE.button.into(),
                ..Border::default()
            },
            ..container::Style::default()
        }
    };
    let height = Length::Fixed(TS.body * BAR_EMS);
    let mut line: Row<Element<'a, TabEvent>> = Row::new().height(height);
    if full > 0 {
        line = line.push(
            container(Space::new())
                .width(Length::FillPortion(full))
                .height(height)
                .style(fill(ink))
                .boxed(),
        );
    }
    let rest = BAR_STEPS as u16 - full;
    if rest > 0 {
        line = line.push(
            container(Space::new())
                .width(Length::FillPortion(rest))
                .height(height)
                .style(fill(track))
                .boxed(),
        );
    }
    line.width(Length::Fill).boxed()
}

/// The colour of a budget at `level`.
fn level_ink(look: Look<'_>, level: BudgetLevel) -> Color {
    match level {
        BudgetLevel::Fine => ti(look.tokens.success),
        BudgetLevel::Close => ti(look.tokens.warning),
        BudgetLevel::Over => ti(look.tokens.error),
    }
}

/// A load's bar, as mockup row 171 draws it: the accent under half the
/// rail, the warning tone to 90 %, the error tone above.
pub(super) fn load_ink(tokens: &ThemeTokens, share: f64) -> Color {
    match BudgetLevel::of(share, 1.0) {
        Some(BudgetLevel::Close) => ti(tokens.warning),
        Some(BudgetLevel::Over) => ti(tokens.error),
        Some(BudgetLevel::Fine) | None => ti(tokens.accent),
    }
}

/// A source or load card that opens its part's sheet when clicked
/// (#1313).
fn part_card<'a>(look: Look<'a>, body: Items<'a>, part: usize) -> Element<'a, TabEvent> {
    button(card(look, body, false))
        .padding(0)
        .style(crate::styles::ghost_button(look.tokens))
        .on_press(TabEvent::PartOpened(part))
        .boxed()
}

/// The totals line (mockup row 171): the scenario, the sources active in it
/// and the rail with the least headroom.
fn scenario_line(look: Look<'_>, read: &PowerTreeRead) -> Localized {
    let summary = scenario_summary(&read.tree);
    match summary.worst_rail {
        Some((rail, share)) => t!(look.i18n, ids::BLOCK_DIAGRAM_POWER_SCENARIO,
            "active" => summary.active_sources,
            "sources" => summary.sources,
            "headroom" => percent(share),
            "rail" => rail),
        None => t!(look.i18n, ids::BLOCK_DIAGRAM_POWER_SCENARIO_NO_LIMIT,
            "active" => summary.active_sources,
            "sources" => summary.sources),
    }
}

fn card<'a>(look: Look<'a>, body: Items<'a>, picked: bool) -> Element<'a, TabEvent> {
    let r = resolve(look.tokens);
    let edge = if picked {
        ti(look.tokens.accent)
    } else {
        ti(look.tokens.border)
    };
    let fill = r.raised;
    container(body.spacing(SP.xs))
        .padding(SP.s)
        .width(Length::Fixed(TS.body * CARD_EMS))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                color: edge,
                width: 1.0,
                radius: Radii::GRAPHITE.button.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// "U1 TPS54331": a part's designator and value.
fn part_name(topology: &PowerTopology, at: usize) -> String {
    topology.parts.get(at).map_or_else(String::new, |part| {
        format!("{} {}", part.designator, part.value)
            .trim()
            .to_string()
    })
}

fn voltage_line(look: Look<'_>, rail: &PowerRail) -> Localized {
    match rail.voltage {
        Some(v) => match v.tolerance_percent {
            Some(tol) => t!(look.i18n, ids::BLOCK_DIAGRAM_POWER_VOLTAGE_TOL,
                "volts" => format!("{:.2}", v.volts), "tolerance" => format!("{tol}")),
            None => t!(look.i18n, ids::BLOCK_DIAGRAM_POWER_VOLTAGE,
                "volts" => format!("{:.2}", v.volts)),
        },
        None => look.t(ids::BLOCK_DIAGRAM_POWER_NO_VOLTAGE),
    }
}

/// The sources of `rail`: kind, part, and the rail's draw against the
/// source's limit.
fn sources<'a>(look: Look<'a>, read: &'a PowerTreeRead, rail: &'a PowerRail) -> Items<'a> {
    let mut list: Items<'a> = Column::new().spacing(SP.s);
    for source in &rail.sources {
        let kind = if source.is_converter {
            ids::BLOCK_DIAGRAM_POWER_CONVERTER
        } else {
            ids::BLOCK_DIAGRAM_POWER_SOURCE
        };
        let mut body: Items<'a> = column![
            strong(look, UserText::new(part_name(&read.topology, source.part))),
            note(look, look.t(kind)),
        ];
        if let Some(limit) = source.limit_a.filter(|l| *l > 0.0) {
            let share = rail.total_a / limit;
            let ink = BudgetLevel::of(rail.total_a, limit)
                .map_or(ti(look.tokens.text_secondary), |level| {
                    level_ink(look, level)
                });
            body = body.push(bar(look, share, ink)).push(note(
                look,
                t!(look.i18n, ids::BLOCK_DIAGRAM_POWER_OF_LIMIT,
                    "used" => amps(rail.total_a), "limit" => amps(limit), "share" => percent(share)),
            ));
        }
        list = list.push(part_card(look, body, source.part));
    }
    if rail.sources.is_empty() {
        list = list.push(note(look, look.t(ids::BLOCK_DIAGRAM_POWER_NO_SOURCE)));
    }
    list
}

fn rail_card<'a>(look: Look<'a>, rail: &'a PowerRail, picked: bool) -> Element<'a, TabEvent> {
    let net = text(UserText::new(rail.net.clone()))
        .size(TS.body)
        .font(iced::Font::MONOSPACE)
        .color(ti(look.tokens.error))
        .boxed();
    let total = t!(look.i18n, ids::BLOCK_DIAGRAM_POWER_TOTAL, "amps" => amps(rail.total_a));
    let body = column![
        net,
        note(look, voltage_line(look, rail)),
        strong(look, total),
        note(
            look,
            t!(look.i18n, ids::BLOCK_DIAGRAM_POWER_LOADS, "count" => rail.loads.len())
        ),
    ];
    let face = card(look, body, picked);
    button(face)
        .padding(0)
        .style(crate::styles::ghost_button(look.tokens))
        .on_press(TabEvent::RailPicked(rail.net.clone()))
        .boxed()
}

fn loads<'a>(look: Look<'a>, read: &'a PowerTreeRead, rail: &'a PowerRail) -> Items<'a> {
    let mut list: Items<'a> = Column::new().spacing(SP.s);
    for load in &rail.loads {
        let mut body: Items<'a> = column![strong(
            look,
            UserText::new(part_name(&read.topology, load.part))
        )];
        match load.current_a {
            Some(current) => {
                let share = (rail.total_a > 0.0).then(|| current / rail.total_a);
                body = body.push(note(
                    look,
                    t!(look.i18n, ids::BLOCK_DIAGRAM_POWER_DRAW, "amps" => amps(current)),
                ));
                if let Some(share) = share {
                    let ink = load_ink(look.tokens, share);
                    body = body.push(bar(look, share, ink)).push(note(
                        look,
                        t!(look.i18n, ids::BLOCK_DIAGRAM_POWER_SHARE, "share" => percent(share)),
                    ));
                }
            }
            None => body = body.push(note(look, look.t(ids::BLOCK_DIAGRAM_POWER_UNKNOWN_DRAW))),
        }
        list = list.push(part_card(look, body, load.part));
    }
    list
}

fn arrow<'a>(look: Look<'a>) -> Element<'a, TabEvent> {
    text(look.t(ids::BLOCK_DIAGRAM_POWER_ARROW))
        .size(TS.body)
        .color(ti(look.tokens.error))
        .boxed()
}

/// The canvas area: a row per rail, or why there is none.
pub(super) fn main<'a>(look: Look<'a>, editor: OpenDiagram<'a>) -> Element<'a, TabEvent> {
    let power = &editor.tab.power;
    let body: Element<'a, TabEvent> = match &power.read {
        None => note(look, look.t(ids::BLOCK_DIAGRAM_POWER_READING)),
        Some(Err(reason)) => note(
            look,
            t!(look.i18n, ids::BLOCK_DIAGRAM_POWER_UNREAD, "reason" => reason.as_str()),
        ),
        Some(Ok(read)) if read.tree.rails.is_empty() => {
            note(look, look.t(ids::BLOCK_DIAGRAM_POWER_EMPTY))
        }
        Some(Ok(read)) => {
            let mut rows: Items<'a> =
                column![strong(look, scenario_line(look, read))].spacing(SP.l);
            for rail in &read.tree.rails {
                let picked = power.rail.as_deref() == Some(rail.net.as_str());
                rows = rows.push(
                    row![
                        sources(look, read, rail),
                        arrow(look),
                        rail_card(look, rail, picked),
                        arrow(look),
                        loads(look, read, rail),
                    ]
                    .spacing(SP.m)
                    .align_y(Alignment::Center)
                    .boxed(),
                );
            }
            rows = rows.push(note(look, look.t(ids::BLOCK_DIAGRAM_POWER_LEGEND)));
            scrollable(rows.padding(SP.l)).height(Length::Fill).boxed()
        }
    };
    container(body)
        .padding(SP.s)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(crate::styles::panel_region(look.tokens))
        .boxed()
}

/// A name on the left, a value on the right.
fn fact<'a>(
    look: Look<'a>,
    name: MessageId,
    value: impl Into<signex_widgets::Label<'a>>,
) -> Element<'a, TabEvent> {
    row![
        text(look.t(name))
            .size(TS.dense)
            .color(resolve(look.tokens).ink3)
            .width(Length::FillPortion(2))
            .boxed(),
        text(value)
            .size(TS.dense)
            .color(ti(look.tokens.text))
            .width(Length::FillPortion(3))
            .boxed(),
    ]
    .spacing(SP.s)
    .boxed()
}

/// The picked rail's facts (mockup: Rail, Nominal, Source, Limit, Load,
/// Headroom).
fn rail_facts<'a>(look: Look<'a>, read: &'a PowerTreeRead, rail: &'a PowerRail) -> Items<'a> {
    let unknown = || look.t(ids::BLOCK_DIAGRAM_POWER_NOT_KNOWN);
    let source: signex_widgets::Label<'a> = match rail.sources.as_slice() {
        [one] => UserText::new(part_name(&read.topology, one.part)).into(),
        _ => unknown().into(),
    };
    let limit = rail.limit_a().map_or_else(
        unknown,
        |limit| t!(look.i18n, ids::BLOCK_DIAGRAM_POWER_DRAW, "amps" => amps(limit)),
    );
    let headroom = match (rail.headroom_a(), rail.limit_a()) {
        (Some(left), Some(limit)) if limit > 0.0 => {
            t!(look.i18n, ids::BLOCK_DIAGRAM_POWER_FREE, "share" => percent(left / limit))
        }
        _ => unknown(),
    };
    column![
        fact(
            look,
            ids::BLOCK_DIAGRAM_POWER_RAIL,
            UserText::new(rail.net.clone())
        ),
        fact(
            look,
            ids::BLOCK_DIAGRAM_POWER_NOMINAL,
            voltage_line(look, rail)
        ),
        fact(look, ids::BLOCK_DIAGRAM_POWER_SOURCE, source),
        fact(look, ids::BLOCK_DIAGRAM_POWER_LIMIT, limit),
        fact(
            look,
            ids::BLOCK_DIAGRAM_POWER_LOAD,
            t!(look.i18n, ids::BLOCK_DIAGRAM_POWER_TOTAL,
            "amps" => amps(rail.total_a))
        ),
        fact(look, ids::BLOCK_DIAGRAM_POWER_HEADROOM, headroom),
    ]
    .spacing(SP.xs)
}

/// The side panel: the rails, then the picked rail's facts.
pub(super) fn side<'a>(look: Look<'a>, editor: OpenDiagram<'a>) -> Element<'a, TabEvent> {
    let mut list: Items<'a> = column![caption(look, ids::BLOCK_DIAGRAM_POWER_RAILS)].spacing(SP.xs);
    if let Some(Ok(read)) = &editor.tab.power.read {
        for rail in &read.tree.rails {
            let picked = editor.tab.power.rail.as_deref() == Some(rail.net.as_str());
            let ink = if picked {
                ti(look.tokens.accent)
            } else {
                ti(look.tokens.text)
            };
            let label = row![
                text(UserText::new(rail.net.clone()))
                    .size(TS.dense)
                    .font(iced::Font::MONOSPACE)
                    .color(ink)
                    .width(Length::Fill),
                text(voltage_line(look, rail))
                    .size(TS.dense)
                    .color(resolve(look.tokens).ink3),
            ]
            .spacing(SP.s);
            list = list.push(
                button(label)
                    .padding([SP.xs, SP.s])
                    .width(Length::Fill)
                    .style(crate::styles::ghost_button(look.tokens))
                    .on_press(TabEvent::RailPicked(rail.net.clone()))
                    .boxed(),
            );
        }
        let picked = editor
            .tab
            .power
            .rail
            .as_deref()
            .and_then(|net| read.tree.rail(net));
        if let Some(rail) = picked {
            list = list
                .push(caption(look, ids::BLOCK_DIAGRAM_POWER_RAIL))
                .push(rail_facts(look, read, rail).boxed());
        }
        for warning in &read.warnings {
            list = list.push(note(
                look,
                t!(look.i18n, ids::BLOCK_DIAGRAM_POWER_WARNING, "text" => warning.as_str()),
            ));
        }
    }
    container(scrollable(list.padding(SP.s)).height(Length::Fill))
        .height(Length::Fill)
        .style(crate::styles::panel_region(look.tokens))
        .boxed()
}

/// The footer's count: "3 rails · 1 converter".
pub(super) fn status(look: Look<'_>, editor: OpenDiagram<'_>) -> Localized {
    match &editor.tab.power.read {
        Some(Ok(read)) => t!(look.i18n, ids::BLOCK_DIAGRAM_POWER_FOOTER,
            "rails" => read.tree.rails.len(),
            "converters" => read.tree.converters.len()),
        _ => look.t(ids::BLOCK_DIAGRAM_POWER_READING),
    }
}

#[cfg(test)]
mod tests {
    use signex_types::theme::{ThemeId, theme_tokens};

    use super::*;

    /// Mockup row 171's load bars: J2 at 78 % of the rail in the warning
    /// tone, U3 at 22 % in the accent, a load over 90 % in the error tone.
    #[test]
    fn a_load_bar_takes_its_colour_from_its_share_of_the_rail() {
        for theme in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
            let tokens = theme_tokens(theme);
            assert_eq!(load_ink(&tokens, 0.22), ti(tokens.accent), "{theme:?}");
            assert_eq!(load_ink(&tokens, 0.78), ti(tokens.warning), "{theme:?}");
            assert_eq!(load_ink(&tokens, 0.95), ti(tokens.error), "{theme:?}");
            assert_ne!(load_ink(&tokens, 0.22), load_ink(&tokens, 0.78));
        }
    }
}
