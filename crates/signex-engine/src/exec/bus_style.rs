//! A bus's own width and colour (#2813). Each change is one undo step; a
//! missing bus or an unchanged value records nothing.

use crate::*;

impl Engine {
    pub(super) fn set_bus_width(
        &mut self,
        before: SchematicSheet,
        bus_id: uuid::Uuid,
        width_mm: f64,
    ) -> Result<CommandResult, EngineError> {
        if !width_mm.is_finite() || width_mm < 0.0 {
            return Err(EngineError::InvalidBusWidth(width_mm));
        }
        let changed = self
            .document
            .buses
            .iter_mut()
            .find(|bus| bus.uuid == bus_id)
            .is_some_and(|bus| {
                let differs = (bus.stroke_width - width_mm).abs() > 1e-9;
                bus.stroke_width = width_mm;
                differs
            });
        Ok(self.finish_bus_style(before, changed))
    }

    pub(super) fn set_bus_color(
        &mut self,
        before: SchematicSheet,
        bus_id: uuid::Uuid,
        color: Option<signex_types::theme::Color>,
    ) -> Result<CommandResult, EngineError> {
        let changed = self
            .document
            .buses
            .iter_mut()
            .find(|bus| bus.uuid == bus_id)
            .is_some_and(|bus| {
                let differs = bus.color != color;
                bus.color = color;
                differs
            });
        Ok(self.finish_bus_style(before, changed))
    }

    fn finish_bus_style(&mut self, before: SchematicSheet, changed: bool) -> CommandResult {
        if !changed {
            return CommandResult::unchanged();
        }
        let patch_pair = PatchPair {
            semantic: SemanticPatch::StyleUpdated,
            document: DocumentPatch::BUSES,
        };
        self.record_history(before, patch_pair);
        CommandResult::changed(patch_pair)
    }
}

#[cfg(test)]
mod tests {
    use signex_types::schematic::{Bus, Point};
    use signex_types::theme::Color;

    use super::*;

    const RED: Color = Color::new(255, 0, 0, 255);

    fn engine_with_bus() -> (Engine, uuid::Uuid) {
        let bus = Bus {
            uuid: uuid::Uuid::from_u128(1),
            start: Point::new(0.0, 0.0),
            end: Point::new(10.0, 0.0),
            ..Bus::default()
        };
        let id = bus.uuid;
        let sheet = SchematicSheet {
            buses: vec![bus],
            ..Default::default()
        };
        (Engine::new(sheet).expect("engine"), id)
    }

    #[test]
    fn width_and_colour_are_one_undo_step_each() {
        let (mut engine, bus_id) = engine_with_bus();
        let width = Command::SetBusWidth {
            bus_id,
            width_mm: 1.0,
        };
        assert!(engine.execute(width).expect("width").changed);
        let color = Command::SetBusColor {
            bus_id,
            color: Some(RED),
        };
        assert!(engine.execute(color).expect("colour").changed);
        assert_eq!(engine.document().buses[0].color, Some(RED));

        engine.undo().expect("undo colour");
        assert_eq!(engine.document().buses[0].color, None);
        assert_eq!(engine.document().buses[0].stroke_width, 1.0);
        engine.undo().expect("undo width");
        assert_eq!(engine.document().buses[0].stroke_width, 0.0);
    }

    #[test]
    fn an_unchanged_value_or_missing_bus_records_nothing() {
        let (mut engine, bus_id) = engine_with_bus();
        let same = Command::SetBusWidth {
            bus_id,
            width_mm: 0.0,
        };
        assert!(!engine.execute(same).expect("same").changed);
        let missing = Command::SetBusColor {
            bus_id: uuid::Uuid::from_u128(2),
            color: Some(RED),
        };
        assert!(!engine.execute(missing).expect("missing").changed);
    }

    #[test]
    fn a_bad_width_is_refused_and_keeps_the_stored_one() {
        let (mut engine, bus_id) = engine_with_bus();
        for width_mm in [-1.0, f64::NAN, f64::INFINITY] {
            let refused = engine.execute(Command::SetBusWidth { bus_id, width_mm });
            assert!(matches!(refused, Err(EngineError::InvalidBusWidth(_))));
        }
        assert_eq!(engine.document().buses[0].stroke_width, 0.0);
    }
}
