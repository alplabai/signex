//! The Compare Board with Schematic table (mockup row 162): the column headings, then
//! per kind a sunken group row and its rows (kind glyph, Object, Schematic,
//! Board, Jump, Action), then the notes. The Action cell is a three-way
//! choice in one selected style; a choice the row does not allow is greyed
//! with the reason as its tooltip, and a caption names what the chosen
//! direction does.

use iced::font::{Style as FontStyle, Weight};
use iced::widget::{Column, Row, Space, container, row, scrollable, tooltip as tip};
use iced::{Alignment, Background, Border, Color, Element, Font, Length, Shadow, Theme, Vector};
use iced::{Widget as _, widget::text::Wrapping};
use signex_engine::pcb::eco::{
    BackBlock, CompareAction, CompareKind, CompareObject, CompareReport, CompareRow, CompareValue,
    Direction, Unavailable,
};
use signex_i18n::{I18n, Localized, MessageId, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::segmented::SegmentedMetrics;
use signex_widgets::{Label, button, text, tooltip};

use super::{Glyph, SP, TS, cb, direction_id, glyph};
use crate::app::compare_board::{CompareBoardState, JumpSide};
use crate::app::{CompareBoardMsg, Message};
use crate::styles::{ti, tokens};

/// Column widths (mockup row 162: 30 / 150 / 190 / 190 / 150 px at the 12 px body);
/// Action takes the rest.
const KIND_W: f32 = TS.body * 30.0 / 12.0;
const OBJECT_W: f32 = TS.body * 150.0 / 12.0;
const SIDE_W: f32 = TS.body * 190.0 / 12.0;
const JUMP_W: f32 = TS.body * 150.0 / 12.0;
/// Row and heading height (26 px at the 12 px body).
const ROW_H: f32 = TS.body * 26.0 / 12.0;
/// Cell padding (8 px at the 12 px body).
const CELL_PAD: f32 = TS.body * 8.0 / 12.0;
/// The kind glyph (13 px) and the jump and direction glyphs (11 px).
const KIND_GLYPH: f32 = TS.body * 13.0 / 12.0;
const SMALL_GLYPH: f32 = TS.body * 11.0 / 12.0;

fn mono() -> Font {
    super::super::graphite::mono()
}

fn group_id(kind: CompareKind) -> MessageId {
    match kind {
        CompareKind::MissingOnBoard => ids::COMPARE_BOARD_GROUP_MISSING,
        CompareKind::ExtraOnBoard => ids::COMPARE_BOARD_GROUP_EXTRA,
        CompareKind::NetDifference => ids::COMPARE_BOARD_GROUP_NETS,
        CompareKind::FootprintMismatch => ids::COMPARE_BOARD_GROUP_FOOTPRINTS,
        CompareKind::ValueChange => ids::COMPARE_BOARD_GROUP_VALUES,
    }
}

fn kind_glyph(kind: CompareKind) -> Glyph {
    match kind {
        CompareKind::MissingOnBoard | CompareKind::ExtraOnBoard => Glyph::Schematic,
        CompareKind::NetDifference => Glyph::Route,
        CompareKind::FootprintMismatch => Glyph::Board,
        CompareKind::ValueChange => Glyph::Edit,
    }
}

fn action_id(action: CompareAction) -> MessageId {
    use CompareAction as A;
    match action {
        A::AddToBoard => ids::COMPARE_BOARD_ACTION_ADD_TO_BOARD,
        A::LinkFootprint => ids::COMPARE_BOARD_ACTION_LINK_FOOTPRINT,
        A::RemoveFromBoard => ids::COMPARE_BOARD_ACTION_REMOVE_FROM_BOARD,
        A::AddNet => ids::COMPARE_BOARD_ACTION_ADD_NET,
        A::RemoveNet => ids::COMPARE_BOARD_ACTION_REMOVE_NET,
        A::RenameNet => ids::COMPARE_BOARD_ACTION_RENAME_NET,
        A::MovePin => ids::COMPARE_BOARD_ACTION_MOVE_PIN,
        A::ConnectPin => ids::COMPARE_BOARD_ACTION_CONNECT_PIN,
        A::DisconnectPin => ids::COMPARE_BOARD_ACTION_DISCONNECT_PIN,
        A::SwapFootprint => ids::COMPARE_BOARD_ACTION_SWAP_FOOTPRINT,
        A::TakeBoardFootprint => ids::COMPARE_BOARD_ACTION_TAKE_FOOTPRINT,
        A::SetValue => ids::COMPARE_BOARD_ACTION_SET_VALUE,
        A::TakeBoardValue => ids::COMPARE_BOARD_ACTION_TAKE_VALUE,
        A::SetDesignator => ids::COMPARE_BOARD_ACTION_SET_DESIGNATOR,
        A::TakeBoardDesignator => ids::COMPARE_BOARD_ACTION_TAKE_DESIGNATOR,
    }
}

/// Why a direction cannot be chosen, in full, for its tooltip.
fn unavailable_text(i18n: &I18n, why: &Unavailable) -> Localized {
    match why {
        Unavailable::NoBoardChange => t!(i18n, ids::COMPARE_BOARD_UNAVAILABLE_NO_BOARD_CHANGE),
        Unavailable::Board(block) => super::super::update_pcb::texts::blocked(i18n, block),
        Unavailable::SchematicLater => t!(i18n, ids::COMPARE_BOARD_UNAVAILABLE_SCHEMATIC_LATER),
        // Taking a board footprint into the schematic is follow-up #2653.
        Unavailable::Schematic(BackBlock::FootprintFromBoard) => {
            t!(i18n, ids::COMPARE_BOARD_UNAVAILABLE_FOOTPRINT_LATER)
        }
        Unavailable::Schematic(block) => super::super::back_annotate::block_text(i18n, block),
    }
}

/// A row line: fixed height, a hairline under it, an optional fill.
fn line_box<'a>(
    content: Row<Element<'a, Message>>,
    fill: Option<Color>,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let rule = tokens::resolve(tokens).line_soft;
    container(content.height(Length::Fill).align_y(Alignment::Center))
        .width(Length::Fill)
        .height(Length::Fixed(ROW_H))
        .style(move |_: &Theme| {
            let mut border = Border {
                color: rule,
                ..Border::default()
            };
            border.bottom = iced::border::Side::default().width(1.0);
            container::Style {
                background: fill.map(Background::Color),
                border,
                ..container::Style::default()
            }
        })
        .boxed()
}

