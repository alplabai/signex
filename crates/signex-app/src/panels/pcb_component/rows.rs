//! The row builders of the Component view (Graphite m94): collapsible
//! sections with an optional control in the header, a label column on the
//! Graphite grid, then an input, a field-look value, a sunken read-only
//! value, a unit, a checkbox, an eye or a warning note. Every row carries its
//! label so the header's search can leave it out; a row linked to the
//! illustration lights up with its callout and reports its hover.

use iced::widget::{Column, Row, Space, container, mouse_area, svg};
use iced::{Background, Border, Element, Length, Padding, Theme, Widget as _};
use signex_i18n::{Localized, MessageId, UserText, t};
use signex_widgets::property_illustration::{FieldId, IllustrationHover, IllustrationMsg};
use signex_widgets::{button, text, text_input, tooltip};

use super::super::form_look;
use super::super::widgets::property_label;
use super::super::{PROPERTY_CONTROL_PORTION, PROPERTY_ROW_PAD_X, PanelContext, PanelMsg};
use super::{PcbComponentFacts, PcbComponentMsg, Shared};
use crate::styles::tokens::resolve;
use crate::styles::{HAIRLINE, PillTone, Radii, Spacing, TypeScale, pill_colors, ti};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The warning triangle of a note (the mockup's `i-warn` glyph).
const WARN_ICON: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><path d="M8 2.5l6 10.5H2z" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"/><path d="M8 6.5v3M8 11.3v.2" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/></svg>"#;

/// Label text that is already rendered: a catalog message kept as text for
/// the row search and the locale's upper case, or a parameter name from the
/// board.
fn rendered(label: String) -> signex_widgets::Label<'static> {
    signex_widgets::raw(label)
}

/// What every row of one view needs: the context, the label ink, the
/// lower-cased search text and the illustration hover.
pub(super) struct Form<'a> {
    pub ctx: &'a PanelContext,
    pub ink: iced::Color,
    search: String,
    hover: &'a IllustrationHover,
}

impl<'a> Form<'a> {
    pub fn new(ctx: &'a PanelContext, facts: &'a PcbComponentFacts) -> Self {
        Self {
            ctx,
            ink: resolve(&ctx.tokens).ink3,
            search: ctx.i18n.fold_for_search(facts.search.trim()),
            hover: &facts.illustration_hover,
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

    /// Whether a search is typed.
    pub fn is_searching(&self) -> bool {
        !self.search.is_empty()
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

/// How a section is drawn: its toggle key, its title, a control at the end
/// of its header, and whether it starts closed.
pub(super) struct SectionSpec<'a> {
    pub key: &'static str,
    pub title: MessageId,
    pub trailing: Option<Element<'a, PanelMsg>>,
    /// The section starts closed; its toggle opens it, and so does a search.
    pub starts_closed: bool,
    /// Drawn even when the search left no row (its header control matched).
    pub keep_empty: bool,
}

impl SectionSpec<'_> {
    pub fn new(key: &'static str, title: MessageId) -> Self {
        Self {
            key,
            title,
            trailing: None,
            starts_closed: false,
            keep_empty: false,
        }
    }
}

/// The rows of one section under its collapsible Graphite header; `None`
/// when the search left no row.
pub(super) fn section<'a>(
    form: &Form<'a>,
    key: &'static str,
    title: MessageId,
    rows: Vec<Element<'a, PanelMsg>>,
) -> Option<Element<'a, PanelMsg>> {
    section_with(form, SectionSpec::new(key, title), rows)
}

/// [`section`] with a header control or a closed start.
pub(super) fn section_with<'a>(
    form: &Form<'a>,
    spec: SectionSpec<'a>,
    rows: Vec<Element<'a, PanelMsg>>,
) -> Option<Element<'a, PanelMsg>> {
    if rows.is_empty() && !spec.keep_empty {
        return None;
    }
    // A search opens a section that starts closed.
    let open = (spec.starts_closed && form.is_searching())
        || super::super::widgets::is_section_open(
            spec.key,
            spec.starts_closed,
            &form.ctx.collapsed_sections,
        );
    let mut col = Column::new()
        .spacing(0)
        .width(Length::Fill)
        .push(section_header(form, spec, open));
    if open {
        let body = rows
            .into_iter()
            .fold(Column::new().spacing(0).width(Length::Fill), |col, row| {
                col.push(row)
            })
            .padding(Padding {
                bottom: SP.xs,
                ..Padding::ZERO
            });
        col = col.push_boxed(body);
    }
    Some(col.boxed())
}

