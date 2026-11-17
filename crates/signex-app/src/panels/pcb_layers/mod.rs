//! The Layers panel of a board (#2223, #2359, #2363): two tabs and a search
//! field. Layers & Colors lists the board's layers (copper, the component
//! layer pairs, the other technical layers, the other layers and the system
//! colours), each with an eye and a colour chip; a click on a name makes
//! that layer the active one, as a tab under the canvas does. View Options
//! shows, hides and fades kinds of objects. The rows are data
//! (`PcbLayersPanel`, rebuilt from the view; `LayersPanelUi`, the panel's
//! own state); `view` only draws them. Layout: `docs/PCB_TOOLS.md`,
//! "Board view configuration".

mod layers_tab;
mod rows;
mod view_options;

use std::collections::HashSet;
use std::sync::{Arc, OnceLock};

use iced::widget::button::{Status, Style as ButtonStyle};
use iced::widget::{Column, Row, container, svg};
use iced::{Background, Border, Color, Element, Length, Theme, Widget};
use signex_i18n::{I18n, ids, t};
use signex_renderer::pcb::{
    BoardObject, BoardPaint, LayerSetId, PcbLayerView, board_copper_layers, board_layers,
    used_layers,
};
use signex_types::pcb::{LayerDef, PcbBoard};
use signex_types::pcb_layer::PcbLayer;
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, text, text_input};

use super::form_look::{self, ghost_button};
use super::graphite::{active_fill, wash_fn};
use super::{PanelContext, PanelMsg, empty_state_card};
use crate::pcb_canvas::layer_style;
use crate::styles::{Radii, Spacing, TypeScale, ti};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

const EYE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 14 14"><path d="M1 7c1.6-3 3.6-4.5 6-4.5S11.4 4 13 7c-1.6 3-3.6 4.5-6 4.5S2.6 10 1 7z" fill="none" stroke="currentColor" stroke-width="1.2"/><circle cx="7" cy="7" r="2" fill="currentColor"/></svg>"#;
const EYE_OFF: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 14 14"><path d="M1 7c1.6-3 3.6-4.5 6-4.5S11.4 4 13 7c-1.6 3-3.6 4.5-6 4.5S2.6 10 1 7z" fill="none" stroke="currentColor" stroke-width="1.2" opacity="0.4"/><path d="M2 12L12 2" stroke="currentColor" stroke-width="1.4"/></svg>"#;

/// The eye icons, built once (the shared icon set will take them over).
/// The Component Properties' text eyes draw the same icons.
pub(in crate::panels) fn eye_handle(hidden: bool) -> svg::Handle {
    static OPEN: OnceLock<svg::Handle> = OnceLock::new();
    static SHUT: OnceLock<svg::Handle> = OnceLock::new();
    let (cell, bytes) = if hidden {
        (&SHUT, EYE_OFF)
    } else {
        (&OPEN, EYE)
    };
    cell.get_or_init(|| svg::Handle::from_memory(bytes)).clone()
}

/// One copper row: its layer, its place in the stack (1 is the top) and
/// whether the stackup marks it as a plane.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CopperRow {
    pub layer: PcbLayer,
    pub number: usize,
    pub plane: bool,
}

/// What the panel shows of the active board and its view.
#[derive(Clone)]
pub struct PcbLayersPanel {
    pub copper: Vec<CopperRow>,
    /// Every layer of the board, in its order: the Active Layer choices.
    pub board_layers: Vec<PcbLayer>,
    /// The layers the user hid (their eyes).
    pub hidden: HashSet<PcbLayer>,
    /// The active layer as drawn; `None` while it is hidden or no board is
    /// open.
    pub active: Option<PcbLayer>,
    /// The Multi-Layer row (vias and through-hole pads) is hidden.
    pub multi_layer_hidden: bool,
    /// The view hides some layer (`PcbLayerView::has_hidden`).
    pub any_hidden: bool,
    pub single_layer: bool,
    /// The board is seen from below.
    pub flipped: bool,
    pub ratsnest_visible: bool,
    pub origin_marker_visible: bool,
    /// The layers the board has objects on, while Used On is on.
    pub used: Option<HashSet<PcbLayer>>,
    /// The view's own colours and transparencies.
    pub paint: BoardPaint,
    /// The kinds of object the view hides.
    pub hidden_objects: Vec<BoardObject>,
    /// The layer set the view shows, if any.
    pub layer_set: Option<LayerSetId>,
    /// The names of the saved layer sets.
    pub saved_sets: Vec<String>,
    pub grid_visible: bool,
    /// The board is shown in 3D.
    pub board_3d: bool,
    /// A board is open; without one the panel shows its empty state.
    pub board_open: bool,
    /// The colour field's labels, which it borrows as text.
    pub colour_label: String,
    pub palette_label: String,
    pub i18n: Arc<I18n>,
}

