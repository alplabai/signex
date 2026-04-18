use iced::widget::canvas;

use signex_types::pcb::{BoardGraphic, BoardText, Footprint, PcbBoard, Segment, Via, Zone};
use signex_types::schematic::Aabb;
use signex_types::theme::CanvasColors;

use crate::colors::to_iced;
use crate::{canvas_font, canvas_font_size_scale};

#[derive(Debug, Clone, Copy)]
pub struct ScreenTransform {
    pub offset_x: f32,
    pub offset_y: f32,
    pub scale: f32,
}

impl ScreenTransform {
    fn world_to_screen(&self, point: signex_types::pcb::Point) -> iced::Point {
        iced::Point::new(
            point.x as f32 * self.scale + self.offset_x,
            point.y as f32 * self.scale + self.offset_y,
        )
    }

    fn scalar_to_screen(&self, value_mm: f64) -> f32 {
        (value_mm as f32 * self.scale).abs()
    }
}

#[derive(Debug, Clone)]
pub struct PcbRenderSnapshot {
    pub outline: Vec<signex_types::pcb::Point>,
    pub footprints: Vec<Footprint>,
    pub segments: Vec<Segment>,
    pub vias: Vec<Via>,
    pub zones: Vec<Zone>,
    pub graphics: Vec<BoardGraphic>,
    pub texts: Vec<BoardText>,
    pub layers: Vec<signex_types::pcb::LayerDef>,
    pub nets: Vec<signex_types::pcb::NetDef>,
    content_bounds: Option<Aabb>,
}

impl PcbRenderSnapshot {
    pub fn from_board(board: &PcbBoard) -> Self {
        Self {
            outline: board.outline.clone(),
            footprints: board.footprints.clone(),
            segments: board.segments.clone(),
            vias: board.vias.clone(),
            zones: board.zones.clone(),
            graphics: board.graphics.clone(),
            texts: board.texts.clone(),
            layers: board.layers.clone(),
            nets: board.nets.clone(),
            content_bounds: content_bounds(board),
        }
    }

    pub fn content_bounds(&self) -> Option<Aabb> {
        self.content_bounds
    }
}

pub fn render_pcb(
    frame: &mut canvas::Frame,
    snapshot: &PcbRenderSnapshot,
    transform: ScreenTransform,
    colors: &CanvasColors,
) {
    draw_zones(frame, snapshot, transform, colors);
    draw_board_outline(frame, &snapshot.outline, transform, colors);
    draw_segments(frame, &snapshot.segments, transform, colors);
    draw_vias(frame, &snapshot.vias, transform, colors);
    draw_graphics(frame, &snapshot.graphics, transform, colors);
    draw_footprints(frame, &snapshot.footprints, transform, colors);
    draw_texts(frame, &snapshot.texts, transform, colors);
}

fn draw_board_outline(
    frame: &mut canvas::Frame,
    outline: &[signex_types::pcb::Point],
    transform: ScreenTransform,
    colors: &CanvasColors,
) {
    if outline.len() < 2 {
        return;
    }

    let path = canvas::Path::new(|builder| {
        let first = transform.world_to_screen(outline[0]);
        builder.move_to(first);
        for point in &outline[1..] {
            builder.line_to(transform.world_to_screen(*point));
        }
        builder.close();
    });
    frame.fill(&path, to_iced(&colors.paper));
    frame.stroke(
        &path,
        canvas::Stroke::default()
            .with_color(to_iced(&colors.body))
            .with_width(2.0),
    );
}

fn draw_segments(
    frame: &mut canvas::Frame,
    segments: &[Segment],
    transform: ScreenTransform,
    colors: &CanvasColors,
) {
    for segment in segments {
        let path = canvas::Path::line(
            transform.world_to_screen(segment.start),
            transform.world_to_screen(segment.end),
        );
        frame.stroke(
            &path,
            canvas::Stroke::default()
                .with_color(to_iced(&colors.wire))
                .with_width(transform.scalar_to_screen(segment.width.max(0.15)).max(1.0)),
        );
    }
}

fn draw_vias(
    frame: &mut canvas::Frame,
    vias: &[Via],
    transform: ScreenTransform,
    colors: &CanvasColors,
) {
    for via in vias {
        let center = transform.world_to_screen(via.position);
        let radius = transform
            .scalar_to_screen((via.diameter / 2.0).max(0.15))
            .max(2.0);
        let path = canvas::Path::circle(center, radius);
        frame.fill(&path, to_iced(&colors.junction));
        frame.stroke(
            &path,
            canvas::Stroke::default()
                .with_color(to_iced(&colors.selection))
                .with_width(1.0),
        );
    }
}

fn draw_zones(
    frame: &mut canvas::Frame,
    snapshot: &PcbRenderSnapshot,
    transform: ScreenTransform,
    colors: &CanvasColors,
) {
    for zone in &snapshot.zones {
        if zone.outline.len() < 3 {
            continue;
        }

        let path = canvas::Path::new(|builder| {
            builder.move_to(transform.world_to_screen(zone.outline[0]));
            for point in &zone.outline[1..] {
                builder.line_to(transform.world_to_screen(*point));
            }
            builder.close();
        });
        let mut fill = to_iced(&colors.bus);
        fill.a = 0.14;
        frame.fill(&path, fill);
        frame.stroke(
            &path,
            canvas::Stroke::default()
                .with_color(to_iced(&colors.bus))
                .with_width(1.0),
        );
    }
}

