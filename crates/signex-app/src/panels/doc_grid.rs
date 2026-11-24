//! A typed schematic Visible Grid or Snap Grid pitch (mockup row 40): read in
//! the row's unit, under the fast grids' limits, and applied on Enter.

use signex_types::coord::Unit;
use signex_types::measure::mm_per;

use super::PanelMsg;
use crate::canvas::grid::{FastGridRefusal, fast_grid_mm, pitch_mm_exact};
use crate::numeric_field::validated_grid_step;

/// Which pitch row of Document Options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocGridField {
    Visible,
    Snap,
}

impl DocGridField {
    /// The message that sets this pitch, as a row of its list does.
    pub(crate) fn set_message(self, mm: f32) -> PanelMsg {
        match self {
            Self::Visible => PanelMsg::SetVisibleGridSize(mm),
            Self::Snap => PanelMsg::SetGridSize(mm),
        }
    }

    /// How the field is named in a refusal line.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Visible => "Visible Grid",
            Self::Snap => "Snap Grid",
        }
    }
}

/// The typed text of a pitch row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocGridMsg {
    /// A keystroke; nothing applies until Enter.
    DraftChanged(DocGridField, String),
    /// Enter in the box.
    DraftSubmitted(DocGridField),
}

/// The unit a pitch row reads and shows: mil in mil, else mm, as its list.
pub(crate) fn pitch_unit(unit: Unit) -> Unit {
    if unit == Unit::Mil {
        Unit::Mil
    } else {
        Unit::Mm
    }
}

/// `text` as a pitch in mm, or why it is refused.
pub(crate) fn typed_pitch_mm(text: &str, unit: Unit) -> Result<f32, String> {
    let step = validated_grid_step(text)?;
    fast_grid_mm(step * mm_per(pitch_unit(unit))).map_err(|why| match why {
        FastGridRefusal::Invalid(reason) => reason,
        FastGridRefusal::BelowMinimum => format!("{text} is finer than the finest grid, 1 nm"),
    })
}

/// A pitch as its box shows it: the number alone, the unit is a tag.
pub(crate) fn pitch_text(mm: f32, unit: Unit) -> String {
    let value = pitch_mm_exact(mm) / mm_per(pitch_unit(unit));
    let text = format!("{value:.6}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_typed_pitch_reads_in_the_rows_unit() {
        assert_eq!(typed_pitch_mm("2", Unit::Mm), Ok(2.0));
        assert_eq!(typed_pitch_mm("50", Unit::Mil), Ok(1.27));
        assert_eq!(typed_pitch_mm("2", Unit::Inch), Ok(2.0));
    }

    /// The fast grids' limits: a number, above zero, no finer than 1 nm,
    /// and one a grid can hold.
    #[test]
    fn a_pitch_outside_the_snap_limits_is_refused() {
        for bad in ["", "abc", "0", "-1", "1e300", "1e-9", "inf"] {
            assert!(typed_pitch_mm(bad, Unit::Mm).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_pitch_shows_as_its_shortest_number() {
        assert_eq!(pitch_text(1.27, Unit::Mm), "1.27");
        assert_eq!(pitch_text(1.27, Unit::Mil), "50");
        assert_eq!(pitch_text(0.635, Unit::Mil), "25");
        assert_eq!(pitch_text(5.08, Unit::Mm), "5.08");
    }
}
