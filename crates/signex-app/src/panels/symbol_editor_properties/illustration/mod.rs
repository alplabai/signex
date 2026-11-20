//! The property illustrations of the symbol editor's Properties panel
//! (mockup row 123): transparent line art at the panel width and 140 px tall, built
//! from the selected object, with callouts linked to the field rows.

use iced::widget::container;
use iced::{Element, Length, Point, Widget as _};
use signex_widgets::property_illustration::{Illustration, property_illustration};

use super::rows::{Form, SP, TS, props};
use crate::library::editor::symbol::SymbolPropsMsg;
use crate::panels::PanelMsg;

mod graphic;
mod pin;
mod symbol;

pub(super) use graphic::graphic;
pub(super) use pin::pin;
pub(super) use symbol::symbol;

/// The view box of every illustration here.
pub(super) const VIEW_W: f32 = 264.0;
pub(super) const VIEW_H: f32 = 140.0;

/// Field ids linking a drawing part to its row.
pub(super) mod field {
    pub const SYM_PART: &str = "sym.part";
    pub const SYM_MIRROR: &str = "sym.mirror";
    pub const PIN_DN: &str = "pin.dn";
    pub const PIN_ETYPE: &str = "pin.etype";
    pub const PIN_XY: &str = "pin.xy";
    pub const PIN_LEN: &str = "pin.len";
    pub const PIN_ORIENT: &str = "pin.orient";
    pub const PIN_INSIDE: &str = "pin.inside";
    pub const PIN_IN_EDGE: &str = "pin.inedge";
    pub const PIN_OUT_EDGE: &str = "pin.outedge";
    pub const PIN_OUTSIDE: &str = "pin.outside";
    /// Feature switches: the pin's name and number are drawn.
    pub const PIN_NAME_SHOWN: &str = "pin.nameshown";
    pub const PIN_NUMBER_SHOWN: &str = "pin.numbershown";
    pub const G_START: &str = "g.start";
    pub const G_END: &str = "g.end";
    pub const G_SIZE: &str = "g.size";
    pub const G_CENTER: &str = "g.center";
    pub const G_RADIUS: &str = "g.radius";
    pub const G_START_ANGLE: &str = "g.startangle";
    pub const G_END_ANGLE: &str = "g.endangle";
    pub const G_POSITION: &str = "g.position";
    pub const G_TEXT_SIZE: &str = "g.textsize";
    pub const G_CONTENT: &str = "g.content";
    pub const G_STROKE: &str = "g.stroke";
    pub const G_FILL: &str = "g.fill";
    /// Feature switch: the graphic has a fill.
    pub const G_FILLED: &str = "g.filled";
}

/// An empty illustration on the shared view box, 140 px tall at the
/// default caption size and scaled with it.
pub(super) fn frame() -> Illustration {
    Illustration::new(VIEW_W, VIEW_H).height_em(VIEW_H / TS.caption)
}

/// The illustration widget, inset like the field rows.
pub(super) fn view<'a>(illustration: Illustration, form: &Form<'_>) -> Element<'a, PanelMsg> {
    let widget = property_illustration(illustration, form.hover, form.tokens, TS.caption)
        .map(|m| props(SymbolPropsMsg::IllustrationHovered(m)));
    container(widget)
        .padding(iced::Padding {
            top: SP.xs,
            right: SP.m,
            bottom: SP.s,
            left: SP.m,
        })
        .width(Length::Fill)
        .boxed()
}

/// Fits symbol mm (y up) into the view box (y down), uniformly and centred
/// in the box less `margin`, mirrored about the y axis when `mirrored`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Fit {
    scale: f64,
    offset: (f64, f64),
    mirrored: bool,
}

