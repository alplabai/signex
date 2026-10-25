use crate::test_support::test_sheet;
use crate::*;
use signex_types::schematic::{HAlign, Label, LabelType, Point, Symbol, TextProp, VAlign};
use uuid::Uuid;

fn label(id: Uuid) -> Label {
    Label {
        uuid: id,
        text: "VIN".into(),
        position: Point { x: 1.0, y: 1.0 },
        rotation: 0.0,
        label_type: LabelType::Net,
        shape: String::new(),
        font_size: 1.27,
        font_family: None,
        justify: HAlign::Left,
        justify_v: VAlign::Bottom,
        unknown_columns: Default::default(),
    }
}

fn engine_with_label() -> (Engine, Uuid) {
    let id = Uuid::new_v4();
    let mut sheet = test_sheet();
    sheet.labels.push(label(id));
    (Engine::new(sheet).unwrap(), id)
}

fn set(engine: &mut Engine, id: Uuid, family: Option<&str>) -> CommandResult {
    engine
        .execute(Command::SetTextFontFamily {
            target: TextTarget::Label(id),
            font_family: family.map(str::to_string),
        })
        .unwrap()
}

#[test]
fn setting_a_family_is_undoable_and_clearing_returns_to_inherit() {
    let (mut engine, id) = engine_with_label();

    let result = set(&mut engine, id, Some("Fira Mono"));
    assert!(result.changed);
    assert_eq!(
        result.patch_pair.unwrap().document,
        DocumentPatch::LABELS,
        "only the label layer needs to redraw"
    );
    assert_eq!(
        engine.document().labels[0].font_family.as_deref(),
        Some("Fira Mono")
    );

    assert!(!set(&mut engine, id, Some("Fira Mono")).changed, "no-op");

    engine.undo().unwrap();
    assert_eq!(engine.document().labels[0].font_family, None);
    engine.redo().unwrap();
    assert!(set(&mut engine, id, None).changed);
    assert_eq!(engine.document().labels[0].font_family, None);
}

#[test]
fn a_blank_family_clears_instead_of_storing_an_empty_name() {
    let (mut engine, id) = engine_with_label();
    set(&mut engine, id, Some("Fira Mono"));
    assert!(set(&mut engine, id, Some("   ")).changed);
    assert_eq!(engine.document().labels[0].font_family, None);
}

#[test]
fn a_missing_target_changes_nothing_and_records_no_history() {
    let (mut engine, _) = engine_with_label();
    assert!(!set(&mut engine, Uuid::new_v4(), Some("X")).changed);
    assert!(!engine.can_undo());
}

#[test]
fn a_symbol_field_family_lives_on_its_text_prop() {
    let id = Uuid::new_v4();
    let mut sheet = test_sheet();
    sheet.symbols.push(Symbol {
        uuid: id,
        ref_text: Some(TextProp {
            position: Point { x: 0.0, y: 0.0 },
            rotation: 0.0,
            font_size: 1.27,
            font_family: None,
            justify_h: HAlign::Left,
            justify_v: VAlign::Bottom,
            hidden: false,
        }),
        ..Default::default()
    });
    let mut engine = Engine::new(sheet).unwrap();

    let reference = Command::SetTextFontFamily {
        target: TextTarget::SymbolReference(id),
        font_family: Some("Fira Mono".into()),
    };
    assert!(engine.execute(reference).unwrap().changed);
    assert_eq!(
        engine.document().symbols[0]
            .ref_text
            .as_ref()
            .unwrap()
            .font_family
            .as_deref(),
        Some("Fira Mono")
    );

    // This symbol carries no value TextProp, so there is nothing to attach
    // to: refused, not silently ignored, and nothing is recorded.
    let value = Command::SetTextFontFamily {
        target: TextTarget::SymbolValue(id),
        font_family: Some("Fira Mono".into()),
    };
    assert!(matches!(
        engine.execute(value),
        Err(EngineError::NoTextToStyle("value"))
    ));
    engine.undo().unwrap();
    assert_eq!(
        engine.document().symbols[0]
            .ref_text
            .as_ref()
            .unwrap()
            .font_family,
        None
    );
    assert!(!engine.can_undo(), "the refused command left no history");
}

#[test]
fn the_document_default_is_undoable() {
    let mut engine = Engine::new(test_sheet()).unwrap();
    let result = engine
        .execute(Command::SetDocumentFontFamily {
            font_family: Some("Fira Mono".into()),
        })
        .unwrap();
    assert!(result.changed);
    assert_eq!(
        result.patch_pair.unwrap().document,
        DocumentPatch::FONT_FAMILY
    );
    assert_eq!(engine.document().font_family.as_deref(), Some("Fira Mono"));

    let same = engine
        .execute(Command::SetDocumentFontFamily {
            font_family: Some("Fira Mono".into()),
        })
        .unwrap();
    assert!(!same.changed);

    engine.undo().unwrap();
    assert_eq!(engine.document().font_family, None);
}

#[test]
fn a_whitespace_only_document_default_clears_instead_of_storing_blank() {
    let mut engine = Engine::new(test_sheet()).unwrap();
    engine
        .execute(Command::SetDocumentFontFamily {
            font_family: Some(" Fira Mono ".into()),
        })
        .unwrap();
    assert_eq!(engine.document().font_family.as_deref(), Some("Fira Mono"));

    let cleared = engine
        .execute(Command::SetDocumentFontFamily {
            font_family: Some("   ".into()),
        })
        .unwrap();
    assert!(cleared.changed);
    assert_eq!(engine.document().font_family, None);

    let again = engine
        .execute(Command::SetDocumentFontFamily {
            font_family: Some("  ".into()),
        })
        .unwrap();
    assert!(!again.changed, "blank over unset is no change");
}
