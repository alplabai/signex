use super::grid_view::{draw_crosshair, draw_grid};
use super::layer_view::{LayerDrawOptions, compare_layer_color, draw_layer, forced_opacity_color};
use super::*;

impl GerberCanvas<'_> {
    pub(super) fn draw_view(
        &self,
        state: &GerberCanvasState,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let _redraw_generation = self.redraw_generation;
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), self.background);

        let artwork_bounds = visible_bounds(self.layers);
        let Some(world_bounds) = page_bounds(artwork_bounds, self.page_size) else {
            if self.grid_visible {
                draw_grid(
                    &mut frame,
                    bounds,
                    self.grid,
                    self.grid_size,
                    self.grid_style,
                    32.0 / 1.27,
                    Point::new(bounds.width / 2.0, bounds.height / 2.0),
                );
            }
            frame.fill_text(canvas::Text {
                content: "Open Gerber files to inspect fabrication layers".into(),
                position: Point::new(bounds.width / 2.0, bounds.height / 2.0),
                color: Color {
                    a: 0.65,
                    ..self.grid
                },
                size: iced::Pixels(14.0),
                align_x: iced::alignment::Horizontal::Center.into(),
                align_y: iced::alignment::Vertical::Center,
                ..canvas::Text::default()
            });
            if self.crosshair_mode != GerberCrosshairMode::None
                && let Some(position) = cursor.position_in(bounds)
            {
                draw_crosshair(&mut frame, bounds, position, self.grid, self.crosshair_mode);
            }
            return vec![frame.into_geometry()];
        };

        let (scale, world_center, screen_center) =
            fit_transform(world_bounds, bounds, self.zoom, self.pan);
        let world_to_screen = |point: signex_gerber::Point| -> Point {
            world_to_screen_point(point, world_center, screen_center, scale, self.mirrored)
        };
        if self.grid_visible {
            draw_grid(
                &mut frame,
                bounds,
                self.grid,
                self.grid_size,
                self.grid_style,
                scale,
                world_to_screen(signex_gerber::Point { x: 0.0, y: 0.0 }),
            );
        }
        draw_page_boundary(&mut frame, world_bounds, scale, &world_to_screen, self.grid);

        let visible_layer_count = self.layers.iter().filter(|layer| layer.visible).count();
        for (visible_ordinal, (layer_index, viewer_layer)) in self
            .layers
            .iter()
            .enumerate()
            .filter(|(_, layer)| layer.visible)
            .enumerate()
        {
            let highlighted_d_code = if self.active_layer == Some(layer_index) {
                self.highlighted_d_code
            } else {
                None
            };
            draw_layer(
                &mut frame,
                LayerDrawOptions {
                    viewer_layer,
                    layer_color: forced_opacity_color(
                        compare_layer_color(
                            viewer_layer.color,
                            visible_ordinal,
                            visible_layer_count,
                            self.compare_mode,
                            self.compare_palette,
                        ),
                        self.forced_opacity_mode,
                        self.forced_opacity,
                    ),
                    active: self.active_layer == Some(layer_index),
                    dim_inactive_layers: self.dim_inactive_layers,
                    inactive_layer_opacity: self.inactive_layer_opacity,
                    scale,
                    world_to_screen: &world_to_screen,
                    background: self.background,
                    highlighted_component: self.highlighted_component,
                    highlighted_net: self.highlighted_net,
                    highlighted_attribute: self.highlighted_attribute,
                    highlighted_d_code,
                    selected_item: self.selected_item,
                    selected_items: self.selected_items,
                    layer_index,
                    sketch_flashes: self.sketch_flashes,
                    sketch_lines: self.sketch_lines,
                    sketch_polygons: self.sketch_polygons,
                    ghost_negative_objects: self.ghost_negative_objects,
                    negative_ghost_color: self.negative_ghost_color,
                    show_d_code_labels: self.show_d_code_labels,
                    d_code_color: self.d_code_color,
                    zoom: self.zoom,
                },
            );
        }
        if let Some(measurement) = self.measurement {
            draw_measurement(
                &mut frame,
                measurement,
                &world_to_screen,
                self.grid,
                self.measurement_annotation.as_deref(),
            );
        }
        let rectangular_selection = if state.zoom_selection_start.is_some() {
            (state.zoom_selection_start, state.zoom_selection_current)
        } else {
            (state.item_selection_start, state.item_selection_current)
        };
        if let (Some(start), Some(end)) = rectangular_selection
            && let Some(selection) = normalized_screen_rectangle(start, end)
        {
            let path =
                canvas::Path::rectangle(Point::new(selection.x, selection.y), selection.size());
            frame.stroke(
                &path,
                canvas::Stroke::default()
                    .with_color(self.grid)
                    .with_width(1.0),
            );
        }
        if self.crosshair_mode != GerberCrosshairMode::None
            && let Some(position) = cursor.position_in(bounds)
        {
            draw_crosshair(&mut frame, bounds, position, self.grid, self.crosshair_mode);
        }
        vec![frame.into_geometry()]
    }
}

pub(in crate::gerber_viewer) fn draw_page_boundary(
    frame: &mut canvas::Frame,
    page: Bounds,
    scale: f32,
    world_to_screen: &impl Fn(signex_gerber::Point) -> Point,
    color: Color,
) {
    let top_left = world_to_screen(signex_gerber::Point {
        x: page.min.x,
        y: page.max.y,
    });
    let path = canvas::Path::rectangle(
        top_left,
        iced::Size::new(page.width() as f32 * scale, page.height() as f32 * scale),
    );
    frame.stroke(
        &path,
        canvas::Stroke::default()
            .with_width(1.0)
            .with_color(Color { a: 0.7, ..color }),
    );
}
