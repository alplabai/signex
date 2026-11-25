//! A file's header and its annotation rows: the kind icon box, the title,
//! the snippet, the reference chips, the status pill and the date; the
//! hover action box; and the expanded source with its Jump button.

use iced::font::Weight;
use iced::widget::{Column, Row, Stack, column, container, hover, row};
use iced::{
    Alignment, Background, Border, Color, Element, Font, Length, Padding, Shadow, Theme, Vector,
    Widget as _,
};
use signex_i18n::{UserText, ids, t};
use signex_widgets::theme_ext::{annotation_glyph_colour, annotation_kind_colour};
use signex_widgets::{button, text};

use super::{
    AnnotationKind, AnnotationRow, AnnotationsMsg, FileGroup, METRICS, PAD, PAD_TIGHT, RefChip,
    Reference, Status, TS, kind_label, msg, status_label, tone,
};
use crate::panels::graphite::hint_tooltip;
use crate::panels::{PanelContext, PanelMsg};
use crate::styles::{DialogButtonKind, PillTone, Radii, dialog_button, ti, tokens::resolve};

/// The kind icon box, its glyph and its corner.
const ICON_BOX: f32 = TS.body * 18.0 / 12.0;
const ICON_GLYPH: f32 = TS.body;
const ICON_RADIUS: f32 = TS.body * 5.0 / 12.0;
/// The reference chips: mono 10.5 px, 17 px tall, radius 4.
const REF_TEXT: f32 = TS.body * 10.5 / 12.0;
const REF_H: f32 = TS.body * 17.0 / 12.0;
const REF_PAD: f32 = TS.body * 5.0 / 12.0;
const REF_RADIUS: f32 = TS.body * 4.0 / 12.0;
/// The source block: mono 10.5 px at 1.5 line height, 7 / 9 padding.
const SOURCE_TEXT: f32 = TS.body * 10.5 / 12.0;
const SOURCE_PAD: [f32; 2] = [TS.body * 7.0 / 12.0, TS.body * 9.0 / 12.0];
/// The hover action box's button and its offset from the row's corner.
const ACTION: f32 = TS.body * 22.0 / 12.0;
const ACTION_INSET: f32 = TS.body * 6.0 / 12.0;
/// The status pill's side padding.
const PILL_PAD: f32 = TS.body * 6.0 / 12.0;
/// The row's grid gaps: 2 px between lines, 8 px between the columns.
const LINE_GAP: f32 = TS.body * 2.0 / 12.0;

const BOOK: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path d="M3 3h4a1.5 1.5 0 011.5 1.5V13a1.2 1.2 0 00-1.2-1.2H3zM13 3H9.5A1.5 1.5 0 008 4.5" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"/><path d="M13 3v8.8H9" fill="none" stroke="currentColor" stroke-width="1.3"/></svg>"#;
const SCHEMATIC: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path d="M2 8h3M11 8h3" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/><rect x="5" y="5" width="6" height="6" rx="1" fill="none" stroke="currentColor" stroke-width="1.3"/></svg>"#;
const NOTE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path d="M4 3h8v7l-3 3H4z" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"/><path d="M6 6h4M6 8.5h3" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/></svg>"#;
const CALCULATION: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path d="M4 4.5h8M4 11.5h8M8 2.5v4M5 9.5l6 0" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/></svg>"#;
const MEASUREMENT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path d="M2 10.5l8.5-8.5 3.5 3.5L5.5 14z" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round"/><path d="M5 7.5l1.5 1.5M7.5 5l1.5 1.5" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/></svg>"#;
const ISSUE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path d="M4 14V2.5M4 3h8l-1.8 3L12 9H4" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round" stroke-linecap="round"/></svg>"#;
const DECISION: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path d="M8 2l6 6-6 6-6-6z" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round"/><path d="M5.8 8.1l1.5 1.5 3-3" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>"#;

/// The UI face at medium weight (titles, an active chip).
pub(super) fn medium() -> Font {
    Font {
        weight: Weight::Medium,
        ..signex_widgets::ui_font::regular()
    }
}

/// The chrome's monospace face (paths, designators, dates, source).
pub(super) fn mono() -> Font {
    crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
}

