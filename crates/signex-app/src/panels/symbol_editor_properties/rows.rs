//! Rows and controls the symbol editor's Properties forms share (mockup
//! row 123): the header block, collapsible sections, a label column that
//! wraps, inputs, paired inputs, read-only boxes, dropdowns, segmented
//! controls, and the hover link between a field row and its callout in the
//! illustration.

use iced::widget::tooltip::Position;
use iced::widget::{Column, Row, Space, column, container, mouse_area, row};
use iced::{Alignment, Background, Border, Color, Element, Font, Length, Theme, Widget as _};
use signex_i18n::{I18n, Localized};
use signex_types::theme::ThemeTokens;
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::property_illustration::{FieldId, IllustrationHover, IllustrationMsg};
use signex_widgets::{Label, button, checkbox, raw, text, text_input, tooltip};

use super::super::{
    CollapsedSections, NumericDraft, NumericFieldKey, PROPERTY_LABEL_COLUMN, PanelMsg, form_look,
    numeric_input,
};
use crate::library::editor::symbol::SymbolPropsMsg;
use crate::styles::tokens::{hover_overlay, resolve};
use crate::styles::{HAIRLINE, PillTone, Spacing, TypeScale, pill, ti};

pub(super) const TS: TypeScale = TypeScale::DEFAULT;
pub(super) const SP: Spacing = Spacing::DEFAULT;

/// Height of a tab of the Symbol / Pins bar, in body sizes (32 px at 12).
const TAB_HEIGHT_EM: f32 = 32.0 / 12.0;
/// Thickness of the accent bar under the chosen tab, in body sizes.
const TAB_BAR_EM: f32 = 2.0 / 12.0;

/// The colours of the forms, from the theme tokens.
#[derive(Debug, Clone, Copy)]
pub(super) struct Look {
    pub ink: Color,
    pub ink3: Color,
    pub ink4: Color,
    pub accent: Color,
    pub accent_soft: Color,
    pub sunken: Color,
    pub line: Color,
    pub line_soft: Color,
    pub err: Color,
    pub warn: Color,
}

impl Look {
    pub fn of(tokens: &ThemeTokens) -> Self {
        let r = resolve(tokens);
        Self {
            ink: ti(tokens.text),
            ink3: r.ink3,
            ink4: r.ink4,
            accent: ti(tokens.accent),
            accent_soft: r.accent_soft,
            sunken: r.sunken,
            line: ti(tokens.border),
            line_soft: r.line_soft,
            err: ti(tokens.error),
            warn: ti(tokens.warning),
        }
    }
}

/// The mono face of identifiers and numbers.
pub(super) fn mono() -> Font {
    crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
}

fn semibold() -> Font {
    Font {
        weight: iced::font::Weight::Semibold,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    }
}

/// A Properties panel message of this panel.
pub(super) fn props(msg: SymbolPropsMsg) -> PanelMsg {
    PanelMsg::SymbolProps(msg)
}

/// What every form builder needs: the look, the catalog, the illustration
/// hover and the sections the user toggled.
pub(super) struct Form<'f> {
    pub look: Look,
    pub tokens: &'f ThemeTokens,
    pub i18n: &'f I18n,
    pub hover: &'f IllustrationHover,
    pub toggled: &'f CollapsedSections,
    pub draft: Option<&'f NumericDraft>,
}

impl<'f> Form<'f> {
    /// The header block: a title row (the title and an optional pill) and a
    /// caption line.
    pub fn header<'a>(
        &self,
        title: Label<'a>,
        tag: Option<(Label<'a>, PillTone)>,
        caption: Option<(Label<'a>, bool)>,
    ) -> Element<'a, PanelMsg> {
        let mut title_row: Row<Element<'a, PanelMsg>> =
            Row::new().spacing(SP.s).align_y(Alignment::Center).push(
                text(title)
                    .size(TS.label)
                    .font(semibold())
                    .color(self.look.ink)
                    .boxed(),
            );
        if let Some((label, tone)) = tag {
            title_row = title_row.push(self.pill(label, tone));
        }
        let mut col: Column<Element<'a, PanelMsg>> =
            Column::new().spacing(SP.xs).push(title_row.boxed());
        if let Some((line, is_mono)) = caption {
            let mut line = text(line).size(TS.caption).color(self.look.ink4);
            if is_mono {
                line = line.font(mono());
            }
            col = col.push(line.boxed());
        }
        container(col)
            .padding(iced::Padding {
                top: SP.m,
                right: SP.m,
                bottom: SP.xs,
                left: SP.m,
            })
            .width(Length::Fill)
            .boxed()
    }