/// A hairline over the chevron, the upper-case title and the header control.
fn section_header<'a>(form: &Form<'a>, spec: SectionSpec<'a>, open: bool) -> Element<'a, PanelMsg> {
    let r = resolve(&form.ctx.tokens);
    let title = form.ctx.i18n.upper_for_display(&form.text(spec.title));
    let label = Row::new()
        .push_boxed(signex_widgets::chevron::chevron(open, TS.section, r.ink4))
        .push_boxed(text(rendered(title)).size(TS.section).color(r.ink3))
        .spacing(SP.xs)
        .align_y(iced::Alignment::Center);
    let toggle = button(label)
        .padding([SP.s, SP.xs])
        .width(Length::Fill)
        .on_press(PanelMsg::ToggleSection(
            super::super::widgets::section_toggle_key(spec.key, spec.starts_closed),
        ))
        .style(|_: &Theme, _| iced::widget::button::Style::default());
    let mut header = Row::new()
        .push_boxed(toggle)
        .align_y(iced::Alignment::Center)
        .padding(Padding {
            left: SP.xs,
            right: f32::from(PROPERTY_ROW_PAD_X),
            ..Padding::ZERO
        });
    if let Some(trailing) = spec.trailing {
        header = header.push(trailing);
    }
    let rule = r.line_soft;
    let hairline = container(Space::new())
        .width(Length::Fill)
        .height(HAIRLINE)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(rule)),
            ..container::Style::default()
        });
    Column::new()
        .push_boxed(hairline)
        .push_boxed(header)
        .padding(Padding {
            top: SP.xs,
            ..Padding::ZERO
        })
        .boxed()
}

/// A label and its control on one row.
pub(super) fn row<'a>(
    form: &Form<'a>,
    label: String,
    control: Element<'a, PanelMsg>,
) -> Element<'a, PanelMsg> {
    labelled(form, label, None, form.ink, control)
}

/// A row whose label shows the catalog text `tip` on hover.
pub(super) fn row_with_label_tip<'a>(
    form: &Form<'a>,
    label: String,
    tip: MessageId,
    control: Element<'a, PanelMsg>,
) -> Element<'a, PanelMsg> {
    labelled(form, label, Some(tip), form.ink, control)
}

/// A row linked to the illustration by `field`: hovering it lights the
/// callouts of `field`, and it lights up (accent label, accent outline)
/// while they are hovered.
pub(super) fn linked_row<'a>(
    form: &Form<'a>,
    label: String,
    field: FieldId,
    control: Element<'a, PanelMsg>,
) -> Element<'a, PanelMsg> {
    let hot = form.hover.is_field_highlighted(&field);
    let r = resolve(&form.ctx.tokens);
    let (ink, control) = if hot {
        let accent = ti(form.ctx.tokens.accent);
        let outlined = container(control)
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                border: Border {
                    width: HAIRLINE,
                    color: accent,
                    radius: Radii::GRAPHITE.control.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            })
            .boxed();
        (r.accent_text, outlined)
    } else {
        (form.ink, control)
    };
    let hover = |msg| PanelMsg::PcbComponent(PcbComponentMsg::IllustrationHovered(msg));
    mouse_area(labelled(form, label, None, ink, control))
        .on_enter(hover(IllustrationMsg::FieldHovered(field.clone())))
        .on_exit(hover(IllustrationMsg::FieldLeft(field)))
        .boxed()
}

/// The one row layout: the label column (with an optional tooltip) and the
/// control.
fn labelled<'a>(
    form: &Form<'a>,
    label: String,
    tip: Option<MessageId>,
    ink: iced::Color,
    control: Element<'a, PanelMsg>,
) -> Element<'a, PanelMsg> {
    let mut label = property_label(rendered(label), ink);
    if let Some(tip) = tip {
        label = form.with_tip(label, tip);
    }
    container(
        Row::new()
            .push(label)
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

/// A row without a label column (a checkbox with its own label).
pub(super) fn wide<'a>(control: Element<'a, PanelMsg>) -> Element<'a, PanelMsg> {
    container(control)
        .padding([SP.xs, f32::from(PROPERTY_ROW_PAD_X)])
        .width(Length::Fill)
        .boxed()
}

