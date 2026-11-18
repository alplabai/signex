//! Pad shapes a hand-edited `.snxfpt` can carry that make no copper (#2242).
//! The board's rule is `signex_types::pcb_pad_shape`; the footprint editor
//! draws such a pad as its plain box, so the caller that opens the file
//! tells the user.

use signex_types::pcb::Point;
use signex_types::pcb_pad_shape::trapezoid_corners;

use super::{Footprint, PadShape};

impl Footprint {
    /// One line per trapezoid pad whose delta makes no copper with the pad's
    /// size, naming the pad and what is wrong; empty when every one does.
    pub fn trapezoid_pad_faults(&self) -> Vec<String> {
        self.pads
            .iter()
            .filter_map(|pad| {
                let PadShape::Trapezoid { delta } = pad.shape else {
                    return None;
                };
                let size = Point::new(pad.size[0], pad.size[1]);
                let fault = trapezoid_corners(size, Point::new(delta[0], delta[1])).err()?;
                Some(format!("pad {}: {fault}", pad.number))
            })
            .collect()
    }
}
