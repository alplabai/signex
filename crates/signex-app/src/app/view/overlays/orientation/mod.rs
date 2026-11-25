//! The orientation tile and the Home View button (mockup row 194) at the
//! bottom-right of the main window's canvas. The tile sits right-aligned
//! one medium gap above the zoom pill and shows where +X and +Y point; Home
//! is a one-button pill one small gap left of the zoom pill. A board has no
//! zoom pill layer of its own, so this layer draws the board's pill too.
//! Placement and behaviour: `docs/PCB_TOOLS.md`, "Orientation tile and
//! Home View".

mod axes;
#[cfg(test)]
mod tests;
mod tile;

use iced::widget::{Stack, Widget as _};
use iced::{Element, Length, Rectangle};
use signex_gfx::view_rotation::ViewRotation;
use signex_i18n::{menu_ids, t};
use signex_types::schematic::PageOrigin;

use super::zoom_pill::pill_origin;
use super::*;
use crate::canvas::zoom_pill::{PillMessages, PillText, home_size, pill_size};
use crate::menu_bar::MenuMessage;
use axes::{ScreenAxes, board_caption};

/// Commands behind the board pill's `-`, `+` and Fit, as on the sheet's.
const ZOOM_OUT_COMMAND: &str = "zoom_out_at_cursor";
const ZOOM_IN_COMMAND: &str = "zoom_in_at_cursor";
const FIT_ALL_COMMAND: &str = "zoom_to_fit";

/// What the main window's canvas shows, as the tile reads it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::app::view) enum ViewOrientation {
    /// A board in 2D, turned and mirrored by `rotation`; `tile_shown` is its
    /// View Options eye.
    Board {
        rotation: ViewRotation,
        tile_shown: bool,
    },
    /// A sheet, its Y counted from `origin`.
    Sheet { origin: PageOrigin },
}

/// Where each part of the corner cluster goes, in window coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::app::view) struct ClusterRects {
    pub pill: Rectangle,
    pub home: Rectangle,
    /// `None` while the tile is hidden.
    pub tile: Option<Rectangle>,
}

/// The cluster against `canvas`: the zoom pill where it always sits, Home a
/// small gap to its left, the tile (`tile`: whether it shows and whether it
/// has a caption row) right-aligned a medium gap above the pill.
pub(in crate::app::view) fn cluster_rects(
    canvas: Rectangle,
    tile: Option<bool>,
) -> ClusterRects {
    let (pill_w, pill_h) = pill_size();
    let (pill_x, pill_y) = pill_origin(canvas);
    let pill = Rectangle::new(
        iced::Point::new(pill_x, pill_y),
        iced::Size::new(pill_w, pill_h),
    );
    let (home_w, home_h) = home_size();
    let home = Rectangle::new(
        iced::Point::new(pill_x - SP.s - home_w, pill_y),
        iced::Size::new(home_w, home_h),
    );
    let tile = tile.map(|has_caption| {
        let (w, h) = tile::tile_size(has_caption);
        Rectangle::new(
            iced::Point::new(pill_x + pill_w - w, pill_y - SP.m - h),
            iced::Size::new(w, h),
        )
    });
    ClusterRects { pill, home, tile }
}

impl Signex {
    /// What the main window's canvas shows: a board in 2D or a sheet.
    /// `None` in any other window, on a board in 3D or in Board Sketch,
    /// and without a document.
    pub(in crate::app::view) fn view_orientation(
        &self,
        window_id: iced::window::Id,
    ) -> Option<ViewOrientation> {
        if self.ui_state.main_window_id != Some(window_id) {
            return None;
        }
        if self.has_active_pcb() {
            if self.board_3d_in_view().is_some() || self.board_sketch_in_view().is_some() {
                return None;
            }
            let path = self.active_tab_path()?;
            return Some(ViewOrientation::Board {
                rotation: self.interaction_state.pcb_canvas.view_rotation(),
                tile_shown: self.pcb_layer_view(&path).orientation_tile_visible(),
            });
        }
        self.document_state
            .engine_for_window(window_id, &self.ui_state)?;
        let origin = self
            .interaction_state
            .canvas_for_window(window_id)
            .active_snapshot()
            .map_or_else(PageOrigin::default, |sheet| sheet.page.origin);
        Some(ViewOrientation::Sheet { origin })
    }

