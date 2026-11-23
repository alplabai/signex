//! The Style section's values for a selected wire or bus (mockup row 43).

use signex_types::schematic::{SchematicSheet, SelectedKind, WireWidthPreset};
use signex_types::theme::Color;
use uuid::Uuid;

/// The width preset and own colour of the selected run; `None` for a kind
/// with no Style section of this shape.
pub(super) fn of_selected(
    sheet: &SchematicSheet,
    kind: SelectedKind,
    id: Uuid,
) -> Option<(WireWidthPreset, Option<Color>)> {
    let (width, color) = match kind {
        SelectedKind::Wire => {
            let wire = sheet.wires.iter().find(|w| w.uuid == id)?;
            (wire.stroke_width, wire.color)
        }
        SelectedKind::Bus => {
            let bus = sheet.buses.iter().find(|b| b.uuid == id)?;
            (bus.stroke_width, bus.color)
        }
        _ => return None,
    };
    Some((WireWidthPreset::from_stroke_mm(width), color))
}

#[cfg(test)]
mod tests {
    use signex_types::schematic::Bus;

    use super::*;

    #[test]
    fn a_selected_bus_reads_its_own_width_and_colour() {
        let id = Uuid::from_u128(1);
        let red = Color::new(255, 0, 0, 255);
        let sheet = SchematicSheet {
            buses: vec![Bus {
                uuid: id,
                stroke_width: 1.0,
                color: Some(red),
                ..Bus::default()
            }],
            ..Default::default()
        };
        assert_eq!(
            of_selected(&sheet, SelectedKind::Bus, id),
            Some((WireWidthPreset::Large, Some(red)))
        );
        assert_eq!(of_selected(&sheet, SelectedKind::Junction, id), None);
        assert_eq!(
            of_selected(&sheet, SelectedKind::Bus, Uuid::from_u128(2)),
            None
        );
    }
}
