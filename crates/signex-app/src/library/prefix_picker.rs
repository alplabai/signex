//! The SI-prefix picker of a typed measurement editor.
//!
//! A measurement slot is a number-only field, this picker and the unit.
//! Users never type a prefix: the picker offers the unit's own prefixes
//! ([`SiPrefix::all_for`]) and is hidden for units that take none
//! (`%`, `ppm/C`, `V/us`, `AWG`).

use iced::Element;
use iced::widget::pick_list;
use signex_types::quantity::{SiPrefix, Unit};

/// One picker entry. Shown with the unit glyph (`kΩ`, `µF`) so the picker
/// also says which unit the number is in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PrefixChoice {
    pub prefix: SiPrefix,
    label: String,
}

impl std::fmt::Display for PrefixChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

fn choice(prefix: SiPrefix, unit: &Unit) -> PrefixChoice {
    PrefixChoice {
        prefix,
        label: format!("{}{}", prefix.symbol(), unit.glyph()),
    }
}

/// The prefixes the picker offers for `unit`: empty when it takes none.
pub(crate) fn prefix_choices(unit: &Unit) -> Vec<PrefixChoice> {
    if !unit.takes_prefix() {
        return Vec::new();
    }
    SiPrefix::all_for(unit)
        .iter()
        .map(|p| choice(*p, unit))
        .collect()
}

/// `prefix` if `unit` offers it, else no prefix. Keeps a stale pick from
/// outliving a change of unit.
pub(crate) fn effective_prefix(unit: &Unit, prefix: SiPrefix) -> SiPrefix {
    if unit.allowed_prefixes().contains(&prefix) {
        prefix
    } else {
        SiPrefix::One
    }
}

/// The picker, or `None` for a unit that takes no prefix.
pub(crate) fn prefix_picker<'a, M: Clone + 'a>(
    unit: &Unit,
    selected: SiPrefix,
    text_size: f32,
    on_pick: impl Fn(SiPrefix) -> M + 'a,
) -> Option<Element<'a, M>> {
    let choices = prefix_choices(unit);
    if choices.is_empty() {
        return None;
    }
    let current = choice(effective_prefix(unit, selected), unit);
    Some(
        pick_list(choices, Some(current), move |c: PrefixChoice| {
            on_pick(c.prefix)
        })
        .text_size(text_size)
        .padding([3, 6])
        .into(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_and_ppm_have_no_picker() {
        for unit in [Unit::Percent, Unit::PpmPerKelvin, Unit::Awg] {
            assert!(prefix_choices(&unit).is_empty(), "{unit:?}");
            assert!(prefix_picker(&unit, SiPrefix::One, 11.0, |p| p).is_none());
        }
    }

    #[test]
    fn an_ohm_picker_offers_the_ohm_prefixes_with_the_unit() {
        let labels: Vec<String> = prefix_choices(&Unit::Ohm)
            .iter()
            .map(|c| c.to_string())
            .collect();
        assert_eq!(labels, ["mΩ", "Ω", "kΩ", "MΩ", "GΩ"]);
        assert!(prefix_picker(&Unit::Ohm, SiPrefix::Kilo, 11.0, |p| p).is_some());
    }

    #[test]
    fn a_prefix_the_unit_lacks_falls_back_to_none() {
        assert_eq!(
            effective_prefix(&Unit::Farad, SiPrefix::Kilo),
            SiPrefix::One
        );
        assert_eq!(effective_prefix(&Unit::Ohm, SiPrefix::Kilo), SiPrefix::Kilo);
    }
}