    /// The tile, the Home button and, on a board, the zoom pill. The layer
    /// spans the window and places them against the canvas of the size it is
    /// laid out at, as the sheet's zoom pill does (#2911).
    pub(in crate::app::view) fn canvas_orientation_overlay(
        &self,
        window_id: iced::window::Id,
    ) -> Option<Element<'_, Message>> {
        let orientation = self.view_orientation(window_id)?;
        let layer = iced::widget::responsive(move |size: iced::Size| {
            let canvas = self.canvas_bounds_sized(None, (size.width, size.height));
            self.orientation_cluster(orientation, canvas)
        });
        Some(layer.boxed())
    }

    fn orientation_cluster(
        &self,
        orientation: ViewOrientation,
        canvas: Rectangle,
    ) -> Element<'_, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = &self.ui_state.i18n;
        let (axes, caption, tile_shown, hint) = match orientation {
            ViewOrientation::Board {
                rotation,
                tile_shown,
            } => (
                ScreenAxes::of_board(rotation),
                board_caption(i18n, rotation.degrees(), rotation.is_mirrored()),
                tile_shown,
                menu_ids::HOME_VIEW_HINT_BOARD,
            ),
            ViewOrientation::Sheet { origin } => (
                ScreenAxes::of_sheet(origin),
                None,
                true,
                menu_ids::HOME_VIEW_HINT_SHEET,
            ),
        };
        let rects = cluster_rects(canvas, tile_shown.then_some(caption.is_some()));
        let place = |element: Element<'static, Message>, at: Rectangle| -> Element<'static, Message> {
            // The `Translate` spans the window; only the card may take the wheel.
            signex_widgets::Translate::new(super::scroll_guard(element), (at.x, at.y)).boxed()
        };
        let home = crate::canvas::zoom_pill::home_view(
            tokens,
            crate::icons::icon_chrome_home(self.ui_state.theme_id),
            Message::Ui(UiMsg::HomeViewRequested),
            t!(i18n, hint).into(),
            caption.is_some(),
        );
        let mut layers: Stack<Element<'_, Message>> = Stack::new()
            .width(Length::Fill)
            .height(Length::Fill)
            .push(place(home, rects.home));
        if let Some(at) = rects.tile {
            let labels = [
                t!(i18n, menu_ids::ORIENTATION_TILE_X).into_string(),
                t!(i18n, menu_ids::ORIENTATION_TILE_Y).into_string(),
            ];
            layers = layers.push(place(tile::view(tokens, axes, labels, caption), at));
        }
        if matches!(orientation, ViewOrientation::Board { .. }) {
            layers = layers.push(place(self.board_zoom_pill(), rects.pill));
        }
        layers.boxed()
    }

    /// The board's zoom pill: the sheet pill's controls and hints, with a
    /// plain read-out (the zoom presets act on a sheet).
    fn board_zoom_pill(&self) -> Element<'static, Message> {
        let i18n = &self.ui_state.i18n;
        let zoom = self.interaction_state.pcb_canvas.zoom_percent();
        crate::canvas::zoom_pill::view_with(
            &self.document_state.panel_ctx.tokens,
            PillMessages {
                zoom_out: Message::Menu(MenuMessage::ZoomOut),
                zoom_in: Message::Menu(MenuMessage::ZoomIn),
                fit_all: Message::CanvasEvent(crate::canvas::CanvasEvent::FitAll),
                presets: None,
            },
            PillText {
                readout: t!(i18n, menu_ids::ZOOM_PILL_PERCENT, "percent" => format!("{zoom:.0}"))
                    .into(),
                fit: t!(i18n, menu_ids::ZOOM_PILL_FIT).into(),
                hints: [
                    self.zoom_pill_hint(ZOOM_OUT_COMMAND),
                    self.zoom_pill_hint(ZOOM_IN_COMMAND),
                    None,
                    self.zoom_pill_hint(FIT_ALL_COMMAND),
                ],
            },
            false,
        )
    }
}
