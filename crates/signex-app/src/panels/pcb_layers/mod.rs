//! The Layers panel of a board (#2223, #2359, #2363, mockup row 103): two tabs.
//! Layers & Colors has a search row (with Used On and the more menu), the
//! Active | Set row and the board's layers by section (copper, the component
//! layer pairs, the other technical layers, the other layers and the system
//! colours), each with an eye and a colour dot; a click on a name makes that
//! layer the active one, as a tab under the canvas does. View Options shows,
//! hides and fades kinds of objects. The rows are data (`PcbLayersPanel`,
//! rebuilt from the view; `LayersPanelUi`, the panel's own state); `view`
//! only draws them. Layout: `docs/PCB_TOOLS.md`, "Board view configuration".

mod colour_card;
mod layers_tab;
mod metrics;
mod more_menu;
mod rows;
mod view_options;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use iced::widget::button::{Status, Style as ButtonStyle};
use iced::widget::{Column, container, svg};
use iced::{Background, Border, Color, Element, Length, Padding, Theme, Widget};
use signex_i18n::{I18n, ids, t};
use signex_renderer::pcb::{
    BoardObject, BoardPaint, LayerSetId, PcbLayerView, board_copper_layers, board_layers,
    plane_layers, used_layers,
};
use signex_types::pcb::PcbBoard;
use signex_types::pcb_layer::PcbLayer;
use signex_types::theme::ThemeTokens;
use signex_widgets::Label;
use signex_widgets::segmented::{SegmentedMetrics, segmented};

use super::graphite::wash_fn;
use super::{PanelContext, PanelMsg, empty_state_card};
use crate::pcb_canvas::layer_style;
use crate::styles::tokens::resolve;
use crate::styles::{Radii, Spacing, TypeScale};

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

/// A section of Layers & Colors whose eye shows or hides all its layers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayerSection {
    Copper,
    Pairs,
    Technical,
    /// Drill Drawing (the Multi-Layer row in it keeps its own eye).
    Other,
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
    /// The board's layer colours and the view's transparencies.
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
    /// The Layer Stack command can run: the more menu's Layer Stack... row
    /// is live.
    pub layer_stack_live: bool,
    /// Why the Layer Stack command cannot run: the greyed row's tooltip.
    pub layer_stack_reason: Option<signex_i18n::MessageId>,
    /// A board is open; without one the panel shows its empty state.
    pub board_open: bool,
    /// The tab path of the board shown, so an open colour card can tell a
    /// switch of board from an edit of the same one.
    pub board: Option<PathBuf>,
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
            layer_stack_live: false,
            layer_stack_reason: None,
            board_open: false,
            board: None,
            i18n: I18n::en_us(),
        }
    }
}