fn draw_graphics(
    frame: &mut canvas::Frame,
    graphics: &[BoardGraphic],
    transform: ScreenTransform,
    colors: &CanvasColors,
) {
    for graphic in graphics {
        match graphic.graphic_type.as_str() {
            "gr_line" | "line" => {
                if let (Some(start), Some(end)) = (graphic.start, graphic.end) {
                    let path = canvas::Path::line(
                        transform.world_to_screen(start),
                        transform.world_to_screen(end),
                    );
                    frame.stroke(
                        &path,
                        canvas::Stroke::default()
                            .with_color(to_iced(&colors.reference))
                            .with_width(
                                transform.scalar_to_screen(graphic.width.max(0.1)).max(1.0),
                            ),
                    );
                }
            }
            "gr_rect" | "rect" => {
                if let (Some(start), Some(end)) = (graphic.start, graphic.end) {
                    let top_left = transform.world_to_screen(start);
                    let bottom_right = transform.world_to_screen(end);
                    let rect = iced::Rectangle::new(
                        iced::Point::new(
                            top_left.x.min(bottom_right.x),
                            top_left.y.min(bottom_right.y),
                        ),
                        iced::Size::new(
                            (bottom_right.x - top_left.x).abs(),
                            (bottom_right.y - top_left.y).abs(),
                        ),
                    );
                    let path = canvas::Path::rectangle(rect.position(), rect.size());
                    frame.stroke(
                        &path,
                        canvas::Stroke::default()
                            .with_color(to_iced(&colors.reference))
                            .with_width(
                                transform.scalar_to_screen(graphic.width.max(0.1)).max(1.0),
                            ),
                    );
                }
            }
            _ => {}
        }
    }
}

fn draw_footprints(
    frame: &mut canvas::Frame,
    footprints: &[Footprint],
    transform: ScreenTransform,
    colors: &CanvasColors,
) {
    for footprint in footprints {
        let center = transform.world_to_screen(footprint.position);
        let body = canvas::Path::circle(center, 5.0);
        frame.fill(&body, to_iced(&colors.body_fill));
        frame.stroke(
            &body,
            canvas::Stroke::default()
                .with_color(to_iced(&colors.body))
                .with_width(1.0),
        );

        for pad in &footprint.pads {
            let pad_center = transform.world_to_screen(signex_types::pcb::Point::new(
                footprint.position.x + pad.position.x,
                footprint.position.y + pad.position.y,
            ));
            let pad_size = iced::Size::new(
                transform.scalar_to_screen(pad.size.x.max(0.2)).max(2.0),
                transform.scalar_to_screen(pad.size.y.max(0.2)).max(2.0),
            );
            let path = canvas::Path::rectangle(
                iced::Point::new(
                    pad_center.x - pad_size.width / 2.0,
                    pad_center.y - pad_size.height / 2.0,
                ),
                pad_size,
            );
            frame.fill(&path, to_iced(&colors.power));
            frame.stroke(
                &path,
                canvas::Stroke::default()
                    .with_color(to_iced(&colors.selection))
                    .with_width(0.8),
            );
        }
    }
}

fn draw_texts(
    frame: &mut canvas::Frame,
    texts: &[BoardText],
    transform: ScreenTransform,
    colors: &CanvasColors,
) {
    for text in texts {
        frame.fill_text(canvas::Text {
            content: text.text.clone(),
            position: transform.world_to_screen(text.position),
            color: to_iced(&colors.value),
            size: iced::Pixels(
                (text.font_size.max(0.8) as f32 * 6.0 * canvas_font_size_scale()).max(9.0),
            ),
            font: canvas_font(),
            ..canvas::Text::default()
        });
    }
}

fn content_bounds(board: &PcbBoard) -> Option<Aabb> {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    let mut include_point = |point: signex_types::pcb::Point| {
        min_x = min_x.min(point.x);
        min_y = min_y.min(point.y);
        max_x = max_x.max(point.x);
        max_y = max_y.max(point.y);
    };

    for point in &board.outline {
        include_point(*point);
    }
    for segment in &board.segments {
        include_point(segment.start);
        include_point(segment.end);
    }
    for via in &board.vias {
        include_point(via.position);
    }
    for footprint in &board.footprints {
        include_point(footprint.position);
        for pad in &footprint.pads {
            include_point(signex_types::pcb::Point::new(
                footprint.position.x + pad.position.x,
                footprint.position.y + pad.position.y,
            ));
        }
    }
    for zone in &board.zones {
        for point in &zone.outline {
            include_point(*point);
        }
    }
    for text in &board.texts {
        include_point(text.position);
    }

    if !min_x.is_finite() {
        None
    } else {
        Some(Aabb::new(min_x, min_y, max_x, max_y))
    }
}
