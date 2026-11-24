//! Rows shared by the track, via and pad panels: a section, a label and
//! value, a typed field with its draft, a dropdown, a link, and the
//! disabled rows (with a tooltip) for what the board cannot store yet.

use iced::widget::{Column, Space, column, container, row};
use iced::{Background, Border, Color, Element, Length, Theme, Widget as _};
use signex_i18n::{Localized, MessageId, UserText, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::{CheckboxLabel as _, Label, button, checkbox, text, text_input, tooltip};

use super::{PcbPrimitiveMsg, PrimitiveCommon, PrimitiveField, SP, TS, format_number};
use crate::panels::{PROPERTY_ROW_PAD_X, PanelContext, PanelMsg, form_look};
use crate::styles::{ti, tokens::resolve};

/// The row inset every Properties row uses.
pub(super) const PAD_X: f32 = PROPERTY_ROW_PAD_X as f32;
const LABEL_PORTION: u16 = 4;
const VALUE_PORTION: u16 = 6;

/// The colours and catalog a panel draws with.
#[derive(Clone, Copy)]
pub(super) struct Look<'a> {
    pub ctx: &'a PanelContext,
    pub ink: Color,
    pub muted: Color,
}

impl<'a> Look<'a> {
    pub fn of(ctx: &'a PanelContext) -> Self {
        Self {
            ctx,
            ink: ti(ctx.tokens.text),
            muted: resolve(&ctx.tokens).ink3,
        }
    }

    pub fn t(&self, id: MessageId) -> Localized {
        t!(self.ctx.i18n, id)
    }

    pub fn tokens(&self) -> &'a ThemeTokens {
        &self.ctx.tokens
    }
}

/// A section: its title, then the rows the search leaves.
pub(super) struct Section<'a> {
    look: Look<'a>,
    rows: Vec<Element<'a, PanelMsg>>,
    title: MessageId,
    collapse: Option<Collapse<'a>>,
    badge: Option<Element<'a, PanelMsg>>,
}

/// How a collapsible section folds: its key in the panel's collapsed set,
/// whether it starts closed, and the summary its header shows.
struct Collapse<'a> {
    key: &'static str,
    closed_by_default: bool,
    summary: Option<Label<'a>>,
}

impl Collapse<'_> {
    /// The entry the panel's set holds for this section
    /// (`widgets::section_toggle_key`).
    fn set_key(&self) -> String {
        crate::panels::widgets::section_toggle_key(self.key, self.closed_by_default)
    }

    fn is_closed(&self, ctx: &PanelContext) -> bool {
        !crate::panels::widgets::is_section_open(
            self.key,
            self.closed_by_default,
            &ctx.collapsed_sections,
        )
    }
}

impl<'a> Section<'a> {
    pub fn new(look: Look<'a>, title: MessageId) -> Self {
        Self {
            look,
            rows: Vec::new(),
            title,
            collapse: None,
            badge: None,
        }
    }

    /// A section the chevron in its header folds; `summary` sits right in
    /// the header (a closed Net Information names the net and its class).
    pub fn collapsible(
        mut self,
        key: &'static str,
        closed_by_default: bool,
        summary: Option<Label<'a>>,
    ) -> Self {
        self.collapse = Some(Collapse {
            key,
            closed_by_default,
            summary,
        });
        self
    }

    /// A chip after the summary in the header of a collapsible section (the
    /// via's protection class, mockup row 96).
    pub fn badge(mut self, badge: Element<'a, PanelMsg>) -> Self {
        self.badge = Some(badge);
        self
    }

    /// Add `row`, labelled `label`, when the search lets it through.
    pub fn push(&mut self, common: &PrimitiveCommon, label: MessageId, row: Element<'a, PanelMsg>) {
        if common.shows(&self.look.ctx.i18n, &self.look.t(label).into_string()) {
            self.rows.push(row);
        }
    }

    /// Add `row`, whose label the caller already matched to the search.
    pub fn push_row(&mut self, row: Element<'a, PanelMsg>) {
        self.rows.push(row);
    }

    /// The section, or nothing when the search left no row.
    pub fn finish(self) -> Option<Element<'a, PanelMsg>> {
        if self.rows.is_empty() {
            return None;
        }
        let title = self.look.t(self.title);
        let Some(collapse) = self.collapse else {
            let mut col: Column<Element<'a, PanelMsg>> = column![
                container(crate::panels::widgets::section_title(
                    title,
                    self.look.tokens(),
                    &self.look.ctx.i18n
                ))
                .padding([SP.s, PAD_X])
            ]
            .spacing(SP.xs);
            for r in self.rows {
                col = col.push(r);
            }
            return Some(col.boxed());
        };
        let is_closed = collapse.is_closed(self.look.ctx);
        let set_key = collapse.set_key();
        let header = super::linked::section_header(
            self.look,
            title,
            !is_closed,
            (collapse.summary, self.badge),
            set_key,
        );
        let mut col: Column<Element<'a, PanelMsg>> = column![header].spacing(SP.xs);
        if !is_closed {
            for r in self.rows {
                col = col.push(r);
            }
        }
        Some(col.boxed())
    }
}

