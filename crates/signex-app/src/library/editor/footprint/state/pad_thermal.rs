//! A footprint pad's own thermal relief rows (#2593, mockup row 125): Air
//! gap, Spoke width and Spokes, each Rule (empty text, the pour decides) or
//! Manual. Which value a typed text sets, and whether a pour can use it.

use signex_types::pad_thermal::{PadThermal, is_spoke_count, is_thermal_length};

/// One of the three thermal relief values of a pad.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PadThermalField {
    AirGap,
    SpokeWidth,
    Spokes,
}

impl PadThermalField {
    /// The `pad_field` name its refused text is kept under; also its undo
    /// key, so keystrokes into one field share one undo step.
    pub fn field(self) -> &'static str {
        match self {
            PadThermalField::AirGap => "thermal_air_gap",
            PadThermalField::SpokeWidth => "thermal_spoke_width",
            PadThermalField::Spokes => "thermal_spokes",
        }
    }

    /// The values with this one read from `text`: empty is Rule, a length
    /// above 0 mm (a count from 1 to 4 for Spokes) is Manual. `None` for text
    /// no pour can use: the edit is refused and the stored value kept.
    pub fn typed(self, mut thermal: PadThermal, text: &str) -> Option<PadThermal> {
        let text = text.trim();
        match self {
            PadThermalField::AirGap => thermal.air_gap = length(text)?,
            PadThermalField::SpokeWidth => thermal.spoke_width = length(text)?,
            PadThermalField::Spokes => thermal.spokes = count(text)?,
        }
        Some(thermal)
    }

    /// This value of `from` set on `to`; the other two are left.
    pub fn copy(self, from: &PadThermal, to: &mut PadThermal) {
        match self {
            PadThermalField::AirGap => to.air_gap = from.air_gap,
            PadThermalField::SpokeWidth => to.spoke_width = from.spoke_width,
            PadThermalField::Spokes => to.spokes = from.spokes,
        }
    }
}

fn length(text: &str) -> Option<Option<f64>> {
    if text.is_empty() {
        return Some(None);
    }
    let mm = crate::numeric_field::parse_field_f64(text)?;
    is_thermal_length(mm).then_some(Some(mm))
}

fn count(text: &str) -> Option<Option<u8>> {
    if text.is_empty() {
        return Some(None);
    }
    let n = text.parse::<u8>().ok()?;
    is_spoke_count(n).then_some(Some(n))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_text_is_rule_and_a_usable_value_is_manual() {
        let set = PadThermalField::AirGap
            .typed(PadThermal::RULE, "0.3")
            .unwrap();
        assert_eq!(set.air_gap, Some(0.3));
        let back = PadThermalField::AirGap.typed(set, "  ").unwrap();
        assert_eq!(back.air_gap, None);
        let spokes = PadThermalField::Spokes
            .typed(PadThermal::RULE, "2")
            .unwrap();
        assert_eq!(spokes.spokes, Some(2));
    }

    #[test]
    fn text_no_pour_can_use_is_refused() {
        for (field, text) in [
            (PadThermalField::AirGap, "0"),
            (PadThermalField::SpokeWidth, "-0.1"),
            (PadThermalField::SpokeWidth, "wide"),
            (PadThermalField::Spokes, "0"),
            (PadThermalField::Spokes, "5"),
            (PadThermalField::Spokes, "2.5"),
        ] {
            assert_eq!(
                field.typed(PadThermal::RULE, text),
                None,
                "{field:?} {text}"
            );
        }
    }

    #[test]
    fn copy_sets_only_its_own_value() {
        let from = PadThermal {
            air_gap: Some(0.4),
            spoke_width: Some(0.2),
            spokes: Some(3),
        };
        let mut to = PadThermal::RULE;
        PadThermalField::SpokeWidth.copy(&from, &mut to);
        assert_eq!(
            to,
            PadThermal {
                spoke_width: Some(0.2),
                ..PadThermal::RULE
            }
        );
    }
}
