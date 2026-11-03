//! Graphical footprint preview of a `.snxfpt`: a board-coloured backdrop
//! with the bottom and top silkscreen, courtyard, fab / body outline and
//! numbered pads, fitted to the box with a margin, plus a size readout and
//! a scale bar. View-only.

mod graphics;
mod layout;
mod palette;

use iced::widget::canvas::{self, Frame, Geometry, Path, Stroke, Text};
use iced::{Point, Radians, Rectangle, Renderer, Size, Theme, Vector, alignment, mouse};
use signex_library::primitive::{Footprint, PadShape};
use signex_types::theme::ThemeTokens;

use self::graphics::{closed_path, draw_graphic};
use self::layout::{Extent, Fit, courtyard, extent, scale_bar_label, scale_bar_mm, size_label};
use self::palette::Palette;
use crate::library::editor::kit::{SP, TS};
use crate::styles::HAIRLINE;

/// Text shown in place of a drawing when the footprint has no geometry.
pub(crate) const EMPTY_FOOTPRINT_TEXT: &str = "Footprint has no geometry";
/// The scale bar aims at this share of the box width.
const SCALE_BAR_SHARE: f32 = 0.25;
/// Pad numbers are this share of the pad's short side, capped at a title.
const PAD_NUMBER_SHARE: f32 = 0.6;

pub struct FootprintPreview {
    footprint: Footprint,
    palette: Palette,
}

impl FootprintPreview {
    pub fn new(footprint: Footprint, tokens: &ThemeTokens) -> Self {
        Self {
            footprint,
            palette: Palette::from_tokens(tokens),
        }
    }
}

/// Where the drawing may go: the box minus a margin and the readout row.
fn drawing_area(bounds: Size) -> Rectangle {
    let footer = TS.caption + SP.s * 2.0;
    Rectangle::new(
        Point::new(SP.l, SP.l),
        Size::new(
            (bounds.width - SP.l * 2.0).max(0.0),
            (bounds.height - SP.l * 2.0 - footer).max(0.0),
        ),
    )
}

impl<M> canvas::Program<M> for FootprintPreview {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let p = &self.palette;
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), p.backdrop);

        let fitted = extent(&self.footprint)
            .and_then(|e| Fit::new(e, drawing_area(bounds.size())).map(|fit| (e, fit)));
        let Some((extent, fit)) = fitted else {
            frame.fill_text(Text {
                content: EMPTY_FOOTPRINT_TEXT.to_string(),
                position: frame.center(),
                color: p.readout,
                size: TS.body.into(),
                align_x: alignment::Horizontal::Center.into(),
                align_y: alignment::Vertical::Center,
                ..Text::default()
            });
            return vec![frame.into_geometry()];
        };

        let fp = &self.footprint;
        // Bottom layers first, then the courtyard, body, pads, top silk.
        for (group, colour) in [(&fp.fab_b, p.fab), (&fp.silk_b, p.silk_bottom)] {
            for g in group {
                draw_graphic(&mut frame, g, colour, &fit);
            }
        }
        draw_courtyard(&mut frame, fp, &fit, p);
        for g in &fp.fab_f {
            draw_graphic(&mut frame, g, p.fab, &fit);
        }
        draw_pads(&mut frame, fp, &fit, p);
        for g in &fp.silk_f {
            draw_graphic(&mut frame, g, p.silk_top, &fit);
        }
        draw_readouts(&mut frame, bounds.size(), &extent, &fit, p);
        vec![frame.into_geometry()]
    }
}

fn draw_courtyard(frame: &mut Frame, fp: &Footprint, fit: &Fit, p: &Palette) {
    let Some(court) = courtyard(fp) else { return };
    let stroke = Stroke::default()
        .with_color(p.courtyard)
        .with_width(HAIRLINE);
    frame.stroke(&closed_path(court, fit), stroke);
}

