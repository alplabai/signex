//! The Move Selection dialog's model side (#66): the ΔX field that takes the
//! focus when the dialog opens, and the offsets it moves by, in mm.

use super::super::super::*;
use iced::widget::operation;
use signex_types::coord::Unit;

/// Widget id of the ΔX field, focused when the dialog's window opens.
pub(crate) static MOVE_DX_INPUT_ID: std::sync::LazyLock<iced::widget::Id> =
    std::sync::LazyLock::new(|| iced::widget::Id::new("signex.move_selection.dx"));

/// Height of the line-art preview, in captions (the illustration's em).
pub(crate) const PREVIEW_EM: f32 = 10.0;

/// Height of the dialog's content: the fields, the preview, a two-line help
/// line and the footer, from the type scale.
pub(crate) fn window_height() -> f32 {
    use crate::styles::{LINE_HEIGHT, Spacing, TypeScale};
    let (ts, sp) = (TypeScale::DEFAULT, Spacing::DEFAULT);
    let caption_line = ts.caption * LINE_HEIGHT;
    let field = ts.label * LINE_HEIGHT + 2.0 * sp.s + 2.0;
    let fields = caption_line + sp.s + field;
    let preview = PREVIEW_EM * ts.caption;
    let help = 2.0 * caption_line;
    sp.l + fields + sp.m + preview + sp.m + help + crate::styles::dense_footer_height()
}

/// The offset `text` stands for in mm, typed in `unit`, for both the preview
/// and Apply. 0 while the text is not a number (empty, `-`: mid-edit).
/// `Err` (the reason) for `nan` / `inf` or a value that overflows, before or
/// after the unit conversion: an infinite offset would send the selection
/// to infinity.
pub(crate) fn offset_mm(text: &str, unit: Unit) -> Result<f64, String> {
    use signex_types::numeric::{FiniteParseError, parse_finite_f64};
    match parse_finite_f64(text) {
        Ok(value) => {
            let mm = value * signex_types::measure::mm_per(unit);
            if mm.is_finite() {
                Ok(mm)
            } else {
                Err(format!("{:?} is out of range", text.trim()))
            }
        }
        Err(FiniteParseError::NotANumber(_)) => Ok(0.0),
        Err(err @ FiniteParseError::NonFinite(_)) => Err(err.to_string()),
    }
}

/// Whole grid steps an offset of `offset_mm` spans on a `grid_mm` grid;
/// `None` for a part step, or when the grid is not positive.
pub(crate) fn grid_steps(offset_mm: f64, grid_mm: f64) -> Option<u64> {
    if !offset_mm.is_finite() || !grid_mm.is_finite() || grid_mm <= 0.0 {
        return None;
    }
    let steps = (offset_mm / grid_mm).abs();
    let whole = steps.round();
    ((steps - whole).abs() < 1e-6).then_some(whole as u64)
}

impl Signex {
    /// The ΔX field has the focus once the dialog's own window exists (the
    /// focus asked for at open time has no window yet).
    pub(crate) fn focus_move_selection_dx_once_open(
        &self,
        modal: crate::app::state::ModalId,
    ) -> Task<Message> {
        if modal != crate::app::state::ModalId::MoveSelection || !self.ui_state.move_selection.open
        {
            return Task::none();
        }
        Task::batch([
            operation::focus(MOVE_DX_INPUT_ID.clone()),
            operation::text_input::select_all(MOVE_DX_INPUT_ID.clone()),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_grid_steps_count_and_a_part_step_is_off_grid() {
        assert_eq!(grid_steps(5.08, 1.27), Some(4));
        assert_eq!(grid_steps(-2.54, 1.27), Some(2));
        assert_eq!(grid_steps(0.0, 1.27), Some(0));
        assert_eq!(grid_steps(1.0, 1.27), None);
        assert_eq!(grid_steps(1.0, 0.0), None);
        assert_eq!(grid_steps(f64::NAN, 1.27), None);
    }

    #[test]
    fn an_offset_reads_in_the_display_unit() {
        assert_eq!(offset_mm("2.54", Unit::Mm), Ok(2.54));
        assert!((offset_mm("100", Unit::Mil).unwrap() - 2.54).abs() < 1e-9);
        assert_eq!(offset_mm(" 2.5 ", Unit::Mm), Ok(2.5));
        assert_eq!(offset_mm("-1", Unit::Mm), Ok(-1.0));
        assert_eq!(offset_mm("", Unit::Mm), Ok(0.0));
        assert_eq!(offset_mm("-", Unit::Mm), Ok(0.0));
    }

    /// #828: `inf` / `nan` / an overflow must not become a move offset.
    #[test]
    fn an_offset_refuses_non_finite_text() {
        for bad in ["nan", "NaN", "inf", "-inf", "1e999", "-1e999"] {
            assert!(offset_mm(bad, Unit::Mm).is_err(), "{bad:?}");
        }
    }

    /// A finite number that overflows once converted to mm is refused too.
    #[test]
    fn an_offset_that_overflows_in_the_unit_is_refused() {
        assert!(offset_mm("1e307", Unit::Mm).is_ok());
        assert!(offset_mm("1e307", Unit::Inch).is_err());
    }
}
