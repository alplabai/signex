//! `Command::AutoplaceFields` (#1667): the fields of the named parts go where
//! the autoplacer puts them. Asked for by name, so it also moves fields the
//! user placed, which a turn or a mirror never does.

use signex_types::schematic::SchematicSheet;
use uuid::Uuid;

use super::autoplace_fields;
use crate::{CommandResult, DocumentPatch, Engine, PatchPair, SemanticPatch};

impl Engine {
    pub(crate) fn exec_autoplace_fields(
        &mut self,
        before: SchematicSheet,
        ids: &[Uuid],
    ) -> CommandResult {
        let snapshot = self.document.clone();
        let mut changed = false;
        for symbol in self
            .document
            .symbols
            .iter_mut()
            .filter(|s| ids.contains(&s.uuid))
        {
            let Some(lib) = snapshot.body_of(symbol) else {
                continue;
            };
            let mut placed = symbol.clone();
            placed.fields_user_placed = false;
            autoplace_fields(&mut placed, lib, &snapshot);
            if placed != *symbol {
                *symbol = placed;
                changed = true;
            }
        }
        if !changed {
            return CommandResult::unchanged();
        }
        let patch_pair = PatchPair {
            semantic: SemanticPatch::SymbolFieldsUpdated,
            document: DocumentPatch::SYMBOLS,
        };
        self.record_history(before, patch_pair);
        CommandResult::changed(patch_pair)
    }
}

#[cfg(test)]
mod tests {
    use crate::test_support::test_sheet;
    use crate::{Command, Engine};
    use signex_types::schematic::{
        HAlign, LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point, Symbol, TextProp,
        VAlign,
    };
    use uuid::Uuid;

    fn field(x: f64, y: f64) -> TextProp {
        TextProp {
            position: Point::new(x, y),
            rotation: 0.0,
            font_size: 1.27,
            font_family: None,
            justify_h: HAlign::Left,
            justify_v: VAlign::Bottom,
            hidden: false,
        }
    }

    /// A one-pin part whose fields the user dragged far off the body.
    fn engine_with_moved_fields() -> (Engine, Uuid) {
        let mut sheet = test_sheet();
        let mut lib: LibSymbol = serde_json::from_str(r#"{"id":"lib:r"}"#).unwrap();
        lib.pins.push(LibPin {
            unit: 1,
            body_style: 1,
            pin: Pin {
                direction: PinDirection::Passive,
                shape_style: PinShapeStyle::Plain,
                position: Point::new(0.0, 3.81),
                rotation: 270.0,
                length: 2.54,
                name: "1".into(),
                number: "1".into(),
                visible: true,
                name_visible: true,
                number_visible: true,
            },
        });
        sheet.lib_symbols.insert(lib.id.clone(), lib);
        let id = Uuid::now_v7();
        sheet.symbols.push(Symbol {
            uuid: id,
            lib_id: "lib:r".into(),
            reference: "R1".into(),
            value: "10k".into(),
            position: Point::new(50.8, 50.8),
            unit: 1,
            ref_text: Some(field(90.0, 10.0)),
            val_text: Some(field(90.0, 12.0)),
            fields_user_placed: true,
            ..Default::default()
        });
        (Engine::new(sheet).unwrap(), id)
    }

    #[test]
    fn fields_the_user_moved_come_back_to_the_part_in_one_undo_step() {
        let (mut engine, id) = engine_with_moved_fields();
        let before = engine.document().symbols[0].clone();
        let result = engine
            .execute(Command::AutoplaceFields {
                symbol_ids: vec![id],
            })
            .unwrap();
        assert!(result.changed);
        let after = &engine.document().symbols[0];
        assert!(!after.fields_user_placed);
        let reference = after.ref_text.as_ref().unwrap().position;
        assert!(
            (reference.x - 50.8).abs() < 20.0 && (reference.y - 50.8).abs() < 20.0,
            "the reference sits by its part: {reference:?}"
        );
        engine.undo().unwrap();
        assert_eq!(engine.document().symbols[0], before);
    }

    #[test]
    fn autoplacing_twice_changes_nothing_the_second_time() {
        let (mut engine, id) = engine_with_moved_fields();
        let ids = vec![id];
        engine
            .execute(Command::AutoplaceFields {
                symbol_ids: ids.clone(),
            })
            .unwrap();
        let again = engine
            .execute(Command::AutoplaceFields { symbol_ids: ids })
            .unwrap();
        assert!(!again.changed);
    }

    #[test]
    fn an_unknown_id_changes_nothing() {
        let (mut engine, _) = engine_with_moved_fields();
        let result = engine
            .execute(Command::AutoplaceFields {
                symbol_ids: vec![Uuid::now_v7()],
            })
            .unwrap();
        assert!(!result.changed);
    }
}
