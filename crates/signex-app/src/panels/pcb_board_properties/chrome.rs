//! Rows and controls the board Properties sections share (#2353), on the
//! Graphite look of the schematic Document Options. Sizes come from the type
//! and spacing scales.

use iced::widget::text::Wrapping;
use iced::widget::tooltip::Position;
use iced::widget::{Column, Row, container, row};
use iced::{Alignment, Background, Border, Color, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, Localized, MessageId, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{Label, button, checkbox, text, text_input, tooltip};

use super::super::form_look;
use super::super::properties_parameters::{chip_label, graphite_filter_chip};
use super::super::{PROPERTY_LABEL_COLUMN, PanelContext, PanelMsg};
use crate::styles::tokens::{hover_overlay, resolve};
use crate::styles::{HAIRLINE, Spacing, TypeScale, ti};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// One row of a section, and the label text the search finds it by.
pub(super) struct Shown<'a> {
    key: String,
    element: Element<'a, PanelMsg>,
}

/// One option of a segmented control: its label, whether it is the chosen
/// one, and what pressing it sends (`None`: disabled).
pub(super) struct Segment {
    pub label: Localized,
    pub active: bool,
    pub msg: Option<PanelMsg>,
}

/// The panel context and the search text, for the section builders.
pub(super) struct Form<'a> {
    pub ctx: &'a PanelContext,
    pub i18n: &'a I18n,
    /// The search text folded for matching; empty keeps every row.
    needle: String,
}

impl<'a> Form<'a> {
    pub fn new(ctx: &'a PanelContext, search: &str) -> Self {
        let i18n: &'a I18n = &ctx.i18n;
        Self {
            ctx,
            i18n,
            needle: i18n.fold_for_search(search.trim()),
        }
    }

    pub fn t(&self, id: MessageId) -> Localized {
        t!(self.i18n, id)
    }

