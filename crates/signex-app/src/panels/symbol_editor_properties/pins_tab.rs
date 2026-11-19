//! The Pins tab (#122): the open symbol's pins as a compact list, with a
//! filter by number or name, part chips and `+ Pin`. A row press selects
//! the pin, so Properties shows the Pin properties and the canvas the pin.
//! A duplicate number marks both rows, an empty name is flagged.

use iced::widget::{Column, Row, Space, column, container, row};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, UserText, ids, t};
use signex_widgets::chips::{ChipMetrics, ChipStyle, chip};
use signex_widgets::{button, text, text_input};

use super::super::{PanelMsg, SymbolEditorPanelContext, SymbolPinSummary, form_look};
use super::rows::{Form, SP, TS, mono, props};
use crate::library::editor::pin_labels::{direction_by_name, electrical_message, part_label};
use crate::library::editor::symbol::{PartChip, SymbolPropsMsg};
use crate::styles::tokens::resolve;
use crate::styles::{HAIRLINE, PillTone, pill, ti};
use signex_types::schematic::PinDirection;

// The mockup's row grid at a 12 px body, as ratios of the body size:
// number 28, name the rest, type 92, part 26; rows 26 tall, header 22.
const NUMBER_EM: f32 = 28.0 / 12.0;
const TYPE_EM: f32 = 92.0 / 12.0;
const PART_EM: f32 = 26.0 / 12.0;
const ROW_EM: f32 = 26.0 / 12.0;
const HEAD_EM: f32 = 22.0 / 12.0;
/// Alpha of a hidden pin's name.
const HIDDEN_ALPHA: f32 = 0.55;

/// What is wrong with one pin's row.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct PinIssue {
    /// The number is empty or another pin has it too.
    pub bad_number: bool,
    /// The name is empty.
    pub no_name: bool,
}

impl PinIssue {
    pub fn any(self) -> bool {
        self.bad_number || self.no_name
    }
}

/// The issues of every pin, by index into `pins`.
pub(super) fn pin_issues(pins: &[SymbolPinSummary]) -> Vec<PinIssue> {
    let mut seen: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for p in pins {
        let number = p.number.trim();
        if !number.is_empty() {
            *seen.entry(number).or_default() += 1;
        }
    }
    pins.iter()
        .map(|p| {
            let number = p.number.trim();
            PinIssue {
                bad_number: number.is_empty() || seen.get(number).is_some_and(|n| *n > 1),
                no_name: p.name.trim().is_empty(),
            }
        })
        .collect()
}

/// The part chips: All, then each part the pins use, lowest first, with
/// their pin counts.
pub(super) fn part_chips(pins: &[SymbolPinSummary]) -> Vec<(PartChip, usize)> {
    let mut parts: std::collections::BTreeMap<u8, usize> = std::collections::BTreeMap::new();
    for p in pins {
        *parts.entry(p.details.part_number).or_default() += 1;
    }
    std::iter::once((PartChip::All, pins.len()))
        .chain(parts.into_iter().map(|(n, c)| (PartChip::Part(n), c)))
        .collect()
}

/// The chip in effect: `chip`, or All when no pin is on its part any more.
pub(super) fn effective_chip(pins: &[SymbolPinSummary], chip: PartChip) -> PartChip {
    match chip {
        PartChip::Part(n) if !pins.iter().any(|p| p.details.part_number == n) => PartChip::All,
        other => other,
    }
}

/// `text` folded for matching: the UI language's case rules, and the
/// typographic minus and en dash read as `-`.
fn fold(i18n: &I18n, text: &str) -> String {
    i18n.fold_for_search(&text.replace(['\u{2212}', '\u{2013}'], "-"))
}

/// The pins the list shows: those on the chip's part whose number or name
/// holds the filter.
pub(super) fn listed_pins<'p>(
    pins: &'p [SymbolPinSummary],
    chip: PartChip,
    filter: &str,
    i18n: &I18n,
) -> Vec<&'p SymbolPinSummary> {
    let chip = effective_chip(pins, chip);
    let needle = fold(i18n, filter.trim());
    pins.iter()
        .filter(|p| match chip {
            PartChip::All => true,
            PartChip::Part(n) => p.details.part_number == n,
        })
        .filter(|p| {
            needle.is_empty() || fold(i18n, &format!("{} {}", p.number, p.name)).contains(&needle)
        })
        .collect()
}

