//! The row builders of the Component view: a label column on the Graphite
//! grid, then an input, a read-only value, a disabled row with a tooltip, a
//! checkbox or an eye. Every row carries its label so the header's search can
//! leave it out.

use iced::widget::{Column, Row, container, svg};
use iced::{Background, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{Localized, MessageId, UserText, ids, t};
use signex_widgets::{button, text, text_input, tooltip};

use super::super::form_look;
use super::super::widgets::property_label;
use super::super::{PROPERTY_CONTROL_PORTION, PROPERTY_ROW_PAD_X, PanelContext, PanelMsg};
use super::{PcbComponentFacts, Shared};
use crate::styles::{Spacing, TypeScale, ti, tokens::resolve};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// What every row of one view needs: the context, the label ink and the
/// lower-cased search text.
pub(super) struct Form<'a> {
    pub ctx: &'a PanelContext,
    pub ink: iced::Color,
    search: String,
}

impl<'a> Form<'a> {
    pub fn new(ctx: &'a PanelContext, facts: &PcbComponentFacts) -> Self {
        Self {
            ctx,
            ink: resolve(&ctx.tokens).ink3,
            search: ctx.i18n.fold_for_search(facts.search.trim()),
        }
    }

    /// The catalog text of `id`.
    pub fn text(&self, id: MessageId) -> String {
        t!(self.ctx.i18n, id).into_string()
    }

    /// Whether a row labelled `label` passes the search.
    pub fn shows(&self, label: &str) -> bool {
        self.search.is_empty() || self.ctx.i18n.fold_for_search(label).contains(&self.search)
    }

    /// `content` with the catalog text `tip` shown on hover.
    pub fn with_tip(
        &self,
        content: Element<'a, PanelMsg>,
        tip: MessageId,
    ) -> Element<'a, PanelMsg> {
        let tokens = &self.ctx.tokens;
        let hint = container(
            text(t!(self.ctx.i18n, tip))
                .size(TS.caption)
                .color(ti(tokens.text)),
        )
        .padding([SP.xs, SP.m])
        .style(signex_widgets::active_bar_dropdown::tooltip_style(
            ti(tokens.hover),
            ti(tokens.border),
        ));
        tooltip(content, hint, iced::widget::tooltip::Position::Bottom)
            .gap(SP.xs)
            .boxed()
    }
}

/// The rows of one section under its collapsible Graphite header; `None`
/// when the search left no row.
pub(super) fn section<'a>(
    form: &Form<'a>,
    key: &str,
    title: MessageId,
    rows: Vec<Element<'a, PanelMsg>>,
) -> Option<Element<'a, PanelMsg>> {
    if rows.is_empty() {
        return None;
    }
    let ctx = form.ctx;
    let title = form.text(title);
    Some(
        ctx.collapsible(key, &title, &ctx.collapsed_sections, move || {
            rows.into_iter()
                .fold(Column::new().spacing(0).width(Length::Fill), |col, row| {
                    col.push(row)
                })
        })
        .boxed(),
    )
}

/// A label and its control on one row.
pub(super) fn row<'a>(
    form: &Form<'a>,
    label: String,
    control: Element<'a, PanelMsg>,
) -> Element<'a, PanelMsg> {
    container(
        Row::new()
            .push(property_label(label, form.ink))
            .push(
                container(control)
                    .width(Length::FillPortion(PROPERTY_CONTROL_PORTION))
                    .boxed(),
            )
            .spacing(SP.m)
            .align_y(iced::Alignment::Center),
    )
    .padding([SP.xs, f32::from(PROPERTY_ROW_PAD_X)])
    .width(Length::Fill)
    .boxed()
}

/// What an editable input sends: a message per keystroke, then one on Enter.
pub(super) type InputEdit<'a> = (Box<dyn Fn(String) -> PanelMsg + 'a>, PanelMsg);

/// A text input showing `shown`. With `edit`, typing sends its first message
/// and Enter the second; without it the input is disabled.
pub(super) fn input<'a>(shown: String, edit: Option<InputEdit<'a>>) -> Element<'a, PanelMsg> {
    let mut field = text_input(UserText::new(String::new()), shown)
        .size(TS.body)
        .padding(SP.s)
        .style(form_look::text_input_style)
        .width(Length::Fill);
    if let Some((on_input, on_submit)) = edit {
        field = field.on_input(on_input).on_submit(on_submit);
    }
    field.boxed()
}

