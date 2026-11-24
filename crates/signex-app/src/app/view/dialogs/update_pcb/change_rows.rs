//! The change-list language shared by Update PCB and Back Annotate (#2533):
//! a group header (fold chevron, tri-state tick, title, `n of m` pill and
//! the `+a · ~b · −c` split) and a dense row (tick, action pill, object in
//! mono, the detail filling the width, an optional caption under it and a
//! status icon whose words are its tooltip).

use iced::widget::text::Wrapping;
use iced::widget::{Column, Row, Space, container, mouse_area, tooltip as tip};
use iced::{Alignment, Background, Border, Color, Element, Length, Theme, Widget as _};
use signex_engine::pcb::eco::GroupTicks;
use signex_i18n::{I18n, Localized, MessageId, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{checkbox, text, tooltip};

use super::super::graphite::mono;
use crate::app::Message;
use crate::app::update_pcb::table::Summary;
use crate::panels::form_look::{checkbox_style, ghost_button, mixed_checkbox_style};
use crate::styles::{PillTone, Spacing, TypeScale, pill, pill_colors, ti, tokens};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Side of the status icon cell: the mockup's 22 px at the 12 px body.
pub(in crate::app::view::dialogs) const STATUS_W: f32 = TS.body * 22.0 / 12.0;
/// Width of the action pill column, from the longest action word.
const PILL_W: f32 = TS.body * 5.5;
/// Width of the object column (a designator, a net or `R12.3`).
const OBJECT_W: f32 = TS.body * 8.0;
/// Width of the tick column.
const TICK_W: f32 = TS.body * 1.5;

/// What a row's status icon says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app::view::dialogs) enum RowStatus {
    /// Nothing to say yet: the cell stays empty so the rows line up.
    Quiet,
    /// The last Validate passed with this row.
    Applies,
    /// Ticked, but a row it needs is not, so it does nothing.
    Skipped,
    /// Cannot be ticked until something is fixed.
    Blocked,
    /// Listed for information; nothing to apply.
    Info,
    /// Would apply, but says what it risks.
    Warn,
}

impl RowStatus {
    fn glyph(self) -> Option<MessageId> {
        match self {
            Self::Quiet => None,
            Self::Applies => Some(ids::UPDATE_PCB_STATUS_OK),
            Self::Skipped => Some(ids::CHANGE_ROW_ICON_SKIPPED),
            Self::Blocked => Some(ids::CHANGE_ROW_ICON_BLOCKED),
            Self::Info => Some(ids::CHANGE_ROW_ICON_INFO),
            Self::Warn => Some(ids::CHANGE_ROW_ICON_WARN),
        }
    }

    fn ink(self, tokens: &ThemeTokens) -> Color {
        match self {
            Self::Applies => ti(tokens.success),
            Self::Blocked => tokens::danger(tokens),
            Self::Warn => ti(tokens.warning),
            Self::Quiet | Self::Skipped | Self::Info => tokens::muted_text(tokens),
        }
    }
}

/// A group header's content. `ticks: None` draws no box (the Notes group);
/// `on_tick` is the message of a click, `None` while nothing can change.
pub(in crate::app::view::dialogs) struct GroupHeader {
    pub title: Localized,
    pub ticks: Option<GroupTicks>,
    pub on_tick: Option<Message>,
    pub folded: bool,
    pub on_fold: Message,
    /// The `n of m` pill, or the Notes count.
    pub count: Localized,
    /// The `+a · ~b · −c` split; `None` hides it.
    pub split: Option<Summary>,
    /// A glyph before the title in its ink (the Notes info or warn mark).
    pub mark: Option<(Localized, Color)>,
}

