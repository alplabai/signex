//! The pieces a Layers panel row is built of (#2363): an eye, a colour chip,
//! a name, and the colours that open under a chip. A control the board view
//! cannot honour yet is greyed and says so in its tooltip.

use std::rc::Rc;

use iced::widget::tooltip::Position;
use iced::widget::{Column, Space, container, row, svg};
use iced::{Alignment, Background, Border, Color, Element, Length, Theme, Widget};
use signex_i18n::{Localized, MessageId, ids, t};
use signex_renderer::pcb::{BoardPaint, DIMMED_ALPHA};
use signex_types::pcb_layer::PcbLayer;
use signex_widgets::{Label, button, text};

use super::{LayersPanelMsg, LayersPanelUi, PcbLayersPanel, SP, TS, eye_handle, row_button};
use crate::app::PcbViewMsg;
use crate::panels::graphite::{active_fill, hint_tooltip};
use crate::panels::{ColorFieldProps, PanelContext, PanelMsg, color_field};
use crate::styles::{Radii, ti, tokens::resolve};

/// The side of a chip or an eye, from the body text size.
pub(super) fn icon_size() -> f32 {
    TS.body + SP.xs
}

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

/// An eye: struck while `hidden`; greyed with the "not yet" tooltip
/// without a message.
pub(super) fn eye<'a>(
    panel: &PcbLayersPanel,
    hidden: bool,
    on_press: Option<PanelMsg>,
    ctx: &PanelContext,
) -> Element<'a, PanelMsg> {
    let tokens = &ctx.tokens;
    let enabled = on_press.is_some();
    let ink = if hidden || !enabled {
        resolve(tokens).ink3
    } else {
        ti(tokens.text)
    };
    let size = icon_size();
    let icon = svg(eye_handle(hidden))
        .width(size)
        .height(size)
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink),
            ..Default::default()
        });
    let eye = button(icon)
        .padding([SP.xs, SP.s])
        .style(row_button(tokens))
        .on_press_maybe(on_press)
        .boxed();
    if !enabled {
        return not_yet(eye, panel, ctx);
    }
    let tip = if hidden {
        t!(panel.i18n, ids::PCB_LAYERS_SHOW)
    } else {
        t!(panel.i18n, ids::PCB_LAYERS_HIDE)
    };
    // Above the eye, so the hint never covers this row's name.
    hint_tooltip(eye, tip, tokens, Position::Top)
}

/// A colour chip: a click opens the colours of `layer`; without a layer it
/// only shows `colour`, greyed; a layer the view cannot recolour yet (the
/// keep-out pair) adds the "not yet" tooltip.
pub(super) fn chip<'a>(
    panel: &PcbLayersPanel,
    colour: Color,
    faint: bool,
    layer: Option<PcbLayer>,
    ctx: &PanelContext,
) -> Element<'a, PanelMsg> {
    let fill = if faint {
        Color {
            a: DIMMED_ALPHA,
            ..colour
        }
    } else {
        colour
    };
    let size = icon_size() - SP.xs;
    let swatch = container(Space::new())
        .width(size)
        .height(size)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                width: 1.0,
                color: Color { a: 0.5, ..colour },
                radius: Radii::GRAPHITE.small.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    let Some(layer) = layer else {
        return container(swatch).padding([SP.xs, SP.xs]).boxed();
    };
    if !BoardPaint::can_recolour(layer) {
        return not_yet(
            container(swatch).padding([SP.xs, SP.xs]).boxed(),
            panel,
            ctx,
        );
    }
    let chip = button(swatch)
        .padding([SP.xs, SP.xs])
        .style(row_button(&ctx.tokens))
        .on_press(PanelMsg::LayersPanel(LayersPanelMsg::ColourChipToggled(
            layer,
        )))
        .boxed();
    hint_tooltip(
        chip,
        t!(panel.i18n, ids::PCB_LAYERS_COLOUR_TIP),
        &ctx.tokens,
        Position::Top,
    )
}

/// The colours of `layer` under its row while its chip is open: the
/// presets, Custom..., and the palette colour again.
pub(super) fn colour_editor<'a>(
    panel: &'a PcbLayersPanel,
    ui: &LayersPanelUi,
    layer: PcbLayer,
    ctx: &PanelContext,
) -> Option<Element<'a, PanelMsg>> {
    let (open, advanced) = ui.picker?;
    if open != layer {
        return None;
    }
    let current = panel.colour(layer);
    let rgba = current.into_rgba8();
    let set = move |colour| PanelMsg::PcbLayers(PcbViewMsg::LayerColourSet { layer, colour });
    let on_clear = panel.paint.colour(layer).map(|_| set(None));
    Some(color_field(ColorFieldProps {
        label: t!(panel.i18n, ids::PCB_LAYERS_COLOUR).into(),
        current: Some(rgba),
        none_label: t!(panel.i18n, ids::PCB_LAYERS_COLOUR_PALETTE).into(),
        show_palette: true,
        show_advanced: advanced,
        muted: resolve(&ctx.tokens).ink3,
        border_c: resolve(&ctx.tokens).line_strong,
        on_toggle: PanelMsg::LayersPanel(LayersPanelMsg::ColourChipToggled(layer)),
        on_advanced: PanelMsg::LayersPanel(LayersPanelMsg::CustomColourOpened),
        on_cancel: PanelMsg::LayersPanel(LayersPanelMsg::CustomColourClosed),
        on_pick: Rc::new(move |[r, g, b, _]: [u8; 4]| set(Some([r, g, b]))),
        on_clear,
    }, &panel.i18n))
}

