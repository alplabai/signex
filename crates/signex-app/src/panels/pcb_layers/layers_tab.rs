//! The Layers & Colors tab (#2363): the layer set, the active layer and the
//! side the board is seen from, then the layers by section, each with an
//! eye and a colour chip, and the system colours.

use iced::widget::{Column, Space, container, pick_list, row};
use iced::{Alignment, Element, Length, Widget};
use signex_i18n::{Arg, Localized, ids, t};
use signex_renderer::pcb::{BuiltinLayerSet, LayerPair, LayerSetId, SINGLE_TECHNICAL_LAYERS};
use signex_types::pcb_layer::{PcbLayer, TechnicalLayer};
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::{CheckboxLabel as _, button, checkbox, text, text_input};

use super::rows::{
    LayerLine, chip, colour_editor, eye, greyed_row, layer_row, not_yet, section, with_editor,
};
use super::{LayersPanelMsg, LayersPanelUi, PcbLayersPanel, SP, TS, toggle_button};
use crate::app::PcbViewMsg;
use crate::panels::form_look::{self, ghost_button};
use crate::panels::graphite::hint_tooltip;
use crate::panels::{PanelContext, PanelMsg};
use crate::pcb_canvas::layer_style;
use crate::styles::tokens::resolve;

pub(super) fn view<'a>(
    panel: &'a PcbLayersPanel,
    ui: &'a LayersPanelUi,
    ctx: &'a PanelContext,
) -> Element<'a, PanelMsg> {
    let i18n = &panel.i18n;
    let mut body = Column::<Element<'a, PanelMsg>>::new()
        .spacing(SP.xs)
        .width(Length::Fill)
        .push(padded(layer_sets(panel, ui)))
        .push(padded(active_layer(panel)))
        .push(padded(view_from_bottom(panel)))
        .push(padded(import_export(panel, ctx)))
        .push(all_layers_row(panel, ctx));
    let sections = [
        section(
            ctx,
            "pcb_layers_signal_plane",
            t!(i18n, ids::PCB_LAYERS_SIGNAL_PLANE),
            copper_rows(panel, ui, ctx),
        ),
        section(
            ctx,
            "pcb_layers_pairs",
            t!(i18n, ids::PCB_LAYERS_PAIRS),
            pair_rows(panel, ui, ctx),
        ),
        section(
            ctx,
            "pcb_layers_technical",
            t!(i18n, ids::PCB_LAYERS_TECHNICAL),
            technical_rows(panel, ui, ctx),
        ),
        section(
            ctx,
            "pcb_layers_other",
            t!(i18n, ids::PCB_LAYERS_OTHER),
            other_rows(panel, ui, ctx),
        ),
        section(
            ctx,
            "pcb_layers_system",
            t!(i18n, ids::PCB_LAYERS_SYSTEM_COLORS),
            system_rows(panel, ui, ctx),
        ),
    ];
    for section in sections.into_iter().flatten() {
        body = body.push(section);
    }
    body.boxed()
}

