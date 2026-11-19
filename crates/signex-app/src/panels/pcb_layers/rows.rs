//! The pieces a Layers panel row is built of (#2363, m103): an eye, a round
//! colour dot, a name, the colour card that opens under a dot, and a
//! section with its header (count pill, section eye). A control the board
//! view cannot honour yet is greyed and says so in its tooltip.

use iced::alignment::Horizontal;
use iced::font::Weight;
use iced::widget::tooltip::Position;
use iced::widget::{Column, Row, Space, container, row, svg};
use iced::{Alignment, Background, Border, Color, Element, Font, Length, Padding, Theme, Widget};
use signex_i18n::{I18n, Localized, MessageId, UserText, ids, t};
use signex_renderer::pcb::BoardPaint;
use signex_types::pcb_layer::PcbLayer;
use signex_widgets::{Label, button, text};

use super::metrics::{
    ACCENT_BAR, ACCENT_BAR_INSET, DOT, DOT_GAP, DOT_RING, EYE, HIDDEN_DOT_ALPHA, ICON, NUMBER,
    PILL, PILL_PAD, ROW, ROW_GAP, SECTION_HEAD,
};
use super::{
    LayersPanelMsg, LayersPanelUi, PcbLayersPanel, SP, TS, colour_card, eye_handle, row_button,
};
use crate::app::PcbViewMsg;
use crate::panels::graphite::hint_tooltip;
use crate::panels::{PanelContext, PanelMsg};
use crate::styles::{PillTone, Radii, pill, ti, tokens::resolve};
use iced::widget::button::Status;

/// `content` with the "cannot do this yet" tooltip.
pub(super) fn not_yet<'a>(
    content: Element<'a, PanelMsg>,
    panel: &PcbLayersPanel,
    ctx: &PanelContext,
) -> Element<'a, PanelMsg> {
    hint_tooltip(
        content,
        t!(panel.i18n, ids::PCB_LAYERS_NOT_YET),
        &ctx.tokens,
        Position::Top,
    )
}

/// A catalog message in capitals for a caption or a section title: already
/// rendered, so it is carried as finished text, as the Library Browser's
/// preview captions are.
pub(super) fn caps(i18n: &I18n, id: MessageId) -> Label<'static> {
    UserText::new(i18n.upper_for_display(&t!(i18n, id).into_string())).into()
}

/// Medium weight of the UI face.
pub(super) fn medium() -> Font {
    Font {
        weight: Weight::Medium,
        ..Font::DEFAULT
    }
}

/// A layer's eye: struck while `hidden`; greyed with the "not yet" tooltip
/// without a message.
pub(super) fn eye<'a>(
    panel: &PcbLayersPanel,
    hidden: bool,
    on_press: Option<PanelMsg>,
    ctx: &PanelContext,
) -> Element<'a, PanelMsg> {
    let tip = if hidden {
        t!(panel.i18n, ids::PCB_LAYERS_SHOW)
    } else {
        t!(panel.i18n, ids::PCB_LAYERS_HIDE)
    };
    eye_with_tip(panel, hidden, on_press, tip, ctx)
}

/// A section's eye: shut while every row of the section is hidden.
fn section_eye<'a>(
    panel: &PcbLayersPanel,
    hidden: bool,
    on_press: Option<PanelMsg>,
    ctx: &PanelContext,
) -> Element<'a, PanelMsg> {
    let tip = if hidden {
        t!(panel.i18n, ids::PCB_LAYERS_SECTION_SHOW)
    } else {
        t!(panel.i18n, ids::PCB_LAYERS_SECTION_HIDE)
    };
    eye_with_tip(panel, hidden, on_press, tip, ctx)
}

fn eye_with_tip<'a>(
    panel: &PcbLayersPanel,
    hidden: bool,
    on_press: Option<PanelMsg>,
    tip: Localized,
    ctx: &PanelContext,
) -> Element<'a, PanelMsg> {
    let tokens = &ctx.tokens;
    let enabled = on_press.is_some();
    let ink = if hidden || !enabled {
        resolve(tokens).ink4
    } else {
        ti(tokens.text)
    };
    let icon = svg(eye_handle(hidden))
        .width(ICON)
        .height(ICON)
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink),
            ..Default::default()
        });
    let eye = button(container(icon).center_x(EYE).center_y(EYE))
        .padding(0)
        .style(row_button(tokens))
        .on_press_maybe(on_press)
        .boxed();
    if !enabled {
        return not_yet(eye, panel, ctx);
    }
    // Above the eye, so the hint never covers this row's name.
    hint_tooltip(eye, tip, &ctx.tokens, Position::Top)
}

