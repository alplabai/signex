//! Fast grid 1 / Fast grid 2 (#1672): the saved grids and their
//! Preferences drafts.

use super::UiState;
use crate::canvas::grid::{FastGridRefusal, fast_grid_field_text, parse_fast_grid_field};
use signex_i18n::{Arg, I18n, ids};

impl UiState {
    /// Why a fast-grid field's `text` would be refused on Save, in the UI
    /// language; `None` when Save takes it. The Editing page shows it under
    /// the field while the user types (mockup row 147), and the Save refusal names it.
    pub(crate) fn fast_grid_problem(text: &str, i18n: &I18n) -> Option<String> {
        let refusal = parse_fast_grid_field(text).err()?;
        Some(match refusal {
            FastGridRefusal::Invalid(reason) => reason,
            FastGridRefusal::BelowMinimum => i18n.message_plain(
                ids::PREFERENCES_APPEARANCE_FAST_GRID_BELOW_MINIMUM,
                &[("value", Arg::from(text.trim()))],
            ),
        })
    }

    /// [`Self::fast_grid_problem`] of the Fast grid 1 (`0`) or Fast grid 2
    /// (`1`) field as it is drafted now.
    pub fn fast_grid_field_error(&self, index: usize) -> Option<String> {
        let text = self.preferences_draft_fast_grids.get(index)?;
        Self::fast_grid_problem(text, &self.i18n)
    }

    /// Show the saved fast grids in their Preferences fields.
    pub(crate) fn seed_fast_grid_drafts(&mut self) {
        self.preferences_draft_fast_grids = self.fast_grids_mm.map(fast_grid_field_text);
        self.preferences_fast_grid_status.clear();
    }

    /// Whether a fast-grid field holds something other than its saved grid.
    /// A refused text counts, so the dialog stays dirty until it is fixed.
    pub(crate) fn fast_grid_drafts_differ(&self) -> bool {
        self.preferences_draft_fast_grids
            .iter()
            .zip(self.fast_grids_mm)
            .any(|(text, saved)| parse_fast_grid_field(text) != Ok(saved))
    }

    /// Check the fast-grid fields before Save commits anything. A refused
    /// field is reported under the fields and in Messages, and the typed
    /// text stays for the user to fix. Returns whether Save must stop.
    pub(crate) fn refuse_invalid_fast_grid_drafts(&mut self) -> bool {
        let refusals: Vec<String> = (1_u32..)
            .zip(&self.preferences_draft_fast_grids)
            .filter_map(|(number, text)| {
                let reason = Self::fast_grid_problem(text, &self.i18n)?;
                Some(self.fast_grid_refusal_text(number, reason))
            })
            .collect();
        for line in &refusals {
            crate::diagnostics::log_warning(line.clone());
        }
        self.preferences_fast_grid_status = refusals.join("\n");
        !refusals.is_empty()
    }

    fn fast_grid_refusal_text(&self, number: u32, reason: String) -> String {
        self.i18n.message_plain(
            ids::PREFERENCES_APPEARANCE_FAST_GRID_REFUSED,
            &[("number", Arg::from(number)), ("reason", Arg::from(reason))],
        )
    }

    /// Commit the fast-grid fields on Save, after
    /// [`Self::refuse_invalid_fast_grid_drafts`] passed them. Each field then
    /// shows its grid the way a fresh open would.
    pub(crate) fn commit_fast_grid_drafts(&mut self) {
        let fields = self
            .preferences_draft_fast_grids
            .iter_mut()
            .zip(self.fast_grids_mm.iter_mut());
        for (text, grid) in fields {
            if let Ok(mm) = parse_fast_grid_field(text) {
                *grid = mm;
            }
            *text = fast_grid_field_text(*grid);
        }
        self.preferences_fast_grid_status.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::grid::{FAST_GRID_MIN_MM, FAST_GRIDS_MM};

    #[test]
    fn a_default_fast_grid_reads_back_as_its_field_text() {
        for mm in FAST_GRIDS_MM {
            assert_eq!(parse_fast_grid_field(&fast_grid_field_text(mm)), Ok(mm));
        }
        assert_eq!(fast_grid_field_text(1.27), "1.27");
    }

    #[test]
    fn a_fast_grid_field_text_reads_back_as_exactly_the_saved_grid() {
        for typed in ["1.2345678", "0.1", "0.3333333333", "0.000001", "12345.6789"] {
            let mm = parse_fast_grid_field(typed).expect(typed);
            assert_eq!(
                parse_fast_grid_field(&fast_grid_field_text(mm)),
                Ok(mm),
                "{typed}"
            );
        }
    }

    #[test]
    fn a_fast_grid_field_refuses_what_the_grid_step_field_refuses() {
        for text in ["0", "-1.27", "nan", "inf", "1e999", "1.27mm", ""] {
            assert!(
                matches!(
                    parse_fast_grid_field(text),
                    Err(FastGridRefusal::Invalid(_))
                ),
                "{text:?}"
            );
        }
    }

    #[test]
    fn a_fast_grid_problem_says_what_save_would_refuse() {
        let en = I18n::en_us();
        assert_eq!(UiState::fast_grid_problem("1.27", &en), None);
        assert_eq!(
            UiState::fast_grid_problem(" 0.0000001 ", &en).as_deref(),
            Some("0.0000001 mm is finer than the 1 nm (0.000001 mm) minimum")
        );
        let zero = UiState::fast_grid_problem("0", &en);
        assert!(
            zero.as_deref().is_some_and(|r| r.contains("above zero")),
            "{zero:?}"
        );
    }

    #[test]
    fn a_fast_grid_finer_than_a_nanometre_is_refused() {
        for text in ["0.0000009", "1e-7", "1e-40"] {
            assert_eq!(
                parse_fast_grid_field(text),
                Err(FastGridRefusal::BelowMinimum),
                "{text:?}"
            );
        }
        assert_eq!(parse_fast_grid_field("0.000001"), Ok(FAST_GRID_MIN_MM));
        assert_eq!(fast_grid_field_text(FAST_GRID_MIN_MM), "0.000001");
    }
}