fn draw_pads(frame: &mut Frame, fp: &Footprint, fit: &Fit, p: &Palette) {
    for pad in &fp.pads {
        let size = Size::new(fit.px(pad.size[0]), fit.px(pad.size[1]));
        let at = fit.to_px(pad.position);
        let short = size.width.min(size.height);
        frame.with_save(|f| {
            f.translate(Vector::new(at.x, at.y));
            f.rotate(Radians(pad.rotation.to_radians() as f32));
            let top_left = Point::new(-size.width / 2.0, -size.height / 2.0);
            let path = match &pad.shape {
                PadShape::Round => Path::circle(Point::ORIGIN, short / 2.0),
                PadShape::Oval => Path::rounded_rectangle(top_left, size, (short / 2.0).into()),
                PadShape::RoundRect { radius_ratio } => {
                    Path::rounded_rectangle(top_left, size, (short * *radius_ratio as f32).into())
                }
                _ => Path::rectangle(top_left, size),
            };
            f.fill(&path, p.pad);
            if let Some(drill) = &pad.drill {
                let d = fit.px(drill.diameter);
                let len = drill.slot_length.map_or(d, |l| fit.px(l).max(d));
                let hole = Path::rounded_rectangle(
                    Point::new(-len / 2.0, -d / 2.0),
                    Size::new(len, d),
                    (d / 2.0).into(),
                );
                f.fill(&hole, p.backdrop);
            }
        });
        if short >= TS.body {
            frame.fill_text(Text {
                content: pad.number.clone(),
                position: at,
                color: p.pad_text,
                size: (short * PAD_NUMBER_SHARE).min(TS.title).into(),
                align_x: alignment::Horizontal::Center.into(),
                align_y: alignment::Vertical::Center,
                ..Text::default()
            });
        }
    }
}

/// Average glyph width as a share of the font size, for the readouts'
/// room check (a glyph is narrower than a full em).
const GLYPH_EM: f32 = 0.55;

/// Size text bottom-left and a round-number scale bar bottom-right.
fn draw_readouts(frame: &mut Frame, bounds: Size, extent: &Extent, fit: &Fit, p: &Palette) {
    let baseline = bounds.height - SP.s;
    frame.fill_text(Text {
        content: size_label(extent),
        position: Point::new(SP.l, baseline),
        color: p.readout,
        size: TS.caption.into(),
        align_y: alignment::Vertical::Bottom,
        ..Text::default()
    });

    // Skip the scale bar when it would run into the size label.
    let label_end = SP.l + size_label(extent).chars().count() as f32 * TS.caption * GLYPH_EM;
    let target_mm = f64::from(bounds.width * SCALE_BAR_SHARE) / fit.scale;
    let Some(bar_mm) = scale_bar_mm(target_mm) else {
        return;
    };
    let bar_px = fit.px(bar_mm);
    let label_w = scale_bar_label(bar_mm).chars().count() as f32 * TS.caption * GLYPH_EM;
    if bounds.width - SP.l - bar_px - SP.m - label_w < label_end {
        return;
    }
    let right = bounds.width - SP.l;
    let y = baseline - TS.caption / 2.0;
    let tick = TS.caption / 2.0;
    let stroke = || Stroke::default().with_color(p.readout).with_width(HAIRLINE);
    let (a, b) = (Point::new(right - bar_px, y), Point::new(right, y));
    frame.stroke(&Path::line(a, b), stroke());
    for end in [a, b] {
        frame.stroke(
            &Path::line(
                Point::new(end.x, end.y - tick / 2.0),
                Point::new(end.x, end.y + tick / 2.0),
            ),
            stroke(),
        );
    }
    frame.fill_text(Text {
        content: scale_bar_label(bar_mm),
        position: Point::new(a.x - SP.m, baseline),
        color: p.readout,
        size: TS.caption.into(),
        align_x: alignment::Horizontal::Right.into(),
        align_y: alignment::Vertical::Bottom,
        ..Text::default()
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_drawing_area_leaves_room_for_the_margin_and_the_readout_row() {
        let bounds = Size::new(400.0, 200.0);
        let area = drawing_area(bounds);
        assert!(area.x > 0.0 && area.y > 0.0);
        assert!(area.x + area.width < bounds.width);
        assert!(area.y + area.height < bounds.height - TS.caption);
    }

    #[test]
    fn a_box_smaller_than_its_margins_has_an_empty_area() {
        let area = drawing_area(Size::new(1.0, 1.0));
        assert_eq!((area.width, area.height), (0.0, 0.0));
    }
}