/// A round colour dot with a thin ring, so a white or pale colour still
/// reads on the light panel; no fill draws only the ring.
pub(super) fn dot_face<'a>(
    fill: Option<Color>,
    faint: bool,
    ctx: &PanelContext,
) -> Element<'a, PanelMsg> {
    let ring = resolve(&ctx.tokens).line_strong;
    let fill = fill.map(|colour| {
        if faint {
            Color {
                a: colour.a * HIDDEN_DOT_ALPHA,
                ..colour
            }
        } else {
            colour
        }
    });
    container(Space::new())
        .width(DOT)
        .height(DOT)
        .style(move |_: &Theme| container::Style {
            background: fill.map(Background::Color),
            border: Border {
                width: 1.0,
                color: ring,
                radius: Radii::GRAPHITE.pill.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// A layer's colour dot: a click opens the colour card of `layer`, and the
/// open dot gets the accent ring. Without a layer it only shows `colour`;
/// a layer the view cannot recolour yet (the keep-out pair) adds the "not
/// yet" tooltip.
pub(super) fn chip<'a>(
    panel: &PcbLayersPanel,
    colour: Color,
    faint: bool,
    layer: Option<PcbLayer>,
    ctx: &PanelContext,
) -> Element<'a, PanelMsg> {
    let face = dot_face(Some(colour), faint, ctx);
    let inert = |face: Element<'a, PanelMsg>| container(face).padding(DOT_RING + DOT_GAP).boxed();
    let Some(layer) = layer else {
        return inert(face);
    };
    if !BoardPaint::can_recolour(layer) {
        return not_yet(inert(face), panel, ctx);
    }
    let open = ctx.pcb_layers_ui.picker == Some(layer);
    let r = resolve(&ctx.tokens);
    let (accent, hover) = (ti(ctx.tokens.accent), r.line_strong);
    let dot = button(face)
        .padding(DOT_RING + DOT_GAP)
        .style(move |_: &Theme, status| {
            let ring = if open {
                accent
            } else if matches!(status, Status::Hovered) {
                hover
            } else {
                Color::TRANSPARENT
            };
            iced::widget::button::Style {
                border: Border {
                    width: DOT_RING,
                    color: ring,
                    radius: Radii::GRAPHITE.pill.into(),
                    ..Border::default()
                },
                ..iced::widget::button::Style::default()
            }
        })
        .on_press(PanelMsg::LayersPanel(LayersPanelMsg::ColourChipToggled(
            layer,
        )))
        .boxed();
    hint_tooltip(
        dot,
        t!(panel.i18n, ids::PCB_LAYERS_COLOUR_TIP),
        &ctx.tokens,
        Position::Top,
    )
}

/// What one layer row shows and does.
pub(super) struct LayerLine {
    pub label: Label<'static>,
    pub layer: PcbLayer,
    /// The copper number in front of the dot.
    pub number: Option<usize>,
    /// Signal or Plane after the name.
    pub kind: Option<Localized>,
}

/// The slot left of a row: the accent bar while the row is the active one.
fn accent_slot<'a>(active: bool, ctx: &PanelContext) -> Element<'a, PanelMsg> {
    let accent = ti(ctx.tokens.accent);
    let bar = container(Space::new())
        .width(ACCENT_BAR)
        .height(ROW - 2.0 * ACCENT_BAR_INSET)
        .style(move |_: &Theme| container::Style {
            background: active.then_some(Background::Color(accent)),
            border: Border {
                radius: ACCENT_BAR.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    container(bar)
        .width(SP.s)
        .height(ROW)
        .align_y(iced::alignment::Vertical::Center)
        .boxed()
}

/// A row in the list: the accent slot, then `line` on the row's fill
/// (accent-soft while active), with the same margin on the right.
pub(super) fn framed<'a>(
    line: Row<Element<'a, PanelMsg>>,
    active: bool,
    ctx: &PanelContext,
) -> Element<'a, PanelMsg> {
    let fill = resolve(&ctx.tokens).accent_soft;
    let body = container(line.height(Length::Fill).align_y(Alignment::Center))
        .width(Length::Fill)
        .height(ROW)
        .padding([0.0, SP.s])
        .style(move |_: &Theme| container::Style {
            background: active.then_some(Background::Color(fill)),
            border: Border {
                radius: Radii::GRAPHITE.small.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    row![accent_slot(active, ctx), body, Space::new().width(SP.s)]
        .width(Length::Fill)
        .boxed()
}

/// The "Active" pill of the active row.
fn active_pill<'a>(panel: &PcbLayersPanel, ctx: &PanelContext) -> Element<'a, PanelMsg> {
    let ink = resolve(&ctx.tokens).accent_soft_ink;
    container(
        text(t!(panel.i18n, ids::PCB_LAYERS_ACTIVE))
            .size(TS.caption)
            .color(ink),
    )
    .padding([0.0, PILL_PAD])
    .height(PILL)
    .align_y(iced::alignment::Vertical::Center)
    .style(pill(&ctx.tokens, PillTone::Accent))
    .boxed()
}

/// A count pill of a section header.
fn count_pill<'a>(count: Localized, ctx: &PanelContext) -> Element<'a, PanelMsg> {
    let ink = resolve(&ctx.tokens).ink3;
    container(text(count).size(TS.caption).color(ink))
        .padding([0.0, PILL_PAD])
        .height(PILL)
        .align_y(iced::alignment::Vertical::Center)
        .style(pill(&ctx.tokens, PillTone::Neutral))
        .boxed()
}

/// One layer row: eye, number, dot, name (a click makes the layer active),
/// the Active pill and the type; the colour card under it while its dot is
/// open.
pub(super) fn layer_row<'a>(
    panel: &'a PcbLayersPanel,
    ui: &LayersPanelUi,
    line: LayerLine,
    ctx: &PanelContext,
) -> Element<'a, PanelMsg> {
    let tokens = &ctx.tokens;
    let r = resolve(tokens);
    let LayerLine {
        label,
        layer,
        number,
        kind,
    } = line;
    let hidden = panel.is_hidden(layer);
    let active = panel.active == Some(layer);
    let ink = if active {
        r.accent_soft_ink
    } else if hidden {
        r.ink4
    } else {
        ti(tokens.text)
    };
    let mut name = text(label).size(TS.body).color(ink).width(Length::Fill);
    if active {
        name = name.font(Font {
            weight: Weight::Semibold,
            ..Font::DEFAULT
        });
    }
    let mut named = Row::<Element<'a, PanelMsg>>::new()
        .spacing(ROW_GAP)
        .height(Length::Fill)
        .align_y(Alignment::Center)
        .push(name.boxed());
    if active {
        named = named.push(active_pill(panel, ctx));
    }
    if let Some(kind) = kind {
        named = named.push(text(kind).size(TS.caption).color(r.ink4).boxed());
    }
    let name = button(named)
        .padding([0.0, SP.xs])
        .width(Length::Fill)
        .height(ROW)
        .style(row_button(tokens))
        .on_press(PanelMsg::PcbLayers(PcbViewMsg::ActiveLayerPicked(layer)));
    let toggled = PanelMsg::PcbLayers(PcbViewMsg::LayerVisibilityToggled(layer));
    let mut line = Row::<Element<'a, PanelMsg>>::new()
        .spacing(ROW_GAP)
        .push(eye(panel, hidden, Some(toggled), ctx));
    if let Some(number) = number {
        let number = t!(panel.i18n, ids::PCB_LAYERS_COPPER_NUMBER, "number" => number);
        line = line.push(
            container(
                text(number)
                    .size(TS.caption)
                    .font(Font::MONOSPACE)
                    .color(r.ink4),
            )
            .width(NUMBER)
            .align_x(Horizontal::Right)
            .boxed(),
        );
    }
    let line = line
        .push(chip(panel, panel.colour(layer), hidden, Some(layer), ctx))
        .push(name.boxed());
    with_editor(
        framed(line, active, ctx),
        colour_card::view(panel, ui, layer, ctx),
    )
}

/// `line` with the colour card of its dot under it, when open.
pub(super) fn with_editor<'a>(
    line: Element<'a, PanelMsg>,
    editor: Option<Element<'a, PanelMsg>>,
) -> Element<'a, PanelMsg> {
    match editor {
        Some(editor) => Column::new().push(line).push(editor).boxed(),
        None => line,
    }
}

/// The head of a section: the count of its listed rows, its eye (hidden,
/// and what a click sends), and whether it starts closed.
#[derive(Default)]
pub(super) struct SectionHead {
    pub count: Option<Localized>,
    pub eye: Option<(bool, Option<PanelMsg>)>,
    pub closed_by_default: bool,
}

/// A section: its header and rows; nothing when the search or Used On left
/// no row. A press on the header opens or closes it. A section closed by
/// default keeps `"{key}.open"` in the collapsed set while it is open.
pub(super) fn section_with<'a>(
    ctx: &'a PanelContext,
    panel: &PcbLayersPanel,
    key: &str,
    title: MessageId,
    head: SectionHead,
    rows: Vec<Element<'a, PanelMsg>>,
) -> Option<Element<'a, PanelMsg>> {
    if rows.is_empty() {
        return None;
    }
    let (toggle, open) = if head.closed_by_default {
        let toggle = format!("{key}.open");
        let open = ctx.collapsed_sections.contains(&toggle);
        (toggle, open)
    } else {
        (key.to_string(), !ctx.collapsed_sections.contains(key))
    };
    let mut col = Column::<Element<'a, PanelMsg>>::new()
        .width(Length::Fill)
        .push(section_header(ctx, panel, title, head, open, toggle));
    if open {
        col = col.push(
            rows.into_iter()
                .fold(Column::new().spacing(0), |col, row| col.push(row))
                .boxed(),
        );
    }
    Some(col.boxed())
}

