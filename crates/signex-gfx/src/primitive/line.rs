//! Line primitive type.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: IPC-2612-1, IEEE 315, IEC 60617, wgpu/WGSL public docs.

/// Straight line segment with width and style.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LineSegment {
    pub p0: [f32; 2],
    pub p1: [f32; 2],
    pub width: f32,
    pub color: [f32; 4],
    pub style: u32,
    pub _pad: u32,
}

impl LineSegment {
    /// `style` bit that marks a dashed (rather than solid) segment.
    pub const STYLE_DASHED: u32 = 1;

    /// Whether this segment renders dashed. The low `style` bit selects the
    /// dash pattern; the rest is reserved. This is the shared predicate the CPU
    /// renderer honours and the GPU `line.wgsl` shader must match — the CPU↔GPU
    /// parity test locks both paths to it.
    pub fn is_dashed(&self) -> bool {
        (self.style & Self::STYLE_DASHED) == Self::STYLE_DASHED
    }

    /// `style` bit that marks a dotted segment (short marks, wide gaps).
    /// Dashed wins when both bits are set.
    pub const STYLE_DOTTED: u32 = 2;

    /// Whether this segment renders dotted.
    pub fn is_dotted(&self) -> bool {
        !self.is_dashed() && (self.style & Self::STYLE_DOTTED) == Self::STYLE_DOTTED
    }

    /// The `(mark, gap)` lengths in screen pixels for a patterned segment,
    /// `None` for a solid one. The CPU replay reads this directly and
    /// `line.wgsl` repeats the same constants.
    pub fn pattern_px(&self) -> Option<(f32, f32)> {
        if self.is_dashed() {
            Some((Self::DASH_PX, Self::DASH_GAP_PX))
        } else if self.is_dotted() {
            Some((Self::DOT_PX, Self::DOT_GAP_PX))
        } else {
            None
        }
    }

    pub const DASH_PX: f32 = 8.0;
    pub const DASH_GAP_PX: f32 = 5.0;
    pub const DOT_PX: f32 = 2.0;
    pub const DOT_GAP_PX: f32 = 4.0;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_style(style: u32) -> LineSegment {
        LineSegment {
            p0: [0.0; 2],
            p1: [1.0, 0.0],
            width: 0.1,
            color: [1.0; 4],
            style,
            _pad: 0,
        }
    }

    #[test]
    fn solid_has_no_pattern() {
        assert_eq!(with_style(0).pattern_px(), None);
    }

    #[test]
    fn dashed_and_dotted_have_distinct_patterns() {
        let dashed = with_style(LineSegment::STYLE_DASHED).pattern_px();
        let dotted = with_style(LineSegment::STYLE_DOTTED).pattern_px();
        assert_eq!(dashed, Some((8.0, 5.0)));
        assert_eq!(dotted, Some((2.0, 4.0)));
    }

    #[test]
    fn dashed_wins_when_both_bits_are_set() {
        let both = with_style(LineSegment::STYLE_DASHED | LineSegment::STYLE_DOTTED);
        assert!(both.is_dashed());
        assert!(!both.is_dotted());
    }

    #[test]
    fn net_bits_above_the_low_byte_do_not_select_a_pattern() {
        assert_eq!(with_style(7 << 8).pattern_px(), None);
    }
}
