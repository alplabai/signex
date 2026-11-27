//! Drop targets of a dragged document tab (mockup row 207): over an edge of
//! the document area, the half that would receive the tab fills with
//! `accent-soft` and a 2 px accent border, as a dock drop target does
//! (`dock_zones`), and names what the drop does. The middle draws nothing:
//! there the tab stays a tab where it is.

use super::dock_zones::{centred, window_layer};
use super::*;
use crate::document_panes::DropEdge;
use iced::Rectangle;
use iced::widget::container;
use signex_i18n::ids;
use signex_widgets::text;

/// The half of `area` a drop on `edge` fills.
pub(super) fn receiving_half(area: Rectangle, edge: DropEdge) -> Rectangle {
    let (half_width, half_height) = (area.width / 2.0, area.height / 2.0);
    match edge {
        DropEdge::Left => Rectangle {
            width: half_width,
            ..area
        },
        DropEdge::Right => Rectangle {
            x: area.x + half_width,
            width: half_width,
            ..area
        },
        DropEdge::Top => Rectangle {
            height: half_height,
            ..area
        },
        DropEdge::Bottom => Rectangle {
            y: area.y + half_height,
            height: half_height,
            ..area
        },
    }
}

fn drop_line(edge: DropEdge, second_view: bool) -> signex_i18n::MessageId {
    if second_view {
        return ids::WORKSPACE_LAYOUTS_PANES_DROP_SECOND_VIEW;
    }
    match edge {
        DropEdge::Left => ids::WORKSPACE_LAYOUTS_PANES_DROP_LEFT,
        DropEdge::Right => ids::WORKSPACE_LAYOUTS_PANES_DROP_RIGHT,
        DropEdge::Top => ids::WORKSPACE_LAYOUTS_PANES_DROP_TOP,
        DropEdge::Bottom => ids::WORKSPACE_LAYOUTS_PANES_DROP_BOTTOM,
    }
}

impl Signex {
    /// The receiving half while a document tab is dragged over an edge of
    /// the main window's document area, or `None`.
    pub(in crate::app::view) fn pane_drop_overlay(&self) -> Option<Element<'_, Message>> {
        let (tab, edge) = self.pane_drop_target()?;
        let name = self.document_state.tabs.get(tab)?.title.clone();
        let second_view = self.ui_state.workspace_layouts.panes.second_view_held;
        let i18n = self.ui_state.i18n.as_ref();
        let tokens = &self.document_view.panel_ctx.tokens;
        let ink = crate::styles::tokens::resolve(tokens).accent_text;
        let label = text(signex_i18n::t!(i18n, drop_line(edge, second_view), "name" => name))
            .size(TS.body)
            .color(ink)
            .font(iced::Font {
                weight: iced::font::Weight::Semibold,
                ..signex_widgets::ui_font::regular()
            })
            .boxed();
        let half = container(centred(label))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(crate::styles::dock_zone_highlight(tokens))
            .boxed();
        let bounds = receiving_half(self.document_area_in(None), edge);
        Some(window_layer([(half, bounds)]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each edge fills its own half of the document area.
    #[test]
    fn a_drop_edge_fills_its_half_of_the_document_area() {
        let area = Rectangle {
            x: 240.0,
            y: 80.0,
            width: 800.0,
            height: 600.0,
        };
        let half = |edge| {
            let r = receiving_half(area, edge);
            (r.x, r.y, r.width, r.height)
        };
        assert_eq!(half(DropEdge::Left), (240.0, 80.0, 400.0, 600.0));
        assert_eq!(half(DropEdge::Right), (640.0, 80.0, 400.0, 600.0));
        assert_eq!(half(DropEdge::Top), (240.0, 80.0, 800.0, 300.0));
        assert_eq!(half(DropEdge::Bottom), (240.0, 380.0, 800.0, 300.0));
    }
}