/// A read-only value in the Graphite value box.
pub(super) fn value<'a>(shown: String) -> Element<'a, PanelMsg> {
    container(
        text(UserText::new(shown))
            .size(TS.body)
            .wrapping(iced::widget::text::Wrapping::None)
            .ellipsis(iced::widget::text::Ellipsis::End)
            .style(|theme: &Theme| iced::widget::text::Style {
                color: Some(form_look::FormLook::of(theme).ink),
                ..Default::default()
            }),
    )
    .padding([SP.s, SP.m])
    .width(Length::Fill)
    .clip(true)
    .style(|theme: &Theme| {
        let (bg, line, radius) = form_look::value_box(theme);
        container::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                width: 1.0,
                radius: radius.into(),
                color: line,
                ..Border::default()
            },
            ..container::Style::default()
        }
    })
    .boxed()
}

/// A row the board cannot store or act on yet: an empty, disabled input
/// whose tooltip says why.
pub(super) fn unsupported<'a>(
    form: &Form<'a>,
    label: MessageId,
    why: MessageId,
) -> Option<Element<'a, PanelMsg>> {
    let label = form.text(label);
    form.shows(&label)
        .then(|| row(form, label, form.with_tip(input(String::new(), None), why)))
}

/// A checkbox row. A mixed value draws as the partly ticked box; without
/// `toggled` the box is disabled.
pub(super) fn check<'a>(value: &Shared<bool>, toggled: Option<PanelMsg>) -> Element<'a, PanelMsg> {
    let mixed = *value == Shared::Mixed;
    let mut boxed = signex_widgets::checkbox(value.same().copied().unwrap_or(true))
        .size(TS.heading)
        .spacing(SP.s)
        .style(if mixed {
            form_look::mixed_checkbox_style
        } else {
            form_look::checkbox_style
        });
    if let Some(message) = toggled {
        boxed = boxed.on_toggle(move |_| message.clone());
    }
    boxed.boxed()
}

/// The eye beside a text field: open while the text shows on the board.
/// `None` for `shown` is a component with no such text: the eye is disabled.
pub(super) fn eye<'a>(
    form: &Form<'a>,
    shown: Option<&Shared<bool>>,
    toggled: Option<PanelMsg>,
) -> Element<'a, PanelMsg> {
    let open = !matches!(shown, Some(Shared::Same(false)));
    let ink = if shown.is_some() {
        ti(form.ctx.tokens.text)
    } else {
        resolve(&form.ctx.tokens).ink4
    };
    let icon = svg(super::super::pcb_layers::eye_handle(!open))
        .width(TS.body)
        .height(TS.body)
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink),
            ..Default::default()
        });
    let mut eye = button(icon)
        .padding([SP.s, SP.s])
        .style(form_look::ghost_button);
    let tip = match shown {
        None => ids::PCB_COMPONENT_TEXT_MISSING,
        Some(Shared::Same(false)) => ids::PCB_COMPONENT_TEXT_HIDDEN,
        Some(_) => ids::PCB_COMPONENT_TEXT_SHOWN,
    };
    if let Some(message) = toggled {
        eye = eye.on_press(message);
    }
    form.with_tip(eye.boxed(), tip)
}

/// A line of explanation under a row, in the warning ink when `warning`.
pub(super) fn note<'a>(form: &Form<'a>, line: Localized, warning: bool) -> Element<'a, PanelMsg> {
    let tokens = &form.ctx.tokens;
    let ink = if warning {
        crate::styles::pill_colors(tokens, crate::styles::PillTone::Warn).1
    } else {
        resolve(tokens).ink3
    };
    container(text(line).size(TS.dense).color(ink))
        .padding([SP.xs, SP.l])
        .width(Length::Fill)
        .boxed()
}

/// `value` without trailing zeros: `12.7`, `90`, `-0.3333`.
pub(super) fn number_text(value: f64) -> String {
    let text = format!("{value:.4}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" {
        "0".into()
    } else {
        text.into()
    }
}
