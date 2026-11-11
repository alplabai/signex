//! Properties of the one footprint selected on a board (#1312): its
//! designator and value, editable, and its footprint name. Typed text is a
//! draft until Enter, which applies it as one board undo step. A pure view
//! over facts the panel context copies from the board.

use std::path::PathBuf;

use iced::widget::{Column, column, container, row};
use iced::{Element, Length, Widget as _};
use signex_engine::pcb::designators::FootprintTextField;
use signex_i18n::{MessageId, UserText, ids, t};
use signex_widgets::{Label, text, text_input};
use uuid::Uuid;

use super::{PanelContext, PanelMsg};
use crate::styles::{PillTone, Spacing, TypeScale, ti, tokens::resolve};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Why the footprint's text cannot be edited, if it cannot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FootprintEditability {
    Editable,
    Locked,
    /// The board opened read-only (a newer format).
    ReadOnly,
}

/// The selected footprint as the panel shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct PcbFootprintFacts {
    pub board: PathBuf,
    pub uuid: Uuid,
    pub designator: String,
    pub value: String,
    /// The library footprint's name (`footprint_id`); may be empty.
    pub footprint_name: String,
    /// Text typed into the Designator field and not yet applied.
    pub designator_draft: Option<String>,
    /// Text typed into the Value field and not yet applied.
    pub value_draft: Option<String>,
    pub editability: FootprintEditability,
    /// How many other footprints carry the same designator.
    pub also_on: usize,
}

/// A draft or an Enter in the selected footprint's Designator or Value
/// field. Both name the board and footprint they were typed for, so a
/// selection change in between cannot send them to another footprint.
#[derive(Debug, Clone, PartialEq)]
pub enum PcbFootprintTextMsg {
    Typed {
        board: PathBuf,
        footprint: Uuid,
        field: FootprintTextField,
        text: String,
    },
    Submitted {
        board: PathBuf,
        footprint: Uuid,
        field: FootprintTextField,
    },
}

/// The footprint view: header, then the Designator, Value and Footprint
/// rows, then why the fields are read-only or what to know about them.
pub fn view<'a>(ctx: &'a PanelContext, fp: &'a PcbFootprintFacts) -> Element<'a, PanelMsg> {
    let tokens = &ctx.tokens;
    let i18n = &*ctx.i18n;
    let r = resolve(tokens);
    let (ink, muted) = (ti(tokens.text), r.ink3);
    let semibold = iced::Font {
        weight: iced::font::Weight::Semibold,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    };
    let header = row![
        text(t!(i18n, ids::PCB_FOOTPRINT_TITLE))
            .size(TS.body)
            .font(semibold)
            .color(ink),
        text(UserText::new(fp.designator.clone()))
            .size(TS.dense)
            .color(muted)
            .wrapping(iced::widget::text::Wrapping::None)
            .ellipsis(iced::widget::text::Ellipsis::End)
            .width(Length::Fill),
    ]
    .spacing(SP.m)
    .align_y(iced::Alignment::Center);
    let label = |key: MessageId| {
        text(t!(i18n, key))
            .size(TS.body)
            .color(muted)
            .width(Length::FillPortion(2))
    };
    let field_row = |key: MessageId, field: FootprintTextField, stored: &str| {
        let typed = match field {
            FootprintTextField::Designator => fp.designator_draft.as_ref(),
            FootprintTextField::Value => fp.value_draft.as_ref(),
        };
        let shown = typed.cloned().unwrap_or_else(|| stored.to_string());
        let mut input = text_input(UserText::new(String::new()), shown)
            .size(TS.body)
            .padding(SP.s)
            .style(super::form_look::text_input_style)
            .width(Length::FillPortion(3));
        if fp.editability == FootprintEditability::Editable {
            let (board, footprint) = (fp.board.clone(), fp.uuid);
            let submitted = PcbFootprintTextMsg::Submitted {
                board: board.clone(),
                footprint,
                field,
            };
            input = input
                .on_input(move |text| {
                    PanelMsg::PcbFootprintText(PcbFootprintTextMsg::Typed {
                        board: board.clone(),
                        footprint,
                        field,
                        text,
                    })
                })
                .on_submit(PanelMsg::PcbFootprintText(submitted));
        }
        row![label(key), input]
            .align_y(iced::Alignment::Center)
            .height(Length::Fixed(TS.body * 2.4))
            .boxed()
    };
    let name: Label<'a> = if fp.footprint_name.is_empty() {
        t!(i18n, ids::PCB_FOOTPRINT_NO_NAME).into()
    } else {
        UserText::new(fp.footprint_name.clone()).into()
    };
    let mut rows: Column<Element<'a, PanelMsg>> = column![
        super::widgets::section_title(
            &t!(i18n, ids::PCB_PROPERTIES_SECTION).into_string(),
            tokens,
            i18n
        ),
        field_row(
            ids::PCB_FOOTPRINT_DESIGNATOR,
            FootprintTextField::Designator,
            &fp.designator
        ),
        field_row(
            ids::PCB_FOOTPRINT_VALUE,
            FootprintTextField::Value,
            &fp.value
        ),
        row![
            label(ids::PCB_FOOTPRINT_NAME),
            text(name)
                .size(TS.body)
                .color(ink)
                .width(Length::FillPortion(3)),
        ]
        .align_y(iced::Alignment::Center)
        .height(Length::Fixed(TS.body * 2.4))
        .boxed(),
    ]
    .spacing(SP.xs);
    let warn = crate::styles::pill_colors(tokens, PillTone::Warn).1;
    for (line, color) in notes(fp, i18n, muted, warn) {
        rows = rows.push(text(line).size(TS.dense).color(color).boxed());
    }
    column![
        container(header).padding([10.0, 12.0]).width(Length::Fill),
        container(rows).padding([SP.s, 12.0]).width(Length::Fill),
    ]
    .width(Length::Fill)
    .boxed()
}

/// The lines under the rows: why the fields are read-only, a designator
/// other footprints share, and how to apply a draft.
fn notes(
    fp: &PcbFootprintFacts,
    i18n: &signex_i18n::I18n,
    muted: iced::Color,
    warn: iced::Color,
) -> Vec<(signex_i18n::Localized, iced::Color)> {
    let mut lines = Vec::new();
    match fp.editability {
        FootprintEditability::Editable => {
            lines.push((t!(i18n, ids::PCB_DESIGNATOR_UPDATE_PCB_NOTE), muted));
        }
        FootprintEditability::Locked => {
            lines.push((t!(i18n, ids::PCB_FOOTPRINT_LOCKED), muted));
        }
        FootprintEditability::ReadOnly => {
            lines.push((t!(i18n, ids::PCB_FOOTPRINT_READ_ONLY), muted));
        }
    }
    if fp.also_on > 0 {
        lines.push((
            t!(
                i18n, ids::PCB_FOOTPRINT_DUPLICATE,
                "designator" => fp.designator.as_str(), "count" => fp.also_on
            ),
            warn,
        ));
    }
    if fp.designator_draft.is_some() || fp.value_draft.is_some() {
        lines.push((t!(i18n, ids::PCB_FOOTPRINT_ENTER_HINT), muted));
    }
    lines
}