/// One cell `width` wide, padded like mockup row 162's `td`.
fn cell<'a>(content: Element<'a, Message>, width: Length) -> Element<'a, Message> {
    container(content)
        .padding([0.0, CELL_PAD])
        .width(width)
        .clip(true)
        .boxed()
}

/// The headings, fixed above the scrolling rows, then the rows.
pub(super) fn view<'a>(
    state: &CompareBoardState,
    report: &CompareReport,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = tokens::resolve(tokens);
    let head = |id| -> Element<'a, Message> {
        text(t!(i18n, id))
            .size(TS.body)
            .color(r.ink3)
            .font(Font {
                weight: Weight::Medium,
                ..Font::DEFAULT
            })
            .wrapping(Wrapping::None)
            .boxed()
    };
    let headings = Row::new()
        .push(cell(Space::new().boxed(), Length::Fixed(KIND_W)))
        .push(cell(
            head(ids::COMPARE_BOARD_COLUMN_OBJECT),
            Length::Fixed(OBJECT_W),
        ))
        .push(cell(
            head(ids::COMPARE_BOARD_COLUMN_SCHEMATIC),
            Length::Fixed(SIDE_W),
        ))
        .push(cell(
            head(ids::COMPARE_BOARD_COLUMN_BOARD),
            Length::Fixed(SIDE_W),
        ))
        .push(cell(
            head(ids::COMPARE_BOARD_COLUMN_JUMP),
            Length::Fixed(JUMP_W),
        ))
        .push(cell(head(ids::COMPARE_BOARD_COLUMN_ACTION), Length::Fill));
    let headings = line_box(headings, Some(ti(tokens.toolbar_bg)), tokens);
    let mut body = Column::new();
    let mut any = false;
    for kind in CompareKind::ALL {
        if state.filter().is_some_and(|k| k != kind) {
            continue;
        }
        let rows: Vec<&CompareRow> = report.rows.iter().filter(|r| r.kind == kind).collect();
        if rows.is_empty() {
            continue;
        }
        any = true;
        body = body.push(group_row(
            t!(i18n, group_id(kind)),
            Some(rows.len()),
            tokens,
        ));
        for row in rows {
            body = body.push(row_view(report, row, i18n, tokens));
        }
    }
    if !any {
        body = body.push(
            container(
                text(t!(i18n, ids::COMPARE_BOARD_FILTERED_EMPTY))
                    .size(TS.body)
                    .color(r.ink4),
            )
            .padding([SP.m, SP.l])
            .boxed(),
        );
    }
    body = push_notes(body, report, i18n, tokens);
    Column::new()
        .push(headings)
        .push(
            scrollable(body)
                .height(Length::Fill)
                .direction(crate::styles::thin_vertical())
                .style(crate::styles::scrollbar_style(tokens))
                .boxed(),
        )
        .height(Length::Fill)
        .boxed()
}