fn padded(content: Element<'_, PanelMsg>) -> Element<'_, PanelMsg> {
    container(content).padding([0.0, SP.m]).boxed()
}

/// A layer set entry of the dropdown.
#[derive(Clone, Debug, PartialEq)]
enum SetChoice {
    Builtin(BuiltinLayerSet),
    Saved(String),
}

/// Layer Sets: the dropdown, a name field and Save, and Delete for the
/// saved set in view.
fn layer_sets<'a>(panel: &'a PcbLayersPanel, ui: &LayersPanelUi) -> Element<'a, PanelMsg> {
    let i18n = &panel.i18n;
    let mut choices: Vec<SetChoice> = BuiltinLayerSet::ALL
        .into_iter()
        .map(SetChoice::Builtin)
        .collect();
    choices.extend(panel.saved_sets.iter().cloned().map(SetChoice::Saved));
    let selected = match &panel.layer_set {
        Some(LayerSetId::Saved(name)) => SetChoice::Saved(name.clone()),
        Some(LayerSetId::Builtin(set)) => SetChoice::Builtin(*set),
        None => SetChoice::Builtin(BuiltinLayerSet::All),
    };
    let label = move |choice: &SetChoice| match choice {
        SetChoice::Builtin(set) => {
            i18n.message_plain(ids::PCB_LAYER_SET, &[("set", Arg::from(set.token()))])
        }
        SetChoice::Saved(name) => name.clone(),
    };
    let dropdown = pick_list(Some(selected), choices, label)
        .graphite()
        .on_select(|choice: SetChoice| match choice {
            SetChoice::Builtin(set) => PanelMsg::PcbLayers(PcbViewMsg::BuiltinLayerSetShown(set)),
            SetChoice::Saved(name) => PanelMsg::LayersPanel(LayersPanelMsg::SavedSetShown(name)),
        })
        .width(Length::Fill);
    let name = text_input(
        t!(i18n, ids::PCB_LAYER_SET_NAME_PLACEHOLDER),
        ui.set_name.clone(),
    )
    .on_input(|name| PanelMsg::LayersPanel(LayersPanelMsg::SetNameChanged(name)))
    .size(TS.body)
    .padding(SP.s)
    .style(form_look::text_input_style)
    .width(Length::Fill);
    let save = button(text(t!(i18n, ids::PCB_LAYER_SET_SAVE_BUTTON)).size(TS.body))
        .padding([SP.xs, SP.m])
        .style(ghost_button)
        .on_press_maybe(
            (!ui.set_name.trim().is_empty())
                .then_some(PanelMsg::LayersPanel(LayersPanelMsg::SetSaved)),
        );
    let delete = button(text(t!(i18n, ids::PCB_LAYER_SET_DELETE)).size(TS.body))
        .padding([SP.xs, SP.m])
        .style(ghost_button)
        .on_press_maybe(match &panel.layer_set {
            Some(LayerSetId::Saved(name)) => Some(PanelMsg::LayersPanel(
                LayersPanelMsg::SetDeleted(name.clone()),
            )),
            _ => None,
        });
    Column::new()
        .spacing(SP.xs)
        .push(labelled(t!(i18n, ids::PCB_LAYERS_SETS), dropdown.boxed()))
        .push(
            row![name, save, delete]
                .spacing(SP.xs)
                .align_y(Alignment::Center),
        )
        .boxed()
}

/// A caption on the left, `control` filling the rest.
fn labelled<'a>(caption: Localized, control: Element<'a, PanelMsg>) -> Element<'a, PanelMsg> {
    row![
        text(caption).size(TS.body).width(Length::FillPortion(2)),
        container(control).width(Length::FillPortion(3)),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center)
    .boxed()
}

/// Active Layer: the same choice as the layer tabs under the canvas.
fn active_layer(panel: &PcbLayersPanel) -> Element<'_, PanelMsg> {
    let i18n = &panel.i18n;
    let label = move |layer: &PcbLayer| layer_style::layer_name(i18n, *layer).into_string();
    let dropdown = pick_list(panel.active, panel.board_layers.clone(), label)
        .graphite()
        .on_select(|layer: PcbLayer| PanelMsg::PcbLayers(PcbViewMsg::ActiveLayerPicked(layer)))
        .width(Length::Fill);
    labelled(t!(i18n, ids::PCB_LAYERS_ACTIVE_LAYER), dropdown.boxed())
}

/// View From Bottom Side: the board view's flip (Flip Board View).
fn view_from_bottom(panel: &PcbLayersPanel) -> Element<'_, PanelMsg> {
    checkbox(panel.flipped)
        .size(TS.body)
        .labelled(t!(panel.i18n, ids::PCB_LAYERS_VIEW_FROM_BOTTOM))
        .text_size(TS.body)
        .style(form_look::checkbox_style)
        .on_toggle(|_| PanelMsg::PcbLayers(PcbViewMsg::BoardFlipToggled))
        .boxed()
}