/// What an editable input sends: a message per keystroke, then one on Enter.
pub(super) type InputEdit<'a> = (Box<dyn Fn(String) -> PanelMsg + 'a>, PanelMsg);

/// A text input showing `shown`. With `edit`, typing sends its first message
/// and Enter the second; without it the input is disabled.
pub(super) fn input<'a>(shown: String, edit: Option<InputEdit<'a>>) -> Element<'a, PanelMsg> {
    input_with(shown, None, edit)
}

/// [`input`] that shows `placeholder` in italics while it is empty: the
/// "Mixed" of a value the selected components do not share.
pub(super) fn input_with<'a>(
    shown: String,
    placeholder: Option<Localized>,
    edit: Option<InputEdit<'a>>,
) -> Element<'a, PanelMsg> {
    let italic = placeholder.is_some() && shown.is_empty();
    let placeholder: signex_widgets::Label<'a> = match placeholder {
        Some(line) => line.into(),
        None => UserText::new(String::new()).into(),
    };
    let mut field = text_input(placeholder, shown)
        .size(TS.body)
        .padding(SP.s)
        .style(form_look::text_input_style)
        .width(Length::Fill);
    if italic {
        field = field.font(iced::Font {
            style: iced::font::Style::Italic,
            ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
        });
    }
    if let Some((on_input, on_submit)) = edit {
        field = field.on_input(on_input).on_submit(on_submit);
    }
    field.boxed()
}

/// A text input inside a framed field (a prefix or an icon beside it): no
/// fill or border of its own, so the frame's field look shows.
pub(super) fn bare_input_style(
    theme: &Theme,
    _status: iced::widget::text_input::Status,
) -> iced::widget::text_input::Style {
    let l = form_look::FormLook::of(theme);
    iced::widget::text_input::Style {
        background: Background::Color(iced::Color::TRANSPARENT),
        border: Border::default(),
        placeholder: l.ink4,
        value: l.ink,
        selection: iced::Color { a: 0.4, ..l.accent },
    }
}

/// `content` (a bare input and what goes with it) in the field frame.
pub(super) fn framed<'a>(form: &Form<'a>, content: Element<'a, PanelMsg>) -> Element<'a, PanelMsg> {
    container(content)
        .padding(Padding {
            left: SP.m,
            right: SP.xs,
            ..Padding::ZERO
        })
        .width(Length::Fill)
        .style(crate::styles::field(&form.ctx.tokens, false))
        .boxed()
}

/// A value in the field look: the footprint name the picker changes.
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
                width: HAIRLINE,
                radius: radius.into(),
                color: line,
                ..Border::default()
            },
            ..container::Style::default()
        }
    })
    .boxed()
}

/// A fact the panel shows but does not edit: the sunken read-only field.
pub(super) fn read_only<'a>(form: &Form<'a>, shown: String) -> Element<'a, PanelMsg> {
    read_only_text(form, text(UserText::new(shown)).size(TS.body))
}

/// [`read_only`] holding a catalog line in the caption size ("One at a
/// time").
pub(super) fn read_only_caption<'a>(form: &Form<'a>, line: Localized) -> Element<'a, PanelMsg> {
    read_only_text(form, text(line).size(TS.caption))
}

fn read_only_text<'a>(
    form: &Form<'a>,
    shown: iced::widget::Text<'a, Theme>,
) -> Element<'a, PanelMsg> {
    container(
        shown
            .wrapping(iced::widget::text::Wrapping::None)
            .ellipsis(iced::widget::text::Ellipsis::End),
    )
    .padding([SP.s, SP.m])
    .width(Length::Fill)
    .clip(true)
    .style(crate::styles::read_only_field(&form.ctx.tokens))
    .boxed()
}

/// A unit after a field ("mm", "°"), in the faint caption ink.
pub(super) fn unit<'a>(form: &Form<'a>, id: MessageId) -> Element<'a, PanelMsg> {
    text(t!(form.ctx.i18n, id))
        .size(TS.caption)
        .color(resolve(&form.ctx.tokens).ink4)
        .boxed()
}

