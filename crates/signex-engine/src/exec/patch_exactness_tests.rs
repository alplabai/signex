//! A `DocumentPatch` must name every collection its command changed.
//!
//! The app's render cache copies only the collections a patch reports, so a
//! patch that is too narrow draws stale geometry that hit-testing disagrees
//! with. For every `Command` kind (and each selection kind for the
//! selection commands), the collections that differ between the document
//! before and after must be a subset of the returned patch, and the same
//! must hold for the undo and redo that replay it.

use crate::test_support::test_sheet;
use crate::{
    AnnotateMode, Command, CommandKind, DocumentPatch, Engine, MirrorAxis, ReorderDirection,
    SymbolTextField, TextTarget,
};
use signex_types::schematic::{
    AnnotateOrder, Bus, BusEntry, ChildSheet, FillType, HAlign, Junction, Label, LabelType, LibPin,
    LibSymbol, NoConnect, Pin, PinDirection, PinShapeStyle, Point, SchDrawing, SchematicSheet,
    SelectedItem, SelectedKind, SheetPin, StrokeColor, Symbol, TextNote, TextProp, VAlign, Wire,
};
use std::collections::BTreeMap;
use uuid::Uuid;

/// Deterministic id: `group` names the collection, `n` the element in it.
fn uid(group: u128, n: u128) -> Uuid {
    Uuid::from_u128(group * 100 + n)
}

const SYMBOL: u128 = 1;
const WIRE: u128 = 2;
const JUNCTION: u128 = 3;
const LABEL: u128 = 4;
const TEXT_NOTE: u128 = 5;
const BUS: u128 = 6;
const BUS_ENTRY: u128 = 7;
const NO_CONNECT: u128 = 8;
const NO_ERC: u128 = 9;
const CHILD: u128 = 10;
const SHEET_PIN: u128 = 11;
const DRAWING: u128 = 12;
const NEW: u128 = 90;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn lib_symbol(id: &str, description: &str) -> LibSymbol {
    LibSymbol {
        id: id.to_string(),
        reference: "U".into(),
        value: String::new(),
        footprint: String::new(),
        datasheet: String::new(),
        description: description.to_string(),
        keywords: String::new(),
        fp_filters: String::new(),
        in_bom: true,
        on_board: true,
        in_pos_files: true,
        duplicate_pin_numbers_are_jumpers: false,
        graphics: Vec::new(),
        // One pin, away from everything, so replacing the symbol is not
        // refused as removing a unit.
        pins: vec![LibPin {
            unit: 1,
            body_style: 1,
            pin: Pin {
                direction: PinDirection::Passive,
                shape_style: PinShapeStyle::Plain,
                position: pt(-200.0, -200.0),
                rotation: 0.0,
                length: 2.54,
                name: "1".into(),
                number: "1".into(),
                visible: true,
                name_visible: true,
                number_visible: true,
            },
        }],
        show_pin_numbers: true,
        show_pin_names: true,
        pin_name_offset: 0.0,
    }
}

fn text_prop(n: u128, y: f64) -> TextProp {
    TextProp {
        position: pt(10.0 * n as f64, y),
        rotation: 0.0,
        font_size: 1.27,
        justify_h: HAlign::Left,
        justify_v: VAlign::Bottom,
        hidden: false,
    }
}

fn symbol(n: u128) -> Symbol {
    Symbol {
        uuid: uid(SYMBOL, n),
        lib_id: "L:S".into(),
        reference: "U?".into(),
        unit: 1,
        position: pt(10.0 * n as f64, 10.0),
        ref_text: Some(text_prop(n, 12.0)),
        val_text: Some(text_prop(n, 14.0)),
        ..Default::default()
    }
}

fn wire(n: u128) -> Wire {
    Wire {
        uuid: uid(WIRE, n),
        start: pt(0.0, 20.0 * n as f64),
        end: pt(5.0, 20.0 * n as f64),
        ..Default::default()
    }
}

