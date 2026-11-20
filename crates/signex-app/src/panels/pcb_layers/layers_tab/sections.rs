//! The sections of Layers & Colors (#2363, mockup row 103): Signal and Plane Layers,
//! Component Layer Pairs, Other Technical Layers, Other Layers (each with a
//! count of the rows it lists and an eye for all of them) and System Colors
//! (closed until opened).

use iced::widget::{Row, Space};
use iced::{Color, Element, Length, Widget};
use signex_i18n::{Localized, ids, t};
use signex_renderer::pcb::SINGLE_TECHNICAL_LAYERS;
use signex_types::pcb_layer::{PcbLayer, TechnicalLayer};
use signex_widgets::text;

use super::super::metrics::ROW_GAP;
use super::super::rows::{
    LayerLine, SectionHead, chip, eye, framed, layer_row, not_yet, section_with,
};
use super::super::{LayerSection, LayersPanelMsg, LayersPanelUi, PcbLayersPanel, TS};
use super::pairs;
use crate::app::PcbViewMsg;
use crate::panels::{PanelContext, PanelMsg};
use crate::pcb_canvas::layer_style;
use crate::styles::ti;
use crate::styles::tokens::resolve;

/// Every section that lists a row, top to bottom.
pub(super) fn all<'a>(
    panel: &'a PcbLayersPanel,
    ui: &LayersPanelUi,
    ctx: &'a PanelContext,
) -> Vec<Element<'a, PanelMsg>> {
    let mut pair_rows = pairs::rows(panel, ui, ctx);
    let pair_count = pair_rows.len();
    if !pair_rows.is_empty() {
        pair_rows.insert(0, pairs::header(panel, ctx));
    }
    let system = system_rows(panel, ui, ctx);
    let system_head = SectionHead {
        count: Some(count(panel, system.len())),
        eye: None,
        closed_by_default: true,
    };
    [
        layer_section(
            ctx,
            panel,
            "pcb_layers_signal_plane",
            ids::PCB_LAYERS_SIGNAL_PLANE,
            section_eye(panel, ui, LayerSection::Copper),
            copper_rows(panel, ui, ctx),
        ),
        counted_section(
            ctx,
            panel,
            "pcb_layers_pairs",
            ids::PCB_LAYERS_PAIRS,
            section_eye(panel, ui, LayerSection::Pairs),
            pair_count,
            pair_rows,
        ),
        layer_section(
            ctx,
            panel,
            "pcb_layers_technical",
            ids::PCB_LAYERS_TECHNICAL,
            section_eye(panel, ui, LayerSection::Technical),
            technical_rows(panel, ui, ctx),
        ),
        layer_section(
            ctx,
            panel,
            "pcb_layers_other",
            ids::PCB_LAYERS_OTHER,
            section_eye(panel, ui, LayerSection::Other),
            other_rows(panel, ui, ctx),
        ),
        section_with(
            ctx,
            panel,
            "pcb_layers_system",
            ids::PCB_LAYERS_SYSTEM_COLORS,
            system_head,
            system,
        ),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// The count pill text for `rows` listed rows.
fn count(panel: &PcbLayersPanel, rows: usize) -> Localized {
    let rows = i64::try_from(rows).unwrap_or(i64::MAX);
    t!(panel.i18n, ids::PCB_LAYERS_SECTION_COUNT, "count" => rows)
}

/// The eye of a section head: whether it is shut and what a click sends.
type SectionEye = (bool, Option<PanelMsg>);

/// The eye of `section` over its listed layers: shut while every one is
/// hidden; a click shows them all while any is hidden, else hides them all.
fn section_eye(panel: &PcbLayersPanel, ui: &LayersPanelUi, section: LayerSection) -> SectionEye {
    let visible = panel.section_any_hidden(section, ui);
    (
        panel.section_hidden(section, ui),
        Some(PanelMsg::LayersPanel(
            LayersPanelMsg::SectionVisibilitySet { section, visible },
        )),
    )
}

/// A section of layers, its count the rows it lists.
fn layer_section<'a>(
    ctx: &'a PanelContext,
    panel: &PcbLayersPanel,
    key: &str,
    title: signex_i18n::MessageId,
    eye: SectionEye,
    rows: Vec<Element<'a, PanelMsg>>,
) -> Option<Element<'a, PanelMsg>> {
    let listed = rows.len();
    counted_section(ctx, panel, key, title, eye, listed, rows)
}