fn kind_glyph(kind: AnnotationKind) -> &'static [u8] {
    match kind {
        AnnotationKind::Note => NOTE,
        AnnotationKind::Calculation => CALCULATION,
        AnnotationKind::Measurement => MEASUREMENT,
        AnnotationKind::Issue => ISSUE,
        AnnotationKind::Decision => DECISION,
    }
}

/// A soft line under a block: a border with only its bottom edge drawn.
fn bottom_line(color: Color) -> Border {
    Border {
        width: 0.0,
        bottom: iced::border::Side::default().width(1.0).color(color),
        ..Border::default()
    }
}

/// A file's header: book icon, the path in mono, the row count; on the
/// sunken fill, 26 px tall.
pub(super) fn file_header<'a>(
    ctx: &'a PanelContext,
    group: &'a FileGroup,
) -> Element<'a, PanelMsg> {
    let r = resolve(&ctx.tokens);
    let ink = ti(ctx.tokens.text);
    let count = t!(ctx.i18n, ids::ANNOTATIONS_FILE_COUNT, "count" => group.rows.len());
    let (fill, line) = (r.sunken, r.line_soft);
    container(
        row![
            super::filters::glyph(BOOK, TS.body, r.ink3),
            text(UserText::new(group.label.as_str()))
                .size(TS.caption)
                .font(mono())
                .color(ink)
                .wrapping(iced::widget::text::Wrapping::None)
                .ellipsis(iced::widget::text::Ellipsis::End)
                .width(Length::Fill),
            text(count).size(TS.caption).color(r.ink3),
        ]
        .spacing(PAD_TIGHT * 1.5)
        .align_y(Alignment::Center),
    )
    .padding([0.0, PAD])
    .center_y(METRICS.status)
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(fill)),
        border: bottom_line(line),
        ..container::Style::default()
    })
    .boxed()
}

/// One annotation. A click toggles its source; hovering, or being expanded,
/// shows the action box with Jump to schematic.
pub(super) fn annotation_row<'a>(
    ctx: &'a PanelContext,
    item: &'a AnnotationRow,
) -> Element<'a, PanelMsg> {
    let tokens = &ctx.tokens;
    let r = resolve(tokens);
    let text_column = Column::new()
        .push(title_line(ctx, item))
        .push_boxed(
            text(UserText::new(item.snippet.as_str()))
                .size(TS.caption)
                .color(r.ink3)
                .line_height(iced::widget::text::LineHeight::Relative(1.45)),
        )
        .push(meta_line(ctx, item))
        .spacing(LINE_GAP)
        .width(Length::Fill);
    let mut content = Column::<Element<'a, PanelMsg>>::new().push_boxed(
        row![kind_box(ctx, item.kind), text_column]
            .spacing(PAD)
            .align_y(Alignment::Start),
    );
    if item.expanded {
        content = content.push(source_block(ctx, item));
    }
    let (hover_fill, selected_fill, line) = (ti(tokens.hover), ti(tokens.selection), r.line_soft);
    let ink = ti(tokens.text);
    let expanded = item.expanded;
    let base = button(content.width(Length::Fill))
        .width(Length::Fill)
        .padding(PAD)
        .on_press(msg(AnnotationsMsg::RowToggled(item.key.clone())))
        .style(move |_: &Theme, status: button::Status| {
            let fill = match status {
                _ if expanded => Some(selected_fill),
                button::Status::Hovered | button::Status::Pressed => Some(hover_fill),
                _ => None,
            };
            button::Style {
                background: fill.map(Background::Color),
                text_color: ink,
                border: bottom_line(line),
                ..button::Style::default()
            }
        });
    let Some(jump) = item.jump.clone() else {
        return base.boxed();
    };
    let actions = container(action_box(ctx, jump))
        .width(Length::Fill)
        .align_x(iced::alignment::Horizontal::Right)
        .padding(Padding {
            top: ACTION_INSET,
            right: ACTION_INSET,
            ..Padding::ZERO
        });
    if item.expanded {
        Stack::new()
            .push(base.boxed())
            .push(actions.boxed())
            .boxed()
    } else {
        hover(base, actions).boxed()
    }
}

