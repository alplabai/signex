//! Fast grid 1 / Fast grid 2 (#1672): the saved grids and their
//! Preferences drafts.

use super::UiState;
use crate::canvas::grid::{fast_grid_field_text, parse_fast_grid_field};

impl UiState {
    /// Show the saved fast grids in their Preferences fields.
    pub(crate) fn seed_fast_grid_drafts(&mut self) {
        self.preferences_draft_fast_grids = self.fast_grids_mm.map(fast_grid_field_text);
    }

    /// Whether a fast-grid field holds something other than its saved grid.
    /// A refused text counts, so Save runs and reports it.
    pub(crate) fn fast_grid_drafts_differ(&self) -> bool {
        self.preferences_draft_fast_grids
            .iter()
            .zip(self.fast_grids_mm)
            .any(|(text, saved)| parse_fast_grid_field(text) != Ok(saved))
    }

    /// Commit the fast-grid fields on Save. A field that is not a finite
    /// number above zero is refused with its reason in Messages; that grid
    /// keeps its previous value and its field shows it again.
    pub(crate) fn commit_fast_grid_drafts(&mut self) {
        let fields = self
            .preferences_draft_fast_grids
            .iter_mut()
            .zip(self.fast_grids_mm.iter_mut());
        for (number, (text, grid)) in (1..).zip(fields) {
            match parse_fast_grid_field(text) {
                Ok(mm) => *grid = mm,
                Err(reason) => crate::diagnostics::log_warning(format!(
                    "Fast grid {number} refused, previous value kept: {reason}"
                )),
            }
            *text = fast_grid_field_text(*grid);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_default_fast_grid_reads_back_as_its_field_text() {
        for mm in crate::canvas::grid::FAST_GRIDS_MM {
            assert_eq!(parse_fast_grid_field(&fast_grid_field_text(mm)), Ok(mm));
        }
        assert_eq!(fast_grid_field_text(1.27), "1.27");
    }

    #[test]
    fn a_fast_grid_field_refuses_what_the_grid_step_field_refuses() {
        for text in ["0", "-1.27", "nan", "inf", "1e999", "1.27mm", ""] {
            assert!(parse_fast_grid_field(text).is_err(), "{text:?}");
        }
    }
}
