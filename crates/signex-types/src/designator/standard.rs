//! Designator standards: which reference letters a component class gets.
//!
//! A project may pick a standard in its `.snxprj` (`annotation.designator_standard`).
//! Placement then derives the reference prefix from the component's class
//! instead of the library symbol's own designator letter. `None` means the
//! legacy behaviour: the symbol's designator decides.

use serde::{Deserialize, Serialize};

use super::prefix::placement_prefix;

/// A reference-designation standard a project can follow.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DesignatorStandard {
    /// IEEE 315 / ANSI Y32.2 letters (`R`, `C`, `D`, `DS`, `Q`, `U`, ...).
    #[serde(rename = "ieee315")]
    Ieee315,
    /// DIN 40719-2 letters (`V` for semiconductors, `N` for analogue ICs, ...).
    #[serde(rename = "din40719_2")]
    Din40719_2,
    /// EN IEC 81346-2 purpose letters (`K`, `P`, `R`, `T`, `X`, ...).
    #[serde(rename = "iec81346_2")]
    Iec81346_2,
}

impl DesignatorStandard {
    /// Every standard, in the order a picker lists them.
    pub const ALL: [Self; 3] = [Self::Ieee315, Self::Din40719_2, Self::Iec81346_2];

    /// Human-readable name for the Project Options picker.
    pub fn label(self) -> &'static str {
        match self {
            Self::Ieee315 => "IEEE 315",
            Self::Din40719_2 => "DIN 40719-2",
            Self::Iec81346_2 => "EN IEC 81346-2",
        }
    }
}

impl std::fmt::Display for DesignatorStandard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// `(ieee315, din40719_2, iec81346_2)` letters for one class key; `None` where
/// that standard names no letter for the class. Each entry is checked against
/// the standard text: sources and page numbers are in
/// `docs/architecture/designator-standards.md`.
type ClassLetters = (
    Option<&'static str>,
    Option<&'static str>,
    Option<&'static str>,
);

fn class_letters(class_key: &str) -> Option<ClassLetters> {
    let all = |ieee, din, iec| Some((Some(ieee), Some(din), Some(iec)));
    match class_key {
        "resistor" => all("R", "R", "R"),
        "capacitor" => all("C", "C", "C"),
        "inductor" => all("L", "L", "C"),
        "diode" => all("D", "V", "R"),
        "led" => all("DS", "H", "P"),
        "transistor_bjt" | "transistor_mosfet" | "transistor_jfet" => all("Q", "V", "Q"),
        "opamp" => all("U", "N", "T"),
        "comparator" => Some((Some("U"), Some("N"), None)),
        "regulator_linear" => Some((Some("U"), Some("N"), None)),
        "regulator_switching" => all("U", "U", "T"),
        "mcu" | "logic" => all("U", "D", "K"),
        "memory" => all("U", "D", "C"),
        "connector" => all("J", "X", "X"),
        "switch" => all("S", "S", "S"),
        "relay" => all("K", "K", "K"),
        "crystal" => Some((Some("Y"), Some("G"), None)),
        "transformer" => all("T", "T", "T"),
        "fuse" => all("F", "F", "F"),
        "antenna" => all("E", "W", "T"),
        "display" => all("DS", "P", "P"),
        "sensor" => all("U", "B", "B"),
        "motor" => all("B", "M", "M"),
        "battery" => all("BT", "G", "G"),
        _ => None,
    }
}

/// Reference letters `standard` assigns to a component class, or `None` for
/// `"generic"`, any key the table does not know, and a class the standard
/// names no letter for.
pub fn class_prefix(standard: DesignatorStandard, class_key: &str) -> Option<&'static str> {
    let (ieee, din, iec) = class_letters(class_key)?;
    match standard {
        DesignatorStandard::Ieee315 => ieee,
        DesignatorStandard::Din40719_2 => din,
        DesignatorStandard::Iec81346_2 => iec,
    }
}