/// Import and Export of the configuration: greyed until a view can be
/// saved to a file.
fn import_export<'a>(panel: &PcbLayersPanel, ctx: &PanelContext) -> Element<'a, PanelMsg> {
    let greyed = |label: Localized| {
        not_yet(
            button(text(label).size(TS.body))
                .padding([SP.xs, SP.m])
                .style(toggle_button(&ctx.tokens, false))
                .boxed(),
            panel,
            ctx,
        )
    };
    row![
        greyed(t!(panel.i18n, ids::PCB_LAYERS_IMPORT)),
        greyed(t!(panel.i18n, ids::PCB_LAYERS_EXPORT)),
    ]
    .spacing(SP.xs)
    .boxed()
}

/// All Layers: an eye that shows every layer while any is hidden, else
/// hides them all; and the Used On filter.
fn all_layers_row<'a>(panel: &PcbLayersPanel, ctx: &PanelContext) -> Element<'a, PanelMsg> {
    let i18n = &panel.i18n;
    let toggled = if panel.has_hidden() {
        PcbViewMsg::AllLayersShown
    } else {
        PcbViewMsg::LayerKindVisibilitySet {
            kind: signex_renderer::pcb::LayerKind::All,
            visible: false,
        }
    };
    let used_on = panel.used.is_some();
    let used = hint_tooltip(
        button(text(t!(i18n, ids::PCB_LAYERS_USED_ON)).size(TS.body))
            .padding([SP.xs, SP.m])
            .style(toggle_button(&ctx.tokens, used_on))
            .on_press(PanelMsg::PcbLayers(PcbViewMsg::UsedLayersOnlySet(!used_on)))
            .boxed(),
        t!(i18n, ids::PCB_LAYERS_USED_ON_TIP),
        &ctx.tokens,
        iced::widget::tooltip::Position::Bottom,
    );
    container(
        row![
            eye(panel, panel.has_hidden(), Some(PanelMsg::PcbLayers(toggled)), ctx),
            text(t!(i18n, ids::PCB_LAYERS_ALL_LAYERS))
                .size(TS.body)
                .width(Length::Fill),
            used,
        ]
        .spacing(SP.xs)
        .align_y(Alignment::Center),
    )
    .padding([0.0, SP.s])
    .boxed()
}

/// Whether the row of `layer` named `label` is listed: it passes the search
/// and, with Used On, the board uses it.
fn listed(panel: &PcbLayersPanel, ui: &LayersPanelUi, layer: PcbLayer, label: &Localized) -> bool {
    panel.lists(layer) && ui.matches(label)
}

fn copper_rows<'a>(
    panel: &'a PcbLayersPanel,
    ui: &LayersPanelUi,
    ctx: &PanelContext,
) -> Vec<Element<'a, PanelMsg>> {
    let i18n = &panel.i18n;
    panel
        .copper
        .iter()
        .filter_map(|copper| {
            let name = layer_style::layer_name(i18n, copper.layer);
            if !listed(panel, ui, copper.layer, &name) {
                return None;
            }
            let kind = if copper.plane {
                t!(i18n, ids::PCB_LAYERS_TYPE_PLANE)
            } else {
                t!(i18n, ids::PCB_LAYERS_TYPE_SIGNAL)
            };
            let line = LayerLine {
                label: name.into(),
                layer: copper.layer,
                number: Some(copper.number),
                kind: Some(kind),
            };
            Some(layer_row(panel, ui, line, ctx))
        })
        .collect()
}

/// The token a pair row's name is looked up by.
fn pair_token(pair: LayerPair) -> &'static str {
    match pair {
        LayerPair::Body3d => "body_3d",
        LayerPair::Assembly => "assembly",
        LayerPair::Coating => "coating",
        LayerPair::ComponentCenter => "component_center",
        LayerPair::ComponentOutline => "component_outline",
        LayerPair::Courtyard => "courtyard",
        LayerPair::Designator => "designator",
        LayerPair::Overlay => "overlay",
        LayerPair::Paste => "paste",
        LayerPair::Solder => "solder",
        LayerPair::Tenting => "tenting",
        LayerPair::Value => "value",
        LayerPair::Adhesive => "adhesive",
        LayerPair::Fiducial => "fiducial",
        LayerPair::Keepout => "keepout",
    }
}

