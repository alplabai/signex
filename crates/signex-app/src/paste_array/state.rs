use std::path::PathBuf;

use signex_types::coord::Unit;
use signex_widgets::property_illustration::{IllustrationHover, IllustrationMsg};

use crate::paste_prefs::PasteDesignators;
use signex_types::measure::{length_field_text, parse_length_field_mm};
use signex_types::text_increment::{STEP_MAX, STEP_MIN};

/// The largest Copies value. Copies counts the original, so this places one
/// fewer new copy.
pub const MAX_COPIES: u32 = 1000;

/// The smallest Copies value: the original and one new copy.
const MIN_COPIES: u32 = 2;

/// Most items (new copies times the items in one copy) one Paste Array places.
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
    /// The Copies field: positions in the array, the original included.
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
    /// Copy 2 is held on the preview: where on it it was grabbed, in mm from
    /// its corner.
    pub preview_drag: Option<(f64, f64)>,
    /// The value being typed on the preview, and its text from before.
    pub preview_edit: Option<(PasteArrayField, String)>,
}

/// A dialog whose fields all parse.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PasteArrayInput {
    /// New copies to place: the Copies field less the original.
    pub new_copies: u32,
    pub dx_mm: f64,
    pub dy_mm: f64,
    pub step: i32,
    pub designators: PasteDesignators,
}