fn label(n: u128) -> Label {
    Label {
        unknown_columns: Default::default(),
        uuid: uid(LABEL, n),
        text: format!("NET{n}"),
        position: pt(30.0, 5.0 * n as f64),
        rotation: 0.0,
        label_type: LabelType::Net,
        shape: String::new(),
        font_size: 1.27,
        justify: HAlign::Left,
        justify_v: VAlign::Bottom,
    }
}

fn text_note(n: u128) -> TextNote {
    TextNote {
        uuid: uid(TEXT_NOTE, n),
        text: format!("note {n}"),
        position: pt(40.0, 5.0 * n as f64),
        rotation: 0.0,
        font_size: 1.27,
        justify_h: HAlign::Left,
        justify_v: VAlign::Bottom,
    }
}

fn child_sheet(n: u128) -> ChildSheet {
    ChildSheet {
        uuid: uid(CHILD, n),
        name: format!("Child{n}"),
        filename: format!("c{n}.snxsch"),
        position: pt(50.0, 30.0 * n as f64),
        size: (20.0, 20.0),
        stroke_width: 0.12,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: vec![SheetPin {
            uuid: uid(SHEET_PIN, n),
            name: "PIN".into(),
            direction: "input".into(),
            position: pt(50.0, 30.0 * n as f64 + 2.0),
            rotation: 0.0,
            auto_generated: false,
            user_moved: false,
        }],
        instances: Vec::new(),
    }
}

fn drawing(n: u128, end_x: f64) -> SchDrawing {
    SchDrawing::Line {
        uuid: uid(DRAWING, n),
        start: pt(60.0, 5.0 * n as f64),
        end: pt(end_x, 5.0 * n as f64),
        width: 0.2,
        stroke_color: None,
    }
}

/// Two elements of every drawn collection, so reorders have something to do.
fn fixture() -> SchematicSheet {
    let mut s = test_sheet();
    s.lib_symbols.insert("L:S".into(), lib_symbol("L:S", "old"));
    s.title_block.insert("title".into(), "T".into());
    for n in 1..=2 {
        s.symbols.push(symbol(n));
        s.wires.push(wire(n));
        s.junctions.push(Junction {
            uuid: uid(JUNCTION, n),
            position: pt(70.0, 5.0 * n as f64),
            ..Default::default()
        });
        s.labels.push(label(n));
        s.text_notes.push(text_note(n));
        s.buses.push(Bus {
            uuid: uid(BUS, n),
            start: pt(80.0, 5.0 * n as f64),
            end: pt(90.0, 5.0 * n as f64),
        });
        s.bus_entries.push(BusEntry {
            uuid: uid(BUS_ENTRY, n),
            position: pt(100.0, 5.0 * n as f64),
            size: (2.5, 2.5),
        });
        s.no_connects.push(NoConnect {
            uuid: uid(NO_CONNECT, n),
            position: pt(110.0, 5.0 * n as f64),
        });
        s.no_erc_directives.push(NoConnect {
            uuid: uid(NO_ERC, n),
            position: pt(120.0, 5.0 * n as f64),
        });
        s.child_sheets.push(child_sheet(n));
        s.drawings.push(drawing(n, 65.0));
    }
    s
}

