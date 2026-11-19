//! Rows and controls the board Properties sections share (#2353), on the
//! Graphite look of the schematic Document Options. Sizes come from the type
//! and spacing scales.

use iced::widget::text::Wrapping;
use iced::widget::tooltip::Position;
use iced::widget::{Column, Row, container, mouse_area, row};
use iced::{Alignment, Border, Color, Element, Length, Padding, Theme, Widget as _};
use signex_i18n::{I18n, Localized, MessageId, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::property_illustration::{FieldId, IllustrationHover, IllustrationMsg};
use signex_widgets::{Label, button, checkbox, text, text_input, tooltip};

use super::super::form_look;
use super::super::properties_parameters::{chip_label, graphite_filter_chip};
use super::super::{PROPERTY_LABEL_COLUMN, PanelContext, PanelMsg, section_title};
use super::PcbBoardMsg;
use crate::styles::tokens::{hover_overlay, resolve};
use crate::styles::{HAIRLINE, PillTone, Spacing, TypeScale, pill, ti};

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

/// A section's header extras: a summary at its right end (a count pill, a
/// caption, a button) and whether it starts closed (#2523, m93).
pub(super) struct Head<'a> {
    summary: Option<Element<'a, PanelMsg>>,
    closed_by_default: bool,
}

impl<'a> Head<'a> {
    /// Open until the user closes it.
    pub fn open() -> Self {
        Self {
            summary: None,
            closed_by_default: false,
        }
    }

    /// Closed until the user opens it.
    pub fn closed() -> Self {
        Self {
            closed_by_default: true,
            ..Self::open()
        }
    }

    pub fn summary(mut self, summary: Element<'a, PanelMsg>) -> Self {
        self.summary = Some(summary);
        self
    }
}

// The key `ToggleSection` flips for the section or fold `key`.
pub(super) use crate::panels::widgets::section_toggle_key as toggle_key;

/// Whether the section or fold `key` shows its body.
pub(super) fn is_open(ctx: &PanelContext, key: &str, closed_by_default: bool) -> bool {
    crate::panels::widgets::is_section_open(key, closed_by_default, &ctx.collapsed_sections)
}

/// The panel context and the search text, for the section builders.
pub(super) struct Form<'a> {
    pub ctx: &'a PanelContext,
    pub i18n: &'a I18n,
    /// The search text folded for matching; empty keeps every row.
    needle: String,
    /// The field the pointer is over, in a row or on the illustration.
    hover: &'a IllustrationHover,
}

impl<'a> Form<'a> {
    pub fn new(ctx: &'a PanelContext, search: &str, hover: &'a IllustrationHover) -> Self {
        let i18n: &'a I18n = &ctx.i18n;
        Self {
            ctx,
            i18n,
            needle: i18n.fold_for_search(search.trim()),
            hover,
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

    pub fn faint(&self) -> Color {
        resolve(self.tokens()).ink4
    }

    pub fn ink(&self) -> Color {
        ti(self.tokens().text)
    }

    pub fn accent(&self) -> Color {
        resolve(self.tokens()).accent_text
    }

    pub fn matches(&self, label: &str) -> bool {
        self.needle.is_empty() || self.i18n.fold_for_search(label).contains(&self.needle)
    }

    /// Whether a search is typed: sections then open on their matches.
    pub fn is_searching(&self) -> bool {
        !self.needle.is_empty()
    }

    /// Whether `field` is hovered, in a row or on the illustration.
    pub fn is_hot(&self, field: &FieldId) -> bool {
        self.hover.is_field_highlighted(field)
    }

    /// `content` reporting the pointer entering and leaving it as a hover
    /// of `field`, so the illustration lights the callout of that field.
    pub fn hover_zone(
        &self,
        field: FieldId,
        content: Element<'a, PanelMsg>,
    ) -> Element<'a, PanelMsg> {
        let hovered = |m: IllustrationMsg| PanelMsg::PcbBoard(PcbBoardMsg::IllustrationHovered(m));
        mouse_area(content)
            .on_enter(hovered(IllustrationMsg::FieldHovered(field.clone())))
            .on_exit(hovered(IllustrationMsg::FieldLeft(field)))
            .boxed()
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
    /// the title matches), open until the user closes it; `None` when it
    /// keeps none.
    pub fn section(
        &self,
        key: &'static str,
        title: MessageId,
        rows: Vec<Shown<'a>>,
    ) -> Option<Element<'a, PanelMsg>> {
        self.section_with(key, title, Head::open(), rows)
    }

    /// [`Self::section`] with a header summary and a closed default. A typed
    /// search opens every section that keeps a row, so its matches show.
    pub fn section_with(
        &self,
        key: &'static str,
        title: MessageId,
        head: Head<'a>,
        rows: Vec<Shown<'a>>,
    ) -> Option<Element<'a, PanelMsg>> {
        let whole = self.matches(&self.t(title).into_string());
        let kept: Vec<Element<'a, PanelMsg>> = rows
            .into_iter()
            .filter(|r| whole || self.matches(&r.key))
            .map(|r| r.element)
            .collect();
        if kept.is_empty() {
            return None;
        }
        let Head {
            summary,
            closed_by_default,
        } = head;
        let open = self.is_searching() || is_open(self.ctx, key, closed_by_default);
        let mut col = Column::new().width(Length::Fill).push(self.section_header(
            title,
            toggle_key(key, closed_by_default),
            open,
            summary,
        ));
        if open {
            col = col.push(
                Column::with_children(kept)
                    .spacing(SP.xs)
                    .width(Length::Fill)
                    .boxed(),
            );
        }
        Some(col.boxed())
    }