/// The sections that are left, one under the other.
pub(super) fn stack<'a>(sections: Vec<Option<Element<'a, PanelMsg>>>) -> Element<'a, PanelMsg> {
    let mut col: Column<Element<'a, PanelMsg>> = column![].spacing(SP.m);
    for s in sections.into_iter().flatten() {
        col = col.push(s);
    }
    col.width(Length::Fill).boxed()
}

fn label_cell<'a>(look: Look<'a>, label: impl Into<Label<'a>>) -> Element<'a, PanelMsg> {
    container(
        text(label)
            .size(TS.body)
            .color(look.muted)
            .wrapping(iced::widget::text::Wrapping::Word),
    )
    .width(Length::FillPortion(LABEL_PORTION))
    .clip(true)
    .boxed()
}

/// A label and its value.
pub(super) fn labelled<'a>(
    look: Look<'a>,
    id: MessageId,
    value: Element<'a, PanelMsg>,
) -> Element<'a, PanelMsg> {
    labelled_text(look, look.t(id), value)
}

/// A label given as text (a layer's name) and its value.
pub(super) fn labelled_text<'a>(
    look: Look<'a>,
    label: impl Into<Label<'a>>,
    value: Element<'a, PanelMsg>,
) -> Element<'a, PanelMsg> {
    container(
        row![
            label_cell(look, label),
            container(value).width(Length::FillPortion(VALUE_PORTION)),
        ]
        .spacing(SP.m)
        .align_y(iced::Alignment::Center),
    )
    .padding([SP.xs, PAD_X])
    .width(Length::Fill)
    .boxed()
}

/// A read-only value in a field box.
pub(super) fn value_box<'a>(look: Look<'a>, value: impl Into<Label<'a>>) -> Element<'a, PanelMsg> {
    container(
        text(value)
            .size(TS.body)
            .color(look.ink)
            .wrapping(iced::widget::text::Wrapping::None),
    )
    .padding([SP.xs, SP.s])
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

/// The ink of a value the panel only reports.
pub(super) fn sunken_ink(look: Look<'_>) -> Color {
    ti(look.tokens().text_secondary)
}

/// `content` on the sunken fill with no frame, so it cannot be taken for a
/// field (mockup rows 96 and 97).
pub(super) fn sunken_box<'a>(content: Element<'a, PanelMsg>) -> Element<'a, PanelMsg> {
    container(content)
        .padding([SP.xs, SP.s])
        .width(Length::Fill)
        .style(form_look::read_only_value)
        .boxed()
}

/// A value the panel only reports, sunken; a long one wraps.
pub(super) fn sunken_value<'a>(
    look: Look<'a>,
    value: impl Into<Label<'a>>,
) -> Element<'a, PanelMsg> {
    sunken_box(
        text(value)
            .size(TS.body)
            .color(sunken_ink(look))
            .wrapping(iced::widget::text::Wrapping::Word)
            .boxed(),
    )
}

/// A length in mm, "N/A" when `None`.
pub(super) fn mm_or_na(look: Look<'_>, value: Option<f64>) -> Localized {
    match value {
        Some(v) => t!(look.ctx.i18n, signex_i18n::ids::PCB_PRIM_MM, "value" => format_number(v)),
        None => look.t(signex_i18n::ids::PCB_PRIM_NOT_APPLICABLE),
    }
}

/// A typed field: the draft while one is typed, else `value` (blank when
/// the selection does not share it). Read-only unless the selection is
/// editable. `unit` follows the field.
pub(super) fn field<'a>(
    common: &PrimitiveCommon,
    field: PrimitiveField,
    value: Option<String>,
    unit: Option<Localized>,
    look: Look<'a>,
) -> Element<'a, PanelMsg> {
    let input = input(common, field, value, look).style(form_look::text_input_style);
    match unit {
        Some(unit) => row![input, text(unit).size(TS.dense).color(look.muted)]
            .spacing(SP.s)
            .align_y(iced::Alignment::Center)
            .boxed(),
        None => input.boxed(),
    }
}