impl PasteArrayInput {
    /// Positions the preview draws, the original included.
    pub fn positions(&self) -> u32 {
        self.new_copies + 1
    }
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
    /// A fresh dialog: Copies 2 (the original and one new copy), `dx_mm` /
    /// `dy_mm` apart, counting by `step`, of a copy holding `items_per_copy`
    /// items.
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
            preview_drag: None,
            preview_edit: None,
        }
    }

    /// The text of `field`: the preview edits the same text as the field.
    pub fn field_text_mut(&mut self, field: PasteArrayField) -> &mut String {
        match field {
            PasteArrayField::Copies => &mut self.count,
            PasteArrayField::XSpacing => &mut self.x_spacing,
            PasteArrayField::YSpacing => &mut self.y_spacing,
            PasteArrayField::Step => &mut self.step,
        }
    }

    /// Copy 2 was pressed at `at`, in mm from the original's corner. A
    /// dialog whose spacing does not parse has no copy 2 to hold.
    pub fn grab_copy_two(&mut self, at: (f64, f64)) {
        self.preview_drag = self
            .input()
            .ok()
            .map(|input| (at.0 - input.dx_mm, at.1 - input.dy_mm));
    }

    /// The held copy 2 moved to `at`: the spacing follows, snapped to the
    /// drag step, in the dialog's unit.
    pub fn drag_copy_two(&mut self, at: (f64, f64)) {
        let Some(grab) = self.preview_drag else {
            return;
        };
        let (dx, dy) = super::preview::dragged_spacing(grab, at);
        self.x_spacing = length_field_text(dx, self.unit);
        self.y_spacing = length_field_text(dy, self.unit);
    }

    /// A click on a drawn value: it is typed on the drawing. A click on the
    /// value already being typed keeps its text from before; another value's
    /// open edit keeps what was typed.
    pub fn start_preview_edit(&mut self, field: PasteArrayField) {
        if self
            .preview_edit
            .as_ref()
            .is_some_and(|(open, _)| *open == field)
        {
            return;
        }
        let before = self.field_text_mut(field).clone();
        self.preview_edit = Some((field, before));
    }

    /// Enter on the drawing: the typed value stays.
    pub fn finish_preview_edit(&mut self) {
        self.preview_edit = None;
    }

    /// Esc on the drawing: the value from before the edit comes back.
    /// Returns whether an edit was open.
    pub fn cancel_preview_edit(&mut self) -> bool {
        let Some((field, before)) = self.preview_edit.take() else {
            return false;
        };
        *self.field_text_mut(field) = before;
        true
    }

    /// The new copies the typed Copies places, when it is a whole number from
    /// 2 to [`MAX_COPIES`]. Copies counts the original, so 1 places nothing.
    pub fn new_copies(&self) -> Option<u32> {
        self.count
            .trim()
            .parse::<u32>()
            .ok()
            .filter(|n| (MIN_COPIES..=MAX_COPIES).contains(n))
            .map(|positions| positions - 1)
    }

    /// How many items the paste places: new copies times the items in one
    /// copy.
    pub fn placed_items(&self) -> Option<usize> {
        self.new_copies()
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
        let new_copies = self.new_copies().ok_or_else(|| {
            (
                PasteArrayField::Copies,
                format!(
                    "Copies must be a whole number from {MIN_COPIES} to {MAX_COPIES}; it counts the original, so 1 places nothing."
                ),
            )
        })?;
        if (new_copies as usize).saturating_mul(self.items_per_copy) > MAX_PLACED_ITEMS {
            return Err((
                PasteArrayField::Copies,
                format!(
                    "{new_copies} new copies of {} items is over the limit of {MAX_PLACED_ITEMS} items; use fewer copies.",
                    self.items_per_copy
                ),
            ));
        }
        let spacing = |text: &str, axis: &str, field: PasteArrayField| {
            parse_length_field_mm(text, self.unit)
                .filter(|mm| (mm * f64::from(new_copies)).abs() <= MAX_EXTENT_MM)
                .ok_or_else(|| {
                    (
                        field,
                        format!(
                            "{axis} spacing must be a number in {} (at most {} for the whole array).",
                            self.unit,
                            length_field_text(MAX_EXTENT_MM / f64::from(new_copies), self.unit),
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
            new_copies,
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
    /// Copy 2 was pressed on the preview, at this point in mm from the
    /// original's corner.
    PreviewCopyGrabbed((f64, f64)),
    /// The held copy 2 was dragged to this point, in mm from the original's
    /// corner.
    PreviewCopyDragged((f64, f64)),
    /// The held copy 2 was let go.
    PreviewCopyDropped,
    /// A value drawn on the preview was clicked, to be typed there.
    PreviewValueClicked(PasteArrayField),
    /// Enter in the value typed on the preview.
    PreviewEditSubmitted,
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
                new_copies: 1,
                dx_mm: 10.0,
                dy_mm: 0.0,
                step: 1,
                designators: PasteDesignators::Keep,
            }
        );
    }

    #[test]
    fn the_count_is_a_whole_number_in_range() {
        for bad in ["0", "1", "1001", "-1", "2.5", "", "x"] {
            let mut s = state();
            s.count = bad.to_owned();
            assert!(s.input().unwrap_err().contains("Copies"), "{bad:?}");
            assert_eq!(s.invalid_field(), Some(PasteArrayField::Copies), "{bad:?}");
        }
        for good in ["2", "1000", " 7 "] {
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
        s.count = "991".to_owned();
        assert!(s.input().is_ok(), "990 new copies, 99 990 items");
        s.count = "992".to_owned();
        assert!(s.input().is_err(), "991 new copies, 100 091 items");
        s.items_per_copy = 200;
        s.count = "501".to_owned();
        assert!(s.input().is_ok(), "exactly the cap is allowed");
        s.count = "502".to_owned();
        assert!(s.input().is_err(), "one new copy over the cap");
    }

    /// #2847: the furthest copy sits Copies - 1 spacings from the original.
    #[test]
    fn the_extent_limit_counts_the_new_copies() {
        let mut s = state();
        s.count = "3".to_owned();
        s.x_spacing = "500000".to_owned();
        assert!(s.input().is_ok(), "2 spacings reach exactly the limit");
        s.x_spacing = "500001".to_owned();
        assert_eq!(s.invalid_field(), Some(PasteArrayField::XSpacing));
    }

    /// #2847: Copies = 4 is four positions and three new copies.
    #[test]
    fn copies_counts_the_original() {
        let mut s = state();
        s.count = "4".to_owned();
        let input = s.input().unwrap();
        assert_eq!((input.new_copies, input.positions()), (3, 4));
        s.count = "1".to_owned();
        assert_eq!(s.new_copies(), None, "1 places nothing");
        assert!(s.input().unwrap_err().contains("1 places nothing"));
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

    /// Mockup row 69: dragging copy 2 writes the spacing fields, snapped to
    /// 1.27 mm and in the dialog's unit; the drop ends the drag.
    #[test]
    fn dragging_copy_two_sets_the_spacing_fields() {
        let mut s = state();
        // Grabbed 1 mm into copy 2 (at 10 mm), moved 5 mm right, 2.5 mm down.
        s.grab_copy_two((11.0, 0.5));
        assert_eq!(s.preview_drag, Some((1.0, 0.5)));
        s.drag_copy_two((16.0, 3.0));
        let input = s.input().unwrap();
        assert!((input.dx_mm - 15.24).abs() < 1e-9, "{}", input.dx_mm);
        assert!((input.dy_mm - 2.54).abs() < 1e-9, "{}", input.dy_mm);
        s.preview_drag = None;
        s.drag_copy_two((40.0, 40.0));
        assert!(
            (s.input().unwrap().dx_mm - 15.24).abs() < 1e-9,
            "no drag held"
        );

        let mut mil =
            PasteArrayState::new(Unit::Mil, 2.54, 0.0, 1, PasteDesignators::Keep, 3, None);
        mil.grab_copy_two((2.54, 0.0));
        mil.drag_copy_two((5.08, 0.0));
        assert_eq!(mil.x_spacing, length_field_text(5.08, Unit::Mil));

        let mut bad = state();
        bad.x_spacing = "x".to_owned();
        bad.grab_copy_two((1.0, 1.0));
        assert_eq!(bad.preview_drag, None, "no copy 2 to hold");
    }

    /// Mockup row 69: a value typed on the drawing is the field's own text;
    /// Enter keeps it, Esc puts the old one back.
    #[test]
    fn a_value_typed_on_the_drawing_is_the_fields_text() {
        let mut s = state();
        s.start_preview_edit(PasteArrayField::Copies);
        *s.field_text_mut(PasteArrayField::Copies) = "6".to_owned();
        assert_eq!(s.count, "6", "the field follows");
        s.start_preview_edit(PasteArrayField::Copies);
        assert!(s.cancel_preview_edit());
        assert_eq!(s.count, "2", "Esc restores the text from before");
        assert!(!s.cancel_preview_edit(), "nothing open");

        s.start_preview_edit(PasteArrayField::Step);
        s.step = "3".to_owned();
        s.finish_preview_edit();
        assert_eq!((s.step.as_str(), s.preview_edit.is_none()), ("3", true));
        assert!(!s.cancel_preview_edit(), "Enter closed it");
    }

    #[test]
    fn the_placed_items_are_new_copies_times_items_per_copy() {
        let mut s = state();
        s.count = "4".to_owned();
        assert_eq!(s.placed_items(), Some(9));
        s.count = "1001".to_owned();
        assert_eq!(s.placed_items(), None);
        s.count = "1".to_owned();
        assert_eq!(s.placed_items(), None);
    }
}
