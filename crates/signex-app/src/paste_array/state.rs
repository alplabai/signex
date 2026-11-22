use std::path::PathBuf;

use signex_types::coord::Unit;
use signex_widgets::property_illustration::{IllustrationHover, IllustrationMsg};

use crate::paste_prefs::PasteDesignators;
use signex_types::measure::{length_field_text, parse_length_field_mm};
use signex_types::text_increment::{STEP_MAX, STEP_MIN};

/// Most copies one Paste Array places.
pub const MAX_COPIES: u32 = 1000;

/// Most items (copies times the items in one copy) one Paste Array places.
pub const MAX_PLACED_ITEMS: usize = 100_000;

/// No array reaches further than this from where it was copied, so a typo
/// cannot push copies out of any real sheet.
const MAX_EXTENT_MM: f64 = 1.0e6;

/// The step "Count up label numbers" turns on with when the preference
/// step is 0.
const COUNT_UP_FALLBACK_STEP: i32 = 1;

/// The open dialog: what is typed, as typed.
#[derive(Debug, Clone)]
pub struct PasteArrayState {
    pub count: String,
    pub x_spacing: String,
    pub y_spacing: String,
    /// The label step; `0` is "Count up label numbers" off, the only state
    /// that checkbox has.
    pub step: String,
    /// What the pasted parts are called; starts as the saved preference.
    pub designators: PasteDesignators,
    /// How many items one copy holds, for the item cap.
    pub items_per_copy: usize,
    /// The unit the spacings are typed in (the active unit when it opened).
    pub unit: Unit,
    /// The sheet the dialog was opened on; OK places on that sheet only.
    pub origin: Option<PathBuf>,
    /// The project's numbers are being read for a Next free choice; OK waits.
    pub seed_pending: bool,
    /// Which opening of the dialog this is (`UiState::paste_array_opens`).
    pub id: u64,
    /// The first label text of the copy, for the help line's worked example
    /// and the preview.
    pub sample_label: Option<String>,
    /// Width and height of one copy, in mm, for the preview.
    pub copy_size_mm: Option<(f64, f64)>,
    /// The sheet grid pitch, in mm, the preview is drawn on.
    pub grid_mm: f64,
    /// The preview callout under the pointer.
    pub hover: IllustrationHover,
    /// The preview is drawn at its taller size.
    pub preview_expanded: bool,
}

/// A dialog whose fields all parse.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PasteArrayInput {
    pub count: u32,
    pub dx_mm: f64,
    pub dy_mm: f64,
    pub step: i32,
    pub designators: PasteDesignators,
}

/// The field a refusal is about; it is drawn red with the reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteArrayField {
    Copies,
    XSpacing,
    YSpacing,
    Step,
}

impl PasteArrayState {
    /// A fresh dialog: 2 copies, `dx_mm` / `dy_mm` apart, counting by `step`,
    /// of a copy holding `items_per_copy` items.
    pub fn new(
        unit: Unit,
        dx_mm: f64,
        dy_mm: f64,
        step: i32,
        designators: PasteDesignators,
        items_per_copy: usize,
        origin: Option<PathBuf>,
    ) -> Self {
        Self {
            count: "2".to_owned(),
            x_spacing: length_field_text(dx_mm, unit),
            y_spacing: length_field_text(dy_mm, unit),
            step: step.to_string(),
            designators,
            items_per_copy,
            unit,
            origin,
            seed_pending: false,
            id: 0,
            sample_label: None,
            copy_size_mm: None,
            grid_mm: 0.0,
            hover: IllustrationHover::default(),
            preview_expanded: false,
        }
    }

    /// The copies typed, when they are a whole number from 1 to
    /// [`MAX_COPIES`].
    pub fn copies(&self) -> Option<u32> {
        self.count
            .trim()
            .parse::<u32>()
            .ok()
            .filter(|n| (1..=MAX_COPIES).contains(n))
    }

    /// How many items the paste places: copies times the items in one copy.
    pub fn placed_items(&self) -> Option<usize> {
        self.copies()
            .map(|n| (n as usize).saturating_mul(self.items_per_copy))
    }

    /// Whether "Count up label numbers" is ticked: any step but 0. A step
    /// still being typed counts as ticked.
    pub fn counts_up(&self) -> bool {
        self.step.trim().parse::<i32>() != Ok(0)
    }

    /// Ticks or clears "Count up label numbers": off is step 0; on restores
    /// `preferred` (the preference step), or 1 when that is 0 too.
    pub fn set_count_up(&mut self, on: bool, preferred: i32) {
        if !on {
            self.step = "0".to_owned();
        } else if !self.counts_up() {
            let step = if preferred == 0 {
                COUNT_UP_FALLBACK_STEP
            } else {
                preferred
            };
            self.step = step.to_string();
        }
    }

    /// The parsed fields, or why OK is disabled.
    pub fn input(&self) -> Result<PasteArrayInput, String> {
        self.check().map_err(|(_, reason)| reason)
    }

    /// The field a refusal is about, if the dialog is refused.
    pub fn invalid_field(&self) -> Option<PasteArrayField> {
        self.check().err().map(|(field, _)| field)
    }

