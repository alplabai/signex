use std::path::PathBuf;

use signex_types::coord::Unit;
use signex_types::measure::{length_field_text, parse_length_field_mm};
use signex_types::text_increment::{STEP_MAX, STEP_MIN};

/// Most copies one Paste Array places.
pub const MAX_COPIES: u32 = 1000;

/// No array reaches further than this from where it was copied, so a typo
/// cannot push copies out of any real sheet.
const MAX_EXTENT_MM: f64 = 1.0e6;

/// The open dialog: what is typed, as typed.
#[derive(Debug, Clone)]
pub struct PasteArrayState {
    pub count: String,
    pub x_spacing: String,
    pub y_spacing: String,
    pub step: String,
    /// The unit the spacings are typed in (the active unit when it opened).
    pub unit: Unit,
    /// The sheet the dialog was opened on; OK places on that sheet only.
    pub origin: Option<PathBuf>,
}

/// A dialog whose fields all parse.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PasteArrayInput {
    pub count: u32,
    pub dx_mm: f64,
    pub dy_mm: f64,
    pub step: i32,
}

impl PasteArrayState {
    /// A fresh dialog: 2 copies, `dx_mm` / `dy_mm` apart, counting by `step`.
    pub fn new(unit: Unit, dx_mm: f64, dy_mm: f64, step: i32, origin: Option<PathBuf>) -> Self {
        Self {
            count: "2".to_owned(),
            x_spacing: length_field_text(dx_mm, unit),
            y_spacing: length_field_text(dy_mm, unit),
            step: step.to_string(),
            unit,
            origin,
        }
    }

    /// The parsed fields, or why OK is disabled.
    pub fn input(&self) -> Result<PasteArrayInput, String> {
        let count = self
            .count
            .trim()
            .parse::<u32>()
            .ok()
            .filter(|n| (1..=MAX_COPIES).contains(n))
            .ok_or_else(|| format!("Copies must be a whole number from 1 to {MAX_COPIES}."))?;
        let spacing = |text: &str, axis: &str| {
            parse_length_field_mm(text, self.unit)
                .filter(|mm| (mm * f64::from(count)).abs() <= MAX_EXTENT_MM)
                .ok_or_else(|| {
                    format!(
                        "{axis} spacing must be a number in {} (at most {} for the whole array).",
                        self.unit,
                        length_field_text(MAX_EXTENT_MM / f64::from(count), self.unit),
                    )
                })
        };
        let dx_mm = spacing(&self.x_spacing, "X")?;
        let dy_mm = spacing(&self.y_spacing, "Y")?;
        let step = self
            .step
            .trim()
            .parse::<i32>()
            .ok()
            .filter(|s| (STEP_MIN..=STEP_MAX).contains(s))
            .ok_or_else(|| format!("Step must be a whole number from {STEP_MIN} to {STEP_MAX}."))?;
        Ok(PasteArrayInput {
            count,
            dx_mm,
            dy_mm,
            step,
        })
    }
}

#[derive(Debug, Clone)]
pub enum PasteArrayMsg {
    CountChanged(String),
    XSpacingChanged(String),
    YSpacingChanged(String),
    StepChanged(String),
    /// OK: place the array.
    ApplyPressed,
    Closed,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> PasteArrayState {
        PasteArrayState::new(Unit::Mm, 10.0, 0.0, 1, None)
    }

    #[test]
    fn a_fresh_dialog_is_valid() {
        let input = state().input().unwrap();
        assert_eq!(
            input,
            PasteArrayInput {
                count: 2,
                dx_mm: 10.0,
                dy_mm: 0.0,
                step: 1
            }
        );
    }

    #[test]
    fn the_count_is_a_whole_number_in_range() {
        for bad in ["0", "1001", "-1", "2.5", "", "x"] {
            let mut s = state();
            s.count = bad.to_owned();
            assert!(s.input().unwrap_err().contains("Copies"), "{bad:?}");
        }
        for good in ["1", "1000", " 7 "] {
            let mut s = state();
            s.count = good.to_owned();
            assert!(s.input().is_ok(), "{good:?}");
        }
    }

    #[test]
    fn spacings_must_be_finite_and_keep_the_array_on_the_sheet() {
        for bad in ["", "abc", "inf", "NaN", "1e999", "1e12"] {
            let mut s = state();
            s.x_spacing = bad.to_owned();
            assert!(s.input().unwrap_err().contains("X spacing"), "{bad:?}");
            let mut s = state();
            s.y_spacing = bad.to_owned();
            assert!(s.input().unwrap_err().contains("Y spacing"), "{bad:?}");
        }
    }

    #[test]
    fn spacings_are_read_in_the_dialogs_unit() {
        let mut s = PasteArrayState::new(Unit::Mil, 2.54, 0.0, 1, None);
        s.y_spacing = "-200".to_owned();
        let input = s.input().unwrap();
        assert!((input.dx_mm - 2.54).abs() < 1e-9);
        assert!((input.dy_mm + 5.08).abs() < 1e-9);
    }

    #[test]
    fn the_step_is_a_whole_number_in_range_and_may_be_zero() {
        for bad in ["101", "-101", "1.5", ""] {
            let mut s = state();
            s.step = bad.to_owned();
            assert!(s.input().unwrap_err().contains("Step"), "{bad:?}");
        }
        let mut s = state();
        s.step = "0".to_owned();
        assert_eq!(s.input().unwrap().step, 0);
    }
}