/// The text input of `field`, unstyled: the draft while one is typed, else
/// `value`. A value the selected items do not share reads "Mixed" in
/// italics. It sends what is typed and Enter while the selection is
/// editable.
pub(super) fn input<'a>(
    common: &PrimitiveCommon,
    field: PrimitiveField,
    value: Option<String>,
    look: Look<'a>,
) -> iced::widget::TextInput<'a, PanelMsg> {
    let shown = common
        .drafts
        .get(&field)
        .cloned()
        .or(value)
        .unwrap_or_default();
    let is_mixed = shown.is_empty() && common.target.items.len() > 1;
    let placeholder = if is_mixed {
        Label::from(look.t(signex_i18n::ids::PCB_PRIM_MIXED))
    } else {
        Label::from(UserText::new(String::new()))
    };
    let mut input = text_input(placeholder, shown)
        .size(TS.body)
        .padding(SP.s)
        .width(Length::Fill);
    if is_mixed {
        input = input.font(iced::Font {
            style: iced::font::Style::Italic,
            ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
        });
    }
    if common.target.is_placement() && super::is_placement_focus(field) {
        input = input.id(super::PLACEMENT_INPUT_ID.clone());
    }
    if common.is_editable() {
        let target = common.target.clone();
        let submitted = PcbPrimitiveMsg::FieldSubmitted {
            target: target.clone(),
            field,
        };
        input = input
            .on_input(move |text| {
                PanelMsg::PcbPrimitive(PcbPrimitiveMsg::FieldTyped {
                    target: target.clone(),
                    field,
                    text,
                })
            })
            .on_submit(PanelMsg::PcbPrimitive(submitted));
    }
    input
}

/// A length field in mm.
pub(super) fn mm_field<'a>(
    common: &PrimitiveCommon,
    id: PrimitiveField,
    value: Option<f64>,
    look: Look<'a>,
) -> Element<'a, PanelMsg> {
    field(common, id, value.map(format_number), None, look)
}

/// Two fields side by side, X then Y.
pub(super) fn pair<'a>(
    a: Element<'a, PanelMsg>,
    b: Element<'a, PanelMsg>,
) -> Element<'a, PanelMsg> {
    crate::panels::element_properties::rows::pair_line(a, b, MM_FIELD_MIN)
}

/// Narrowest an mm field gets beside its partner: its padding, the unit
/// after it and a six-character value. Below it the pair stacks.
const MM_FIELD_MIN: f32 = 3.0 * SP.s + TS.dense * 1.6 + TS.body * 3.75;

/// A dropdown over `options`; blank when the selection does not share a
/// value, inert when it cannot be edited.
pub(super) fn dropdown<'a, T>(
    common: &PrimitiveCommon,
    options: &'a [T],
    selected: Option<T>,
    on_select: impl Fn(T) -> PcbPrimitiveMsg + 'a,
) -> Element<'a, PanelMsg>
where
    T: Clone + Eq + std::fmt::Display + 'static,
{
    let list = iced::widget::pick_list(selected, options, |t: &T| t.to_string())
        .graphite()
        .padding(SP.s)
        .width(Length::Fill);
    if common.is_editable() {
        list.on_select(move |v| PanelMsg::PcbPrimitive(on_select(v)))
            .boxed()
    } else {
        list.boxed()
    }
}

/// A square colour chip for a layer.
pub(super) fn chip<'a>(colour: Color) -> Element<'a, PanelMsg> {
    container(Space::new())
        .width(Length::Fixed(TS.body))
        .height(Length::Fixed(TS.body))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(colour)),
            border: Border {
                width: 1.0,
                color: Color { a: 0.5, ..colour },
                radius: (TS.body / 5.0).into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// A text button styled as a link, with a tooltip; plain text without a
/// message.
pub(super) fn link<'a>(
    look: Look<'a>,
    label: impl Into<Label<'a>>,
    on_press: Option<PcbPrimitiveMsg>,
    tip: MessageId,
) -> Element<'a, PanelMsg> {
    let accent = ti(look.tokens().accent);
    let Some(msg) = on_press else {
        return value_box(look, label);
    };
    let link = button(text(label).size(TS.body).color(accent))
        .padding([SP.xs, 0.0])
        .style(|_: &Theme, _| iced::widget::button::Style::default())
        .on_press(PanelMsg::PcbPrimitive(msg));
    with_tip(look, link.boxed(), tip)
}

/// `content` with the catalog text `tip` under it on hover.
pub(super) fn with_tip<'a>(
    look: Look<'a>,
    content: Element<'a, PanelMsg>,
    tip: MessageId,
) -> Element<'a, PanelMsg> {
    tooltip(
        content,
        container(text(look.t(tip)).size(TS.body))
            .padding([SP.xs, SP.s])
            .style(crate::styles::context_menu(look.tokens())),
        iced::widget::tooltip::Position::Bottom,
    )
    .boxed()
}

