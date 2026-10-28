//! The unit picker of a typed measurement editor.
//!
//! Users never type a unit: it comes from the class template where one
//! is set, otherwise from this list ([`Unit::STANDARD`], labelled with the
//! unit glyph: `Ω`, `µ`-free `F`, `ppm/°C`). A unit string the build does
//! not know ([`Unit::Other`]) is not in the list; the editors show it as a
//! read-only label and keep it as written.

use iced::Element;
use iced::widget::pick_list;
use signex_types::quantity::Unit;

/// One entry: a standard unit, or `None` for "no unit" (a plain value).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct UnitChoice(pub Option<Unit>);

impl std::fmt::Display for UnitChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            Some(unit) => f.write_str(unit.glyph()),
            None => f.write_str("\u{2014}"),
        }
    }
}

/// The picker's entries: every standard unit, preceded by "no unit" when
/// `allow_none`.
pub(crate) fn unit_choices(allow_none: bool) -> Vec<UnitChoice> {
    let none = allow_none.then_some(UnitChoice(None));
    none.into_iter()
        .chain(Unit::STANDARD.iter().cloned().map(|u| UnitChoice(Some(u))))
        .collect()
}

/// Read a stored unit token for an editor. `Ok(None)` is no unit,
/// `Ok(Some(_))` a standard unit the picker can show, `Err(raw)` an
/// unknown unit string to show read-only and keep untouched.
pub(crate) fn unit_of_token(token: &str) -> Result<Option<Unit>, &str> {
    let token = token.trim();
    if token.is_empty() {
        return Ok(None);
    }
    Unit::from_alias(token).map(Some).ok_or(token)
}

/// The picker, with `selected` shown. `on_pick` gets the unit's canonical
/// token (`"ohm"`), or an empty string for "no unit".
pub(crate) fn unit_picker<'a, M: Clone + 'a>(
    selected: Option<Unit>,
    allow_none: bool,
    text_size: f32,
    on_pick: impl Fn(String) -> M + 'a,
) -> Element<'a, M> {
    pick_list(
        unit_choices(allow_none),
        Some(UnitChoice(selected)),
        move |c: UnitChoice| on_pick(c.0.map(|u| u.token().to_string()).unwrap_or_default()),
    )
    .text_size(text_size)
    .padding([3, 6])
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_picker_offers_every_standard_unit_with_its_glyph() {
        let labels: Vec<String> = unit_choices(true).iter().map(|c| c.to_string()).collect();
        assert_eq!(labels[0], "\u{2014}");
        assert!(labels.contains(&"Ω".to_string()));
        assert!(labels.contains(&"ppm/°C".to_string()));
        assert_eq!(labels.len(), Unit::STANDARD.len() + 1);
        assert_eq!(unit_choices(false).len(), Unit::STANDARD.len());
    }

    #[test]
    fn an_unknown_token_is_not_a_picker_unit() {
        assert_eq!(unit_of_token(""), Ok(None));
        assert_eq!(unit_of_token("kΩ"), Err("kΩ"));
        assert_eq!(unit_of_token("furlong"), Err("furlong"));
        assert_eq!(unit_of_token("ohm"), Ok(Some(Unit::Ohm)));
    }
}
