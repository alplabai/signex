//! The change-list language shared by Update PCB and Back Annotate (#2533):
//! a group header (fold chevron, tri-state tick, title, `n of m` pill and
//! the `+a · ~b · −c` split) and a dense row (tick, action pill, object in
//! mono, the detail filling the width, an optional caption under it and a
//! status icon whose words are its tooltip). Every mark is a drawn glyph
//! (mockup rows 81, 82), so none depends on the UI font having it.

use iced::widget::text::Wrapping;
use iced::widget::{Column, Row, Space, container, mouse_area, svg, tooltip as tip};
use iced::{Alignment, Background, Border, Color, Element, Length, Theme, Widget as _};
use signex_engine::pcb::eco::GroupTicks;
use signex_i18n::{I18n, Localized, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{text, tooltip};

use super::super::graphite::mono;
use crate::app::Message;
use crate::app::update_pcb::table::Summary;
use crate::panels::form_look::ghost_button;
use crate::styles::{
    DialogIcon, PillTone, Radii, Spacing, TypeScale, pill, pill_colors, ti, tinted_glyph, tokens,
};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Side of a status glyph: one body line.
pub(in crate::app::view::dialogs) const GLYPH_SIDE: f32 = TS.body;
/// Side of a tick box: the mockup's 14 px at the 12 px body.
const TICK_SIDE: f32 = TS.body * 14.0 / 12.0;

const GLYPH_CHECK: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path d="M3.5 8.4l3 3L12.5 5" fill="none" stroke="#000" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/></svg>"##;
const GLYPH_DASH: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path d="M4.5 8h7" fill="none" stroke="#000" stroke-width="1.8" stroke-linecap="round"/></svg>"##;

/// A drawn mark: the tick, the dash, and the dialogs' blocked, info and
/// warn glyphs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app::view::dialogs) enum StatusGlyph {
    Check,
    Dash,
    Blocked,
    Info,
    Warn,
}

impl StatusGlyph {
    fn handle(self) -> svg::Handle {
        match self {
            Self::Check => crate::icons::static_glyph(GLYPH_CHECK),
            Self::Dash => crate::icons::static_glyph(GLYPH_DASH),
            Self::Blocked => DialogIcon::Error.glyph_handle(),
            Self::Info => DialogIcon::Info.glyph_handle(),
            Self::Warn => DialogIcon::Warn.glyph_handle(),
        }
    }

    /// The glyph, [`GLYPH_SIDE`] square, in `ink`.
    pub(in crate::app::view::dialogs) fn view<'a>(self, ink: Color) -> Element<'a, Message> {
        tinted_glyph(self.handle(), GLYPH_SIDE, ink)
    }
}

/// The state a tick box shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app::view::dialogs) enum Tick {
    Off,
    On,
    /// Some rows of the group are ticked and some are not.
    Mixed,
}

impl Tick {
    pub(in crate::app::view::dialogs) fn of(on: bool) -> Self {
        if on { Self::On } else { Self::Off }
    }

    /// A group's box: on when every open row is ticked, mixed when some are.
    pub(in crate::app::view::dialogs) fn of_group(ticks: GroupTicks) -> Self {
        if ticks.all() {
            Self::On
        } else if ticks.mixed() {
            Self::Mixed
        } else {
            Self::Off
        }
    }
}

/// Fill, hairline and mark of a tick box.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::app::view::dialogs) struct TickLook {
    pub fill: Color,
    pub edge: Color,
    pub mark: Option<(StatusGlyph, Color)>,
}

/// A tick box's look (mockup row 81): ticked or mixed is solid accent with
/// the white tick or dash; empty is the field; a box that cannot change sits
/// on the sunken fill, a ticked one on the strong line.
pub(in crate::app::view::dialogs) fn tick_look(
    tick: Tick,
    enabled: bool,
    tokens: &ThemeTokens,
) -> TickLook {
    let r = tokens::resolve(tokens);
    let white = tokens::on_accent(tokens);
    let mark = match tick {
        Tick::Off => None,
        Tick::On => Some((StatusGlyph::Check, white)),
        Tick::Mixed => Some((StatusGlyph::Dash, white)),
    };
    let (fill, edge) = match (tick, enabled) {
        (Tick::Off, true) => (r.field, r.line_strong),
        (Tick::Off, false) => (r.sunken, r.line_soft),
        (_, true) => {
            let accent = tokens::accent_fill(tokens);
            (accent, accent)
        }
        (_, false) => (r.line_strong, r.line_strong),
    };
    TickLook { fill, edge, mark }
}

/// A tick box; a press sends `on_press`, none leaves it inert.
pub(in crate::app::view::dialogs) fn tick_box<'a>(
    tick: Tick,
    on_press: Option<Message>,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let look = tick_look(tick, on_press.is_some(), tokens);
    let mark: Element<'a, Message> = match look.mark {
        Some((glyph, ink)) => tinted_glyph(glyph.handle(), TICK_SIDE, ink),
        None => Space::new().width(TICK_SIDE).height(TICK_SIDE).boxed(),
    };
    let face = container(mark)
        .center(TICK_SIDE)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(look.fill)),
            border: Border {
                width: 1.0,
                color: look.edge,
                radius: Radii::GRAPHITE.key.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    let mut press = signex_widgets::button(face)
        .padding(0)
        .style(|_: &Theme, _| iced::widget::button::Style::default());
    if let Some(message) = on_press {
        press = press.on_press(message);
    }
    press.boxed()
}

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
    pub(in crate::app::view::dialogs) fn glyph(self) -> Option<StatusGlyph> {
        match self {
            Self::Quiet => None,
            Self::Applies => Some(StatusGlyph::Check),
            Self::Skipped => Some(StatusGlyph::Dash),
            Self::Blocked => Some(StatusGlyph::Blocked),
            Self::Info => Some(StatusGlyph::Info),
            Self::Warn => Some(StatusGlyph::Warn),
        }
    }

    pub(in crate::app::view::dialogs) fn ink(self, tokens: &ThemeTokens) -> Color {
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
    pub mark: Option<(StatusGlyph, Color)>,
}

/// The header of one group, on the sunken band of the list: the shared
/// chevron, the tri-state tick, the mark, the title, `n of m` and the split.
pub(in crate::app::view::dialogs) fn group_header<'a>(
    header: GroupHeader,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let fold = signex_widgets::button(signex_widgets::chevron::chevron(
        !header.folded,
        TS.body,
        tokens::muted_text(tokens),
    ))
    .padding(0)
    .on_press(header.on_fold.clone())
    .style(ghost_button);
    let mut line: Row<Element<'a, Message>> = Row::new()
        .spacing(SP.s)
        .align_y(Alignment::Center)
        .push(fold.boxed());
    if let Some(ticks) = header.ticks {
        line = line.push(tick_box(Tick::of_group(ticks), header.on_tick, tokens));
    }
    if let Some((glyph, ink)) = header.mark {
        line = line.push(glyph.view(ink));
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
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let ink = if r.dimmed {
        tokens::muted_text(tokens)
    } else {
        ti(tokens.text)
    };
    let tick = tick_box(
        Tick::of(r.on),
        r.on_toggle.map(|toggle| toggle(!r.on)),
        tokens,
    );
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
        .push(status_cell(r.status, r.status_tip, tokens));
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
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let cell: Element<'a, Message> = match status.glyph() {
        Some(glyph) => container(glyph.view(status.ink(tokens)))
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