/// A checkbox with its label, ticked when `checked` is `Some(true)`; it
/// sends `on_toggle` when the selection can be edited.
pub(super) fn live_checkbox<'a>(
    look: Look<'a>,
    common: &PrimitiveCommon,
    label: MessageId,
    checked: Option<bool>,
    on_toggle: impl Fn(bool) -> PcbPrimitiveMsg + 'a,
) -> Element<'a, PanelMsg> {
    let check = checkbox::<PanelMsg, Theme, iced::Renderer>(checked == Some(true))
        .labelled(look.t(label))
        .size(TS.body)
        .text_size(TS.body)
        .style(form_look::checkbox_style);
    if common.is_editable() {
        check
            .on_toggle(move |on| PanelMsg::PcbPrimitive(on_toggle(on)))
            .boxed()
    } else {
        check.boxed()
    }
}

/// A disabled field box showing `value`, with `tip`.
pub(super) fn fixed_value<'a>(
    look: Look<'a>,
    value: impl Into<Label<'a>>,
    tip: MessageId,
) -> Element<'a, PanelMsg> {
    let boxed = container(text(value).size(TS.body).color(resolve(look.tokens()).ink4))
        .padding([SP.xs, SP.s])
        .width(Length::Fill)
        .style(|theme: &Theme| {
            let (bg, line, radius) = form_look::value_box(theme);
            container::Style {
                background: Some(Background::Color(Color { a: 0.5, ..bg })),
                border: Border {
                    width: 1.0,
                    radius: radius.into(),
                    color: line,
                    ..Border::default()
                },
                ..container::Style::default()
            }
        });
    with_tip(look, boxed.boxed(), tip)
}

/// The Net row: its dropdown and pick button.
pub(super) fn net_row<'a>(look: Look<'a>, common: &'a PrimitiveCommon) -> Element<'a, PanelMsg> {
    labelled(
        look,
        signex_i18n::ids::PCB_PRIM_NET,
        net_value(look, common),
    )
}

/// The Net dropdown and its pick button.
pub(super) fn net_value<'a>(look: Look<'a>, common: &'a PrimitiveCommon) -> Element<'a, PanelMsg> {
    use signex_i18n::ids;
    let target = common.target.clone();
    let list = dropdown(common, &common.nets, common.net.clone(), move |choice| {
        PcbPrimitiveMsg::NetPicked {
            target: target.clone(),
            net: choice.number,
        }
    });
    let icon = iced::widget::svg(crate::icons::icon_dd_cross_probe(look.ctx.theme_id))
        .width(Length::Fixed(TS.label))
        .height(Length::Fixed(TS.label));
    let mut pick = button(icon)
        .padding([SP.xs, SP.s])
        .style(form_look::option_tile(common.picking_net));
    if common.is_editable() {
        pick = pick.on_press(PanelMsg::PcbPrimitive(PcbPrimitiveMsg::NetPickStarted {
            target: common.target.clone(),
        }));
    }
    let pick = with_tip(look, pick.boxed(), ids::PCB_PRIM_PICK_NET);
    row![container(list).width(Length::Fill), pick]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center)
        .boxed()
}

/// The Layer row of an arc, string or dimension (#2373): its colour chip
/// and a dropdown of the layers it can move to.
pub(super) fn any_layer_row<'a>(
    look: Look<'a>,
    common: &'a PrimitiveCommon,
    layers: &'a [super::AreaLayerChoice],
    layer: Option<&'a super::AreaLayerChoice>,
) -> Element<'a, PanelMsg> {
    labelled(
        look,
        signex_i18n::ids::PCB_PRIM_LAYER,
        any_layer_value(look, common, layers, layer),
    )
}

/// The colour chip and Layer dropdown of an arc, string, dimension or
/// drawing.
pub(super) fn any_layer_value<'a>(
    look: Look<'a>,
    common: &'a PrimitiveCommon,
    layers: &'a [super::AreaLayerChoice],
    layer: Option<&'a super::AreaLayerChoice>,
) -> Element<'a, PanelMsg> {
    let target = common.target.clone();
    let list = dropdown(common, layers, layer.cloned(), move |choice| {
        PcbPrimitiveMsg::AreaLayerPicked {
            target: target.clone(),
            layer: choice.layer,
        }
    });
    let swatch = layer.map_or_else(
        || {
            container(Space::new())
                .width(Length::Fixed(TS.body))
                .boxed()
        },
        |l| {
            chip(crate::pcb_canvas::layer_style::layer_colour(
                l.layer,
                look.tokens(),
            ))
        },
    );
    row![swatch, container(list).width(Length::Fill)]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center)
        .boxed()
}