/// The reference prefix a freshly placed part gets.
///
/// A symbol designator starting with `#` (power ports, flags, virtual parts)
/// always wins: those are net anchors, not designated components, under every
/// standard. Otherwise the project's standard decides by class; with no
/// standard, or a class it does not cover, the symbol's own designator letters
/// apply (the legacy behaviour).
pub fn resolve_prefix(
    standard: Option<DesignatorStandard>,
    class_key: Option<&str>,
    symbol_designator: &str,
) -> String {
    if symbol_designator.trim().starts_with('#') {
        return placement_prefix(symbol_designator);
    }
    standard
        .zip(class_key)
        .and_then(|(standard, class)| class_prefix(standard, class))
        .map_or_else(|| placement_prefix(symbol_designator), str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialized_names_are_pinned() {
        for (standard, text) in [
            (DesignatorStandard::Ieee315, "\"ieee315\""),
            (DesignatorStandard::Din40719_2, "\"din40719_2\""),
            (DesignatorStandard::Iec81346_2, "\"iec81346_2\""),
        ] {
            assert_eq!(serde_json::to_string(&standard).unwrap(), text);
            assert_eq!(
                serde_json::from_str::<DesignatorStandard>(text).unwrap(),
                standard
            );
        }
        assert!(serde_json::from_str::<DesignatorStandard>("\"iso9999\"").is_err());
    }

    #[test]
    fn labels_name_the_standards() {
        assert_eq!(DesignatorStandard::Ieee315.to_string(), "IEEE 315");
        assert_eq!(DesignatorStandard::Din40719_2.to_string(), "DIN 40719-2");
        assert_eq!(DesignatorStandard::Iec81346_2.to_string(), "EN IEC 81346-2");
    }

    #[test]
    fn the_table_gives_each_standard_its_letters() {
        use DesignatorStandard::*;
        assert_eq!(class_prefix(Ieee315, "led"), Some("DS"));
        assert_eq!(class_prefix(Din40719_2, "diode"), Some("V"));
        assert_eq!(class_prefix(Iec81346_2, "regulator_switching"), Some("T"));
        assert_eq!(class_prefix(Iec81346_2, "transistor_mosfet"), Some("Q"));
        assert_eq!(class_prefix(Iec81346_2, "opamp"), Some("T"));
        assert_eq!(class_prefix(Iec81346_2, "antenna"), Some("T"));
        assert_eq!(class_prefix(Iec81346_2, "inductor"), Some("C"));
        assert_eq!(class_prefix(Din40719_2, "display"), Some("P"));
        assert_eq!(class_prefix(Din40719_2, "regulator_switching"), Some("U"));
        assert_eq!(class_prefix(Ieee315, "battery"), Some("BT"));
        assert_eq!(class_prefix(Din40719_2, "generic"), None);
        assert_eq!(class_prefix(Ieee315, "no_such_class"), None);
    }

    #[test]
    fn no_standard_matches_the_legacy_placement_prefix() {
        for designator in ["U?", "#PWR?", "r1", "?", "#?"] {
            for class in [None, Some("diode"), Some("generic")] {
                assert_eq!(
                    resolve_prefix(None, class, designator),
                    placement_prefix(designator)
                );
            }
        }
    }

    #[test]
    fn power_and_flag_designators_ignore_every_standard_and_class() {
        for standard in DesignatorStandard::ALL {
            for class in [None, Some("diode"), Some("connector"), Some("generic")] {
                assert_eq!(resolve_prefix(Some(standard), class, "#PWR?"), "#PWR");
                assert_eq!(resolve_prefix(Some(standard), class, " #FLG?"), "#FLG");
            }
        }
    }

    #[test]
    fn a_standard_and_known_class_override_the_symbol_letter() {
        let din = Some(DesignatorStandard::Din40719_2);
        assert_eq!(resolve_prefix(din, Some("diode"), "D?"), "V");
    }

    #[test]
    fn generic_and_unknown_classes_fall_back_to_the_symbol_designator() {
        let std = Some(DesignatorStandard::Iec81346_2);
        assert_eq!(resolve_prefix(std, Some("generic"), "x?"), "X");
        assert_eq!(resolve_prefix(std, Some("zzz"), "Q?"), "Q");
        assert_eq!(resolve_prefix(std, None, "Q?"), "Q");
    }

    #[test]
    fn a_class_the_standard_does_not_name_keeps_the_symbol_letter() {
        use DesignatorStandard::*;
        for class in ["comparator", "regulator_linear", "crystal"] {
            assert_eq!(class_prefix(Iec81346_2, class), None, "{class}");
            assert_eq!(resolve_prefix(Some(Iec81346_2), Some(class), "U?"), "U");
            assert!(class_prefix(Ieee315, class).is_some(), "{class}");
            assert!(class_prefix(Din40719_2, class).is_some(), "{class}");
        }
    }
}