/// The patch bits for the collections that differ between `a` and `b`.
/// Destructuring every field forces a decision when `SchematicSheet` grows.
fn changed_collections(a: &SchematicSheet, b: &SchematicSheet) -> DocumentPatch {
    fn differ<T: std::fmt::Debug>(x: &T, y: &T) -> bool {
        format!("{x:?}") != format!("{y:?}")
    }
    fn sorted<V>(m: &std::collections::HashMap<String, V>) -> BTreeMap<&String, &V> {
        m.iter().collect()
    }
    let SchematicSheet {
        uuid: _,
        // Not drawn, so no collection to refresh.
        version: _,
        generator: _,
        generator_version: _,
        unknown: _,
        // Loaded baseline only; the app owns the live overrides and no command edits them.
        net_colors: _,
        paper_size,
        root_sheet_page,
        symbols,
        wires,
        junctions,
        labels,
        child_sheets,
        no_connects,
        text_notes,
        buses,
        bus_entries,
        drawings,
        no_erc_directives,
        title_block,
        lib_symbols,
    } = a;
    let mut patch = DocumentPatch::NONE;
    let mut flag = |changed: bool, bit: DocumentPatch| {
        if changed {
            patch |= bit;
        }
    };
    flag(differ(symbols, &b.symbols), DocumentPatch::SYMBOLS);
    flag(differ(wires, &b.wires), DocumentPatch::WIRES);
    flag(differ(junctions, &b.junctions), DocumentPatch::JUNCTIONS);
    flag(differ(labels, &b.labels), DocumentPatch::LABELS);
    flag(
        differ(child_sheets, &b.child_sheets),
        DocumentPatch::CHILD_SHEETS,
    );
    flag(
        differ(no_connects, &b.no_connects),
        DocumentPatch::NO_CONNECTS,
    );
    flag(differ(text_notes, &b.text_notes), DocumentPatch::TEXT_NOTES);
    flag(differ(buses, &b.buses), DocumentPatch::BUSES);
    flag(
        differ(bus_entries, &b.bus_entries),
        DocumentPatch::BUS_ENTRIES,
    );
    flag(differ(drawings, &b.drawings), DocumentPatch::DRAWINGS);
    flag(
        differ(no_erc_directives, &b.no_erc_directives),
        DocumentPatch::NO_ERC_DIRECTIVES,
    );
    flag(
        differ(&sorted(lib_symbols), &sorted(&b.lib_symbols)),
        DocumentPatch::LIB_SYMBOLS,
    );
    flag(
        differ(paper_size, &b.paper_size)
            || differ(root_sheet_page, &b.root_sheet_page)
            || differ(&sorted(title_block), &sorted(&b.title_block)),
        DocumentPatch::PAPER,
    );
    patch
}

fn assert_covers(
    what: &str,
    patch: DocumentPatch,
    before: &SchematicSheet,
    after: &SchematicSheet,
) {
    let diff = changed_collections(before, after);
    assert!(
        patch.contains(diff),
        "{what}: patch {patch:?} is narrower than the collections that changed ({diff:?})"
    );
}

const ALL_KINDS: [CommandKind; 36] = [
    CommandKind::ReplaceDocument,
    CommandKind::MoveSelection,
    CommandKind::RotateSelection,
    CommandKind::MirrorSelection,
    CommandKind::DeleteSelection,
    CommandKind::UpdateText,
    CommandKind::UpdateLabelProps,
    CommandKind::SetSymbolRotation,
    CommandKind::SetSymbolLocked,
    CommandKind::SetSymbolDnp,
    CommandKind::UpdateSymbolTextSize,
    CommandKind::UpdateSymbolLibId,
    CommandKind::UpdateSymbolFootprint,
    CommandKind::UpdateSymbolFields,
    CommandKind::PlaceWireSegment,
    CommandKind::PlaceBus,
    CommandKind::PlaceLabel,
    CommandKind::PlaceSymbol,
    CommandKind::RegisterLibSymbol,
    CommandKind::ReplaceLibSymbol,
    CommandKind::PlaceChildSheet,
    CommandKind::PlaceJunction,
    CommandKind::PlaceNoConnect,
    CommandKind::PlaceNoErcDirective,
    CommandKind::PlaceBusEntry,
    CommandKind::PlaceTextNote,
    CommandKind::PlaceSchDrawing,
    CommandKind::UpdateSchDrawing,
    CommandKind::UpdateChildSheetStyle,
    CommandKind::AnnotateAll,
    CommandKind::MoveSymbolAbsolute,
    CommandKind::ReorderObjects,
    CommandKind::ReconcileChildSheetPins,
    CommandKind::RenameChildSheetFile,
    CommandKind::ModifyChildSheet,
    CommandKind::SetPaperSize,
];

