use super::*;

pub(in crate::gerber_viewer) fn visible_bounds(layers: &[ViewerLayer]) -> Option<Bounds> {
    layers
        .iter()
        .filter(|layer| layer.visible)
        .filter_map(|layer| layer.layer.geometry.bounds)
        .reduce(|left, right| Bounds {
            min: signex_gerber::Point {
                x: left.min.x.min(right.min.x),
                y: left.min.y.min(right.min.y),
            },
            max: signex_gerber::Point {
                x: left.max.x.max(right.max.x),
                y: left.max.y.max(right.max.y),
            },
        })
}

pub(in crate::gerber_viewer) fn page_bounds(
    artwork_bounds: Option<Bounds>,
    page_size: GerberPageSize,
) -> Option<Bounds> {
    let artwork = artwork_bounds?;
    let Some((width, height)) = page_size.dimensions_millimetres() else {
        return Some(artwork);
    };
    let center_x = (artwork.min.x + artwork.max.x) * 0.5;
    let center_y = (artwork.min.y + artwork.max.y) * 0.5;
    Some(Bounds {
        min: signex_gerber::Point {
            x: center_x - width * 0.5,
            y: center_y - height * 0.5,
        },
        max: signex_gerber::Point {
            x: center_x + width * 0.5,
            y: center_y + height * 0.5,
        },
    })
}

pub(in crate::gerber_viewer) fn world_to_screen_point(
    point: signex_gerber::Point,
    world_center: Point,
    screen_center: Point,
    scale: f32,
    mirrored: bool,
) -> Point {
    let horizontal_direction = if mirrored { -1.0 } else { 1.0 };
    Point::new(
        screen_center.x + (point.x as f32 - world_center.x) * scale * horizontal_direction,
        screen_center.y - (point.y as f32 - world_center.y) * scale,
    )
}

pub(in crate::gerber_viewer) fn screen_to_world_point(
    point: Point,
    world_center: Point,
    screen_center: Point,
    scale: f32,
    mirrored: bool,
) -> signex_gerber::Point {
    let horizontal_direction = if mirrored { -1.0 } else { 1.0 };
    signex_gerber::Point {
        x: f64::from(world_center.x + (point.x - screen_center.x) / scale * horizontal_direction),
        y: f64::from(world_center.y - (point.y - screen_center.y) / scale),
    }
}

pub(in crate::gerber_viewer) fn fit_transform(
    world_bounds: Bounds,
    viewport: Rectangle,
    zoom: f32,
    pan: iced::Vector,
) -> (f32, Point, Point) {
    let view_width = (viewport.width - CANVAS_MARGIN * 2.0).max(1.0);
    let view_height = (viewport.height - CANVAS_MARGIN * 2.0).max(1.0);
    let world_width = world_bounds.width().max(0.001) as f32;
    let world_height = world_bounds.height().max(0.001) as f32;
    let fitted_scale = (view_width / world_width).min(view_height / world_height);
    let scale = fitted_scale * zoom.clamp(MIN_ZOOM, MAX_ZOOM);
    let world_center = Point::new(
        ((world_bounds.min.x + world_bounds.max.x) * 0.5) as f32,
        ((world_bounds.min.y + world_bounds.max.y) * 0.5) as f32,
    );
    let screen_center = Point::new(viewport.width / 2.0 + pan.x, viewport.height / 2.0 + pan.y);
    (scale, world_center, screen_center)
}

pub(in crate::gerber_viewer) fn normalized_screen_rectangle(
    start: Point,
    end: Point,
) -> Option<Rectangle> {
    const MINIMUM_SELECTION_PIXELS: f32 = 4.0;

    let width = (end.x - start.x).abs();
    let height = (end.y - start.y).abs();
    if !width.is_finite()
        || !height.is_finite()
        || width < MINIMUM_SELECTION_PIXELS
        || height < MINIMUM_SELECTION_PIXELS
    {
        return None;
    }
    Some(Rectangle::new(
        Point::new(start.x.min(end.x), start.y.min(end.y)),
        iced::Size::new(width, height),
    ))
}

pub(in crate::gerber_viewer) fn zoom_transform_for_selection(
    base_bounds: Bounds,
    selection: Bounds,
    viewport: Rectangle,
) -> Option<(f32, iced::Vector)> {
    if !selection.is_finite() || selection.width() <= 0.0 || selection.height() <= 0.0 {
        return None;
    }
    let view_width = (viewport.width - CANVAS_MARGIN * 2.0).max(1.0);
    let view_height = (viewport.height - CANVAS_MARGIN * 2.0).max(1.0);
    let base_scale = (view_width / base_bounds.width().max(0.001) as f32)
        .min(view_height / base_bounds.height().max(0.001) as f32);
    let selection_scale =
        (view_width / selection.width() as f32).min(view_height / selection.height() as f32);
    let zoom = (selection_scale / base_scale).clamp(MIN_ZOOM, MAX_ZOOM);
    let scale = base_scale * zoom;
    let base_center = Point::new(
        ((base_bounds.min.x + base_bounds.max.x) * 0.5) as f32,
        ((base_bounds.min.y + base_bounds.max.y) * 0.5) as f32,
    );
    let selection_center = Point::new(
        ((selection.min.x + selection.max.x) * 0.5) as f32,
        ((selection.min.y + selection.max.y) * 0.5) as f32,
    );
    Some((
        zoom,
        iced::Vector::new(
            (base_center.x - selection_center.x) * scale,
            (selection_center.y - base_center.y) * scale,
        ),
    ))
}
