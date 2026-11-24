//! Dock drop zones (mockup row 8). While a floating panel is dragged, the three dock
//! targets show as faint dashed outlines with their names; the one a release
//! would dock into (`DockArea::drop_preview`) fills with `accent-soft`, gets
//! a 2 px accent border and its name in `accent-text` semibold.

use super::*;
use crate::dock::{DropGeometry, PanelPosition};
use crate::styles::DashedOutline;
use iced::widget::{Space, Stack, canvas, container};
use iced::{Point, Rectangle, Size, Theme};
use signex_i18n::menu_ids;
use signex_widgets::text;

/// Gap between a zone's outline and the edges of its dock region.
const ZONE_INSET: f32 = SP.m;

/// One dock target as the preview draws it, in window coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct DropZone {
    pub(super) position: PanelPosition,
    pub(super) bounds: Rectangle,
    /// A release now would dock the panel here.
    pub(super) is_target: bool,
}

/// The three zones for a drop `geometry`, between the chrome's bottom `top`
/// and the status bar's top `bottom`. The side zones run that full height;
/// the bottom zone sits between them, under the canvas, as the docks do.
/// Each is inset by [`ZONE_INSET`]; `target` is the one a release would
/// dock into.
pub(super) fn drop_zones(
    geometry: &DropGeometry,
    top: f32,
    bottom: f32,
    target: Option<PanelPosition>,
) -> [DropZone; 3] {
    let window_w = geometry.window.0;
    let left = geometry.preview_depth(PanelPosition::Left);
    let right = geometry.preview_depth(PanelPosition::Right);
    let low = geometry.preview_depth(PanelPosition::Bottom);
    let height = bottom - top;
    let zone = |position, x: f32, y: f32, width: f32, height: f32| DropZone {
        position,
        bounds: Rectangle {
            x: x + ZONE_INSET,
            y: y + ZONE_INSET,
            width: (width - 2.0 * ZONE_INSET).max(0.0),
            height: (height - 2.0 * ZONE_INSET).max(0.0),
        },
        is_target: target == Some(position),
    };
    [
        zone(PanelPosition::Left, 0.0, top, left, height),
        zone(PanelPosition::Right, window_w - right, top, right, height),
        zone(
            PanelPosition::Bottom,
            left,
            bottom - low,
            window_w - left - right,
            low,
        ),
    ]
}

/// The name a zone shows: the dock tab menu's row for that region.
fn zone_label(position: PanelPosition) -> signex_i18n::MessageId {
    match position {
        PanelPosition::Left => menu_ids::CTX_DOCK_LEFT,
        PanelPosition::Right => menu_ids::CTX_DOCK_RIGHT,
        PanelPosition::Bottom => menu_ids::CTX_DOCK_BOTTOM,
    }
}

/// Canvas program of a dashed rounded outline filling its bounds.
struct Outline(DashedOutline);

impl<M> canvas::Program<M> for Outline {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let w = self.0.width;
        let path = canvas::Path::rounded_rectangle(
            Point::new(w / 2.0, w / 2.0),
            Size::new((bounds.width - w).max(0.0), (bounds.height - w).max(0.0)),
            self.0.radius.into(),
        );
        let mut stroke = canvas::Stroke::default()
            .with_color(self.0.color)
            .with_width(w);
        stroke.line_dash = canvas::LineDash {
            segments: &self.0.dash,
            offset: 0,
        };
        frame.stroke(&path, stroke);
        vec![frame.into_geometry()]
    }
}

impl Signex {
    /// The drop zones while a floating panel is dragged, or `None` when no
    /// drag (and no drop preview) is in flight.
    pub(super) fn dock_drag_zones(&self) -> Option<[DropZone; 3]> {
        let ui = &self.ui_state;
        let dock = &self.document_state.dock;
        let dragging = dock.floating.iter().any(|fp| fp.dragging);
        if !dragging && dock.drop_preview.is_none() {
            return None;
        }
        let geometry = dock.drop_geometry(
            ui.window_size,
            (ui.left_width, ui.right_width, ui.bottom_height),
        );
        let tokens = &self.document_state.panel_ctx.tokens;
        let top = super::canvas_region::chrome_top(tokens);
        let bottom = ui.window_size.1 - crate::status_bar::status_bar_height(tokens);
        Some(drop_zones(&geometry, top, bottom, dock.drop_preview))
    }

