//! PCB panel: the open board's nets as a table (net, nodes, routed length),
//! with the board's net and pad counts in the footer. A pure view over data
//! the panel context copies from the board; nothing here edits the board.

use iced::widget::{Column, Space, column, container, row, scrollable, text};
use iced::{Background, Element, Length, Theme, Widget as _};
use signex_types::pcb::PcbBoard;

use super::{PanelContext, PanelMsg};
use crate::styles::{Spacing, TypeScale, ti, tokens::resolve};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Height of one table row.
const ROW_H: f32 = TS.body * 2.4;
/// Width of the Nodes and Length columns.
const NUM_W: f32 = TS.body * 4.5;

/// One net row, all from the board.
#[derive(Debug, Clone, PartialEq)]
pub struct PcbNetRow {
    pub name: String,
    /// Pads on the net.
    pub nodes: usize,
    /// Total length of the net's track segments, in mm.
    pub length_mm: f64,
}

/// The board's nets and totals.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PcbNetsSummary {
    pub nets: Vec<PcbNetRow>,
    pub pads: usize,
}

impl PcbNetsSummary {
    /// Summarise `board`: pads per net number, segment length per net number.
    pub fn of(board: &PcbBoard) -> Self {
        let mut nodes = std::collections::HashMap::<u32, usize>::new();
        let mut pads = 0;
        for fp in &board.footprints {
            for pad in &fp.pads {
                pads += 1;
                if let Some(net) = &pad.net {
                    *nodes.entry(net.number).or_default() += 1;
                }
            }
        }
        let mut length = std::collections::HashMap::<u32, f64>::new();
        for seg in &board.segments {
            let (dx, dy) = (seg.end.x - seg.start.x, seg.end.y - seg.start.y);
            *length.entry(seg.net).or_default() += dx.hypot(dy);
        }
        // The board's net list, plus any net a pad names that the list lacks.
        let mut names = std::collections::BTreeMap::<u32, String>::new();
        for n in &board.nets {
            names.insert(n.number, n.name.clone());
        }
        for fp in &board.footprints {
            for pad in &fp.pads {
                if let Some(net) = &pad.net {
                    names.entry(net.number).or_insert_with(|| net.name.clone());
                }
            }
        }
        let nets = names
            .into_iter()
            .filter(|(_, name)| !name.is_empty())
            .map(|(number, name)| PcbNetRow {
                name,
                nodes: nodes.get(&number).copied().unwrap_or(0),
                length_mm: length.get(&number).copied().unwrap_or(0.0),
            })
            .collect();
        Self { nets, pads }
    }
}

/// Render the panel for the active PCB tab.
pub fn view<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let tokens = &ctx.tokens;
    let r = resolve(tokens);
    let (ink, muted) = (ti(tokens.text), r.ink3);
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let Some(summary) = ctx.pcb_nets.as_ref() else {
        return container(
            text("Open a `.snxpcb` to see its nets here.")
                .size(TS.caption)
                .color(muted),
        )
        .padding([6.0, SP.m])
        .width(Length::Fill)
        .boxed();
    };
    let rule = r.line_soft;
    let cell = |content: Element<'a, PanelMsg>, w: Length| {
        container(content)
            .width(w)
            .height(ROW_H)
            .align_y(iced::alignment::Vertical::Center)
            .clip(true)
            .boxed()
    };
    let head = |s: &'static str, w: Length| cell(text(s).size(TS.section).color(muted).boxed(), w);
    let separator = move || -> Element<'a, PanelMsg> {
        container(Space::new())
            .width(Length::Fill)
            .height(1)
            .style(move |_: &Theme| iced::widget::container::Style {
                background: Some(Background::Color(rule)),
                ..iced::widget::container::Style::default()
            })
            .boxed()
    };
    let mut table: Column<Element<'a, PanelMsg>> = column![
        row![
            head("NET", Length::Fill),
            head("NODES", Length::Fixed(NUM_W)),
            head("LENGTH", Length::Fixed(NUM_W)),
        ]
        .padding([0.0, SP.l]),
        separator(),
    ]
    .width(Length::Fill);
    if summary.nets.is_empty() {
        table = table.push(
            container(text("This board has no nets.").size(TS.body).color(muted))
                .padding([SP.m, SP.l])
                .boxed(),
        );
    }
    for net in &summary.nets {
        let length = if net.length_mm > 0.0 {
            format!("{:.2}", net.length_mm)
        } else {
            "\u{2014}".to_string()
        };
        table = table.push(
            row![
                cell(
                    text(net.name.as_str())
                        .size(TS.body)
                        .font(mono)
                        .color(ink)
                        .boxed(),
                    Length::Fill
                ),
                cell(
                    text(net.nodes.to_string())
                        .size(TS.body)
                        .font(mono)
                        .color(ink)
                        .boxed(),
                    Length::Fixed(NUM_W)
                ),
                cell(
                    text(length).size(TS.body).font(mono).color(ink).boxed(),
                    Length::Fixed(NUM_W)
                ),
            ]
            .padding([0.0, SP.l])
            .boxed(),
        );
        table = table.push(separator());
    }
    let footer = container(
        text(format!(
            "{} net{} \u{00B7} {} pad{}",
            summary.nets.len(),
            if summary.nets.len() == 1 { "" } else { "s" },
            summary.pads,
            if summary.pads == 1 { "" } else { "s" },
        ))
        .size(TS.caption)
        .color(muted),
    )
    .padding([SP.m, SP.l])
    .width(Length::Fill);
    column![
        scrollable(table)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(super::form_look::scrollbar_style),
        separator(),
        footer,
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .boxed()
}