/// The pairs table: a header, then Top | Layer | Bottom per row, each side
/// with its own eye and chip. A row with no layer is greyed.
fn pair_rows<'a>(
    panel: &'a PcbLayersPanel,
    ui: &LayersPanelUi,
    ctx: &PanelContext,
) -> Vec<Element<'a, PanelMsg>> {
    let i18n = &panel.i18n;
    let mut rows: Vec<Element<'a, PanelMsg>> = Vec::new();
    for pair in LayerPair::ALL {
        let name = t!(i18n, ids::PCB_LAYERS_PAIR, "pair" => pair_token(pair));
        if !ui.matches(&name) {
            continue;
        }
        let Some((top, bottom)) = pair.layers() else {
            if panel.used.is_none() {
                let row = hint_tooltip(
                    greyed_row(panel, name, ctx),
                    t!(i18n, ids::PCB_LAYERS_NO_LAYER),
                    &ctx.tokens,
                    iced::widget::tooltip::Position::Top,
                );
                rows.push(row);
            }
            continue;
        };
        if !panel.lists(top) && !panel.lists(bottom) {
            continue;
        }
        let side = |layer: PcbLayer| {
            let hidden = panel.is_hidden(layer);
            let toggled = PanelMsg::PcbLayers(PcbViewMsg::LayerVisibilityToggled(layer));
            row![
                eye(panel, hidden, Some(toggled), ctx),
                chip(panel, panel.colour(layer), hidden, Some(layer), ctx),
            ]
            .spacing(0)
            .align_y(Alignment::Center)
        };
        let ink = if panel.is_hidden(top) && panel.is_hidden(bottom) {
            resolve(&ctx.tokens).ink3
        } else {
            crate::styles::ti(ctx.tokens.text)
        };
        let line = container(
            row![
                side(top),
                text(name)
                    .size(TS.body)
                    .color(ink)
                    .width(Length::Fill)
                    .align_x(iced::alignment::Horizontal::Center),
                side(bottom),
            ]
            .spacing(SP.xs)
            .align_y(Alignment::Center),
        )
        .padding([0.0, SP.s])
        .width(Length::Fill)
        .boxed();
        let line = with_editor(line, colour_editor(panel, ui, top, ctx));
        rows.push(with_editor(line, colour_editor(panel, ui, bottom, ctx)));
    }
    if !rows.is_empty() {
        rows.insert(0, pair_header(panel, ctx));
    }
    rows
}

/// Top | Layer | Bottom over the pair rows.
fn pair_header<'a>(panel: &PcbLayersPanel, ctx: &PanelContext) -> Element<'a, PanelMsg> {
    let muted = resolve(&ctx.tokens).ink3;
    let i18n = &panel.i18n;
    let head = |label: Localized| text(label).size(TS.caption).color(muted);
    container(
        row![
            head(t!(i18n, ids::PCB_LAYERS_PAIR_TOP)),
            container(head(t!(i18n, ids::PCB_LAYERS_PAIR_LAYER)))
                .width(Length::Fill)
                .align_x(iced::alignment::Horizontal::Center),
            head(t!(i18n, ids::PCB_LAYERS_PAIR_BOTTOM)),
        ]
        .align_y(Alignment::Center),
    )
    .padding([SP.xs, SP.m])
    .width(Length::Fill)
    .boxed()
}

/// A plain layer row for `layer`, if the search and Used On list it.
fn plain_row<'a>(
    panel: &'a PcbLayersPanel,
    ui: &LayersPanelUi,
    layer: PcbLayer,
    ctx: &PanelContext,
) -> Option<Element<'a, PanelMsg>> {
    let name = layer_style::layer_name(&panel.i18n, layer);
    if !listed(panel, ui, layer, &name) {
        return None;
    }
    let line = LayerLine {
        label: name.into(),
        layer,
        number: None,
        kind: None,
    };
    Some(layer_row(panel, ui, line, ctx))
}

fn technical_rows<'a>(
    panel: &'a PcbLayersPanel,
    ui: &LayersPanelUi,
    ctx: &PanelContext,
) -> Vec<Element<'a, PanelMsg>> {
    SINGLE_TECHNICAL_LAYERS
        .into_iter()
        .filter_map(|t| plain_row(panel, ui, PcbLayer::Technical(t), ctx))
        .collect()
}