    /// Chevron and the uppercase title, which open or close the section, and
    /// the summary at the right end.
    fn section_header(
        &self,
        title: MessageId,
        toggle: String,
        open: bool,
        summary: Option<Element<'a, PanelMsg>>,
    ) -> Element<'a, PanelMsg> {
        let label = row![
            signex_widgets::chevron::chevron(open, TS.section, self.faint()),
            section_title(self.t(title), self.tokens(), self.i18n).wrapping(Wrapping::None),
        ]
        .spacing(SP.s)
        .align_y(Alignment::Center);
        let toggle = button(label)
            .padding(0)
            .width(Length::Fill)
            .on_press(PanelMsg::ToggleSection(toggle))
            .style(|_: &Theme, _| button::Style::default());
        let mut line: Row<Element<'a, PanelMsg>> = Row::new()
            .spacing(SP.s)
            .align_y(Alignment::Center)
            .width(Length::Fill)
            .push(toggle.boxed());
        if let Some(summary) = summary {
            line = line.push(summary);
        }
        container(line)
            .padding(Padding {
                top: SP.m,
                right: SP.l,
                bottom: SP.s,
                left: SP.l,
            })
            .width(Length::Fill)
            .boxed()
    }

    /// A small count pill, as a section header or a grid card shows it.
    pub fn pill(&self, label: Localized, tone: PillTone) -> Element<'a, PanelMsg> {
        container(text(label).size(TS.caption).wrapping(Wrapping::None))
            .padding([0.0, SP.s])
            .style(pill(self.tokens(), tone))
            .boxed()
    }

    /// A faint caption at a section header's right end.
    pub fn header_caption(&self, label: Localized) -> Element<'a, PanelMsg> {
        text(label)
            .size(TS.caption)
            .color(self.faint())
            .wrapping(Wrapping::None)
            .boxed()
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
        container(text(line).size(TS.caption).color(self.faint()))
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
        self.field_lit(label, content, false)
    }

    /// [`Self::field`] whose label takes the accent while `hot` (its callout
    /// is hovered on the illustration).
    pub fn field_lit(
        &self,
        label: &Localized,
        content: Element<'a, PanelMsg>,
        hot: bool,
    ) -> Element<'a, PanelMsg> {
        let ink = if hot { self.accent() } else { self.muted() };
        let label = text(label.clone())
            .size(TS.body)
            .color(ink)
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

    /// A labelled control too wide for the field column (a segmented choice
    /// with long labels): the label on its own line, the control below at
    /// the full form width (#2262).
    pub fn stacked(
        &self,
        label: &Localized,
        content: Element<'a, PanelMsg>,
    ) -> Element<'a, PanelMsg> {
        let label = text(label.clone())
            .size(TS.body)
            .color(self.muted())
            .wrapping(Wrapping::None);
        container(iced::widget::column![label, content].spacing(SP.xs))
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

    /// Link text in the UI face (a "go there" row), sending `msg`, with
    /// `tip` on hover.
    pub fn text_link(
        &self,
        label: Localized,
        msg: PanelMsg,
        tip: Localized,
    ) -> Element<'a, PanelMsg> {
        let accent = self.accent();
        let link = button(text(label).size(TS.body).color(accent))
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
        let shown = text(value).size(TS.body).font(mono()).color(self.faint());
        self.tip(shown.boxed(), why)
    }

    /// A checkbox and its label; `msg: None` draws it disabled with `why` on
    /// hover.
    pub fn check_box(
        &self,
        label: &Localized,
        checked: bool,
        msg: Option<PanelMsg>,
        why: Option<Localized>,
    ) -> Element<'a, PanelMsg> {
        let enabled = msg.is_some();
        let mut box_ = checkbox(checked)
            .size(TS.heading)
            .style(form_look::checkbox_style);
        if let Some(msg) = msg {
            box_ = box_.on_toggle(move |_| msg.clone());
        }
        // The label follows the box, so a long one stays on one line (#2386).
        let ink = if enabled { self.muted() } else { self.faint() };
        let label = text(label.clone())
            .size(TS.body)
            .color(ink)
            .wrapping(Wrapping::None);
        let pair = row![box_, label].spacing(SP.s).align_y(Alignment::Center);
        match why {
            Some(why) => self.tip(pair.boxed(), why),
            None => pair.boxed(),
        }
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
            cells = cells.push(segment_cell(text(label).size(TS.body).boxed(), active, msg));
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

    /// A small framed button (the mockup's plain `btn sm`), with `tip`;
    /// `msg: None` greys it.
    pub fn framed_action(
        &self,
        label: Localized,
        msg: Option<PanelMsg>,
        tip: Option<Localized>,
    ) -> Element<'a, PanelMsg> {
        let enabled = msg.is_some();
        let btn = button(text(label).size(TS.caption).wrapping(Wrapping::None))
            .padding([SP.xs, SP.m])
            .on_press_maybe(msg)
            .style(move |theme: &Theme, status: button::Status| {
                let mut style = form_look::value_field_button(theme, status);
                if !enabled {
                    style.text_color = form_look::FormLook::of(theme).ink4;
                }
                style
            });
        match tip {
            Some(tip) => self.tip(btn.boxed(), tip),
            None => btn.boxed(),
        }
    }

    /// The primary button of a card (Run DRC).
    pub fn primary(&self, label: Localized, msg: PanelMsg) -> Element<'a, PanelMsg> {
        button(text(label).size(TS.caption).wrapping(Wrapping::None))
            .padding([SP.xs, SP.m])
            .on_press(msg)
            .style(crate::styles::primary_button(self.tokens()))
            .boxed()
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

    /// A small Graphite card around `body`: a hairline frame inset in the
    /// section (#2386). Board Information, the Grid and Guide Managers and the
    /// Sheet use it.
    pub fn card(&self, body: Element<'a, PanelMsg>) -> Element<'a, PanelMsg> {
        self.card_lit(body, false)
    }

    /// [`Self::card`] ringed in the accent while `hot`: its callout is
    /// hovered on the illustration.
    pub fn card_lit(&self, body: Element<'a, PanelMsg>, hot: bool) -> Element<'a, PanelMsg> {
        let edge = if hot {
            ti(self.tokens().accent)
        } else {
            resolve(self.tokens()).line_soft
        };
        let framed = container(body)
            .padding([SP.s, SP.m])
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                border: Border {
                    width: HAIRLINE,
                    color: edge,
                    radius: crate::styles::RADIUS.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            });
        container(framed)
            .padding([SP.xs, SP.l])
            .width(Length::Fill)
            .boxed()
    }
}

/// One cell of a segmented control: `content` on the cell look, active or
/// not; `msg: None` greys it.
pub(super) fn segment_cell<'a>(
    content: Element<'a, PanelMsg>,
    active: bool,
    msg: Option<PanelMsg>,
) -> Element<'a, PanelMsg> {
    let enabled = msg.is_some();
    let cell = container(content)
        .width(Length::Fill)
        .align_x(iced::alignment::Horizontal::Center);
    button(cell)
        .padding([SP.s, SP.xs])
        .width(Length::Fill)
        .on_press_maybe(msg)
        .style(move |theme: &Theme, status: button::Status| {
            let hovered = matches!(status, button::Status::Hovered);
            let mut style = form_look::segment_cell(theme, active, hovered && !active);
            if !enabled {
                style.text_color = form_look::FormLook::of(theme).ink4;
            }
            style
        })
        .boxed()
}

/// The mono face of property values.
pub(super) fn mono() -> iced::Font {
    crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
}