/// A section of layers with `listed` rows and an eye for all its listed
/// layers.
fn counted_section<'a>(
    ctx: &'a PanelContext,
    panel: &PcbLayersPanel,
    key: &str,
    title: signex_i18n::MessageId,
    eye: SectionEye,
    listed: usize,
    rows: Vec<Element<'a, PanelMsg>>,
) -> Option<Element<'a, PanelMsg>> {
    let head = SectionHead {
        count: Some(count(panel, listed)),
        eye: Some(eye),
        closed_by_default: false,
    };
    section_with(ctx, panel, key, title, head, rows)
}

/// Whether the row of `layer` named `label` is listed: it passes the search
/// and, with Used On, the board uses it.
fn listed(panel: &PcbLayersPanel, ui: &LayersPanelUi, layer: PcbLayer, label: &Localized) -> bool {
    panel.lists(layer) && ui.matches(&panel.i18n, label)
}

/// The layers of the rows `section` lists: what its eye shows, hides and
/// reads. A pair row brings both its sides; the Multi-Layer row is no layer.
pub(in crate::panels::pcb_layers) fn section_layers(
    panel: &PcbLayersPanel,
    ui: &LayersPanelUi,
    section: LayerSection,
) -> Vec<PcbLayer> {
    let row_listed = |layer: &PcbLayer| {
        listed(
            panel,
            ui,
            *layer,
            &layer_style::layer_name(&panel.i18n, *layer),
        )
    };
    match section {
        LayerSection::Copper => panel
            .copper
            .iter()
            .map(|row| row.layer)
            .filter(row_listed)
            .collect(),
        LayerSection::Pairs => pairs::listed(panel, ui)
            .flat_map(|(_, (top, bottom))| [top, bottom])
            .collect(),
        LayerSection::Technical => SINGLE_TECHNICAL_LAYERS
            .into_iter()
            .map(PcbLayer::Technical)
            .filter(row_listed)
            .collect(),
        LayerSection::Other => [PcbLayer::Technical(TechnicalLayer::DrillDrawing)]
            .into_iter()
            .filter(row_listed)
            .collect(),
    }
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

/// Drill Drawing and the Multi-Layer row: vias and through-hole pads, never
/// the active layer.
fn other_rows<'a>(
    panel: &'a PcbLayersPanel,
    ui: &LayersPanelUi,
    ctx: &PanelContext,
) -> Vec<Element<'a, PanelMsg>> {
    let i18n = &panel.i18n;
    let mut rows: Vec<Element<'a, PanelMsg>> = Vec::new();
    let drill = PcbLayer::Technical(TechnicalLayer::DrillDrawing);
    rows.extend(plain_row(panel, ui, drill, ctx));
    let multi = t!(i18n, ids::PCB_LAYER_MULTI_LAYER);
    if ui.matches(i18n, &multi) {
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
    colour: Color,
    ctx: &PanelContext,
) -> Element<'a, PanelMsg> {
    let r = resolve(&ctx.tokens);
    let ink = if hidden {
        r.ink4
    } else if toggled.is_none() {
        r.ink3
    } else {
        ti(ctx.tokens.text)
    };
    let line = Row::<Element<'a, PanelMsg>>::new()
        .spacing(ROW_GAP)
        .push(eye(panel, hidden, toggled, ctx))
        .push(not_yet(chip(panel, colour, hidden, None, ctx), panel, ctx))
        .push(text(label).size(TS.body).color(ink).boxed())
        .push(Space::new().width(Length::Fill).boxed());
    framed(line, false, ctx)
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
            if panel.used.is_some() || !ui.matches(i18n, &label) {
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
