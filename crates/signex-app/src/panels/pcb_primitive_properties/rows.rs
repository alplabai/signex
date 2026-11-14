//! Rows shared by the track, via and pad panels: a section, a label and
//! value, a typed field with its draft, a dropdown, a link, and the
//! disabled rows (with a tooltip) for what the board cannot store yet.

use iced::widget::{Column, Space, column, container, row};
use iced::{Background, Border, Color, Element, Length, Theme, Widget as _};
use signex_i18n::{Localized, MessageId, UserText, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::{CheckboxLabel as _, Label, button, checkbox, text, text_input, tooltip};

use super::{
    PcbPrimitiveMsg, PrimitiveCommon, PrimitiveField, SP, TS, format_number,
};
use crate::panels::{PROPERTY_ROW_PAD_X, PanelContext, PanelMsg, form_look};
use crate::styles::{ti, tokens::resolve};

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
}

impl<'a> Section<'a> {
    pub fn new(look: Look<'a>, title: MessageId) -> Self {
        Self {
            look,
            rows: Vec::new(),
            title,
        }
    }

    /// Add `row`, labelled `label`, when the search lets it through.
    pub fn push(&mut self, common: &PrimitiveCommon, label: MessageId, row: Element<'a, PanelMsg>) {
        if common.shows(&self.look.t(label).into_string()) {
            self.rows.push(row);
        }
    }

    /// The section, or nothing when the search left no row.
    pub fn finish(self) -> Option<Element<'a, PanelMsg>> {
        if self.rows.is_empty() {
            return None;
        }
        let title = self.look.t(self.title).into_string();
        let mut col: Column<Element<'a, PanelMsg>> = column![
            container(crate::panels::widgets::section_title(
                &title,
                self.look.tokens(),
                &self.look.ctx.i18n
            ))
            .padding([SP.s, PROPERTY_ROW_PAD_X])
        ]
        .spacing(SP.xs);
        for r in self.rows {
            col = col.push(r);
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

fn label_cell<'a>(look: Look<'a>, id: MessageId) -> Element<'a, PanelMsg> {
    container(
        text(look.t(id))
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
    container(
        row![
            label_cell(look, id),
            container(value).width(Length::FillPortion(VALUE_PORTION)),
        ]
        .spacing(SP.m)
        .align_y(iced::Alignment::Center),
    )
    .padding([SP.xs, PROPERTY_ROW_PAD_X])
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
    let shown = common
        .drafts
        .get(&field)
        .cloned()
        .or(value)
        .unwrap_or_default();
    let mut input = text_input(UserText::new(String::new()), shown)
        .size(TS.body)
        .padding(SP.s)
        .style(form_look::text_input_style)
        .width(Length::Fill);
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
    match unit {
        Some(unit) => row![input, text(unit).size(TS.dense).color(look.muted)]
            .spacing(SP.s)
            .align_y(iced::Alignment::Center)
            .boxed(),
        None => input.boxed(),
    }
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
pub(super) fn pair<'a>(a: Element<'a, PanelMsg>, b: Element<'a, PanelMsg>) -> Element<'a, PanelMsg> {
    row![a, b].spacing(SP.s).boxed()
}

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
        container(text(look.t(tip)).size(TS.body)).padding([SP.xs, SP.s]),
        iced::widget::tooltip::Position::Bottom,
    )
    .style(crate::styles::context_menu(look.tokens()))
    .boxed()
}

/// Option tiles where `active` is chosen; inert, with `tip`, because the
/// board stores only one of the options.
pub(super) fn fixed_tiles<'a>(
    look: Look<'a>,
    options: &[MessageId],
    active: usize,
    tip: MessageId,
) -> Element<'a, PanelMsg> {
    let mut tiles = row![].spacing(SP.xs);
    for (i, id) in options.iter().enumerate() {
        tiles = tiles.push(
            button(text(look.t(*id)).size(TS.body))
                .padding([SP.xs, SP.s])
                .style(form_look::option_tile(i == active)),
        );
    }
    with_tip(look, tiles.boxed(), tip)
}

/// A disabled checkbox with its label and `tip`.
pub(super) fn fixed_checkbox<'a>(
    look: Look<'a>,
    label: MessageId,
    checked: bool,
    tip: MessageId,
) -> Element<'a, PanelMsg> {
    let check = checkbox::<PanelMsg, Theme, iced::Renderer>(checked)
        .labelled(look.t(label))
        .size(TS.body)
        .text_size(TS.body)
        .style(form_look::checkbox_style);
    with_tip(look, check.boxed(), tip)
}

