//! Footprint content layers — silk graphics, courtyard outline, pads,
//! and the Array source-pad "+N" badges. Drawn above the backdrop and
//! below the interaction ghosts / overlays. Extracted verbatim from
//! `Program::draw`.

use iced::widget::canvas::{self, Path, Stroke};
use iced::{Color, Point};

use crate::library::editor::footprint::layers::FpLayer;

use super::pad::draw_pad;
use super::silk::draw_silk_graphics;
use super::super::{FootprintCanvas, FootprintCanvasState};

impl FootprintCanvas<'_> {
    /// v0.18.16 — silk-front + silk-back graphics.
    pub(in crate::library::editor::footprint::canvas) fn draw_silk_layers(
        &self,
        frame: &mut canvas::Frame,
        cstate: &FootprintCanvasState,
    ) {
        if self.state.layer_visibility.get(FpLayer::FSilks) {
            draw_silk_graphics(
                frame,
                cstate,
                self.silk_f,
                FpLayer::FSilks,
                self.state.selected_silk_f,
            );
        }
        if self.state.layer_visibility.get(FpLayer::BSilks) {
            draw_silk_graphics(frame, cstate, self.silk_b, FpLayer::BSilks, None);
        }
    }

    /// Courtyard — outline-following polygon takes precedence over the
    /// bbox rectangle when present (v0.27); fall back to the bbox for
    /// legacy state.
    pub(in crate::library::editor::footprint::canvas) fn draw_courtyard(
        &self,
        frame: &mut canvas::Frame,
        cstate: &FootprintCanvasState,
    ) {
        if self.state.layer_visibility.get(FpLayer::EdgeCuts) {
            let edge_color = FpLayer::EdgeCuts.color();
            if let Some(outline) = self.state.courtyard_outline_mm.as_ref() {
                if outline.len() >= 3 {
                    let path = Path::new(|b| {
                        let first = cstate.world_to_screen(outline[0]);
                        b.move_to(first);
                        for v in outline.iter().skip(1) {
                            b.line_to(cstate.world_to_screen(*v));
                        }
                        b.line_to(first);
                    });
                    frame.stroke(
                        &path,
                        Stroke::default().with_width(1.5).with_color(edge_color),
                    );
                }
            } else if let Some(c) = self.state.courtyard_mm {
                let p0 = cstate.world_to_screen((c.min_x, c.min_y));
                let p1 = cstate.world_to_screen((c.max_x, c.max_y));
                let rect = Path::rectangle(
                    Point::new(p0.x, p0.y),
                    iced::Size::new(p1.x - p0.x, p1.y - p0.y),
                );
                frame.stroke(
                    &rect,
                    Stroke::default().with_width(1.5).with_color(edge_color),
                );
            }
        }
    }

    /// Pads — render last of the content layers so they sit on top.
    pub(in crate::library::editor::footprint::canvas) fn draw_pads_layer(
        &self,
        frame: &mut canvas::Frame,
        cstate: &FootprintCanvasState,
    ) {
        for (idx, pad) in self.state.pads.iter().enumerate() {
            if !self.state.layer_visibility.get(pad.primary_layer()) {
                continue;
            }
            // v0.27 — multi-select highlight: primary OR extras.
            let is_selected = self.state.selected_pad == Some(idx)
                || self.state.selected_pads_extra.contains(&idx);
            draw_pad(frame, cstate, pad, is_selected);
        }
    }

    /// v0.25 — Array source-pad indicator: a "+N" badge at the
    /// top-right of a pad that is the `source` of an Array, showing the
    /// replica count.
    pub(in crate::library::editor::footprint::canvas) fn draw_array_badges(
        &self,
        frame: &mut canvas::Frame,
        cstate: &FootprintCanvasState,
    ) {
        if let Some(sketch) = self.sketch {
            let array_source_counts: std::collections::HashMap<
                signex_sketch::id::SketchEntityId,
                usize,
            > = sketch
                .arrays
                .iter()
                .filter_map(|a| {
                    use signex_sketch::array::ArrayKind;
                    let (source, count) = match &a.kind {
                        ArrayKind::Linear {
                            source, count_expr, ..
                        } => (*source, count_expr.trim().parse::<usize>().unwrap_or(0)),
                        ArrayKind::Grid {
                            source,
                            nx_expr,
                            ny_expr,
                            ..
                        } => {
                            let nx = nx_expr.trim().parse::<usize>().unwrap_or(0);
                            let ny = ny_expr.trim().parse::<usize>().unwrap_or(0);
                            (*source, nx * ny)
                        }
                        ArrayKind::Polar {
                            source, count_expr, ..
                        } => (*source, count_expr.trim().parse::<usize>().unwrap_or(0)),
                    };
                    if count > 0 {
                        Some((source, count))
                    } else {
                        None
                    }
                })
                .fold(std::collections::HashMap::new(), |mut acc, (id, count)| {
                    *acc.entry(id).or_insert(0) += count;
                    acc
                });

            if !array_source_counts.is_empty() && cstate.scale >= 12.0 {
                for pad in self.state.pads.iter() {
                    let Some(entity_id) = pad.sketch_entity_id else {
                        continue;
                    };
                    let Some(replica_count) = array_source_counts.get(&entity_id) else {
                        continue;
                    };
                    if !self.state.layer_visibility.get(pad.primary_layer()) {
                        continue;
                    }
                    let (_, _, x1, y1) = pad.rotated_aabb_mm();
                    let p1 = cstate.world_to_screen((x1, y1));
                    // Badge: 22×12 px rounded rect, accent fill, white
                    // "+N" text, positioned above + right of the bbox.
                    let badge_w: f32 = 22.0;
                    let badge_h: f32 = 12.0;
                    let bx = p1.x + 4.0;
                    let by = p1.y - 6.0 - badge_h;
                    let badge_rect =
                        Path::rectangle(Point::new(bx, by), iced::Size::new(badge_w, badge_h));
                    // Altium-orange accent fill.
                    frame.fill(&badge_rect, Color::from_rgba(0.96, 0.62, 0.18, 0.95));
                    frame.stroke(
                        &badge_rect,
                        Stroke::default()
                            .with_width(0.8)
                            .with_color(Color::from_rgba(0.0, 0.0, 0.0, 0.6)),
                    );
                    frame.fill_text(canvas::Text {
                        content: format!("+{replica_count}"),
                        position: Point::new(bx + badge_w / 2.0, by + 1.0),
                        color: Color::WHITE,
                        size: 9.5.into(),
                        align_x: iced::alignment::Horizontal::Center.into(),
                        align_y: iced::alignment::Vertical::Top,
                        ..canvas::Text::default()
                    });
                }
            }
        }
    }
}