/// The kind's colour box with its glyph: white in Light, near black in
/// Dark.
fn kind_box<'a>(ctx: &'a PanelContext, kind: AnnotationKind) -> Element<'a, PanelMsg> {
    let fill = annotation_kind_colour(tone(kind), &ctx.tokens);
    let glyph = super::filters::glyph(
        kind_glyph(kind),
        ICON_GLYPH,
        annotation_glyph_colour(&ctx.tokens),
    );
    let boxed = container(glyph)
        .center(ICON_BOX)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                radius: ICON_RADIUS.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed();
    hint_tooltip(
        boxed,
        kind_label(&ctx.i18n, kind),
        &ctx.tokens,
        iced::widget::tooltip::Position::Bottom,
    )
}

/// The title, medium weight, ending in an ellipsis.
fn title_line<'a>(ctx: &'a PanelContext, item: &'a AnnotationRow) -> Element<'a, PanelMsg> {
    text(UserText::new(item.title.as_str()))
        .size(TS.body)
        .font(medium())
        .color(ti(ctx.tokens.text))
        .wrapping(iced::widget::text::Wrapping::None)
        .ellipsis(iced::widget::text::Ellipsis::End)
        .width(Length::Fill)
        .boxed()
}

/// The reference chips and the status pill, wrapping; the date on the
/// right.
fn meta_line<'a>(ctx: &'a PanelContext, item: &'a AnnotationRow) -> Element<'a, PanelMsg> {
    let mut chips: Row<Element<'a, PanelMsg>> =
        Row::new().spacing(PAD_TIGHT).align_y(Alignment::Center);
    for chip in &item.chips {
        chips = chips.push(reference_chip(ctx, chip));
    }
    if let Some(status) = item.status {
        chips = chips.push(status_pill(ctx, status));
    }
    let mut line = Row::<Element<'a, PanelMsg>>::new()
        .push_boxed(container(chips.wrap().vertical_spacing(PAD_TIGHT)).width(Length::Fill))
        .align_y(Alignment::Start)
        .spacing(PAD_TIGHT);
    if let Some(date) = item.date {
        line = line.push_boxed(
            text(UserText::new(date.month_day()))
                .size(TS.caption)
                .font(mono())
                .color(resolve(&ctx.tokens).ink4),
        );
    }
    container(line)
        .padding(Padding {
            top: LINE_GAP,
            ..Padding::ZERO
        })
        .width(Length::Fill)
        .boxed()
}

/// A reference: mono, a strong line; a net on the sunken fill (iced draws
/// no dashed border); a part the design lost in amber, with why.
fn reference_chip<'a>(ctx: &'a PanelContext, chip: &'a RefChip) -> Element<'a, PanelMsg> {
    let tokens = &ctx.tokens;
    let r = resolve(tokens);
    let is_net = matches!(chip.reference, Reference::Net(_));
    let (ink, line, fill) = if chip.gone {
        (ti(tokens.warning), ti(tokens.warning), Some(r.warn_soft))
    } else if is_net {
        (ti(tokens.text), r.line_strong, Some(r.sunken))
    } else {
        (ti(tokens.text), r.line_strong, None)
    };
    let (accent, accent_text, gone) = (ti(tokens.accent), r.accent_text, chip.gone);
    let label = text(UserText::new(chip.label.as_str()))
        .size(REF_TEXT)
        .font(mono())
        .wrapping(iced::widget::text::Wrapping::None);
    let chip_button = button(container(label).center_y(Length::Fill))
        .height(REF_H)
        .padding([0.0, REF_PAD])
        .on_press(msg(AnnotationsMsg::ReferenceClicked(
            chip.reference.clone(),
        )))
        .style(move |_: &Theme, status: button::Status| {
            let hovered =
                !gone && matches!(status, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: fill.map(Background::Color),
                text_color: if hovered { accent_text } else { ink },
                border: Border {
                    color: if hovered { accent } else { line },
                    width: 1.0,
                    radius: REF_RADIUS.into(),
                    ..Border::default()
                },
                ..button::Style::default()
            }
        })
        .boxed();
    let tip = if chip.gone {
        t!(ctx.i18n, ids::ANNOTATIONS_PART_DELETED)
    } else {
        t!(ctx.i18n, ids::ANNOTATIONS_JUMP_TO_SCHEMATIC)
    };
    hint_tooltip(
        chip_button,
        tip,
        tokens,
        iced::widget::tooltip::Position::Bottom,
    )
}