impl Fit {
    /// The fit of the points `points` (at least one).
    pub fn of(points: &[[f64; 2]], margin: f32, mirrored: bool) -> Option<Self> {
        let flip = |p: &[f64; 2]| if mirrored { -p[0] } else { p[0] };
        let (mut min_x, mut max_x) = (f64::INFINITY, f64::NEG_INFINITY);
        let (mut min_y, mut max_y) = (f64::INFINITY, f64::NEG_INFINITY);
        for p in points
            .iter()
            .filter(|p| p[0].is_finite() && p[1].is_finite())
        {
            let x = flip(p);
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_y = min_y.min(p[1]);
            max_y = max_y.max(p[1]);
        }
        if !min_x.is_finite() {
            return None;
        }
        let box_w = f64::from(VIEW_W - 2.0 * margin);
        let box_h = f64::from(VIEW_H - 2.0 * margin);
        let (w, h) = ((max_x - min_x).max(1e-6), (max_y - min_y).max(1e-6));
        let scale = (box_w / w).min(box_h / h);
        // Centre the box of the points in the view.
        let offset = (
            f64::from(VIEW_W) / 2.0 - (min_x + max_x) / 2.0 * scale,
            f64::from(VIEW_H) / 2.0 + (min_y + max_y) / 2.0 * scale,
        );
        Some(Self {
            scale,
            offset,
            mirrored,
        })
    }

    pub fn point(&self, p: [f64; 2]) -> Point {
        let x = if self.mirrored { -p[0] } else { p[0] };
        Point::new(
            (x * self.scale + self.offset.0) as f32,
            (self.offset.1 - p[1] * self.scale) as f32,
        )
    }

    pub fn length(&self, mm: f64) -> f32 {
        (mm * self.scale) as f32
    }

    /// The screen arc of a counter-clockwise symbol arc from `start_deg` to
    /// `end_deg`, as the widget draws it: clockwise on screen from the
    /// first angle to the second, in radians, the second never below the
    /// first.
    pub fn arc(&self, start_deg: f64, end_deg: f64) -> (f32, f32) {
        let (s, e) = (start_deg.to_radians(), end_deg.to_radians());
        // y flips, so a counter-clockwise sweep runs clockwise on screen
        // between the negated angles; a mirror reflects the angles about
        // the vertical axis as well.
        let (from, to) = if self.mirrored {
            (s - std::f64::consts::PI, e - std::f64::consts::PI)
        } else {
            (-e, -s)
        };
        let to = if to < from {
            to + std::f64::consts::TAU
        } else {
            to
        };
        (from as f32, to as f32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fit_centres_the_points_and_flips_y() {
        let fit = Fit::of(&[[-5.0, -2.0], [5.0, 2.0]], 20.0, false).unwrap();
        let centre = fit.point([0.0, 0.0]);
        assert!((centre.x - VIEW_W / 2.0).abs() < 1e-3);
        assert!((centre.y - VIEW_H / 2.0).abs() < 1e-3);
        assert!(fit.point([0.0, 2.0]).y < centre.y, "y up is up on screen");
    }

    #[test]
    fn a_mirrored_fit_swaps_left_and_right() {
        let plain = Fit::of(&[[0.0, 0.0], [4.0, 1.0]], 10.0, false).unwrap();
        let mirrored = Fit::of(&[[0.0, 0.0], [4.0, 1.0]], 10.0, true).unwrap();
        assert!(plain.point([4.0, 0.0]).x > plain.point([0.0, 0.0]).x);
        assert!(mirrored.point([4.0, 0.0]).x < mirrored.point([0.0, 0.0]).x);
    }

    #[test]
    fn nothing_to_fit_has_no_fit() {
        assert!(Fit::of(&[], 10.0, false).is_none());
    }

    #[test]
    fn a_quarter_arc_keeps_its_sweep_on_screen() {
        let fit = Fit::of(&[[-1.0, -1.0], [1.0, 1.0]], 10.0, false).unwrap();
        let (from, to) = fit.arc(300.0, 30.0);
        assert!((to - from - 90f32.to_radians()).abs() < 1e-4);
    }
}