    pub fn tokens(&self) -> &'a ThemeTokens {
        &self.ctx.tokens
    }

    pub fn muted(&self) -> Color {
        resolve(self.tokens()).ink3
    }

    pub fn ink(&self) -> Color {
        ti(self.tokens().text)
    }

    fn matches(&self, label: &str) -> bool {
        self.needle.is_empty() || self.i18n.fold_for_search(label).contains(&self.needle)
    }

    /// `element` as a row the search finds by any of `labels`.
    pub fn row(&self, labels: &[&Localized], element: Element<'a, PanelMsg>) -> Shown<'a> {
        let key = labels
            .iter()
            .map(|label| label.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        Shown { key, element }
    }

    /// A collapsible section of the rows the search keeps (all of them when
    /// the title matches); `None` when it keeps none.
    pub fn section(
        &self,
        key: &'static str,
        title: MessageId,
        rows: Vec<Shown<'a>>,
    ) -> Option<Element<'a, PanelMsg>> {
        let title = self.t(title).into_string();
        let whole = self.matches(&title);
        let kept: Vec<Element<'a, PanelMsg>> = rows
            .into_iter()
            .filter(|r| whole || self.matches(&r.key))
            .map(|r| r.element)
            .collect();
        if kept.is_empty() {
            return None;
        }
        let ctx = self.ctx;
        let body = move || {
            Column::with_children(kept)
                .spacing(SP.xs)
                .width(Length::Fill)
        };
        Some(
            ctx.collapsible(key, &title, &ctx.collapsed_sections, body)
                .boxed(),
        )
    }

    /// A muted line: an empty table, a search with no match.
    pub fn note(&self, line: Localized) -> Element<'a, PanelMsg> {
        container(text(line).size(TS.body).color(self.muted()))
            .padding([SP.s, SP.l])
            .width(Length::Fill)
            .boxed()
    }

    /// A small heading over a group of rows.
    pub fn caption(&self, line: Localized) -> Element<'a, PanelMsg> {
        container(
            text(line)
                .size(TS.caption)
                .color(resolve(self.tokens()).ink4),
        )
        .padding([SP.s, SP.l])
        .width(Length::Fill)
        .boxed()
    }

    /// `content` with `tip` shown under it on hover.
    pub fn tip(&self, content: Element<'a, PanelMsg>, tip: Localized) -> Element<'a, PanelMsg> {
        let tokens = self.tokens();
        let hint = container(text(tip).size(TS.caption).color(ti(tokens.text)))
            .padding([SP.xs, SP.m])
            .style(signex_widgets::active_bar_dropdown::tooltip_style(
                hover_overlay(tokens),
                ti(tokens.border),
            ));
        tooltip(content, hint, Position::Bottom).gap(SP.xs).boxed()
    }

    /// Label | content: the Graphite property row. The label sits on one
    /// line in this panel's own column (#2386).
    pub fn field(
        &self,
        label: &Localized,
        content: Element<'a, PanelMsg>,
    ) -> Element<'a, PanelMsg> {
        let label = text(label.clone())
            .size(TS.body)
            .color(self.muted())
            .wrapping(Wrapping::None);
        container(
            row![
                container(label).width(PROPERTY_LABEL_COLUMN).clip(true),
                container(content).width(Length::Fill),
            ]
            .spacing(SP.m)
            .align_y(Alignment::Center),
        )
        .padding([SP.xs, SP.l])
        .width(Length::Fill)
        .boxed()
    }

    /// A read-only value in the mono face.
    pub fn value(&self, value: impl Into<Label<'a>>) -> Element<'a, PanelMsg> {
        text(value)
            .size(TS.body)
            .font(mono())
            .color(self.ink())
            .wrapping(Wrapping::None)
            .boxed()
    }

    /// A value that sends `msg` when clicked, with `tip` on hover.
    pub fn link(
        &self,
        value: impl Into<Label<'a>>,
        msg: PanelMsg,
        tip: Localized,
    ) -> Element<'a, PanelMsg> {
        let accent = resolve(self.tokens()).accent_text;
        let link = button(text(value).size(TS.body).font(mono()).color(accent))
            .padding(0)
            .on_press(msg)
            .style(|_: &Theme, _| button::Style::default());
        self.tip(link.boxed(), tip)
    }

    /// A value the board cannot hold yet: muted, with `why` on hover.
    pub fn unavailable(
        &self,
        value: impl Into<Label<'a>>,
        why: Localized,
    ) -> Element<'a, PanelMsg> {
        let shown = text(value)
            .size(TS.body)
            .font(mono())
            .color(resolve(self.tokens()).ink4);
        self.tip(shown.boxed(), why)
    }

    /// Label | checkbox; `msg: None` draws it disabled with `why` on hover.
    pub fn check(
        &self,
        label: &Localized,
        checked: bool,
        msg: Option<PanelMsg>,
        why: Option<Localized>,
    ) -> Element<'a, PanelMsg> {
        let mut box_ = checkbox(checked)
            .size(TS.heading)
            .style(form_look::checkbox_style);
        if let Some(msg) = msg {
            box_ = box_.on_toggle(move |_| msg.clone());
        }
        let control = match why {
            Some(why) => self.tip(box_.boxed(), why),
            None => box_.boxed(),
        };
        // The label follows the box and takes the row, so a long one stays on
        // one line (#2386).
        let label = text(label.clone())
            .size(TS.body)
            .color(self.muted())
            .wrapping(Wrapping::None);
        container(
            row![control, label]
                .spacing(SP.m)
                .align_y(Alignment::Center),
        )
        .padding([SP.xs, SP.l])
        .width(Length::Fill)
        .boxed()
    }

    /// A Selection Filter or snap chip; `msg: None` draws it inert with
    /// `why` on hover.
    pub fn chip(
        &self,
        label: impl Into<Label<'a>>,
        on: bool,
        msg: Option<PanelMsg>,
        why: Option<Localized>,
    ) -> Element<'a, PanelMsg> {
        let enabled = msg.is_some();
        let lit = on && enabled;
        let chip = button(chip_label(label.into(), lit))
            .padding([SP.xs, SP.m])
            .on_press_maybe(msg)
            .style(move |theme: &Theme, status: button::Status| {
                graphite_filter_chip(theme, lit, matches!(status, button::Status::Hovered))
            });
        match why {
            Some(why) => self.tip(chip.boxed(), why),
            None => chip.boxed(),
        }
    }

    /// A segmented control on the sunken frame, one cell per option.
    pub fn segmented(&self, options: Vec<Segment>) -> Element<'a, PanelMsg> {
        let mut cells: Row<Element<'a, PanelMsg>> = Row::new().width(Length::Fill);
        for Segment { label, active, msg } in options {
            let enabled = msg.is_some();
            let cell = button(
                text(label)
                    .size(TS.body)
                    .align_x(iced::alignment::Horizontal::Center),
            )
            .padding([SP.s, 0.0])
            .width(Length::Fill)
            .on_press_maybe(msg)
            .style(move |theme: &Theme, status: button::Status| {
                let hovered = matches!(status, button::Status::Hovered);
                let mut style = form_look::segment_cell(theme, active, hovered && !active);
                if !enabled {
                    style.text_color = form_look::FormLook::of(theme).ink4;
                }
                style
            });
            cells = cells.push(cell.boxed());
        }
        container(cells)
            .padding(SP.xs)
            .width(Length::Fill)
            .style(form_look::segmented_frame)
            .boxed()
    }

    /// A small ghost button; `msg: None` draws it disabled with `why` on hover.
    pub fn action(
        &self,
        label: Localized,
        msg: Option<PanelMsg>,
        why: Option<Localized>,
    ) -> Element<'a, PanelMsg> {
        let enabled = msg.is_some();
        let btn = button(text(label).size(TS.body))
            .padding([SP.xs, SP.m])
            .on_press_maybe(msg)
            .style(move |theme: &Theme, status: button::Status| {
                let mut style = form_look::ghost_button(theme, status);
                if !enabled {
                    style.text_color = form_look::FormLook::of(theme).ink4;
                }
                style
            });
        match why {
            Some(why) => self.tip(btn.boxed(), why),
            None => btn.boxed(),
        }
    }

    /// A text field that sends what is typed and Enter.
    pub fn input(
        &self,
        placeholder: Localized,
        value: String,
        typed: impl Fn(String) -> PanelMsg + 'a,
        submitted: PanelMsg,
    ) -> Element<'a, PanelMsg> {
        text_input(placeholder, value)
            .size(TS.body)
            .padding([SP.xs, SP.s])
            .on_input(typed)
            .on_submit(submitted)
            .style(form_look::text_input_style)
            .width(Length::Fill)
            .boxed()
    }

    /// One line of a Graphite table: equal cells on a hairline.
    pub fn table_row(
        &self,
        cells: Vec<Element<'a, PanelMsg>>,
        header: bool,
    ) -> Element<'a, PanelMsg> {
        let even = vec![1; cells.len()];
        self.table_row_in(cells, &even, header)
    }

    /// [`Self::table_row`] with each cell taking its `portions` share of the
    /// width; a cell clips what does not fit, so nothing wraps (#2386).
    pub fn table_row_in(
        &self,
        cells: Vec<Element<'a, PanelMsg>>,
        portions: &[u16],
        header: bool,
    ) -> Element<'a, PanelMsg> {
        let mut line: Row<Element<'a, PanelMsg>> = Row::new()
            .spacing(SP.s)
            .align_y(Alignment::Center)
            .width(Length::Fill);
        for (cell, portion) in cells
            .into_iter()
            .zip(portions.iter().copied().chain(std::iter::repeat(1)))
        {
            let cell = container(cell)
                .width(Length::FillPortion(portion))
                .clip(true);
            line = line.push(cell.boxed());
        }
        let edge = resolve(self.tokens()).line_soft;
        container(line)
            .padding([SP.s, SP.l])
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: header.then_some(Background::Color(Color { a: 0.04, ..edge })),
                border: Border {
                    width: HAIRLINE,
                    color: edge,
                    ..Border::default()
                },
                ..container::Style::default()
            })
            .boxed()
    }

    /// A table heading cell.
    pub fn heading(&self, id: MessageId) -> Element<'a, PanelMsg> {
        text(self.t(id))
            .size(TS.caption)
            .color(resolve(self.tokens()).ink4)
            .wrapping(Wrapping::None)
            .boxed()
    }
}

/// The mono face of property values.
fn mono() -> iced::Font {
    crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
}