/// Drill Drawing, Drill Guide (no layer yet) and the Multi-Layer row: vias
/// and through-hole pads, never the active layer.
fn other_rows<'a>(
    panel: &'a PcbLayersPanel,
    ui: &LayersPanelUi,
    ctx: &PanelContext,
) -> Vec<Element<'a, PanelMsg>> {
    let i18n = &panel.i18n;
    let mut rows: Vec<Element<'a, PanelMsg>> = Vec::new();
    let drill = PcbLayer::Technical(TechnicalLayer::DrillDrawing);
    rows.extend(plain_row(panel, ui, drill, ctx));
    let guide = t!(i18n, ids::PCB_LAYERS_DRILL_GUIDE);
    if panel.used.is_none() && ui.matches(&guide) {
        rows.push(greyed_row(panel, guide, ctx));
    }
    let multi = t!(i18n, ids::PCB_LAYER_MULTI_LAYER);
    if ui.matches(&multi) {
        let toggled = PanelMsg::PcbLayers(PcbViewMsg::MultiLayerVisibilityToggled);
        let hidden = panel.multi_layer_hidden;
        let colour = layer_style::multi_layer_colour();
        rows.push(fixed_row(panel, multi, hidden, Some(toggled), colour, ctx));
    }
    rows
}

/// A row that is no layer: an eye (greyed without a message), its colour
/// shown greyed, and the name.
fn fixed_row<'a>(
    panel: &PcbLayersPanel,
    label: Localized,
    hidden: bool,
    toggled: Option<PanelMsg>,
    colour: iced::Color,
    ctx: &PanelContext,
) -> Element<'a, PanelMsg> {
    let ink = if hidden || toggled.is_none() {
        resolve(&ctx.tokens).ink3
    } else {
        crate::styles::ti(ctx.tokens.text)
    };
    container(
        row![
            eye(panel, hidden, toggled, ctx),
            not_yet(chip(panel, colour, hidden, None, ctx), panel, ctx),
            container(text(label).size(TS.body).color(ink)).padding([SP.xs, SP.s]),
            Space::new().width(Length::Fill),
        ]
        .spacing(SP.xs)
        .align_y(Alignment::Center),
    )
    .padding([0.0, SP.s])
    .width(Length::Fill)
    .boxed()
}

/// The token of each System Colors row, in its order.
const SYSTEM_ROWS: [&str; 14] = [
    "connection_lines",
    "selection",
    "pad_holes",
    "via_holes",
    "origin_marker",
    "component_reference",
    "body_reference",
    "snap_points",
    "drc_markers",
    "violation_markers",
    "board",
    "sheet",
    "workspace_start",
    "workspace_end",
];

/// System Colors: Connection Lines and the Origin Marker show and hide;
/// the other rows and every colour here are greyed until the view can
/// change them.
fn system_rows<'a>(
    panel: &'a PcbLayersPanel,
    ui: &LayersPanelUi,
    ctx: &PanelContext,
) -> Vec<Element<'a, PanelMsg>> {
    let i18n = &panel.i18n;
    let muted = resolve(&ctx.tokens).ink4;
    SYSTEM_ROWS
        .into_iter()
        .filter_map(|token| {
            let label = t!(i18n, ids::PCB_LAYERS_SYSTEM, "row" => token);
            if panel.used.is_some() || !ui.matches(&label) {
                return None;
            }
            let row = match token {
                "connection_lines" => fixed_row(
                    panel,
                    label,
                    !panel.ratsnest_visible,
                    Some(PanelMsg::PcbLayers(PcbViewMsg::RatsnestVisibilityToggled)),
                    layer_style::ratsnest_colour(),
                    ctx,
                ),
                "origin_marker" => fixed_row(
                    panel,
                    label,
                    !panel.origin_marker_visible,
                    Some(PanelMsg::PcbLayers(
                        PcbViewMsg::OriginMarkerVisibilityToggled,
                    )),
                    layer_style::origin_marker_colour(),
                    ctx,
                ),
                _ => fixed_row(panel, label, false, None, muted, ctx),
            };
            Some(row)
        })
        .collect()
}