/// A disabled field box showing `value`, with `tip`.
pub(super) fn fixed_value<'a>(
    look: Look<'a>,
    value: impl Into<Label<'a>>,
    tip: MessageId,
) -> Element<'a, PanelMsg> {
    let boxed = container(
        text(value)
            .size(TS.body)
            .color(resolve(look.tokens()).ink4),
    )
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

/// The lock toggle beside a location, disabled with `tip`.
pub(super) fn lock<'a>(look: Look<'a>, locked: bool, tip: MessageId) -> Element<'a, PanelMsg> {
    fixed_checkbox(look, signex_i18n::ids::PCB_PRIM_LOCK, locked, tip)
}

/// A mask-expansion section: Rule chosen, Manual and the value disabled
/// until the board stores an expansion per item.
pub(super) fn mask_section<'a>(
    look: Look<'a>,
    common: &PrimitiveCommon,
    title: MessageId,
    sides: bool,
) -> Option<Element<'a, PanelMsg>> {
    use signex_i18n::ids;
    let tip = ids::PCB_PRIM_MASK_UNSUPPORTED;
    let mut s = Section::new(look, title);
    s.push(
        common,
        ids::PCB_PRIM_RULE,
        fixed_tiles(look, &[ids::PCB_PRIM_RULE, ids::PCB_PRIM_MANUAL], 0, tip),
    );
    if sides {
        for side in [ids::PCB_PRIM_TOP, ids::PCB_PRIM_BOTTOM] {
            s.push(common, side, labelled(look, side, fixed_value(look, look.t(ids::PCB_PRIM_RULE), tip)));
            s.push(
                common,
                ids::PCB_PRIM_TENTED,
                labelled(look, ids::PCB_PRIM_TENTED, fixed_checkbox(look, side, false, tip)),
            );
        }
        s.push(
            common,
            ids::PCB_PRIM_FROM_HOLE_EDGE,
            labelled(
                look,
                ids::PCB_PRIM_FROM_HOLE_EDGE,
                fixed_checkbox(look, ids::PCB_PRIM_FROM_HOLE_EDGE, false, tip),
            ),
        );
    } else {
        s.push(
            common,
            ids::PCB_PRIM_EXPANSION,
            labelled(
                look,
                ids::PCB_PRIM_EXPANSION,
                fixed_value(look, look.t(ids::PCB_PRIM_RULE), tip),
            ),
        );
    }
    s.finish()
}

/// The Testpoint section: Fabrication and Assembly, top and bottom, all
/// disabled until the board stores them.
pub(super) fn testpoint_section<'a>(
    look: Look<'a>,
    common: &PrimitiveCommon,
) -> Option<Element<'a, PanelMsg>> {
    use signex_i18n::ids;
    let tip = ids::PCB_PRIM_TESTPOINT_UNSUPPORTED;
    let mut s = Section::new(look, ids::PCB_PRIM_TESTPOINT);
    for kind in [ids::PCB_PRIM_FABRICATION, ids::PCB_PRIM_ASSEMBLY] {
        let sides = row![
            fixed_checkbox(look, ids::PCB_PRIM_TOP, false, tip),
            fixed_checkbox(look, ids::PCB_PRIM_BOTTOM, false, tip),
        ]
        .spacing(SP.m);
        s.push(common, kind, labelled(look, kind, sides.boxed()));
    }
    s.finish()
}

/// The Template and Library rows, disabled until templates exist.
pub(super) fn template_rows<'a>(look: Look<'a>, common: &PrimitiveCommon, s: &mut Section<'a>) {
    use signex_i18n::ids;
    let tip = ids::PCB_PRIM_TEMPLATE_UNSUPPORTED;
    s.push(
        common,
        ids::PCB_PRIM_TEMPLATE,
        labelled(look, ids::PCB_PRIM_TEMPLATE, fixed_value(look, look.t(ids::PCB_PRIM_NONE), tip)),
    );
    s.push(
        common,
        ids::PCB_PRIM_LIBRARY,
        labelled(
            look,
            ids::PCB_PRIM_LIBRARY,
            fixed_value(look, look.t(ids::PCB_PRIM_LIBRARY_LOCAL), tip),
        ),
    );
}

/// The Net dropdown and its pick button.
pub(super) fn net_row<'a>(look: Look<'a>, common: &'a PrimitiveCommon) -> Element<'a, PanelMsg> {
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
    labelled(
        look,
        ids::PCB_PRIM_NET,
        row![container(list).width(Length::Fill), pick]
            .spacing(SP.s)
            .align_y(iced::Alignment::Center)
            .boxed(),
    )
}