impl PcbLayersPanel {
    pub fn new(board: &PcbBoard, view: &PcbLayerView, i18n: Arc<I18n>) -> Self {
        let layers = &board.layers;
        let planes = plane_layers(layers);
        let copper = board_copper_layers(layers)
            .into_iter()
            .enumerate()
            .map(|(i, c)| CopperRow {
                layer: PcbLayer::Copper(c),
                number: i + 1,
                plane: planes.contains(&c),
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
            paint: view.paint_of(board),
            hidden_objects: BoardObject::ALL
                .into_iter()
                .filter(|kind| !view.shows_object(*kind))
                .collect(),
            layer_set: view.layer_set().map(|set| set.id.clone()),
            board_open: true,
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

    /// The colour `layer` is drawn in: the board's, else the palette's.
    pub fn colour(&self, layer: PcbLayer) -> Color {
        layer_style::painted_layer_colour(&self.paint, layer)
    }

    /// The colour of `layer` as the colour card's hex field writes it.
    pub fn hex_of(&self, layer: PcbLayer) -> String {
        let [r, g, b, _] = self.colour(layer).into_rgba8();
        format!("#{r:02X}{g:02X}{b:02X}")
    }

    /// Whether the view draws objects of `kind`.
    pub fn shows_object(&self, kind: BoardObject) -> bool {
        !self.hidden_objects.contains(&kind)
    }

    /// The layers the eye of `section` shows and hides: those of the rows
    /// the panel lists (the search and Used On leave the others alone). The
    /// Multi-Layer row is no layer and keeps its own eye.
    pub fn section_layers(&self, section: LayerSection, ui: &LayersPanelUi) -> Vec<PcbLayer> {
        layers_tab::section_layers(self, ui, section)
    }

    /// Every listed layer of `section` is hidden: its eye is shut.
    pub fn section_hidden(&self, section: LayerSection, ui: &LayersPanelUi) -> bool {
        let layers = self.section_layers(section, ui);
        !layers.is_empty() && layers.iter().all(|layer| self.is_hidden(*layer))
    }

    /// Some listed layer of `section` is hidden: its eye shows them all.
    pub fn section_any_hidden(&self, section: LayerSection, ui: &LayersPanelUi) -> bool {
        self.section_layers(section, ui)
            .iter()
            .any(|layer| self.is_hidden(*layer))
    }
}

/// The tab the Layers panel shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LayersTab {
    #[default]
    LayersAndColors,
    ViewOptions,
}

/// The Layers panel's own state (#2363, mockup row 103), kept across rebuilds:
/// view-only, never part of a board view.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LayersPanelUi {
    pub tab: LayersTab,
    /// The row filter: rows whose name holds it, ignoring case.
    pub search: String,
    /// The layer whose colour card is open.
    pub picker: Option<PcbLayer>,
    /// The text of the colour card's hex field.
    pub hex: String,
    /// The more menu beside the search field is open.
    pub more_menu_open: bool,
    /// A new layer set is being named: the name row shows under the search.
    pub naming: bool,
    /// The name typed for a new layer set.
    pub set_name: String,
}

/// What the user did in the Layers panel besides a board-view event.
#[derive(Clone, Debug, PartialEq)]
pub enum LayersPanelMsg {
    TabPicked(LayersTab),
    SearchChanged(String),
    /// A colour dot was clicked: its colour card opens, or closes again.
    ColourChipToggled(PcbLayer),
    /// The colour card's hex field was edited.
    HexEdited(String),
    /// The more button was clicked.
    MoreMenuToggled,
    /// A click outside the more menu closed it.
    MoreMenuDismissed,
    /// Save current as set... in the more menu: the name row opens.
    SetNamingStarted,
    /// The name row was closed without saving.
    SetNamingCancelled,
    SetNameChanged(String),
    /// A saved layer set was picked in the Set dropdown.
    SavedSetShown(String),
    /// Save in the name row: the shown layers under the typed name.
    SetSaved,
    /// Delete current set... in the more menu: the saved set the view shows.
    SetDeleted(String),
    /// A section's eye: show or hide every layer of the section.
    SectionVisibilitySet {
        section: LayerSection,
        visible: bool,
    },
    /// Layer Stack... in the more menu.
    LayerStackPicked,
    /// View From Bottom Side in the more menu: the view flips and the menu
    /// closes, as every other live row of it does.
    BoardFlipPicked,
    /// The 3D switch of View Options.
    Board3dToggled,
}

impl LayersPanelUi {
    /// The set name field, focused when the name row opens.
    pub const SET_NAME_INPUT: iced::widget::Id =
        iced::widget::Id::new("signex.layers_panel.set_name");

    /// Apply what changes only the panel; `false` for a message that acts on
    /// the app (a layer set, a section's eye, a typed colour, 3D).
    pub fn apply(&mut self, msg: &LayersPanelMsg) -> bool {
        match msg {
            LayersPanelMsg::TabPicked(tab) => self.tab = *tab,
            LayersPanelMsg::SearchChanged(query) => self.search.clone_from(query),
            LayersPanelMsg::ColourChipToggled(layer) => {
                self.picker = (self.picker != Some(*layer)).then_some(*layer);
            }
            LayersPanelMsg::MoreMenuToggled => self.more_menu_open = !self.more_menu_open,
            LayersPanelMsg::MoreMenuDismissed => self.more_menu_open = false,
            LayersPanelMsg::SetNamingStarted => {
                self.more_menu_open = false;
                self.naming = true;
            }
            LayersPanelMsg::SetNamingCancelled => {
                self.naming = false;
                self.set_name.clear();
            }
            LayersPanelMsg::SetNameChanged(name) => self.set_name.clone_from(name),
            LayersPanelMsg::HexEdited(hex) => {
                self.hex.clone_from(hex);
                return false;
            }
            LayersPanelMsg::SetDeleted(_)
            | LayersPanelMsg::LayerStackPicked
            | LayersPanelMsg::BoardFlipPicked => {
                self.more_menu_open = false;
                return false;
            }
            LayersPanelMsg::SavedSetShown(_)
            | LayersPanelMsg::SetSaved
            | LayersPanelMsg::SectionVisibilitySet { .. }
            | LayersPanelMsg::Board3dToggled => return false,
        }
        true
    }