/// What one layer row shows and does.
pub(super) struct LayerLine {
    pub label: Label<'static>,
    pub layer: PcbLayer,
    /// The copper number in front of the name.
    pub number: Option<usize>,
    /// Signal or Plane after the name.
    pub kind: Option<Localized>,
}

/// One layer row: eye, chip, name (a click makes the layer active), and
/// the colours under it while its chip is open.
pub(super) fn layer_row<'a>(
    panel: &'a PcbLayersPanel,
    ui: &LayersPanelUi,
    line: LayerLine,
    ctx: &PanelContext,
) -> Element<'a, PanelMsg> {
    let tokens = &ctx.tokens;
    let LayerLine {
        label,
        layer,
        number,
        kind,
    } = line;
    let hidden = panel.is_hidden(layer);
    let ink = if hidden {
        resolve(tokens).ink3
    } else {
        ti(tokens.text)
    };
    let muted = resolve(tokens).ink3;
    let mut name = row![].spacing(SP.s).align_y(Alignment::Center);
    if let Some(number) = number {
        let number = t!(panel.i18n, ids::PCB_LAYERS_COPPER_NUMBER, "number" => number);
        name = name.push(text(number).size(TS.caption).color(muted));
    }
    name = name.push(text(label).size(TS.body).color(ink).width(Length::Fill));
    if let Some(kind) = kind {
        name = name.push(text(kind).size(TS.caption).color(muted));
    }
    let name = button(name)
        .padding([SP.xs, SP.s])
        .width(Length::Fill)
        .style(row_button(tokens))
        .on_press(PanelMsg::PcbLayers(PcbViewMsg::ActiveLayerPicked(layer)));
    let toggled = PanelMsg::PcbLayers(PcbViewMsg::LayerVisibilityToggled(layer));
    let line = row![
        eye(panel, hidden, Some(toggled), ctx),
        chip(panel, panel.colour(layer), hidden, Some(layer), ctx),
        name,
    ]
    .spacing(SP.xs)
    .align_y(Alignment::Center);
    let is_active = panel.active == Some(layer);
    let active_c = active_fill(tokens);
    let line = container(line)
        .width(Length::Fill)
        .padding([0.0, SP.s])
        .style(move |_: &Theme| container::Style {
            background: is_active.then_some(Background::Color(active_c)),
            border: Border {
                radius: Radii::GRAPHITE.small.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed();
    with_editor(line, colour_editor(panel, ui, layer, ctx))
}

/// `line` with the colours of its chip under it, when open.
pub(super) fn with_editor<'a>(
    line: Element<'a, PanelMsg>,
    editor: Option<Element<'a, PanelMsg>>,
) -> Element<'a, PanelMsg> {
    match editor {
        Some(editor) => Column::new().push(line).push(editor).boxed(),
        None => line,
    }
}

/// A row the board has no layer for: name greyed, eye and chip inert.
pub(super) fn greyed_row<'a>(
    panel: &PcbLayersPanel,
    label: Localized,
    ctx: &PanelContext,
) -> Element<'a, PanelMsg> {
    let muted = resolve(&ctx.tokens).ink4;
    let line = row![
        eye(panel, false, None, ctx),
        chip(panel, muted, true, None, ctx),
        container(text(label).size(TS.body).color(muted)).padding([SP.xs, SP.s]),
    ]
    .spacing(SP.xs)
    .align_y(Alignment::Center);
    container(line)
        .width(Length::Fill)
        .padding([0.0, SP.s])
        .boxed()
}

/// A section: its header and rows; nothing when the search or Used On left
/// no row.
pub(super) fn section<'a>(
    ctx: &'a PanelContext,
    key: &str,
    header: MessageId,
    rows: Vec<Element<'a, PanelMsg>>,
) -> Option<Element<'a, PanelMsg>> {
    if rows.is_empty() {
        return None;
    }
    Some(
        ctx.collapsible(key, header, &ctx.collapsed_sections, || {
            rows.into_iter()
                .fold(Column::new().spacing(0), |col, row| col.push(row))
        })
        .boxed(),
    )
}