/// The status as a pill: warn for Open, ok for Done, Accepted and Up to
/// date, info for the rest.
fn status_pill<'a>(ctx: &'a PanelContext, status: Status) -> Element<'a, PanelMsg> {
    let tone = match status {
        Status::Open => PillTone::Warn,
        Status::Done | Status::Accepted | Status::Current => PillTone::Ok,
        _ => PillTone::Info,
    };
    container(text(status_label(&ctx.i18n, status)).size(TS.caption))
        .padding([0.0, PILL_PAD])
        .center_y(REF_H)
        .style(crate::styles::pill(&ctx.tokens, tone))
        .boxed()
}

/// The raised box of row actions: Jump to schematic.
fn action_box<'a>(ctx: &'a PanelContext, jump: Reference) -> Element<'a, PanelMsg> {
    let tokens = &ctx.tokens;
    let r = resolve(tokens);
    let jump_button =
        button(container(super::filters::glyph(SCHEMATIC, TS.body, r.ink3)).center(Length::Fill))
            .width(ACTION)
            .height(ACTION)
            .padding(0)
            .on_press(msg(AnnotationsMsg::ReferenceClicked(jump)))
            .style(crate::styles::menu_item(tokens))
            .boxed();
    let jump_button = hint_tooltip(
        jump_button,
        t!(ctx.i18n, ids::ANNOTATIONS_JUMP_TO_SCHEMATIC),
        tokens,
        iced::widget::tooltip::Position::Bottom,
    );
    let (fill, line, shadow) = (r.raised, ti(tokens.border), r.shadow);
    container(jump_button)
        .padding(1)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                color: line,
                width: 1.0,
                radius: Radii::GRAPHITE.small.into(),
                ..Border::default()
            },
            shadow: Shadow {
                color: shadow,
                offset: Vector::new(0.0, 2.0),
                blur_radius: 6.0,
            },
            ..container::Style::default()
        })
        .boxed()
}

/// An expanded row's source: "<file> · line N", the block as written on the
/// sunken fill, and Jump to its first part.
fn source_block<'a>(ctx: &'a PanelContext, item: &'a AnnotationRow) -> Element<'a, PanelMsg> {
    let tokens = &ctx.tokens;
    let r = resolve(tokens);
    let caption = t!(
        ctx.i18n, ids::ANNOTATIONS_SOURCE_AT,
        "file" => item.file_name.as_str(), "line" => item.line.to_string()
    );
    let (fill, line) = (r.sunken, ti(tokens.border));
    let source = container(
        text(UserText::new(item.source.as_str()))
            .size(SOURCE_TEXT)
            .font(mono())
            .line_height(iced::widget::text::LineHeight::Relative(1.5))
            .color(ti(tokens.text)),
    )
    .padding(SOURCE_PAD)
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(fill)),
        border: Border {
            color: line,
            width: 1.0,
            radius: Radii::GRAPHITE.small.into(),
            ..Border::default()
        },
        ..container::Style::default()
    });
    let mut block = column![text(caption).size(TS.caption).color(r.ink3), source,]
        .spacing(LINE_GAP * 1.5)
        .padding(Padding {
            top: TOOL_GAP_DETAIL,
            ..Padding::ZERO
        })
        .width(Length::Fill);
    if let Some(jump) = &item.jump {
        let name = jump.component().map_or_else(|| jump.name(), str::to_string);
        block = block.push_boxed(
            container(dialog_button(
                t!(ctx.i18n, ids::ANNOTATIONS_JUMP_TO, "name" => name),
                DialogButtonKind::Secondary,
                Some(msg(AnnotationsMsg::ReferenceClicked(jump.clone()))),
                tokens,
            ))
            .padding(Padding {
                top: PAD_TIGHT,
                ..Padding::ZERO
            }),
        );
    }
    block.boxed()
}

/// The gap above an expanded row's source.
const TOOL_GAP_DETAIL: f32 = TS.body * 6.0 / 12.0;