/// Adding a `CommandKind` breaks this match: add a sample below and list the
/// kind in `ALL_KINDS`.
#[allow(dead_code)]
fn every_kind_is_listed(kind: CommandKind) {
    match kind {
        CommandKind::ReplaceDocument
        | CommandKind::MoveSelection
        | CommandKind::RotateSelection
        | CommandKind::MirrorSelection
        | CommandKind::DeleteSelection
        | CommandKind::UpdateText
        | CommandKind::UpdateLabelProps
        | CommandKind::SetSymbolRotation
        | CommandKind::SetSymbolLocked
        | CommandKind::SetSymbolDnp
        | CommandKind::UpdateSymbolTextSize
        | CommandKind::UpdateSymbolLibId
        | CommandKind::UpdateSymbolFootprint
        | CommandKind::UpdateSymbolFields
        | CommandKind::PlaceWireSegment
        | CommandKind::PlaceBus
        | CommandKind::PlaceLabel
        | CommandKind::PlaceSymbol
        | CommandKind::RegisterLibSymbol
        | CommandKind::ReplaceLibSymbol
        | CommandKind::PlaceChildSheet
        | CommandKind::PlaceJunction
        | CommandKind::PlaceNoConnect
        | CommandKind::PlaceNoErcDirective
        | CommandKind::PlaceBusEntry
        | CommandKind::PlaceTextNote
        | CommandKind::PlaceSchDrawing
        | CommandKind::UpdateSchDrawing
        | CommandKind::UpdateChildSheetStyle
        | CommandKind::AnnotateAll
        | CommandKind::MoveSymbolAbsolute
        | CommandKind::ReorderObjects
        | CommandKind::ReconcileChildSheetPins
        | CommandKind::RenameChildSheetFile
        | CommandKind::ModifyChildSheet
        | CommandKind::SetPaperSize => {}
    }
}

/// `(selected kind, id group)` for each selectable thing in the fixture.
const SELECTABLE: [(SelectedKind, u128); 14] = [
    (SelectedKind::Symbol, SYMBOL),
    (SelectedKind::SymbolRefField, SYMBOL),
    (SelectedKind::SymbolValField, SYMBOL),
    (SelectedKind::Wire, WIRE),
    (SelectedKind::Bus, BUS),
    (SelectedKind::BusEntry, BUS_ENTRY),
    (SelectedKind::Junction, JUNCTION),
    (SelectedKind::NoConnect, NO_CONNECT),
    (SelectedKind::NoErcDirective, NO_ERC),
    (SelectedKind::Label, LABEL),
    (SelectedKind::SheetPin, SHEET_PIN),
    (SelectedKind::TextNote, TEXT_NOTE),
    (SelectedKind::ChildSheet, CHILD),
    (SelectedKind::Drawing, DRAWING),
];