/// The tone of a pin's type pill: inputs info, outputs ok, bidirectional
/// accent, power inputs err, the rest neutral.
pub(super) fn type_tone(direction: Option<PinDirection>) -> PillTone {
    match direction {
        Some(PinDirection::Input) => PillTone::Info,
        Some(PinDirection::Output) => PillTone::Ok,
        Some(PinDirection::Bidirectional) => PillTone::Accent,
        Some(PinDirection::PowerInput) => PillTone::Err,
        _ => PillTone::Neutral,
    }
}

pub(super) fn view<'a>(
    sym: &'a SymbolEditorPanelContext,
    form: &Form<'_>,
) -> Vec<Element<'a, PanelMsg>> {
    let i18n = form.i18n;
    let ui = &sym.props;
    let issues = pin_issues(&sym.pins);
    let listed = listed_pins(&sym.pins, ui.part_chip, &ui.pin_filter, i18n);

    let filter = text_input(
        t!(i18n, ids::EDITORS_SYMPROPS_PIN_FILTER),
        ui.pin_filter.as_str(),
    )
    .on_input(|q| props(SymbolPropsMsg::PinFilterChanged(q)))
    .size(TS.body)
    .padding([SP.xs, SP.s])
    .style(form_look::text_input_style)
    .width(Length::Fill);
    let add = form.ghost(
        t!(i18n, ids::EDITORS_SYMPROPS_ADD_PIN),
        Some(props(SymbolPropsMsg::AddPinPressed)),
        Some(t!(i18n, ids::EDITORS_SYMPROPS_ADD_PIN_TIP)),
    );
    // Until the Pin table document tab (#122b) lands, the table shows here.
    let table_toggle = form.ghost(
        t!(
            i18n,
            if ui.pin_table {
                ids::EDITORS_SYMPROPS_PIN_TABLE_CLOSE
            } else {
                ids::EDITORS_SYMPROPS_PIN_TABLE_OPEN
            }
        ),
        (!sym.pins.is_empty()).then_some(props(SymbolPropsMsg::PinTableToggled)),
        None,
    );
    let tools = container(
        column![
            row![filter, add].spacing(SP.s).align_y(Alignment::Center),
            table_toggle,
        ]
        .spacing(SP.xs),
    )
    .padding(iced::Padding {
        top: SP.m,
        right: SP.m,
        bottom: SP.xs,
        left: SP.m,
    })
    .width(Length::Fill);

    let mut col: Vec<Element<'a, PanelMsg>> =
        vec![tools.boxed(), chips(sym, form), summary(sym, &issues, form)];
    if ui.pin_table && !listed.is_empty() {
        col.push(super::pin_table::view(&listed, form));
        return col;
    }
    col.push(header(form));
    if listed.is_empty() {
        col.push(empty(sym.pins.is_empty(), form));
    }
    for pin in listed {
        let issue = issues.get(pin.idx).copied().unwrap_or_default();
        col.push(pin_row(pin, issue, form));
    }
    col
}

