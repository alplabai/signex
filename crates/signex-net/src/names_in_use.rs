//! The net names a sheet writes, for a check that a new name is free (#2744).

use signex_types::schematic::SchematicSheet;

/// Every net name `sheet` writes, trimmed: its label texts of every kind and
/// its power ports' nets. A name may appear more than once.
pub fn sheet_net_names(sheet: &SchematicSheet) -> impl Iterator<Item = &str> {
    let labels = sheet.labels.iter().map(|l| l.text.as_str());
    let power = sheet
        .symbols
        .iter()
        .filter(|s| s.is_power)
        .map(|s| s.value.as_str());
    labels
        .chain(power)
        .map(str::trim)
        .filter(|name| !name.is_empty())
}

#[cfg(test)]
mod tests {
    use signex_types::schematic::{Label, LabelType, Point, Symbol};

    use super::*;

    #[test]
    fn labels_of_every_kind_and_power_ports_name_nets() {
        let mut sheet = SchematicSheet::default();
        for (text, kind) in [
            (" FB ", LabelType::Net),
            ("SDA", LabelType::Global),
            ("CLK", LabelType::Hierarchical),
            ("   ", LabelType::Net),
        ] {
            sheet
                .labels
                .push(Label::new(text, Point::new(0.0, 0.0), kind));
        }
        sheet.symbols.push(Symbol {
            is_power: true,
            value: "VCC".into(),
            ..Default::default()
        });
        sheet.symbols.push(Symbol {
            value: "10k".into(),
            ..Default::default()
        });
        let names: Vec<&str> = sheet_net_names(&sheet).collect();
        assert_eq!(names, ["FB", "SDA", "CLK", "VCC"]);
    }
}