    /// Whether a row named `label` passes the search, both sides folded
    /// with the UI language's case rules.
    pub fn matches(&self, i18n: &I18n, label: &impl std::fmt::Display) -> bool {
        let query = self.search.trim();
        query.is_empty()
            || i18n
                .fold_for_search(&label.to_string())
                .contains(&i18n.fold_for_search(query))
    }

    /// The colour the hex field holds, once it reads as `#RRGGBB` (the `#`
    /// may be left out).
    pub fn hex_colour(&self) -> Option<[u8; 3]> {
        let text = self.hex.trim();
        let digits = text.strip_prefix('#').unwrap_or(text);
        if digits.len() != 6 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let byte = |at: usize| u8::from_str_radix(&digits[at..at + 2], 16).ok();
        Some([byte(0)?, byte(2)?, byte(4)?])
    }

    /// Keep the open colour card true to the board after a rebuild: it
    /// closes when the panel now shows another board (`before` is the board
    /// it showed), and its hex field takes the layer's colour again when it
    /// reads as another colour (an undo, a redo or a reset while it was
    /// open), so the next keystroke never applies a stale colour. A
    /// half-typed colour is left as typed.
    pub fn follow_board(&mut self, before: Option<&Path>, now: &PcbLayersPanel) {
        let Some(layer) = self.picker else {
            return;
        };
        if before != now.board.as_deref() {
            self.picker = None;
            return;
        }
        let [r, g, b, _] = now.colour(layer).into_rgba8();
        if self.hex_colour().is_some_and(|typed| typed != [r, g, b]) {
            self.hex = now.hex_of(layer);
        }
    }
}

/// Padding around the tabs; the search row follows them.
const TABS_PAD: Padding = Padding {
    top: SP.m,
    right: SP.m,
    bottom: SP.s,
    left: SP.m,
};
/// Padding around the search row.
const SEARCH_PAD: Padding = Padding {
    top: SP.xs,
    right: SP.m,
    bottom: SP.s,
    left: SP.m,
};

/// Where the more menu card starts: just under the search row.
fn more_menu_top() -> f32 {
    let tabs = SegmentedMetrics::from_text_size(TS.body).track_height;
    TABS_PAD.top + tabs + TABS_PAD.bottom + SEARCH_PAD.top + metrics::FIELD + SP.xs
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
    // The dock's title bar names the panel: no title in the body.
    let content = match ui.tab {
        LayersTab::LayersAndColors => layers_tab::view(panel, ui, ctx),
        LayersTab::ViewOptions => view_options::view(panel, ctx),
    };
    let body = Column::<Element<'_, PanelMsg>>::new()
        .width(Length::Fill)
        .push(container(tabs(panel, ui, ctx)).padding(TABS_PAD).boxed())
        .push(content)
        .boxed();
    if ui.tab == LayersTab::LayersAndColors && ui.more_menu_open {
        return more_menu::overlay(body, panel, ctx);
    }
    body
}

/// The two tabs, as a segmented control.
fn tabs<'a>(
    panel: &PcbLayersPanel,
    ui: &LayersPanelUi,
    ctx: &PanelContext,
) -> Element<'a, PanelMsg> {
    let i18n = &panel.i18n;
    let r = resolve(&ctx.tokens);
    let style = r.segmented(TS.body).fill_width(true);
    segmented(
        [
            (
                LayersTab::LayersAndColors,
                Label::from(t!(i18n, ids::PCB_LAYERS_TAB_LAYERS)),
            ),
            (
                LayersTab::ViewOptions,
                Label::from(t!(i18n, ids::PCB_LAYERS_TAB_VIEW_OPTIONS)),
            ),
        ],
        ui.tab,
        style,
        |tab| PanelMsg::LayersPanel(LayersPanelMsg::TabPicked(tab)),
    )
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

#[cfg(test)]
mod tests;