    /// A small rounded tag in a status tone.
    pub fn pill<'a>(&self, label: Label<'a>, tone: PillTone) -> Element<'a, PanelMsg> {
        container(
            text(label)
                .size(TS.caption)
                .wrapping(iced::widget::text::Wrapping::None),
        )
        .padding([0.0, SP.s])
        .style(pill(self.tokens, tone))
        .boxed()
    }

    /// A catalog title in capitals, for the locale.
    fn upper(&self, title: Localized) -> Label<'static> {
        // raw: the catalog title, upper-cased for the locale.
        raw(self.i18n.upper_for_display(&title.into_string()))
    }

    /// A small heading inside a section ("LOCAL COLORS").
    pub fn mini_heading<'a>(&self, title: Localized) -> Element<'a, PanelMsg> {
        container(
            text(self.upper(title))
                .size(TS.caption)
                .color(self.look.ink4),
        )
        .padding(iced::Padding {
            top: SP.s,
            right: SP.m,
            bottom: SP.xs,
            left: SP.m,
        })
        .width(Length::Fill)
        .boxed()
    }

    /// Whether the section `key` is open: `default_open`, flipped once the
    /// user toggled it.
    pub fn is_open(&self, key: &str, default_open: bool) -> bool {
        default_open != self.toggled.contains(key)
    }

    /// A collapsible section: its header, and `body` while it is open. A
    /// `count` is shown as a pill after the title.
    pub fn section<'a>(
        &self,
        key: &'static str,
        title: Localized,
        default_open: bool,
        count: Option<usize>,
        body: impl FnOnce() -> Vec<Element<'a, PanelMsg>>,
    ) -> Element<'a, PanelMsg> {
        let open = self.is_open(key, default_open);
        let ink = self.look.ink3;
        let mut label: Row<Element<'a, PanelMsg>> = Row::new()
            .spacing(SP.s)
            .align_y(Alignment::Center)
            .push(signex_widgets::chevron::chevron(open, TS.section, ink).boxed())
            .push(text(self.upper(title)).size(TS.section).color(ink).boxed());
        if let Some(n) = count {
            label = label.push(self.pill(
                signex_i18n::UserText::new(self.i18n.count(n as i64)).into(),
                PillTone::Neutral,
            ));
        }
        let header = button(
            container(label)
                .padding(iced::Padding {
                    top: SP.m,
                    right: SP.m,
                    bottom: SP.xs,
                    left: SP.m,
                })
                .width(Length::Fill),
        )
        .padding(0)
        .width(Length::Fill)
        .on_press(PanelMsg::ToggleSection(key.to_string()))
        .style(|_: &Theme, _| button::Style::default());
        let mut col: Column<Element<'a, PanelMsg>> =
            Column::new().width(Length::Fill).push(header.boxed());
        if open {
            col = col.push(
                Column::with_children(body())
                    .spacing(0)
                    .width(Length::Fill)
                    .padding(iced::Padding {
                        bottom: SP.xs,
                        ..iced::Padding::ZERO
                    })
                    .boxed(),
            );
        }
        col.boxed()
    }

    /// Label | control, the label wrapping to a second line rather than
    /// being cut. `field` links the row to its callout in the illustration.
    pub fn field<'a>(
        &self,
        label: Localized,
        control: Element<'a, PanelMsg>,
        field: Option<&'static str>,
    ) -> Element<'a, PanelMsg> {
        let label = text(label).size(TS.body).color(self.look.ink3);
        let content = row![
            container(label).width(PROPERTY_LABEL_COLUMN),
            container(control).width(Length::Fill),
        ]
        .spacing(SP.s)
        .align_y(Alignment::Center);
        self.linked(content.boxed(), field, [SP.xs, SP.m])
    }

    /// `content` linked to the illustration's `field`: highlighted while the
    /// field or its callout is hovered, reporting its own hover.
    pub fn linked<'a>(
        &self,
        content: Element<'a, PanelMsg>,
        field: Option<&'static str>,
        padding: [f32; 2],
    ) -> Element<'a, PanelMsg> {
        let lit = field.is_some_and(|f| self.hover.is_field_highlighted(&FieldId::new(f)));
        let wash = self.look.accent_soft;
        let framed = container(content)
            .padding(padding)
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: lit.then_some(Background::Color(wash)),
                ..container::Style::default()
            });
        match field {
            Some(f) => mouse_area(framed)
                .on_enter(props(SymbolPropsMsg::IllustrationHovered(
                    IllustrationMsg::FieldHovered(FieldId::new(f)),
                )))
                .on_exit(props(SymbolPropsMsg::IllustrationHovered(
                    IllustrationMsg::FieldLeft(FieldId::new(f)),
                )))
                .boxed(),
            None => framed.boxed(),
        }
    }

    /// Two controls sharing one row, half the width each.
    pub fn pair<'a>(
        &self,
        first: Element<'a, PanelMsg>,
        second: Element<'a, PanelMsg>,
    ) -> Element<'a, PanelMsg> {
        row![
            container(first).width(Length::FillPortion(1)),
            container(second).width(Length::FillPortion(1)),
        ]
        .spacing(SP.s)
        .align_y(Alignment::Center)
        .boxed()
    }

    /// A text field; `mono` for identifiers (numbers, names).
    pub fn input<'a>(
        &self,
        placeholder: Localized,
        value: String,
        is_mono: bool,
        on_input: impl Fn(String) -> PanelMsg + 'a,
    ) -> Element<'a, PanelMsg> {
        let mut field = text_input(placeholder, value)
            .size(TS.body)
            .padding([SP.xs, SP.s])
            .on_input(on_input)
            .style(form_look::text_input_style)
            .width(Length::Fill);
        if is_mono {
            field = field.font(mono());
        }
        field.boxed()
    }

    /// A numeric field in mm wired to the typing drafts, with its unit
    /// after it when `unit` is set.
    pub fn number<'a>(
        &self,
        key: NumericFieldKey,
        value: f64,
        unit: Option<Localized>,
    ) -> Element<'a, PanelMsg> {
        let field = numeric_input("", key, value, self.draft)
            .size(TS.body)
            .padding([SP.xs, SP.s])
            .font(mono())
            .style(form_look::text_input_style)
            .width(Length::Fill);
        match unit {
            Some(unit) => row![field, text(unit).size(TS.caption).color(self.look.ink4)]
                .spacing(SP.xs)
                .align_y(Alignment::Center)
                .boxed(),
            None => field.boxed(),
        }
    }

    /// A read-only value on the sunken field: one line, ending in an
    /// ellipsis when it does not fit.
    pub fn read_only<'a>(&self, value: Label<'a>, is_mono: bool) -> Element<'a, PanelMsg> {
        use iced::widget::text::{Ellipsis, Wrapping};
        let mut line = text(value)
            .size(TS.body)
            .color(self.look.ink3)
            .wrapping(Wrapping::None)
            .ellipsis(Ellipsis::End)
            .width(Length::Fill);
        if is_mono {
            line = line.font(mono());
        }
        self.sunken(line.boxed())
    }

    /// A read-only note on the sunken field, wrapping.
    pub fn note<'a>(&self, value: Localized) -> Element<'a, PanelMsg> {
        self.sunken(text(value).size(TS.body).color(self.look.ink3).boxed())
    }

    fn sunken<'a>(&self, content: Element<'a, PanelMsg>) -> Element<'a, PanelMsg> {
        let (fill, edge) = (self.look.sunken, self.look.line_soft);
        container(content)
            .padding([SP.xs, SP.s])
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(fill)),
                border: Border {
                    width: HAIRLINE,
                    color: edge,
                    radius: super::super::graphite::PROPERTY_FIELD_RADIUS.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            })
            .boxed()
    }

    /// A muted caption line.
    pub fn caption<'a>(&self, line: Localized) -> Element<'a, PanelMsg> {
        container(text(line).size(TS.caption).color(self.look.ink4))
            .padding([SP.m, SP.m])
            .width(Length::Fill)
            .boxed()
    }

    /// A checkbox and its label.
    pub fn check<'a>(
        &self,
        label: Localized,
        checked: bool,
        msg: PanelMsg,
    ) -> Element<'a, PanelMsg> {
        row![
            checkbox(checked)
                .size(TS.heading)
                .style(form_look::checkbox_style)
                .on_toggle(move |_| msg.clone()),
            text(label).size(TS.body).color(self.look.ink),
        ]
        .spacing(SP.s)
        .align_y(Alignment::Center)
        .boxed()
    }

    /// A dropdown over `choices`; `selected` names the chosen value.
    pub fn dropdown<'a, V>(
        &self,
        choices: Vec<Choice<V>>,
        selected: Option<V>,
        on_select: impl Fn(V) -> PanelMsg + 'a,
    ) -> Element<'a, PanelMsg>
    where
        V: Clone + PartialEq + 'a,
    {
        let chosen = selected.and_then(|v| choices.iter().find(|c| c.value == v).cloned());
        iced::widget::pick_list(chosen, choices, |c: &Choice<V>| c.label.clone())
            .graphite()
            .on_select(move |c: Choice<V>| on_select(c.value))
            .padding([SP.xs, SP.s])
            .width(Length::Fill)
            .boxed()
    }

    /// A segmented control on the sunken frame, one cell per option.
    pub fn segmented<'a>(
        &self,
        options: Vec<(Localized, bool, PanelMsg)>,
    ) -> Element<'a, PanelMsg> {
        let mut cells: Row<Element<'a, PanelMsg>> = Row::new().width(Length::Fill);
        for (label, active, msg) in options {
            let cell = button(
                text(label)
                    .size(TS.body)
                    .align_x(iced::alignment::Horizontal::Center),
            )
            .padding([SP.xs, SP.xs])
            .width(Length::Fill)
            .on_press(msg)
            .style(move |theme: &Theme, status: button::Status| {
                let hovered = matches!(status, button::Status::Hovered);
                form_look::segment_cell(theme, active, hovered && !active)
            });
            cells = cells.push(cell.boxed());
        }
        container(cells)
            .padding(SP.xs)
            .width(Length::Fill)
            .style(form_look::segmented_frame)
            .boxed()
    }

    /// A small ghost button with `tip` on hover; `msg: None` draws it
    /// disabled.
    pub fn ghost<'a>(
        &self,
        label: Localized,
        msg: Option<PanelMsg>,
        tip: Option<Localized>,
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
        match tip {
            Some(tip) => self.tip(btn.boxed(), tip),
            None => btn.boxed(),
        }
    }

    /// `content` with `tip` shown under it on hover.
    pub fn tip<'a>(&self, content: Element<'a, PanelMsg>, tip: Localized) -> Element<'a, PanelMsg> {
        let hint = container(text(tip).size(TS.caption).color(self.look.ink))
            .padding([SP.xs, SP.m])
            .style(signex_widgets::active_bar_dropdown::tooltip_style(
                hover_overlay(self.tokens),
                self.look.line,
            ));
        tooltip(content, hint, Position::Bottom).gap(SP.xs).boxed()
    }

    /// The Symbol / Pins tab bar: underlined tabs, an optional count after a
    /// label, a hairline under the bar.
    pub fn tabs<'a>(
        &self,
        tabs: Vec<(Localized, Option<usize>, bool, PanelMsg)>,
    ) -> Element<'a, PanelMsg> {
        let height = TS.body * TAB_HEIGHT_EM;
        let mut bar: Row<Element<'a, PanelMsg>> = Row::new().padding([0.0, SP.s]);
        for (label, count, active, msg) in tabs {
            let ink = if active {
                self.look.ink
            } else {
                self.look.ink3
            };
            let mut label_text = text(label).size(TS.body).color(ink);
            if active {
                label_text = label_text.font(Font {
                    weight: iced::font::Weight::Medium,
                    ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
                });
            }
            let mut content: Row<Element<'a, PanelMsg>> = Row::new()
                .spacing(SP.xs)
                .align_y(Alignment::Center)
                .push(label_text.boxed());
            if let Some(n) = count {
                content = content.push(
                    text(signex_i18n::UserText::new(self.i18n.count(n as i64)))
                        .size(TS.caption)
                        .font(mono())
                        .color(self.look.ink4)
                        .boxed(),
                );
            }
            let tab = button(
                container(content)
                    .height(height - TS.body * TAB_BAR_EM)
                    .align_y(iced::alignment::Vertical::Center),
            )
            .padding([0.0, SP.l])
            .on_press(msg)
            .style(|_: &Theme, _| button::Style::default());
            let accent = self.look.accent;
            let underline = container(Space::new())
                .height(TS.body * TAB_BAR_EM)
                .width(Length::Fill)
                .style(move |_: &Theme| container::Style {
                    background: active.then_some(Background::Color(accent)),
                    border: Border {
                        radius: (TS.body * TAB_BAR_EM).into(),
                        ..Border::default()
                    },
                    ..container::Style::default()
                });
            bar = bar.push(
                column![tab, container(underline).padding([0.0, SP.m])]
                    .width(Length::Shrink)
                    .boxed(),
            );
        }
        let line = self.look.line;
        let rule = container(Space::new())
            .height(HAIRLINE)
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(line)),
                ..container::Style::default()
            });
        column![bar, rule]
            .width(Length::Fill)
            .padding(iced::Padding {
                top: SP.xs,
                ..iced::Padding::ZERO
            })
            .boxed()
    }
}

/// One entry of a [`Form::dropdown`]: a value and its label. Two entries are
/// equal when their values are.
#[derive(Debug, Clone)]
pub(super) struct Choice<V> {
    pub value: V,
    pub label: String,
}

impl<V: PartialEq> PartialEq for Choice<V> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl<V> Choice<V> {
    pub fn new(value: V, label: Localized) -> Self {
        Self {
            value,
            label: label.into_string(),
        }
    }
}