fn samples() -> Vec<(String, Command)> {
    let mut out: Vec<(String, Command)> = Vec::new();
    let mut add = |name: &str, cmd: Command| out.push((name.to_string(), cmd));

    for (kind, group) in SELECTABLE {
        let item = SelectedItem::new(uid(group, 1), kind);
        add(
            &format!("MoveSelection {kind:?}"),
            Command::MoveSelection {
                items: vec![item],
                dx: 2.54,
                dy: 2.54,
            },
        );
        add(
            &format!("RotateSelection {kind:?}"),
            Command::RotateSelection {
                items: vec![item],
                angle_degrees: 90.0,
            },
        );
        add(
            &format!("MirrorSelection {kind:?}"),
            Command::MirrorSelection {
                items: vec![item],
                axis: MirrorAxis::Horizontal,
            },
        );
        add(
            &format!("DeleteSelection {kind:?}"),
            Command::DeleteSelection { items: vec![item] },
        );
        add(
            &format!("ReorderObjects {kind:?}"),
            Command::ReorderObjects {
                items: vec![item],
                direction: ReorderDirection::ToFront,
            },
        );
    }

    let mut doc = fixture();
    doc.wires.push(wire(NEW));
    doc.paper_size = "A3".into();
    add(
        "ReplaceDocument",
        Command::ReplaceDocument { document: doc },
    );

    for (name, target) in [
        ("Label", TextTarget::Label(uid(LABEL, 1))),
        ("TextNote", TextTarget::TextNote(uid(TEXT_NOTE, 1))),
        (
            "SymbolReference",
            TextTarget::SymbolReference(uid(SYMBOL, 1)),
        ),
        ("SymbolValue", TextTarget::SymbolValue(uid(SYMBOL, 1))),
    ] {
        add(
            &format!("UpdateText {name}"),
            Command::UpdateText {
                target,
                value: "changed".into(),
            },
        );
    }
    add(
        "UpdateLabelProps",
        Command::UpdateLabelProps {
            label_id: uid(LABEL, 1),
            font_size_mm: Some(2.0),
            justify: Some(HAlign::Right),
            rotation_degrees: Some(90.0),
        },
    );
    let sym = uid(SYMBOL, 1);
    add(
        "SetSymbolRotation",
        Command::SetSymbolRotation {
            symbol_id: sym,
            rotation_degrees: 90.0,
        },
    );
    add(
        "SetSymbolLocked",
        Command::SetSymbolLocked {
            symbol_id: sym,
            locked: true,
        },
    );
    add(
        "SetSymbolDnp",
        Command::SetSymbolDnp {
            symbol_id: sym,
            dnp: true,
        },
    );
    for field in [SymbolTextField::Reference, SymbolTextField::Value] {
        add(
            &format!("UpdateSymbolTextSize {field:?}"),
            Command::UpdateSymbolTextSize {
                symbol_id: sym,
                field,
                font_size_mm: 2.0,
            },
        );
    }
    add(
        "UpdateSymbolLibId",
        Command::UpdateSymbolLibId {
            symbol_id: sym,
            lib_id: "L:T".into(),
        },
    );
    add(
        "UpdateSymbolFootprint",
        Command::UpdateSymbolFootprint {
            symbol_id: sym,
            footprint: "FP:0603".into(),
        },
    );
    add(
        "UpdateSymbolFields",
        Command::UpdateSymbolFields {
            symbol_id: sym,
            reference: "U9".into(),
            value: "10k".into(),
            footprint: "FP:0603".into(),
        },
    );
    add(
        "SetSymbolField",
        Command::SetSymbolField {
            symbol_id: sym,
            key: "MPN".into(),
            value: "X1".into(),
        },
    );
    add(
        "PlaceWireSegment",
        Command::PlaceWireSegment { wire: wire(NEW) },
    );
    add(
        "PlaceBus",
        Command::PlaceBus {
            bus: Bus {
                uuid: uid(BUS, NEW),
                start: pt(0.0, 200.0),
                end: pt(9.0, 200.0),
            },
        },
    );
    add("PlaceLabel", Command::PlaceLabel { label: label(NEW) });
    add(
        "PlaceSymbol",
        Command::PlaceSymbol {
            symbol: symbol(NEW),
        },
    );
    add(
        "RegisterLibSymbol",
        Command::RegisterLibSymbol {
            lib_symbol: lib_symbol("L:NEW", "new"),
        },
    );
    add(
        "ReplaceLibSymbol",
        Command::ReplaceLibSymbol {
            lib_symbol: lib_symbol("L:S", "new"),
            library_versions: vec![(sym, "2".into())],
        },
    );
    add(
        "PlaceChildSheet",
        Command::PlaceChildSheet {
            child_sheet: child_sheet(NEW),
        },
    );
    add(
        "PlaceJunction",
        Command::PlaceJunction {
            junction: Junction {
                uuid: uid(JUNCTION, NEW),
                position: pt(3.0, 3.0),
                ..Default::default()
            },
        },
    );
    add(
        "PlaceNoConnect",
        Command::PlaceNoConnect {
            no_connect: NoConnect {
                uuid: uid(NO_CONNECT, NEW),
                position: pt(4.0, 4.0),
            },
        },
    );
    add(
        "PlaceNoErcDirective",
        Command::PlaceNoErcDirective {
            directive: NoConnect {
                uuid: uid(NO_ERC, NEW),
                position: pt(5.0, 5.0),
            },
        },
    );
    add(
        "PlaceBusEntry",
        Command::PlaceBusEntry {
            bus_entry: BusEntry {
                uuid: uid(BUS_ENTRY, NEW),
                position: pt(6.0, 6.0),
                size: (2.5, 2.5),
            },
        },
    );
    add(
        "PlaceTextNote",
        Command::PlaceTextNote {
            text_note: text_note(NEW),
        },
    );
    add(
        "PlaceSchDrawing",
        Command::PlaceSchDrawing {
            drawing: drawing(NEW, 70.0),
        },
    );
    add(
        "UpdateSchDrawing",
        Command::UpdateSchDrawing {
            drawing: drawing(1, 99.0),
        },
    );
    add(
        "UpdateChildSheetStyle",
        Command::UpdateChildSheetStyle {
            sheet_id: uid(CHILD, 1),
            stroke_width: Some(0.5),
            stroke_color: Some(Some(StrokeColor {
                r: 1,
                g: 2,
                b: 3,
                a: 255,
            })),
            fill_color: None,
        },
    );
    for mode in [
        AnnotateMode::Incremental,
        AnnotateMode::ResetAndRenumber,
        AnnotateMode::ResetOnly,
    ] {
        add(
            &format!("AnnotateAll {mode:?}"),
            Command::AnnotateAll {
                mode,
                order: AnnotateOrder::default(),
            },
        );
    }
    add(
        "MoveSymbolAbsolute",
        Command::MoveSymbolAbsolute {
            symbol_id: sym,
            x: 33.0,
            y: 44.0,
        },
    );
    add(
        "ReconcileChildSheetPins",
        Command::ReconcileChildSheetPins {
            child_filename: "c1.snxsch".into(),
            ports: vec![crate::SheetPort {
                name: "SDA".into(),
                direction: "input".into(),
            }],
        },
    );
    add(
        "RenameChildSheetFile",
        Command::RenameChildSheetFile {
            from: "c1.snxsch".into(),
            to: "renamed.snxsch".into(),
        },
    );
    add(
        "ModifyChildSheet",
        Command::ModifyChildSheet {
            sheet_id: uid(CHILD, 1),
            name: Some("Renamed".into()),
            filename: Some("renamed.snxsch".into()),
        },
    );
    add(
        "SetPaperSize",
        Command::SetPaperSize {
            paper_size: "A3".into(),
        },
    );
    out
}