fn chips<'a>(sym: &SymbolEditorPanelContext, form: &Form<'_>) -> Element<'a, PanelMsg> {
    let i18n = form.i18n;
    let t_ = form.tokens;
    let r = resolve(t_);
    let style = ChipStyle {
        fill: r.field,
        edge: ti(t_.border),
        hover: ti(t_.hover),
        ink: r.ink3,
        on_fill: r.accent_soft,
        on_ink: r.accent_soft_ink,
    };
    let metrics = ChipMetrics::from_text_size(TS.body);
    let active = effective_chip(&sym.pins, sym.props.part_chip);
    let mut wrap: Row<Element<'a, PanelMsg>> = Row::new().spacing(metrics.row_gap);
    for (value, count) in part_chips(&sym.pins) {
        let label = match value {
            PartChip::All => t!(i18n, ids::EDITORS_SYMPROPS_CHIP_ALL),
            PartChip::Part(n) => {
                t!(i18n, ids::EDITORS_SYMPROPS_PART_NUMBERED, "part" => u32::from(n))
            }
        };
        wrap = wrap.push(chip(
            label,
            Some(u32::try_from(count).unwrap_or(u32::MAX)),
            value == active,
            style,
            metrics,
            i18n,
            Some(props(SymbolPropsMsg::PartChipPicked(value))),
        ));
    }
    container(wrap.wrap().vertical_spacing(metrics.row_gap))
        .padding([SP.xs, SP.m])
        .width(Length::Fill)
        .boxed()
}

/// "8 pins · 0 hidden · 3 parts", and how many rows have issues.
fn summary<'a>(
    sym: &SymbolEditorPanelContext,
    issues: &[PinIssue],
    form: &Form<'_>,
) -> Element<'a, PanelMsg> {
    let i18n = form.i18n;
    let hidden = sym.pins.iter().filter(|p| p.details.hidden).count();
    let parts = part_chips(&sym.pins).len() - 1;
    let mut line: Row<Element<'a, PanelMsg>> = Row::new().spacing(SP.xs).push(
        text(t!(
            i18n,
            ids::EDITORS_PINS_SUMMARY,
            "pins" => sym.pins.len(),
            "hidden" => hidden,
            "parts" => parts
        ))
        .size(TS.caption)
        .color(form.look.ink4)
        .boxed(),
    );
    let bad = issues.iter().filter(|i| i.any()).count();
    if bad > 0 {
        line = line.push(
            text(t!(i18n, ids::EDITORS_SYMPROPS_PINS_ISSUES, "count" => bad))
                .size(TS.caption)
                .color(form.look.err)
                .boxed(),
        );
    }
    container(line)
        .padding(iced::Padding {
            top: 0.0,
            right: SP.m,
            bottom: SP.xs,
            left: SP.m,
        })
        .width(Length::Fill)
        .boxed()
}

/// The four cells of a header or pin row on the mockup's grid.
fn cells<'a>(
    number: Element<'a, PanelMsg>,
    name: Element<'a, PanelMsg>,
    kind: Element<'a, PanelMsg>,
    part: Element<'a, PanelMsg>,
    height: f32,
) -> Element<'a, PanelMsg> {
    container(
        row![
            container(number).width(TS.body * NUMBER_EM).clip(true),
            container(name).width(Length::Fill).clip(true),
            container(kind).width(TS.body * TYPE_EM).clip(true),
            container(part).width(TS.body * PART_EM).clip(true),
        ]
        .spacing(SP.s)
        .align_y(Alignment::Center),
    )
    .height(height)
    .padding([0.0, SP.m])
    .align_y(iced::alignment::Vertical::Center)
    .width(Length::Fill)
    .boxed()
}

fn rule<'a>(form: &Form<'_>) -> Element<'a, PanelMsg> {
    let line = form.look.line_soft;
    container(Space::new())
        .height(HAIRLINE)
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(line)),
            ..container::Style::default()
        })
        .boxed()
}

/// `#  NAME  TYPE  PT` in caption capitals on the sunken band.
fn header<'a>(form: &Form<'_>) -> Element<'a, PanelMsg> {
    let i18n = form.i18n;
    let ink = form.look.ink3;
    let head = |id: signex_i18n::MessageId| text(t!(i18n, id)).size(TS.caption).color(ink).boxed();
    let band = form.look.sunken;
    let row = cells(
        head(ids::EDITORS_PINS_COLUMN_NUMBER),
        head(ids::EDITORS_PINS_COLUMN_NAME),
        head(ids::EDITORS_SYMPROPS_COLUMN_TYPE),
        head(ids::EDITORS_SYMPROPS_COLUMN_PART),
        TS.body * HEAD_EM,
    );
    column![
        rule(form),
        container(row)
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(band)),
                ..container::Style::default()
            }),
        rule(form),
    ]
    .boxed()
}