/// A control and its unit on one line.
pub(super) fn with_unit<'a>(
    form: &Form<'a>,
    control: Element<'a, PanelMsg>,
    id: MessageId,
) -> Element<'a, PanelMsg> {
    Row::new()
        .push(control)
        .push(unit(form, id))
        .spacing(SP.xs)
        .align_y(iced::Alignment::Center)
        .boxed()
}

/// A checkbox. A mixed value draws as the partly ticked box; without
/// `toggled` the box is disabled.
pub(super) fn check<'a>(value: &Shared<bool>, toggled: Option<PanelMsg>) -> Element<'a, PanelMsg> {
    checkbox_of(value, toggled).boxed()
}

/// [`check`] with its catalog label after the box.
pub(super) fn check_labelled<'a>(
    value: &Shared<bool>,
    label: Localized,
    toggled: Option<PanelMsg>,
) -> Element<'a, PanelMsg> {
    use signex_widgets::CheckboxLabel as _;
    checkbox_of(value, toggled)
        .labelled(label)
        .text_size(TS.body)
        .boxed()
}

fn checkbox_of<'a>(
    value: &Shared<bool>,
    toggled: Option<PanelMsg>,
) -> iced::widget::Checkbox<'a, PanelMsg, Theme> {
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
    boxed
}

/// The eye beside a text field: open while the text shows on the board.
/// `None` for `shown` is a component with no such text: the eye is disabled.
pub(super) fn eye<'a>(
    form: &Form<'a>,
    shown: Option<&Shared<bool>>,
    toggled: Option<PanelMsg>,
) -> Element<'a, PanelMsg> {
    let open = !matches!(shown, Some(Shared::Same(false)));
    let r = resolve(&form.ctx.tokens);
    let ink = if shown.is_some() && open {
        ti(form.ctx.tokens.text)
    } else {
        r.ink4
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
        None => signex_i18n::ids::PCB_COMPONENT_TEXT_MISSING,
        Some(Shared::Same(false)) => signex_i18n::ids::PCB_COMPONENT_TEXT_HIDDEN,
        Some(_) => signex_i18n::ids::PCB_COMPONENT_TEXT_SHOWN,
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
        pill_colors(tokens, PillTone::Warn).1
    } else {
        resolve(tokens).ink3
    };
    container(text(line).size(TS.dense).color(ink))
        .padding([SP.xs, SP.l])
        .width(Length::Fill)
        .boxed()
}

/// A warning note on the warn-soft ground: the triangle, the line, and an
/// action button at the end.
pub(super) fn callout<'a>(
    form: &Form<'a>,
    line: Localized,
    action: Element<'a, PanelMsg>,
) -> Element<'a, PanelMsg> {
    let (ground, ink) = pill_colors(&form.ctx.tokens, PillTone::Warn);
    let icon = svg(svg::Handle::from_memory(WARN_ICON))
        .width(TS.body)
        .height(TS.body)
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink),
            ..Default::default()
        });
    let body = Row::new()
        .push_boxed(icon)
        .push_boxed(text(line).size(TS.caption).color(ink).width(Length::Fill))
        .push(action)
        .spacing(SP.s)
        .align_y(iced::Alignment::Start);
    container(
        container(body)
            .padding([SP.s, SP.m])
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(ground)),
                border: Border {
                    radius: Radii::GRAPHITE.button.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            }),
    )
    .padding([SP.xs, f32::from(PROPERTY_ROW_PAD_X)])
    .width(Length::Fill)
    .boxed()
}

/// A small button in the caption size: "Change…", "Update PCB".
pub(super) fn small_button<'a>(
    line: Localized,
    pressed: Option<PanelMsg>,
) -> Element<'a, PanelMsg> {
    let enabled = pressed.is_some();
    button(text(line).size(TS.caption))
        .padding([SP.xs, SP.m])
        .on_press_maybe(pressed)
        .style(move |theme: &Theme, status| {
            let mut style = form_look::ghost_button(theme, status);
            if !enabled {
                style.text_color = form_look::FormLook::of(theme).ink4;
            }
            style
        })
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