impl Default for PcbLayersPanel {
    fn default() -> Self {
        Self {
            copper: Vec::new(),
            board_layers: Vec::new(),
            hidden: HashSet::new(),
            active: None,
            multi_layer_hidden: false,
            any_hidden: false,
            single_layer: false,
            flipped: false,
            ratsnest_visible: true,
            origin_marker_visible: true,
            used: None,
            paint: BoardPaint::default(),
            hidden_objects: Vec::new(),
            layer_set: None,
            saved_sets: Vec::new(),
            grid_visible: true,
            board_3d: false,
            board_open: false,
            colour_label: String::new(),
            palette_label: String::new(),
            i18n: I18n::en_us(),
        }
    }
}

/// Whether the stackup marks a copper layer as a plane.
fn is_plane(layers: &[LayerDef], layer: PcbLayer) -> bool {
    layers.iter().any(|def| {
        signex_types::format::parse_pcb_layer(&def.name) == Some(layer)
            && ["plane", "power"]
                .iter()
                .any(|kind| def.layer_type.eq_ignore_ascii_case(kind))
    })
}

impl PcbLayersPanel {
    pub fn new(board: &PcbBoard, view: &PcbLayerView, i18n: Arc<I18n>) -> Self {
        let layers = &board.layers;
        let copper = board_copper_layers(layers)
            .into_iter()
            .enumerate()
            .map(|(i, c)| CopperRow {
                layer: PcbLayer::Copper(c),
                number: i + 1,
                plane: is_plane(layers, PcbLayer::Copper(c)),
            })
            .collect();
        let hidden = PcbLayer::all()
            .filter(|layer| view.is_hidden(*layer))
            .collect();
        Self {
            copper,
            board_layers: board_layers(layers),
            hidden,
            active: view.shown_active(),
            multi_layer_hidden: !view.multi_layer_visible(),
            any_hidden: view.has_hidden(),
            single_layer: view.single_layer(),
            flipped: view.flipped(),
            ratsnest_visible: view.ratsnest_visible(),
            origin_marker_visible: view.origin_marker_visible(),
            used: view.used_only().then(|| used_layers(board)),
            paint: view.paint().clone(),
            hidden_objects: BoardObject::ALL
                .into_iter()
                .filter(|kind| !view.shows_object(*kind))
                .collect(),
            layer_set: view.layer_set().map(|set| set.id.clone()),
            board_open: true,
            colour_label: t!(i18n, ids::PCB_LAYERS_COLOUR).into_string(),
            palette_label: t!(i18n, ids::PCB_LAYERS_COLOUR_PALETTE).into_string(),
            i18n,
            ..Self::default()
        }
    }

    pub fn is_hidden(&self, layer: PcbLayer) -> bool {
        self.hidden.contains(&layer)
    }

    pub fn has_hidden(&self) -> bool {
        self.any_hidden
    }

    /// Whether Used On keeps `layer`: always while it is off.
    pub fn lists(&self, layer: PcbLayer) -> bool {
        self.used.as_ref().is_none_or(|used| used.contains(&layer))
    }

    /// The colour `layer` is drawn in, in this view.
    pub fn colour(&self, layer: PcbLayer) -> Color {
        layer_style::painted_layer_colour(&self.paint, layer)
    }

    /// Whether the view draws objects of `kind`.
    pub fn shows_object(&self, kind: BoardObject) -> bool {
        !self.hidden_objects.contains(&kind)
    }
}

/// The tab the Layers panel shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LayersTab {
    #[default]
    LayersAndColors,
    ViewOptions,
}

/// The Layers panel's own state (#2363), kept across rebuilds: view-only,
/// never part of a board view.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LayersPanelUi {
    pub tab: LayersTab,
    /// The row filter: rows whose name holds it, ignoring case.
    pub search: String,
    /// The layer whose colour chip is open, and whether its custom picker
    /// is.
    pub picker: Option<(PcbLayer, bool)>,
    /// The name typed for a new layer set.
    pub set_name: String,
}

/// What the user did in the Layers panel besides a board-view event.
#[derive(Clone, Debug, PartialEq)]
pub enum LayersPanelMsg {
    TabPicked(LayersTab),
    SearchChanged(String),
    /// A colour chip was clicked: its colours open, or close again.
    ColourChipToggled(PcbLayer),
    CustomColourOpened,
    CustomColourClosed,
    SetNameChanged(String),
    /// A saved layer set was picked in the Layer Sets dropdown.
    SavedSetShown(String),
    /// Save in the Layer Sets row: the shown layers under the typed name.
    SetSaved,
    /// Delete in the Layer Sets row: the saved set the view shows.
    SetDeleted(String),
    /// The 3D switch of View Options.
    Board3dToggled,
}