#[test]
fn every_command_patch_covers_the_collections_it_changed() {
    let mut exercised: Vec<CommandKind> = Vec::new();
    for (name, cmd) in samples() {
        let kind = cmd.kind();
        let mut engine = Engine::new(fixture()).unwrap();
        let before = engine.document().clone();
        // A refused command rolls back; there is nothing to compare.
        let Ok(result) = engine.execute(cmd) else {
            continue;
        };
        let patch = result
            .patch_pair
            .map_or(DocumentPatch::NONE, |p| p.document);
        assert_covers(&name, patch, &before, engine.document());
        if !result.changed {
            continue;
        }
        exercised.push(kind);

        let after = engine.document().clone();
        let undo = engine.undo().unwrap().expect("an edit was recorded");
        assert_covers(
            &format!("{name} (undo)"),
            undo.document,
            &after,
            engine.document(),
        );
        let undone = engine.document().clone();
        let redo = engine.redo().unwrap().expect("an undone edit");
        assert_covers(
            &format!("{name} (redo)"),
            redo.document,
            &undone,
            engine.document(),
        );
    }
    for kind in ALL_KINDS {
        assert!(
            exercised.contains(&kind),
            "no sample for {kind:?} changed the fixture; fix or add a sample"
        );
    }
}

#[test]
fn a_batch_patch_covers_every_collection_its_commands_changed() {
    let mut engine = Engine::new(fixture()).unwrap();
    let before = engine.document().clone();
    let result = engine
        .execute_batch([
            Command::PlaceWireSegment { wire: wire(NEW) },
            Command::PlaceLabel { label: label(NEW) },
            Command::SetPaperSize {
                paper_size: "A3".into(),
            },
        ])
        .unwrap();
    let patch = result
        .patch_pair
        .map_or(DocumentPatch::NONE, |p| p.document);
    assert_covers("batch", patch, &before, engine.document());
}