/// A sunken group row: its caption in capitals, then its count in mono.
fn group_row<'a>(
    caption: Localized,
    count: Option<usize>,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = tokens::resolve(tokens);
    let mut line = Row::new()
        .push(cell(Space::new().boxed(), Length::Fixed(KIND_W)))
        .push(
            text(caption)
                .size(TS.section)
                .color(r.ink3)
                .font(Font {
                    weight: Weight::Medium,
                    ..Font::DEFAULT
                })
                .wrapping(Wrapping::None)
                .boxed(),
        )
        .spacing(SP.s)
        .align_y(Alignment::Center);
    if let Some(count) = count {
        line = line.push(
            text(UserText::new(count.to_string()))
                .size(TS.section)
                .color(r.ink3)
                .font(mono())
                .boxed(),
        );
    }
    line_box(line, Some(r.sunken), tokens)
}

/// The notes of both lists under a NOTES group row; none, nothing.
fn push_notes<'a>(
    mut body: Column<Element<'a, Message>>,
    report: &CompareReport,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Column<Element<'a, Message>> {
    let notes: Vec<Localized> = report
        .eco_notes()
        .iter()
        .map(|note| super::super::update_pcb::texts::note(i18n, note))
        .chain(
            report
                .back_notes()
                .iter()
                .map(|note| super::super::back_annotate::note_text(i18n, note)),
        )
        .collect();
    if notes.is_empty() {
        return body;
    }
    body = body.push(group_row(
        t!(i18n, ids::COMPARE_BOARD_GROUP_NOTES),
        None,
        tokens,
    ));
    let ink = tokens::resolve(tokens).ink3;
    for note in notes {
        body = body.push(
            container(text(note).size(TS.caption).color(ink))
                .padding([SP.xs, KIND_W + CELL_PAD])
                .width(Length::Fill)
                .boxed(),
        );
    }
    body
}

/// What a side holds, as its cell text.
fn value_text(i18n: &I18n, value: &CompareValue) -> Label<'static> {
    match value {
        CompareValue::Text(s) => Label::from(UserText::new(s.clone())),
        CompareValue::Part { value, footprint } => Label::from(t!(
            i18n,
            ids::COMPARE_BOARD_PART,
            "value" => value.clone(),
            "footprint" => footprint.clone()
        )),
        CompareValue::NoNet => Label::from(t!(i18n, ids::COMPARE_BOARD_NO_NET)),
    }
}