impl LayersPanelUi {
    /// Apply what changes only the panel; `false` for a message that acts on
    /// the app (a layer set, 3D).
    pub fn apply(&mut self, msg: &LayersPanelMsg) -> bool {
        match msg {
            LayersPanelMsg::TabPicked(tab) => self.tab = *tab,
            LayersPanelMsg::SearchChanged(query) => self.search.clone_from(query),
            LayersPanelMsg::ColourChipToggled(layer) => {
                self.picker = match self.picker {
                    Some((open, _)) if open == *layer => None,
                    _ => Some((*layer, false)),
                };
            }
            LayersPanelMsg::CustomColourOpened => {
                self.picker = self.picker.map(|(layer, _)| (layer, true));
            }
            LayersPanelMsg::CustomColourClosed => self.picker = None,
            LayersPanelMsg::SetNameChanged(name) => self.set_name.clone_from(name),
            LayersPanelMsg::SavedSetShown(_)
            | LayersPanelMsg::SetSaved
            | LayersPanelMsg::SetDeleted(_)
            | LayersPanelMsg::Board3dToggled => return false,
        }
        true
    }

    /// Whether a row named `label` passes the search.
    pub fn matches(&self, label: &impl std::fmt::Display) -> bool {
        let query = self.search.trim();
        query.is_empty()
            || label
                .to_string()
                .to_lowercase()
                .contains(&query.to_lowercase())
    }
}

pub(super) fn view(ctx: &PanelContext) -> Element<'_, PanelMsg> {
    let panel = &ctx.pcb_layers;
    let i18n = &panel.i18n;
    if !panel.board_open {
        return empty_state_card(
            &ctx.tokens,
            t!(i18n, ids::PCB_LAYERS_TITLE),
            t!(i18n, ids::PCB_LAYERS_EMPTY),
            None,
        );
    }
    let ui = &ctx.pcb_layers_ui;
    let title = text(t!(i18n, ids::PCB_LAYERS_TITLE))
        .size(TS.label)
        .color(ti(ctx.tokens.text));
    let search = text_input(t!(i18n, ids::PCB_LAYERS_SEARCH), ui.search.clone())
        .on_input(|query| PanelMsg::LayersPanel(LayersPanelMsg::SearchChanged(query)))
        .size(TS.body)
        .padding(SP.s)
        .style(form_look::text_input_style)
        .width(Length::Fill);
    let body = match ui.tab {
        LayersTab::LayersAndColors => layers_tab::view(panel, ui, ctx),
        LayersTab::ViewOptions => view_options::view(panel, ctx),
    };
    Column::<Element<'_, PanelMsg>>::new()
        .spacing(SP.xs)
        .width(Length::Fill)
        .push(container(title).padding([SP.s, SP.m]).boxed())
        .push(container(tabs(panel, ui)).padding([0.0, SP.m]).boxed())
        .push(container(search).padding([0.0, SP.m]).boxed())
        .push(body)
        .boxed()
}

/// The two tabs, as a segmented control.
fn tabs<'a>(panel: &PcbLayersPanel, ui: &LayersPanelUi) -> Element<'a, PanelMsg> {
    let i18n = &panel.i18n;
    let tab = |label, which: LayersTab| {
        let on = ui.tab == which;
        button(text(label).size(TS.body))
            .padding([SP.xs, SP.m])
            .width(Length::Fill)
            .on_press(PanelMsg::LayersPanel(LayersPanelMsg::TabPicked(which)))
            .style(move |theme: &Theme, status: Status| {
                form_look::segment_cell(theme, on, matches!(status, Status::Hovered))
            })
            .boxed()
    };
    container(
        Row::new()
            .push(tab(
                t!(i18n, ids::PCB_LAYERS_TAB_LAYERS),
                LayersTab::LayersAndColors,
            ))
            .push(tab(
                t!(i18n, ids::PCB_LAYERS_TAB_VIEW_OPTIONS),
                LayersTab::ViewOptions,
            ))
            .spacing(SP.xs),
    )
    .padding(SP.xs)
    .width(Length::Fill)
    .style(form_look::segmented_frame)
    .boxed()
}

/// A transparent row button with the Graphite hover wash.
fn row_button(tokens: &ThemeTokens) -> impl Fn(&Theme, Status) -> ButtonStyle + 'static {
    let wash = wash_fn(tokens);
    move |_: &Theme, status| ButtonStyle {
        background: matches!(status, Status::Hovered).then_some(Background::Color(wash(0.06))),
        border: Border {
            radius: Radii::GRAPHITE.small.into(),
            ..Border::default()
        },
        ..ButtonStyle::default()
    }
}

/// A two-state button (Used On, a View Options switch): the active fill
/// while on.
fn toggle_button(
    tokens: &ThemeTokens,
    on: bool,
) -> impl Fn(&Theme, Status) -> ButtonStyle + 'static {
    let on_fill = active_fill(tokens);
    move |theme: &Theme, status| {
        let mut style = ghost_button(theme, status);
        if on {
            style.background = Some(Background::Color(on_fill));
        }
        if matches!(status, Status::Disabled) {
            style.text_color = form_look::FormLook::of(theme).ink4;
        }
        style
    }
}

#[cfg(test)]
mod tests;
