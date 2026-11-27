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

use super::zoom_pill::{ZoomPillCanvas, pill_origin};
use super::*;
use crate::canvas::zoom_pill::{home_size, pill_size};
use axes::{ScreenAxes, board_caption};

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

impl ViewOrientation {
    pub fn axes(self) -> ScreenAxes {
        match self {
            Self::Board { rotation, .. } => ScreenAxes::of_board(rotation),
            Self::Sheet { origin } => ScreenAxes::of_sheet(origin),
        }
    }

    /// The tile's caption row: a board's turn and flip; a sheet has none.
    pub fn caption(self, i18n: &signex_i18n::I18n) -> Option<signex_i18n::Localized> {
        match self {
            Self::Board { rotation, .. } => {
                board_caption(i18n, rotation.degrees(), rotation.is_mirrored())
            }
            Self::Sheet { .. } => None,
        }
    }

    /// Whether the tile shows: a board's View Options eye; always on a sheet.
    pub fn tile_shown(self) -> bool {
        match self {
            Self::Board { tile_shown, .. } => tile_shown,
            Self::Sheet { .. } => true,
        }
    }

    /// Whether the Home button carries its accent dot: the view is turned or
    /// flipped, exactly when the tile has a caption.
    pub fn away_from_home(self, i18n: &signex_i18n::I18n) -> bool {
        self.caption(i18n).is_some()
    }
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
pub(in crate::app::view) fn cluster_rects(canvas: Rectangle, tile: Option<bool>) -> ClusterRects {
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
        let tokens = &self.document_view.panel_ctx.tokens;
        let i18n = &self.ui_state.i18n;
        let caption = orientation.caption(i18n);
        let away = orientation.away_from_home(i18n);
        let on_board = matches!(orientation, ViewOrientation::Board { .. });
        let hint = if on_board {
            menu_ids::HOME_VIEW_HINT_BOARD
        } else {
            menu_ids::HOME_VIEW_HINT_SHEET
        };
        let rects = cluster_rects(canvas, orientation.tile_shown().then_some(away));
        let home = crate::canvas::zoom_pill::home_view(
            tokens,
            crate::icons::icon_chrome_home(self.ui_state.theme_id),
            Message::Ui(UiMsg::HomeViewRequested),
            t!(i18n, hint).into(),
            away,
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
            let tile = tile::view(tokens, orientation.axes(), labels, caption);
            layers = layers.push(place(tile, at));
        }
        if on_board {
            // The sheet's pill (`zoom_pill.rs`), on the board's zoom.
            let zoom = self.interaction_state.pcb_canvas.zoom_percent();
            let pill = self.zoom_pill_at(zoom, ZoomPillCanvas::Board);
            layers = layers.push(place(pill, rects.pill));
        }
        layers.boxed()
    }
}

/// `element` at `at` on the window-wide layer.
fn place<'a>(element: Element<'a, Message>, at: Rectangle) -> Element<'a, Message> {
    // The `Translate` spans the window; only the card may take the wheel.
    signex_widgets::Translate::new(super::scroll_guard(element), (at.x, at.y)).boxed()
}
