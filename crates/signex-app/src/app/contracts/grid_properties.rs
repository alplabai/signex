//! The Grid Properties draft's own edits (mockup row 142): the multipliers
//! it offers, its step, and the values set on the preview's labels. The
//! preview (`view::dialogs::grid_properties::preview`) only draws them.

use signex_i18n::I18n;
use signex_types::coord::Unit;
use signex_types::measure::length_field_text;
use signex_widgets::dimension_label::DimensionLabelMessage;

use super::state::GridPropertiesState;

/// The multipliers the dialog offers, smallest first.
pub(crate) const MULTIPLIERS: [u32; 4] = [1, 2, 5, 10];

/// Which value a preview label edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PreviewLabel {
    Step,
    Coarse,
}

/// The multiplier closest to `ratio` (coarse over step); a tie keeps the
/// smaller one.
pub(crate) fn nearest_multiplier(ratio: f64) -> u32 {
    let mut best = MULTIPLIERS[0];
    for m in MULTIPLIERS {
        if (f64::from(m) - ratio).abs() < (f64::from(best) - ratio).abs() {
            best = m;
        }
    }
    best
}

impl GridPropertiesState {
    /// The draft's step, when it is a number above zero.
    pub(crate) fn draft_step(&self) -> Option<f64> {
        crate::numeric_field::validated_grid_step(&self.step_mm).ok()
    }

    /// Writes a value the user set on a preview label into the draft: the
    /// step text, or the multiplier closest to the coarse spacing typed.
    /// The step is validated on OK, as one typed in the Step field.
    pub(crate) fn apply_label(&mut self, label: PreviewLabel, value_mm: f64) {
        match label {
            PreviewLabel::Step => self.step_mm = length_field_text(value_mm, Unit::Mm),
            PreviewLabel::Coarse => {
                if let Some(step) = self.draft_step() {
                    self.multiplier = nearest_multiplier(value_mm / step);
                }
            }
        }
    }

    /// Routes a preview label event: an Enter commits the typed value into
    /// the draft. Returns whether a label field opened, so the caller
    /// focuses it.
    pub(crate) fn label_edited(&mut self, event: DimensionLabelMessage, i18n: &I18n) -> bool {
        use crate::app::view::dialogs::grid_properties::preview::{PreviewSize, labels};
        // Positions do not matter to the update; the values and order do.
        let size = PreviewSize::new(0.0, self.preview_expanded, 1.0);
        let (kinds, labels): (Vec<_>, Vec<_>) = labels(self, size, i18n).into_iter().unzip();
        let outcome = self.preview_labels.update(event, &labels);
        if let Some(done) = outcome.committed
            && let Some(&kind) = kinds.get(done.index)
        {
            self.apply_label(kind, done.value);
        }
        outcome.focus_input
    }
}