/// One pin; a press selects it.
fn pin_row<'a>(pin: &SymbolPinSummary, issue: PinIssue, form: &Form<'_>) -> Element<'a, PanelMsg> {
    let i18n = form.i18n;
    let look = form.look;
    let number = if pin.number.trim().is_empty() {
        text(t!(i18n, ids::EDITORS_SYMPROPS_NO_NUMBER))
    } else {
        text(UserText::new(pin.number.as_str()))
    }
    .size(TS.body)
    .font(mono())
    .color(if issue.bad_number {
        look.err
    } else {
        look.ink3
    })
    .wrapping(iced::widget::text::Wrapping::None);
    let name = if issue.no_name {
        text(t!(i18n, ids::EDITORS_SYMPROPS_NO_NAME))
            .size(TS.body)
            .font(iced::Font {
                style: iced::font::Style::Italic,
                ..iced::Font::DEFAULT
            })
            .color(look.warn)
    } else {
        let ink = if pin.details.hidden {
            iced::Color {
                a: look.ink.a * HIDDEN_ALPHA,
                ..look.ink
            }
        } else {
            look.ink
        };
        text(UserText::new(pin.name.as_str()))
            .size(TS.body)
            .font(mono())
            .color(ink)
    }
    .wrapping(iced::widget::text::Wrapping::None);
    let type_label = match electrical_message(&pin.electrical) {
        Some(id) => text(t!(i18n, id)),
        None => text(UserText::new(pin.electrical.as_str())),
    };
    let kind = container(
        type_label
            .size(TS.caption)
            .wrapping(iced::widget::text::Wrapping::None),
    )
    .padding([0.0, SP.s])
    .style(pill(
        form.tokens,
        type_tone(direction_by_name(&pin.electrical)),
    ));
    let part = text(UserText::new(part_label(pin.details.part_number)))
        .size(TS.body)
        .font(mono())
        .color(look.ink3);
    let hover = ti(form.tokens.hover);
    let pressable = button(cells(
        number.boxed(),
        name.boxed(),
        kind.boxed(),
        part.boxed(),
        TS.body * ROW_EM,
    ))
    .padding(0)
    .width(Length::Fill)
    .on_press(PanelMsg::SymEditorSelectPin(pin.idx))
    .style(move |_: &Theme, status: button::Status| button::Style {
        background: matches!(status, button::Status::Hovered | button::Status::Pressed)
            .then_some(Background::Color(hover)),
        border: Border::default(),
        ..button::Style::default()
    });
    Column::new()
        .push(pressable.boxed())
        .push(rule(form))
        .boxed()
}

/// No pin passes the filter and chip, or the symbol has no pins yet.
fn empty<'a>(no_pins: bool, form: &Form<'_>) -> Element<'a, PanelMsg> {
    let i18n = form.i18n;
    let (big, hint) = if no_pins {
        (
            ids::EDITORS_SYMPROPS_NO_PINS,
            ids::EDITORS_SYMPROPS_NO_PINS_HINT,
        )
    } else {
        (
            ids::EDITORS_SYMPROPS_NO_MATCH,
            ids::EDITORS_SYMPROPS_NO_MATCH_HINT,
        )
    };
    let edge = resolve(form.tokens).line_strong;
    let card = container(
        column![
            text(t!(i18n, big))
                .size(TS.body)
                .color(form.look.ink)
                .align_x(iced::alignment::Horizontal::Center),
            text(t!(i18n, hint))
                .size(TS.body)
                .color(form.look.ink3)
                .align_x(iced::alignment::Horizontal::Center),
        ]
        .spacing(SP.xs)
        .align_x(iced::alignment::Horizontal::Center)
        .width(Length::Fill),
    )
    .padding([SP.xl, SP.l])
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        border: Border {
            width: HAIRLINE,
            color: edge,
            radius: crate::styles::Radii::GRAPHITE.card.into(),
            ..Border::default()
        },
        ..container::Style::default()
    });
    container(card).padding(SP.m).width(Length::Fill).boxed()
}