/// A side's cell: its value in mono (the board's, which differs, in the
/// warning ink and bold), or the absence in italic.
fn side_cell<'a>(
    value: Option<&CompareValue>,
    absent: MessageId,
    differs: bool,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = tokens::resolve(tokens);
    let content = match value {
        None => text(t!(i18n, absent))
            .size(TS.dense)
            .color(r.ink4)
            .font(Font {
                style: FontStyle::Italic,
                ..Font::DEFAULT
            }),
        Some(value) if differs => text(value_text(i18n, value))
            .size(TS.dense)
            .color(ti(tokens.warning))
            .font(Font {
                weight: Weight::Semibold,
                ..mono()
            }),
        Some(value) => text(value_text(i18n, value))
            .size(TS.dense)
            .color(ti(tokens.text))
            .font(mono()),
    };
    cell(
        content.wrapping(Wrapping::None).boxed(),
        Length::Fixed(SIDE_W),
    )
}

fn object_text(i18n: &I18n, object: &CompareObject) -> Label<'static> {
    match object {
        CompareObject::Designator(s) | CompareObject::Net(s) => {
            Label::from(UserText::new(s.clone()))
        }
        CompareObject::Pin { reference, pad } => Label::from(t!(
            i18n,
            ids::COMPARE_BOARD_PIN,
            "part" => reference.clone(),
            "pin" => pad.clone()
        )),
    }
}

/// One difference.
fn row_view<'a>(
    report: &CompareReport,
    row: &CompareRow,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = tokens::resolve(tokens);
    let object = text(object_text(i18n, &row.object))
        .size(TS.dense)
        .color(ti(tokens.text))
        .font(Font {
            weight: Weight::Bold,
            ..mono()
        })
        .wrapping(Wrapping::None);
    let line = Row::new()
        .push(cell(
            glyph(kind_glyph(row.kind), KIND_GLYPH, r.ink4),
            Length::Fixed(KIND_W),
        ))
        .push(cell(object.boxed(), Length::Fixed(OBJECT_W)))
        .push(side_cell(
            row.schematic.as_ref(),
            ids::COMPARE_BOARD_NOT_IN_SCHEMATIC,
            false,
            i18n,
            tokens,
        ))
        .push(side_cell(
            row.board.as_ref(),
            ids::COMPARE_BOARD_NOT_ON_BOARD,
            true,
            i18n,
            tokens,
        ))
        .push(cell(jump_links(row, i18n, tokens), Length::Fixed(JUMP_W)))
        .push(cell(action_cell(report, row, i18n, tokens), Length::Fill));
    line_box(line, None, tokens)
}

/// The jump links of the sides that hold the object.
fn jump_links<'a>(row: &CompareRow, i18n: &I18n, tokens: &ThemeTokens) -> Element<'a, Message> {
    let ink = tokens::resolve(tokens).accent_text;
    let link = |side: JumpSide, which: Glyph, id: MessageId| -> Element<'a, Message> {
        button(
            row![
                glyph(which, SMALL_GLYPH, ink),
                text(t!(i18n, id)).size(TS.caption).color(ink)
            ]
            .spacing(SP.xs)
            .align_y(Alignment::Center),
        )
        .padding(0)
        .on_press(cb(CompareBoardMsg::JumpRequested {
            side,
            target: row.jump.clone(),
        }))
        .style(|_: &Theme, _| signex_widgets::button::Style::default())
        .boxed()
    };
    let mut links = Row::new().spacing(SP.m).align_y(Alignment::Center);
    if row.schematic.is_some() {
        links = links.push(link(
            JumpSide::Schematic,
            Glyph::Schematic,
            ids::COMPARE_BOARD_JUMP_SCHEMATIC,
        ));
    }
    if row.board.is_some() {
        links = links.push(link(
            JumpSide::Board,
            Glyph::Board,
            ids::COMPARE_BOARD_JUMP_BOARD,
        ));
    }
    links.boxed()
}

/// The three-way choice, then the caption naming what the chosen
/// direction does.
fn action_cell<'a>(
    report: &CompareReport,
    row: &CompareRow,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = tokens::resolve(tokens);
    let caption = if report.waits(row) {
        t!(i18n, ids::COMPARE_BOARD_ACTION_WAITS)
    } else {
        match row.action(row.direction) {
            Some(action) => t!(i18n, action_id(action)),
            None => t!(i18n, ids::COMPARE_BOARD_ACTION_SKIP),
        }
    };
    Row::new()
        .push(direction_control(row, i18n, tokens))
        .push(
            text(caption)
                .size(TS.caption)
                .color(r.ink4)
                .wrapping(Wrapping::None)
                .boxed(),
        )
        .spacing(SP.m)
        .align_y(Alignment::Center)
        .boxed()
}

