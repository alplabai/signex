//! Editor-side wrappers for the shared content layers (see
//! [`super::layers`]) plus the Array source-pad "+N" badges. Drawn above
//! the backdrop and below the interaction ghosts / overlays.

use iced::widget::canvas::{self, Path, Stroke};
use iced::{Color, Point};

use super::super::{FootprintCanvas, FootprintCanvasState};
use super::layers::{LayerInput, draw_courtyard, draw_pads, draw_silk};
use super::pad::PadLabels;

impl FootprintCanvas<'_> {
    /// The read-only view of the model the shared content layers draw.
    fn layer_input(&self) -> LayerInput<'_> {
        LayerInput {
            pads: &self.state.pads,
            silk_f: self.silk_f,
            silk_b: self.silk_b,
            visibility: self.state.layer_visibility,
            courtyard_outline: self.state.courtyard_outline(),
            courtyard_rect: self.state.courtyard_mm,
            selected_pad: self.state.selected_pad,
            selected_pads_extra: &self.state.selected_pads_extra,
            selected_silk_f: self.state.selected_silk_f,
            pad_labels: PadLabels::Editor,
            palette: super::graphite::look_from_bg(self.bg_color)
                .map_or(self.state.layer_palette, |l| l.palette()),
        }
    }

    /// Silk-front + silk-back graphics.
    pub(in crate::library::editor::footprint::canvas) fn draw_silk_layers(
        &self,
        frame: &mut canvas::Frame,
        cstate: &FootprintCanvasState,
    ) {
        draw_silk(frame, cstate, &self.layer_input());
    }

    /// Courtyard outline (or bbox fallback).
    pub(in crate::library::editor::footprint::canvas) fn draw_courtyard(
        &self,
        frame: &mut canvas::Frame,
        cstate: &FootprintCanvasState,
    ) {
        draw_courtyard(frame, cstate, &self.layer_input());
    }

    /// Pads, on top of the other content layers.
    pub(in crate::library::editor::footprint::canvas) fn draw_pads_layer(
        &self,
        frame: &mut canvas::Frame,
        cstate: &FootprintCanvasState,
    ) {
        draw_pads(frame, cstate, &self.layer_input());
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
