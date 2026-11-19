//! How a widget shows and reads a length: the caller's locale and display
//! unit. Lengths stay millimetres in the caller's model; the unit only
//! decides the number shown and the number typed.

use signex_i18n::{I18n, LocalNumber, UserText};
use signex_types::coord::Unit;
use signex_types::measure::{length_field_text, mm_per, parse_length_field_mm};

use crate::Label;

/// Decimals a shown length is rounded to, in the display unit.
const SHOWN_DECIMALS: usize = 4;
/// Most decimals a length is rounded to.
const MAX_DECIMALS: usize = 15;

/// The caller's locale and display unit, passed to every view and update
/// that shows or parses a length.
#[derive(Debug, Clone, Copy)]
pub struct LengthFormat<'a> {
    pub i18n: &'a I18n,
    pub unit: Unit,
}

impl<'a> LengthFormat<'a> {
    pub fn new(i18n: &'a I18n, unit: Unit) -> Self {
        Self { i18n, unit }
    }

    /// `mm` in the display unit as the locale writes it, rounded to
    /// [`SHOWN_DECIMALS`] decimals, without the unit symbol.
    pub fn number(&self, mm: f64) -> String {
        self.number_rounded(mm, SHOWN_DECIMALS)
    }

    /// `mm` in the display unit as the locale writes it, rounded to
    /// `decimals` decimals (trailing zeros dropped), without the unit symbol.
    pub fn number_rounded(&self, mm: f64, decimals: usize) -> String {
        // An f64 holds about 15 significant digits; more decimals add none.
        let scale = 10f64.powi(decimals.min(MAX_DECIMALS) as i32);
        let value = (mm / mm_per(self.unit) * scale).round() / scale;
        // `-0` reads as a sign on nothing.
        let value = if value == 0.0 { 0.0 } else { value };
        self.i18n.number(LocalNumber(value))
    }

    /// [`Self::number`] as a widget label.
    pub fn label(&self, mm: f64) -> Label<'static> {
        UserText::new(self.number(mm)).into()
    }

    /// The text an edit field opens with for `mm`.
    pub fn field_text(&self, mm: f64) -> String {
        length_field_text(mm, self.unit)
    }

    /// The millimetres a typed length stands for; `None` when the text is
    /// not a finite number.
    pub fn parse(&self, text: &str) -> Option<f64> {
        parse_length_field_mm(text, self.unit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_length_shows_and_reads_in_the_display_unit() {
        let mm = LengthFormat::new(I18n::en_us_ref(), Unit::Mm);
        assert_eq!(mm.number(0.5), "0.5");
        assert_eq!(mm.number(0.123456), "0.1235");
        assert_eq!(mm.number(-0.00001), "0");
        assert_eq!(mm.parse(" 0.25 "), Some(0.25));
        assert_eq!(mm.parse("abc"), None);
        assert_eq!(mm.parse("inf"), None);

        let mil = LengthFormat::new(I18n::en_us_ref(), Unit::Mil);
        assert_eq!(mil.number(0.254), "10");
        assert_eq!(mil.field_text(0.254), "10.00");
        let typed = mil.parse("10").expect("a number");
        assert!((typed - 0.254).abs() < 1e-12);
    }
}