/// The header of one group, on the sunken band of the list.
pub(in crate::app::view::dialogs) fn group_header<'a>(
    header: GroupHeader,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let fold_id = if header.folded {
        ids::UPDATE_PCB_FOLD_CLOSED
    } else {
        ids::UPDATE_PCB_FOLD_OPEN
    };
    let fold = signex_widgets::button(
        text(t!(i18n, fold_id))
            .size(TS.body)
            .color(tokens::muted_text(tokens)),
    )
    .padding([0.0, SP.xs])
    .on_press(header.on_fold.clone())
    .style(ghost_button);
    let mut line: Row<Element<'a, Message>> = Row::new()
        .spacing(SP.s)
        .align_y(Alignment::Center)
        .push(fold.boxed());
    if let Some(ticks) = header.ticks {
        let mut boxed = checkbox(ticks.all() || ticks.mixed()).size(TS.body);
        boxed = if ticks.mixed() {
            boxed.style(mixed_checkbox_style)
        } else {
            boxed.style(checkbox_style)
        };
        if let Some(message) = header.on_tick {
            boxed = boxed.on_toggle(move |_| message.clone());
        }
        line = line.push(boxed.boxed());
    }
    if let Some((glyph, ink)) = header.mark {
        line = line.push(text(glyph).size(TS.body).color(ink).boxed());
    }
    let (pill_fill, pill_ink) = pill_colors(tokens, PillTone::Neutral);
    line = line
        .push(
            text(header.title)
                .size(TS.label)
                .font(iced::Font {
                    weight: iced::font::Weight::Semibold,
                    ..signex_widgets::ui_font::regular()
                })
                .color(ti(tokens.text))
                .boxed(),
        )
        .push(
            container(text(header.count).size(TS.caption).color(pill_ink))
                .padding([0.0, SP.s])
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(pill_fill)),
                    border: Border {
                        radius: crate::styles::Radii::GRAPHITE.pill.into(),
                        ..Border::default()
                    },
                    ..container::Style::default()
                })
                .boxed(),
        )
        .push(Space::new().width(Length::Fill).boxed());
    if let Some(split) = header.split {
        line = line.push(split_view(split, i18n, tokens));
    }
    let band = tokens::resolve(tokens).sunken;
    let edge = tokens::resolve(tokens).line_soft;
    let bar = container(line.padding([SP.xs, SP.m]))
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(band)),
            border: Border {
                width: 1.0,
                color: edge,
                ..Border::default()
            },
            ..container::Style::default()
        });
    mouse_area(bar).on_press(header.on_fold).boxed()
}

/// `+a · ~b · −c` in the add, change and remove inks; a zero part is left
/// out.
fn split_view<'a>(split: Summary, i18n: &I18n, tokens: &ThemeTokens) -> Element<'a, Message> {
    let parts = [
        (split.add, ids::CHANGE_ROW_SPLIT_ADD, ti(tokens.success)),
        (
            split.change,
            ids::CHANGE_ROW_SPLIT_CHANGE,
            tokens::resolve(tokens).info,
        ),
        (
            split.remove,
            ids::CHANGE_ROW_SPLIT_REMOVE,
            tokens::danger(tokens),
        ),
    ];
    let muted = tokens::muted_text(tokens);
    let mut line: Row<Element<'a, Message>> = Row::new().spacing(SP.xs).align_y(Alignment::Center);
    let mut first = true;
    for (count, id, ink) in parts {
        if count == 0 {
            continue;
        }
        if !first {
            line = line.push(
                text(t!(i18n, ids::CHANGE_ROW_SPLIT_SEPARATOR))
                    .size(TS.caption)
                    .color(muted)
                    .boxed(),
            );
        }
        first = false;
        line = line.push(
            text(t!(i18n, id, "count" => count))
                .size(TS.caption)
                .font(mono())
                .color(ink)
                .boxed(),
        );
    }
    line.boxed()
}

/// One row's content. `on_toggle: None` leaves the box inert (a blocked or
/// read-only row, or while the dialog is busy).
pub(in crate::app::view::dialogs) struct ChangeRow<'a> {
    pub on: bool,
    pub on_toggle: Option<Box<dyn Fn(bool) -> Message + 'a>>,
    /// The action pill (Add / Change / Remove) and its tone.
    pub pill: Option<(Localized, PillTone)>,
    /// The object in mono; `None` lets the detail start at the pill.
    pub object: Option<String>,
    /// The short detail, filling the rest of the width.
    pub detail: Element<'a, Message>,
    /// A caption under the detail (a block reason, a warning).
    pub caption: Option<(Localized, Color)>,
    pub status: RowStatus,
    /// What the status says, in words, as the icon's tooltip.
    pub status_tip: Option<Localized>,
    /// The whole change as a sentence, as the object's tooltip.
    pub sentence: Option<Localized>,
    /// Drawn with the selection tint (the row Space ticks, a linked row).
    pub focused: bool,
    /// A click on the row.
    pub on_press: Option<Message>,
    /// Ink of the object and detail: muted for a blocked or info row.
    pub dimmed: bool,
}