    /// [`Self::input`] with the field the refusal is about.
    pub fn check(&self) -> Result<PasteArrayInput, (PasteArrayField, String)> {
        let count = self.copies().ok_or_else(|| {
            (
                PasteArrayField::Copies,
                format!("Copies must be a whole number from 1 to {MAX_COPIES}."),
            )
        })?;
        if (count as usize).saturating_mul(self.items_per_copy) > MAX_PLACED_ITEMS {
            return Err((
                PasteArrayField::Copies,
                format!(
                    "{count} copies of {} items is over the limit of {MAX_PLACED_ITEMS} items; use fewer copies.",
                    self.items_per_copy
                ),
            ));
        }
        let spacing = |text: &str, axis: &str, field: PasteArrayField| {
            parse_length_field_mm(text, self.unit)
                .filter(|mm| (mm * f64::from(count)).abs() <= MAX_EXTENT_MM)
                .ok_or_else(|| {
                    (
                        field,
                        format!(
                            "{axis} spacing must be a number in {} (at most {} for the whole array).",
                            self.unit,
                            length_field_text(MAX_EXTENT_MM / f64::from(count), self.unit),
                        ),
                    )
                })
        };
        let dx_mm = spacing(&self.x_spacing, "X", PasteArrayField::XSpacing)?;
        let dy_mm = spacing(&self.y_spacing, "Y", PasteArrayField::YSpacing)?;
        let step = self
            .step
            .trim()
            .parse::<i32>()
            .ok()
            .filter(|s| (STEP_MIN..=STEP_MAX).contains(s))
            .ok_or_else(|| {
                (
                    PasteArrayField::Step,
                    format!("Step must be a whole number from {STEP_MIN} to {STEP_MAX}."),
                )
            })?;
        Ok(PasteArrayInput {
            count,
            dx_mm,
            dy_mm,
            step,
            designators: self.designators,
        })
    }
}

#[derive(Debug, Clone)]
pub enum PasteArrayMsg {
    CountChanged(String),
    XSpacingChanged(String),
    YSpacingChanged(String),
    StepChanged(String),
    /// "Count up label numbers" was ticked (`true`) or cleared.
    CountUpToggled(bool),
    DesignatorsChosen(PasteDesignators),
    /// The pointer moved over the preview's spacing callouts.
    PreviewHovered(IllustrationMsg),
    /// The preview's expand button was pressed.
    PreviewExpandToggled,
    /// OK: place the array.
    ApplyPressed,
    Closed,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> PasteArrayState {
        PasteArrayState::new(Unit::Mm, 10.0, 0.0, 1, PasteDesignators::Keep, 3, None)
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
                step: 1,
                designators: PasteDesignators::Keep,
            }
        );
    }

    #[test]
    fn the_count_is_a_whole_number_in_range() {
        for bad in ["0", "1001", "-1", "2.5", "", "x"] {
            let mut s = state();
            s.count = bad.to_owned();
            assert!(s.input().unwrap_err().contains("Copies"), "{bad:?}");
            assert_eq!(s.invalid_field(), Some(PasteArrayField::Copies), "{bad:?}");
        }
        for good in ["1", "1000", " 7 "] {
            let mut s = state();
            s.count = good.to_owned();
            assert!(s.input().is_ok(), "{good:?}");
            assert_eq!(s.invalid_field(), None, "{good:?}");
        }
    }

    #[test]
    fn the_total_item_count_is_capped() {
        let mut s = PasteArrayState::new(Unit::Mm, 10.0, 0.0, 1, PasteDesignators::Keep, 101, None);
        s.count = "1000".to_owned();
        assert!(s.input().unwrap_err().contains("100000 items"));
        assert_eq!(s.invalid_field(), Some(PasteArrayField::Copies));
        s.count = "990".to_owned();
        assert!(s.input().is_ok(), "99 990 items is within the cap");
        s.items_per_copy = 100;
        s.count = "1000".to_owned();
        assert!(s.input().is_ok(), "exactly the cap is allowed");
    }

    #[test]
    fn spacings_must_be_finite_and_keep_the_array_on_the_sheet() {
        for bad in ["", "abc", "inf", "NaN", "1e999", "1e12"] {
            let mut s = state();
            s.x_spacing = bad.to_owned();
            assert!(s.input().unwrap_err().contains("X spacing"), "{bad:?}");
            assert_eq!(s.invalid_field(), Some(PasteArrayField::XSpacing));
            let mut s = state();
            s.y_spacing = bad.to_owned();
            assert!(s.input().unwrap_err().contains("Y spacing"), "{bad:?}");
            assert_eq!(s.invalid_field(), Some(PasteArrayField::YSpacing));
        }
    }

    #[test]
    fn spacings_are_read_in_the_dialogs_unit() {
        let mut s = PasteArrayState::new(Unit::Mil, 2.54, 0.0, 1, PasteDesignators::Keep, 3, None);
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
            assert_eq!(s.invalid_field(), Some(PasteArrayField::Step));
        }
        let mut s = state();
        s.step = "0".to_owned();
        assert_eq!(s.input().unwrap().step, 0);
    }

    #[test]
    fn clearing_count_up_sets_step_zero_and_ticking_restores_a_step() {
        let mut s = state();
        s.step = "3".to_owned();
        assert!(s.counts_up());
        s.set_count_up(false, 2);
        assert_eq!(s.step, "0");
        assert!(!s.counts_up());
        assert_eq!(s.input().unwrap().step, 0, "off places with step 0");
        s.set_count_up(true, 2);
        assert_eq!(s.step, "2", "on takes the preference step");
        s.set_count_up(true, 5);
        assert_eq!(s.step, "2", "ticking an on box keeps the typed step");
        s.set_count_up(false, 2);
        s.set_count_up(true, 0);
        assert_eq!(s.step, "1", "a 0 preference turns on with 1");
    }

    #[test]
    fn the_placed_items_are_copies_times_items_per_copy() {
        let mut s = state();
        s.count = "4".to_owned();
        assert_eq!(s.placed_items(), Some(12));
        s.count = "1001".to_owned();
        assert_eq!(s.placed_items(), None);
    }
}