/// Update PCB / Update schematic / Skip on a sunken track, the chosen one
/// raised, every choice in the same selected style (decided 2026-11-19).
fn direction_control<'a>(
    row: &CompareRow,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = tokens::resolve(tokens);
    let m = SegmentedMetrics::from_text_size(TS.caption);
    let mut segments = Row::new().spacing(m.gap);
    for direction in Direction::ALL {
        let is_on = row.direction == direction;
        let why = row.unavailable(direction);
        let ink = match (is_on, why.is_some()) {
            (true, _) => ti(tokens.text),
            (false, false) => r.ink3,
            (false, true) => Color {
                a: crate::styles::DISABLED_OPACITY,
                ..r.ink4
            },
        };
        let mut face = Row::new().spacing(SP.xs).align_y(Alignment::Center);
        match direction {
            Direction::UpdatePcb => face = face.push(glyph(Glyph::ArrowRight, SMALL_GLYPH, ink)),
            Direction::UpdateSchematic => {
                face = face.push(glyph(Glyph::ArrowLeft, SMALL_GLYPH, ink));
            }
            Direction::Skip => {}
        }
        let mut label = text(t!(i18n, direction_id(direction)))
            .size(TS.caption)
            .color(ink)
            .wrapping(Wrapping::None);
        if is_on {
            label = label.font(Font {
                weight: Weight::Semibold,
                ..Font::DEFAULT
            });
        }
        face = face.push(label.boxed());
        let (fill, shadow) = (r.raised, r.shadow);
        let segment = button(face)
            .padding([0.0, m.segment_pad_h])
            .height(Length::Fixed(m.segment_height))
            .on_press_maybe(why.is_none().then(|| {
                cb(CompareBoardMsg::RowDirectionPicked {
                    row: row.id(),
                    direction,
                })
            }))
            .style(move |_: &Theme, _| signex_widgets::button::Style {
                background: is_on.then_some(Background::Color(fill)),
                text_color: ink,
                border: Border {
                    radius: m.segment_radius.into(),
                    ..Border::default()
                },
                shadow: if is_on {
                    Shadow {
                        color: shadow,
                        offset: Vector::new(0.0, m.shadow_offset),
                        blur_radius: m.shadow_blur,
                    }
                } else {
                    Shadow::default()
                },
                ..signex_widgets::button::Style::default()
            });
        let segment: Element<'a, Message> = match why {
            Some(why) => tooltip(
                segment,
                hint(unavailable_text(i18n, &why), tokens),
                tip::Position::Top,
            )
            .boxed(),
            None => segment.boxed(),
        };
        segments = segments.push(segment);
    }
    let track = r.sunken;
    container(segments)
        .padding(m.track_pad)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(track)),
            border: Border {
                radius: m.track_radius.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// A small hover hint in the Graphite tooltip chrome.
fn hint<'a>(label: Localized, tokens: &ThemeTokens) -> Element<'a, Message> {
    let r = tokens::resolve(tokens);
    container(text(label).size(TS.caption).color(ti(tokens.text)))
        .padding([SP.xs, SP.s])
        .style(signex_widgets::active_bar_dropdown::tooltip_style(
            r.raised,
            r.line_strong,
        ))
        .boxed()
}

#[cfg(test)]
mod tests {
    use signex_engine::pcb::eco::{BackBlock, Unavailable};
    use signex_i18n::I18n;

    use super::unavailable_text;

    #[test]
    fn a_footprint_mismatch_names_its_open_follow_up() {
        let why = Unavailable::Schematic(BackBlock::FootprintFromBoard);
        let text = unavailable_text(I18n::en_us_ref(), &why).into_string();
        assert!(text.contains("#2653"), "{text}");
        assert!(!text.contains("#2248"), "{text}");
    }
}