/// One dense row of the list.
pub(in crate::app::view::dialogs) fn change_row<'a>(
    r: ChangeRow<'a>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let ink = if r.dimmed {
        tokens::muted_text(tokens)
    } else {
        ti(tokens.text)
    };
    let mut tick = checkbox(r.on).size(TS.body).style(checkbox_style);
    if let Some(toggle) = r.on_toggle {
        tick = tick.on_toggle(toggle);
    }
    let mut line: Row<Element<'a, Message>> = Row::new()
        .spacing(SP.m)
        .align_y(Alignment::Center)
        .push(container(tick).width(TICK_W).boxed());
    if let Some((word, tone)) = r.pill {
        let ink = pill_colors(tokens, tone).1;
        line = line.push(
            container(
                container(text(word).size(TS.caption).color(ink))
                    .padding([0.0, SP.s])
                    .style(pill(tokens, tone)),
            )
            .width(PILL_W)
            .boxed(),
        );
    }
    if let Some(object) = r.object {
        let object = text(UserText::new(object))
            .size(TS.body)
            .font(mono())
            .color(ink)
            .wrapping(Wrapping::None)
            .width(OBJECT_W)
            .boxed();
        line = line.push(match r.sentence {
            Some(sentence) => {
                tooltip(object, hint(sentence, tokens), tip::Position::Bottom).boxed()
            }
            None => object,
        });
    }
    let mut middle = Column::new().spacing(SP.xs).push(r.detail);
    if let Some((caption, color)) = r.caption {
        middle = middle.push(text(caption).size(TS.caption).color(color).boxed());
    }
    line = line
        .push(container(middle).width(Length::Fill).clip(true).boxed())
        .push(status_cell(r.status, r.status_tip, i18n, tokens));
    let tint = tokens::selection_tint(tokens);
    let edge = tokens::resolve(tokens).line_soft;
    let focused = r.focused;
    let body = container(line.padding([SP.xs, SP.m]))
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: focused.then_some(Background::Color(tint)),
            border: Border {
                width: 0.0,
                color: edge,
                ..Border::default()
            },
            ..container::Style::default()
        });
    match r.on_press {
        Some(message) => mouse_area(body).on_press(message).boxed(),
        None => body.boxed(),
    }
}

/// The detail text of a row in its ink, on one line.
pub(in crate::app::view::dialogs) fn detail_text<'a>(
    detail: impl Into<signex_widgets::Label<'a>>,
    dimmed: bool,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let ink = if dimmed {
        tokens::muted_text(tokens)
    } else {
        ti(tokens.text)
    };
    text(detail)
        .size(TS.body)
        .color(ink)
        .wrapping(Wrapping::None)
        .boxed()
}

/// The square status cell: the icon, its words as the tooltip.
fn status_cell<'a>(
    status: RowStatus,
    words: Option<Localized>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let cell: Element<'a, Message> = match status.glyph() {
        Some(id) => container(text(t!(i18n, id)).size(TS.body).color(status.ink(tokens)))
            .center(STATUS_W)
            .boxed(),
        None => Space::new().width(STATUS_W).height(STATUS_W).boxed(),
    };
    match words {
        Some(words) => tooltip(cell, hint(words, tokens), tip::Position::Left).boxed(),
        None => cell,
    }
}

/// A small hover hint in the Graphite tooltip chrome.
pub(in crate::app::view::dialogs) fn hint<'a>(
    label: Localized,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = tokens::resolve(tokens);
    container(text(label).size(TS.caption).color(ti(tokens.text)))
        .padding([SP.xs, SP.s])
        .style(signex_widgets::active_bar_dropdown::tooltip_style(
            r.raised,
            r.line_strong,
        ))
        .boxed()
}