    /// Dock drop preview (mockup row 8): every dock target outlined and named, the
    /// one a release would dock into filled.
    pub(in crate::app::view) fn dock_drag_zone_overlay(&self) -> Option<Element<'_, Message>> {
        let zones = self.dock_drag_zones()?;
        let tokens = &self.document_state.panel_ctx.tokens;
        let r = crate::styles::tokens::resolve(tokens);
        let i18n = &self.ui_state.i18n;
        let outline = crate::styles::dock_zone_outline(tokens);
        let mut layer = Stack::new().width(Length::Fill).height(Length::Fill);
        for zone in zones {
            let Rectangle {
                x,
                y,
                width,
                height,
            } = zone.bounds;
            let name = text(signex_i18n::t!(i18n, zone_label(zone.position))).size(TS.body);
            let label = if zone.is_target {
                name.color(r.accent_text).font(iced::Font {
                    weight: iced::font::Weight::Semibold,
                    ..signex_widgets::ui_font::regular()
                })
            } else {
                name.color(r.ink4)
            };
            let centred = container(label)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center);
            let card: Element<'_, Message> = if zone.is_target {
                centred
                    .style(crate::styles::dock_zone_highlight(tokens))
                    .boxed()
            } else {
                let dashes: canvas::Canvas<Outline, Message> =
                    canvas::Canvas::new(Outline(outline));
                iced::widget::stack![
                    dashes.width(Length::Fill).height(Length::Fill).boxed(),
                    centred.boxed(),
                ]
                .boxed()
            };
            let sized = container(card).width(width).height(height).boxed();
            layer = layer.push(signex_widgets::Translate::new(sized, (x, y)).boxed());
        }
        // Keep the layer window-sized even if every zone is empty.
        layer = layer.push(
            Space::new()
                .width(Length::Fill)
                .height(Length::Fill)
                .boxed(),
        );
        Some(layer.boxed())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dock::RegionExtent;

    fn geometry() -> DropGeometry {
        let open = |size| RegionExtent {
            occupied: true,
            collapsed: false,
            size,
        };
        DropGeometry {
            window: (1400.0, 900.0),
            left: open(240.0),
            right: open(300.0),
            bottom: open(160.0),
        }
    }

    /// The side zones run from the chrome to the status bar at their dock's
    /// depth; the bottom zone sits between them; all inset alike.
    #[test]
    fn the_three_zones_cover_their_dock_regions() {
        let [left, right, bottom] = drop_zones(&geometry(), 70.0, 876.0, None);
        assert_eq!(left.position, PanelPosition::Left);
        assert_eq!(left.bounds.x, ZONE_INSET);
        assert_eq!(left.bounds.y, 70.0 + ZONE_INSET);
        assert_eq!(left.bounds.width, 240.0 - 2.0 * ZONE_INSET);
        assert_eq!(left.bounds.height, 806.0 - 2.0 * ZONE_INSET);
        assert_eq!(right.position, PanelPosition::Right);
        assert_eq!(right.bounds.x, 1100.0 + ZONE_INSET);
        assert_eq!(bottom.position, PanelPosition::Bottom);
        assert_eq!(bottom.bounds.x, 240.0 + ZONE_INSET);
        assert_eq!(bottom.bounds.width, 860.0 - 2.0 * ZONE_INSET);
        assert_eq!(bottom.bounds.y, 876.0 - 160.0 + ZONE_INSET);
    }

    /// Only the region a release would dock into is the target; with none,
    /// every zone is a plain outline.
    #[test]
    fn only_the_drop_target_is_highlighted() {
        let zones = drop_zones(&geometry(), 70.0, 876.0, Some(PanelPosition::Right));
        let targets: Vec<PanelPosition> = zones
            .iter()
            .filter(|z| z.is_target)
            .map(|z| z.position)
            .collect();
        assert_eq!(targets, [PanelPosition::Right]);
        let idle = drop_zones(&geometry(), 70.0, 876.0, None);
        assert!(idle.iter().all(|z| !z.is_target));
    }

    /// The zones are named with the dock tab menu's strings.
    #[test]
    fn zone_names_reuse_the_dock_menu_strings() {
        let en = signex_i18n::I18n::en_us();
        let names: Vec<String> = [
            PanelPosition::Left,
            PanelPosition::Right,
            PanelPosition::Bottom,
        ]
        .into_iter()
        .map(|p| signex_i18n::t!(en, zone_label(p)).into_string())
        .collect();
        assert_eq!(names, ["Dock Left", "Dock Right", "Dock Bottom"]);
    }

    /// A drag shows all three zones, and only while a panel is dragged.
    #[test]
    fn zones_show_only_while_a_floating_panel_is_dragged() {
        let (mut app, _boot) = Signex::new();
        assert!(app.dock_drag_zones().is_none());
        assert!(app.dock_drag_zone_overlay().is_none());
        app.document_state.dock.drop_preview = Some(PanelPosition::Left);
        let zones = app.dock_drag_zones().expect("zones while previewing");
        assert!(zones[0].is_target && !zones[1].is_target && !zones[2].is_target);
        assert!(app.dock_drag_zone_overlay().is_some());
    }

    /// The highlight is the accent: soft fill and a 2 px accent border; the
    /// outline is 1.5 px `line-strong`, dashed, in Graphite Light and Dark.
    #[test]
    fn target_and_outline_styles_follow_the_mock() {
        use signex_types::theme::{ThemeId, theme_tokens};
        for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
            let t = theme_tokens(id);
            let r = crate::styles::tokens::resolve(&t);
            let style = crate::styles::dock_zone_highlight(&t)(&Theme::Dark);
            assert_eq!(
                style.background,
                Some(iced::Background::Color(r.accent_soft))
            );
            assert_eq!(style.border.width, 2.0);
            assert_eq!(style.border.color, crate::styles::ti(t.accent));
            let outline = crate::styles::dock_zone_outline(&t);
            assert_eq!(outline.color, r.line_strong);
            assert_eq!(outline.width, 1.5);
            assert!(outline.dash.iter().all(|d| *d > 0.0));
        }
    }
}
