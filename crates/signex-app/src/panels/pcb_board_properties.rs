//! Properties panel of the active PCB: the board's own properties (size,
//! copper layers, thickness, nets) when nothing is selected, and a card
//! naming the board while board items are selected (no per-item view yet).
//! A pure view over data the panel context copies from the board.

use iced::widget::{Column, column, container, row};
use iced::{Element, Length, Widget as _};
use signex_i18n::{MessageId, UserText, ids, t};
use signex_types::pcb::PcbBoard;
use signex_widgets::{Label, text};

use super::{PanelContext, PanelMsg};
use crate::styles::{Spacing, TypeScale, ti, tokens::resolve};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The board facts the panel shows, all read from the board.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PcbBoardSummary {
    /// The document's name, as its tab shows it.
    pub name: String,
    /// Width and height of the outline's extent in mm; `None` without an
    /// outline.
    pub size_mm: Option<(f64, f64)>,
    pub copper_layers: usize,
    pub thickness_mm: f64,
    /// Board items selected in the main window.
    pub selected: usize,
}

impl PcbBoardSummary {
    pub fn of(board: &PcbBoard, name: String, selected: usize) -> Self {
        let size_mm = board.outline.split_first().map(|(first, rest)| {
            let (mut lo, mut hi) = ((first.x, first.y), (first.x, first.y));
            for p in rest {
                lo = (lo.0.min(p.x), lo.1.min(p.y));
                hi = (hi.0.max(p.x), hi.1.max(p.y));
            }
            (hi.0 - lo.0, hi.1 - lo.1)
        });
        Self {
            name,
            size_mm,
            copper_layers: signex_renderer::pcb::board_copper_layers(&board.layers).len(),
            thickness_mm: board.thickness,
            selected,
        }
    }
}

/// The panel body for `board`. The net count is the PCB panel's, so the two
/// always agree.
pub fn view<'a>(ctx: &'a PanelContext, board: &'a PcbBoardSummary) -> Element<'a, PanelMsg> {
    let tokens = &ctx.tokens;
    let i18n = &*ctx.i18n;
    if board.selected > 0 {
        return container(super::empty_state::empty_state_card(
            tokens,
            t!(i18n, ids::PCB_PROPERTIES_TITLE),
            t!(
                i18n, ids::PCB_PROPERTIES_SELECTION,
                "count" => board.selected, "board" => board.name.clone()
            ),
            Some(t!(i18n, ids::PCB_PROPERTIES_SELECTION_HINT)),
        ))
        .padding(SP.m)
        .width(Length::Fill)
        .boxed();
    }
    let r = resolve(tokens);
    let (ink, muted) = (ti(tokens.text), r.ink3);
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let semibold = iced::Font {
        weight: iced::font::Weight::Semibold,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    };
    let header = row![
        text(t!(i18n, ids::PCB_PROPERTIES_TITLE))
            .size(TS.body)
            .font(semibold)
            .color(ink),
        text(UserText::new(board.name.clone()))
            .size(TS.dense)
            .color(muted)
            .wrapping(iced::widget::text::Wrapping::None)
            .ellipsis(iced::widget::text::Ellipsis::End)
            .width(Length::Fill),
    ]
    .spacing(SP.m)
    .align_y(iced::Alignment::Center);
    let kv = |key: MessageId, value: Label<'a>| -> Element<'a, PanelMsg> {
        row![
            text(t!(i18n, key))
                .size(TS.body)
                .color(muted)
                .width(Length::FillPortion(2)),
            text(value)
                .size(TS.body)
                .font(mono)
                .color(ink)
                .width(Length::FillPortion(3)),
        ]
        .align_y(iced::Alignment::Center)
        .height(Length::Fixed(TS.body * 2.4))
        .boxed()
    };
    let size: Label<'a> = match board.size_mm {
        Some((w, h)) => t!(
            i18n, ids::PCB_PROPERTIES_SIZE_VALUE,
            "width" => format!("{w:.2}"), "height" => format!("{h:.2}")
        )
        .into(),
        None => t!(i18n, ids::PCB_PROPERTIES_NO_OUTLINE).into(),
    };
    let nets = ctx.pcb_nets.as_ref().map_or(0, |s| s.nets.len());
    let rows: Column<Element<'a, PanelMsg>> = column![
        super::widgets::section_title(
            &t!(i18n, ids::PCB_PROPERTIES_SECTION).into_string(),
            tokens,
            i18n
        ),
        kv(ids::PCB_PROPERTIES_SIZE, size),
        kv(
            ids::PCB_PROPERTIES_LAYERS,
            t!(i18n, ids::PCB_PROPERTIES_LAYERS_VALUE, "count" => board.copper_layers).into(),
        ),
        kv(
            ids::PCB_PROPERTIES_THICKNESS,
            t!(
                i18n, ids::PCB_PROPERTIES_THICKNESS_VALUE,
                "mm" => format!("{:.2}", board.thickness_mm)
            )
            .into(),
        ),
        kv(
            ids::PCB_PROPERTIES_NETS,
            UserText::new(nets.to_string()).into()
        ),
    ]
    .spacing(SP.xs);
    column![
        container(header).padding([10.0, 12.0]).width(Length::Fill),
        container(rows).padding([SP.s, 12.0]).width(Length::Fill),
    ]
    .width(Length::Fill)
    .boxed()
}

#[cfg(test)]
#[path = "pcb_board_properties_tests.rs"]
mod tests;
