//! The Grid Properties draft's own edits (mockup row 142): the multipliers
//! it offers, its step, and the preview's labels: which values they show
//! and the values set on them. The preview
//! (`view::dialogs::grid_properties::preview`) only lays them out.

use iced::Point;
use signex_i18n::I18n;
use signex_types::coord::Unit;
use signex_types::measure::length_field_text;
use signex_widgets::LengthFormat;
use signex_widgets::dimension_label::{DimensionLabel, DimensionLabelMessage};

use super::state::GridPropertiesState;
use crate::library::editor::footprint::GridDisplay;

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

    /// The values the preview labels show, in order, in mm: the step, then
    /// the coarse spacing unless the coarse grid is hidden. None while the
    /// step is not a number above zero.
    pub(crate) fn preview_label_values(&self) -> Vec<(PreviewLabel, f64)> {
        let Some(step) = self.draft_step() else {
            return Vec::new();
        };
        let mut out = vec![(PreviewLabel::Step, step)];
        if self.coarse_display != GridDisplay::Hidden {
            let coarse = step * f64::from(self.multiplier.max(1));
            out.push((PreviewLabel::Coarse, coarse));
        }
        out
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

    /// The preview labels as the label update reads them: each label's
    /// value and unit, not where it sits.
    fn update_labels<'a>(&self, i18n: &'a I18n) -> (Vec<PreviewLabel>, Vec<DimensionLabel<'a>>) {
        let format = LengthFormat::new(i18n, Unit::Mm);
        self.preview_label_values()
            .into_iter()
            .map(|(kind, value)| (kind, DimensionLabel::new(value, format, Point::ORIGIN)))
            .unzip()
    }

    /// Whether the open preview label holds text that is not a length.
    /// OK, Enter and Tab are refused then: the label stays open, outlined
    /// red, until its text is a length or Esc closes it.
    pub(crate) fn open_label_refused(&self, i18n: &I18n) -> bool {
        let (_, labels) = self.update_labels(i18n);
        self.preview_labels.draft_refused(&labels)
    }

    /// Routes a preview label event: an Enter commits the typed value into
    /// the draft, and is refused while the text is not a length. Returns
    /// whether a label field opened, so the caller focuses it.
    pub(crate) fn label_edited(&mut self, event: DimensionLabelMessage, i18n: &I18n) -> bool {
        let (kinds, labels) = self.update_labels(i18n);
        let commits = matches!(
            event,
            DimensionLabelMessage::DraftSubmitted | DimensionLabelMessage::TabPressed { .. }
        );
        if commits && self.preview_labels.draft_refused(&labels) {
            return false;
        }
        let outcome = self.preview_labels.update(event, &labels);
        if let Some(done) = outcome.committed
            && let Some(&kind) = kinds.get(done.index)
        {
            self.apply_label(kind, done.value);
        }
        outcome.focus_input
    }
}