/// The header line of a section: a hairline above, then the chevron, the
/// title in capitals, the count pill and the section eye.
fn section_header<'a>(
    ctx: &PanelContext,
    panel: &PcbLayersPanel,
    title: MessageId,
    head: SectionHead,
    open: bool,
    toggle: String,
) -> Element<'a, PanelMsg> {
    let r = resolve(&ctx.tokens);
    let mut label = Row::<Element<'a, PanelMsg>>::new()
        .spacing(SP.s)
        .height(Length::Fill)
        .align_y(Alignment::Center)
        .push(signex_widgets::chevron::chevron(open, TS.section, r.ink4).boxed())
        .push(
            text(caps(&panel.i18n, title))
                .size(TS.section)
                .color(r.ink3)
                .font(medium())
                .width(Length::Fill)
                .boxed(),
        );
    if let Some(count) = head.count {
        label = label.push(count_pill(count, ctx));
    }
    let press = button(label)
        .padding(0)
        .width(Length::Fill)
        .height(SECTION_HEAD)
        .on_press(PanelMsg::ToggleSection(toggle))
        .style(|_: &Theme, _| iced::widget::button::Style::default());
    let mut line = Row::<Element<'a, PanelMsg>>::new()
        .spacing(SP.s)
        .align_y(Alignment::Center)
        .push(press.boxed());
    if let Some((hidden, on_press)) = head.eye {
        line = line.push(section_eye(panel, hidden, on_press, ctx));
    }
    let line_soft = r.line_soft;
    let rule = container(Space::new())
        .width(Length::Fill)
        .height(1.0)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(line_soft)),
            ..container::Style::default()
        });
    Column::<Element<'a, PanelMsg>>::new()
        .width(Length::Fill)
        .push(Space::new().height(SP.s).boxed())
        .push(rule.boxed())
        .push(
            container(line)
                .height(SECTION_HEAD)
                .padding(Padding {
                    left: SP.xs,
                    right: SP.s,
                    ..Padding::ZERO
                })
                .boxed(),
        )
        .boxed()
}
