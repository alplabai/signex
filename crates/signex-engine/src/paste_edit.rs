//! Edits a paste makes to the copy before it lands: new designators and
//! stepped label texts.

use signex_types::schematic::{Symbol, group_into_parts};
use signex_types::text_increment::increment_trailing_number;

use crate::PlacementPayload;

fn is_numbered_part(symbol: &Symbol) -> bool {
    !symbol.is_power && !symbol.reference.starts_with('#')
}

impl PlacementPayload {
    /// A copy whose parts carry new designators. `new_reference` is asked
    /// once per physical part, with the designator it has now; `None` keeps
    /// it. The units of a multi-unit part share one designator in the copy
    /// and so share the new one. Power and `#` symbols are left alone.
    pub fn redesignate_parts(&self, mut new_reference: impl FnMut(&str) -> Option<String>) -> Self {
        let mut fragment = self.fragment.clone();
        let eligible: Vec<usize> = (0..fragment.symbols.len())
            .filter(|&i| is_numbered_part(&fragment.symbols[i]))
            .collect();
        let parts = group_into_parts(eligible.iter().map(|&i| fragment.symbols[i].part_key()));
        let renamed: Vec<(Vec<usize>, String)> = parts
            .iter()
            .filter_map(|members| {
                let first = &fragment.symbols[eligible[members[0]]];
                let new = new_reference(first.reference.trim())?;
                Some((members.iter().map(|&m| eligible[m]).collect(), new))
            })
            .collect();
        for (members, reference) in renamed {
            for index in members {
                fragment.symbols[index].reference = reference.clone();
            }
        }
        Self::new(fragment)
    }

    /// The commands that register the library bodies the payload carries,
    /// for a caller that places the same payload many times.
    pub fn register_commands(&self) -> Vec<crate::Command> {
        self.fragment
            .lib_symbols
            .values()
            .cloned()
            .map(|lib_symbol| crate::Command::RegisterLibSymbol { lib_symbol })
            .collect()
    }

    /// A copy without its library bodies, cheap to clone per copy; register
    /// them once with [`Self::register_commands`].
    pub fn without_bodies(&self) -> Self {
        let mut fragment = self.fragment.clone();
        fragment.lib_symbols.clear();
        Self::new(fragment)
    }

    /// A copy whose label texts have their trailing number moved by `step`.
    pub fn with_labels_stepped(&self, step: i64) -> Self {
        let mut fragment = self.fragment.clone();
        for label in &mut fragment.labels {
            label.text = increment_trailing_number(&label.text, step);
        }
        Self::new(fragment)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::test_sheet;
    use signex_types::schematic::{Label, LabelType, Point};
    use uuid::Uuid;

    fn part(reference: &str, lib: &str, unit: u32) -> Symbol {
        Symbol {
            uuid: Uuid::new_v4(),
            lib_id: lib.to_owned(),
            reference: reference.to_owned(),
            unit,
            ..Default::default()
        }
    }

    fn payload(symbols: Vec<Symbol>) -> PlacementPayload {
        let mut sheet = test_sheet();
        sheet.symbols = symbols;
        PlacementPayload::new(sheet)
    }

    fn references(p: &PlacementPayload) -> Vec<String> {
        p.fragment()
            .symbols
            .iter()
            .map(|s| s.reference.clone())
            .collect()
    }

    #[test]
    fn each_part_is_renamed_once_and_units_share_the_new_name() {
        let p = payload(vec![
            part("U1", "lib:opamp", 1),
            part("R4", "lib:r", 1),
            part("U1", "lib:opamp", 2),
        ]);
        let mut asked = Vec::new();
        let renamed = p.redesignate_parts(|old| {
            asked.push(old.to_owned());
            Some(format!("{}?", &old[..1]))
        });
        assert_eq!(asked, ["U1", "R4"], "a two-unit part is asked once");
        assert_eq!(references(&renamed), ["U?", "R?", "U?"]);
        assert_eq!(
            references(&p),
            ["U1", "R4", "U1"],
            "the original is untouched"
        );
    }

    #[test]
    fn power_symbols_and_declined_parts_keep_their_designators() {
        let mut power = part("#PWR01", "power:gnd", 1);
        power.is_power = true;
        let p = payload(vec![power, part("C2", "lib:c", 1), part("R9", "lib:r", 1)]);
        let renamed = p.redesignate_parts(|old| (old != "C2").then(|| "X1".to_owned()));
        assert_eq!(references(&renamed), ["#PWR01", "C2", "X1"]);
    }

    #[test]
    fn bodies_are_registered_once_and_left_out_of_the_copies() {
        let mut sheet = test_sheet();
        for id in ["lib:a", "lib:b"] {
            let body: signex_types::schematic::LibSymbol =
                serde_json::from_str(&format!(r#"{{"id":"{id}"}}"#)).unwrap();
            sheet.lib_symbols.insert(body.id.clone(), body);
        }
        sheet.symbols.push(part("R1", "lib:a", 1));
        let p = PlacementPayload::new(sheet);
        assert_eq!(p.register_commands().len(), 2);
        let bare = p.without_bodies();
        assert!(bare.fragment().lib_symbols.is_empty());
        let placed = bare.place_commands(Point::new(1.0, 1.0)).unwrap();
        assert!(
            !placed
                .iter()
                .any(|c| matches!(c, crate::Command::RegisterLibSymbol { .. }))
        );
        assert_eq!(p.fragment().lib_symbols.len(), 2, "the original keeps them");
    }

    #[test]
    fn labels_step_by_the_given_amount() {
        let mut sheet = test_sheet();
        for text in ["D0", "CLK", "A07"] {
            sheet.labels.push(Label {
                uuid: Uuid::new_v4(),
                text: text.to_owned(),
                position: Point::new(0.0, 0.0),
                rotation: 0.0,
                label_type: LabelType::Net,
                shape: String::new(),
                font_size: 1.27,
                font_family: None,
                justify: Default::default(),
                justify_v: Default::default(),
                unknown_columns: Default::default(),
            });
        }
        let stepped = PlacementPayload::new(sheet).with_labels_stepped(3);
        let texts: Vec<_> = stepped
            .fragment()
            .labels
            .iter()
            .map(|l| l.text.as_str())
            .collect();
        assert_eq!(texts, ["D3", "CLK", "A10"]);
    }
}
