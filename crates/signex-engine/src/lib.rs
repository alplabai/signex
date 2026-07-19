mod command;
mod error;
mod patch;

mod annotation;
mod history;
mod selection;
mod sheet;
mod transform;

mod exec;

use std::path::{Path, PathBuf};

pub use command::{
    AnnotateMode, Command, CommandKind, MirrorAxis, ReorderDirection, SheetPort, SymbolTextField,
    TextTarget,
};
pub use error::EngineError;
use history::HistoryEntry;
pub use patch::{CommandResult, DocumentPatch, PatchPair, SemanticPatch};
pub use selection::{ClipboardSelection, SelectionAnchor, SelectionDetails};
use signex_types::schematic::SchematicSheet;

const JUNCTION_TOLERANCE_MM: f64 = 0.01;

#[derive(Debug)]
pub struct Engine {
    document: SchematicSheet,
    path: Option<PathBuf>,
    history: Vec<HistoryEntry>,
    redo_stack: Vec<HistoryEntry>,
}

impl Engine {
    pub fn new(document: SchematicSheet) -> Result<Self, EngineError> {
        Self::new_with_path(document, None)
    }

    pub fn new_with_path(
        document: SchematicSheet,
        path: Option<PathBuf>,
    ) -> Result<Self, EngineError> {
        Ok(Self {
            document,
            path,
            history: Vec::new(),
            redo_stack: Vec::new(),
        })
    }

    /// Open a `.snxsch` file from disk.
    ///
    /// Foreign-format files (any extension other than `.snxsch`) are not
    /// readable here; an optional GPL-3.0 import companion handles
    /// conversion to `.snxsch` and is shipped separately from this
    /// Apache-2.0 crate.
    pub fn open(path: &Path) -> Result<Self, EngineError> {
        let text = std::fs::read_to_string(path)
            .map_err(|error| EngineError::OpenFailed(anyhow::Error::msg(error.to_string())))?;
        let snx = signex_types::format::SnxSchematic::parse(&text)
            .map_err(|error| EngineError::OpenFailed(anyhow::Error::msg(error.to_string())))?;

        Ok(Self {
            document: snx.sheet,
            path: Some(path.to_path_buf()),
            history: Vec::new(),
            redo_stack: Vec::new(),
        })
    }

    pub fn save(&mut self) -> Result<(), EngineError> {
        let Some(path) = self.path.clone() else {
            return Err(EngineError::MissingPath);
        };

        self.save_as(&path)
    }

    pub fn save_as(&mut self, path: &Path) -> Result<(), EngineError> {
        let snx = signex_types::format::SnxSchematic::new(self.document.clone());
        let content = snx
            .write_string()
            .map_err(|error| EngineError::SaveFailed(std::io::Error::other(error.to_string())))?;
        // HI-6: atomic write — a crash mid-save no longer truncates the
        // destination. The user's prior file stays intact until the
        // rename succeeds.
        signex_types::atomic_io::atomic_write(path, content.as_bytes())
            .map_err(EngineError::SaveFailed)?;
        self.path = Some(path.to_path_buf());
        Ok(())
    }

    pub fn execute(&mut self, cmd: Command) -> Result<CommandResult, EngineError> {
        let before = self.document.clone();

        match cmd {
            Command::ReplaceDocument { .. }
            | Command::UpdateText { .. }
            | Command::UpdateLabelProps { .. }
            | Command::SetSymbolRotation { .. }
            | Command::UpdateSymbolTextSize { .. }
            | Command::UpdateSymbolLibId { .. }
            | Command::UpdateSymbolFootprint { .. }
            | Command::SetSymbolField { .. }
            | Command::UpdateSymbolFields { .. } => self.exec_edits(before, cmd),
            Command::DeleteSelection { .. }
            | Command::MoveSelection { .. }
            | Command::RotateSelection { .. }
            | Command::MirrorSelection { .. }
            | Command::PlaceBus { .. }
            | Command::PlaceLabel { .. }
            | Command::PlaceSymbol { .. }
            | Command::PlaceWireSegment { .. }
            | Command::PlaceJunction { .. }
            | Command::PlaceNoConnect { .. }
            | Command::PlaceBusEntry { .. }
            | Command::PlaceTextNote { .. }
            | Command::PlaceSchDrawing { .. } => self.exec_place(before, cmd),
            Command::UpdateSchDrawing { .. }
            | Command::UpdateChildSheetStyle { .. }
            | Command::AnnotateAll { .. }
            | Command::MoveSymbolAbsolute { .. }
            | Command::ReorderObjects { .. }
            | Command::ReconcileChildSheetPins { .. }
            | Command::SetPaperSize { .. } => self.exec_structure(before, cmd),
        }
    }

    pub fn document(&self) -> &SchematicSheet {
        &self.document
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn set_path(&mut self, path: Option<PathBuf>) {
        self.path = path;
    }

    pub fn set_document(&mut self, document: SchematicSheet) {
        self.document = document;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::schematic::{
        ChildSheet, FillType, GRID_MM, Label, LabelType, Point, SelectedItem, SelectedKind,
        SheetPin,
    };

    fn test_sheet() -> SchematicSheet {
        SchematicSheet {
            uuid: uuid::Uuid::new_v4(),
            version: 0,
            generator: String::new(),
            generator_version: String::new(),
            paper_size: "A4".to_string(),
            root_sheet_page: "1".to_string(),
            symbols: Vec::new(),
            wires: Vec::new(),
            junctions: Vec::new(),
            labels: Vec::new(),
            child_sheets: Vec::new(),
            no_connects: Vec::new(),
            text_notes: Vec::new(),
            buses: Vec::new(),
            bus_entries: Vec::new(),
            drawings: Vec::new(),
            no_erc_directives: Vec::new(),
            title_block: std::collections::HashMap::new(),
            lib_symbols: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn collect_exposed_sheet_ports_prefers_hierarchical_over_global() {
        let mut document = test_sheet();
        document.labels.push(Label {
            uuid: uuid::Uuid::new_v4(),
            text: "ALERT".to_string(),
            position: Point::new(0.0, 0.0),
            rotation: 0.0,
            label_type: LabelType::Global,
            shape: "output".to_string(),
            font_size: 1.27,
            justify: signex_types::schematic::HAlign::Left,
            justify_v: signex_types::schematic::VAlign::Bottom,
        });
        document.labels.push(Label {
            uuid: uuid::Uuid::new_v4(),
            text: "ALERT".to_string(),
            position: Point::new(1.0, 1.0),
            rotation: 0.0,
            label_type: LabelType::Hierarchical,
            shape: "input".to_string(),
            font_size: 1.27,
            justify: signex_types::schematic::HAlign::Left,
            justify_v: signex_types::schematic::VAlign::Bottom,
        });

        let engine = Engine::new(document).unwrap();
        let ports = engine.collect_exposed_sheet_ports();

        assert_eq!(ports.len(), 1);
        assert_eq!(ports[0].name, "ALERT");
        assert_eq!(ports[0].direction, "input");
    }

    #[test]
    fn reconcile_child_sheet_pins_adds_new_and_removes_stale_auto_generated() {
        let mut document = test_sheet();
        document.child_sheets.push(ChildSheet {
            uuid: uuid::Uuid::new_v4(),
            name: "Child".to_string(),
            filename: "child.snxsch".to_string(),
            position: Point::new(10.0, 20.0),
            size: (30.0, 30.0),
            stroke_width: 0.12,
            fill: FillType::None,
            stroke_color: None,
            fill_color: None,
            fields_autoplaced: false,
            pins: vec![
                SheetPin {
                    uuid: uuid::Uuid::new_v4(),
                    name: "OLD_AUTO".to_string(),
                    direction: "input".to_string(),
                    position: Point::new(10.0, 22.0),
                    rotation: 0.0,
                    auto_generated: true,
                    user_moved: false,
                },
                SheetPin {
                    uuid: uuid::Uuid::new_v4(),
                    name: "MANUAL".to_string(),
                    direction: "input".to_string(),
                    position: Point::new(10.0, 24.0),
                    rotation: 0.0,
                    auto_generated: false,
                    user_moved: false,
                },
            ],
            instances: Vec::new(),
        });

        let mut engine = Engine::new(document).unwrap();
        let result = engine
            .execute(Command::ReconcileChildSheetPins {
                child_filename: "child.snxsch".to_string(),
                ports: vec![
                    SheetPort {
                        name: "SDA".to_string(),
                        direction: "input".to_string(),
                    },
                    SheetPort {
                        name: "SCL".to_string(),
                        direction: "output".to_string(),
                    },
                ],
            })
            .unwrap();

        assert!(result.changed);
        let pins = &engine.document().child_sheets[0].pins;
        assert!(
            pins.iter()
                .any(|pin| pin.name == "MANUAL" && !pin.auto_generated)
        );
        assert!(
            pins.iter()
                .any(|pin| pin.name == "SDA" && pin.auto_generated)
        );
        assert!(
            pins.iter()
                .any(|pin| pin.name == "SCL" && pin.auto_generated)
        );
        assert!(!pins.iter().any(|pin| pin.name == "OLD_AUTO"));
    }

    #[test]
    fn reconcile_preserves_position_for_user_moved_pin() {
        let mut document = test_sheet();
        let moved_uuid = uuid::Uuid::new_v4();
        document.child_sheets.push(ChildSheet {
            uuid: uuid::Uuid::new_v4(),
            name: "Child".to_string(),
            filename: "child.snxsch".to_string(),
            position: Point::new(10.0, 20.0),
            size: (30.0, 30.0),
            stroke_width: 0.12,
            fill: FillType::None,
            stroke_color: None,
            fill_color: None,
            fields_autoplaced: false,
            pins: vec![SheetPin {
                uuid: moved_uuid,
                name: "SDA".to_string(),
                direction: "input".to_string(),
                position: Point::new(25.0, 33.0),
                rotation: 90.0,
                auto_generated: true,
                user_moved: true,
            }],
            instances: Vec::new(),
        });

        let mut engine = Engine::new(document).unwrap();
        let _ = engine
            .execute(Command::ReconcileChildSheetPins {
                child_filename: "child.snxsch".to_string(),
                ports: vec![SheetPort {
                    name: "SDA".to_string(),
                    direction: "output".to_string(),
                }],
            })
            .unwrap();

        let pin = engine.document().child_sheets[0]
            .pins
            .iter()
            .find(|pin| pin.uuid == moved_uuid)
            .unwrap();
        assert_eq!(pin.position, Point::new(25.0, 33.0));
        assert_eq!(pin.rotation, 90.0);
        assert_eq!(pin.direction, "output");
    }

    #[test]
    fn moving_sheet_pin_locks_to_nearest_sheet_edge() {
        let mut document = test_sheet();
        let pin_uuid = uuid::Uuid::new_v4();
        let sheet_uuid = uuid::Uuid::new_v4();

        document.child_sheets.push(ChildSheet {
            uuid: sheet_uuid,
            name: "Child".to_string(),
            filename: "child.snxsch".to_string(),
            position: Point::new(10.0, 20.0),
            size: (30.0, 30.0),
            stroke_width: 0.12,
            fill: FillType::None,
            stroke_color: None,
            fill_color: None,
            fields_autoplaced: false,
            pins: vec![SheetPin {
                uuid: pin_uuid,
                name: "SDA".to_string(),
                direction: "input".to_string(),
                position: Point::new(10.0, 25.0),
                rotation: 0.0,
                auto_generated: true,
                user_moved: false,
            }],
            instances: Vec::new(),
        });

        let mut engine = Engine::new(document).unwrap();

        let _ = engine
            .execute(Command::MoveSelection {
                items: vec![SelectedItem::new(pin_uuid, SelectedKind::SheetPin)],
                dx: 35.0,
                dy: -100.0,
            })
            .unwrap();

        let moved = engine.document().child_sheets[0]
            .pins
            .iter()
            .find(|pin| pin.uuid == pin_uuid)
            .unwrap();

        assert_eq!(moved.position.x, 40.0);
        assert_eq!(moved.rotation, 180.0);
        assert_eq!(moved.position.y, 20.0 + GRID_MM);
        assert!(moved.user_moved);
    }

    #[test]
    fn set_paper_size_persists_no_ops_and_undoes() {
        let mut engine = Engine::new(test_sheet()).expect("engine");
        assert_eq!(engine.document().paper_size, "A4");

        // Change is applied, patched as PAPER, and recorded in history.
        let result = engine
            .execute(Command::SetPaperSize {
                paper_size: "A3".to_string(),
            })
            .expect("set paper size");
        assert!(result.changed);
        assert_eq!(
            result.patch_pair.expect("patch").document,
            DocumentPatch::PAPER
        );
        assert_eq!(engine.document().paper_size, "A3");

        // Same value again is a no-op (no history garbage).
        let result = engine
            .execute(Command::SetPaperSize {
                paper_size: "A3".to_string(),
            })
            .expect("no-op set");
        assert!(!result.changed);

        // Undo restores the previous format.
        engine.undo().expect("undo");
        assert_eq!(engine.document().paper_size, "A4");
    }

    fn wire(a: Point, b: Point) -> signex_types::schematic::Wire {
        signex_types::schematic::Wire {
            uuid: uuid::Uuid::new_v4(),
            start: a,
            end: b,
            stroke_width: 0.0,
        }
    }

    /// Drawing a stub and then a trunk through the stub's endpoint is the
    /// ordinary way a T gets drawn, and it used to leave no junction dot:
    /// `needed_junction` only ever inspected the *new* wire's own two
    /// endpoints. The netlist treats an undotted T as disconnected (issue
    /// #107), so the connection was silently lost (issue #402).
    #[test]
    fn a_trunk_drawn_through_an_existing_wires_endpoint_gets_a_junction() {
        let mut document = test_sheet();
        document
            .wires
            .push(wire(Point::new(5.0, 0.0), Point::new(5.0, 10.0)));
        let mut engine = Engine::new(document).expect("engine");

        engine
            .execute(Command::PlaceWireSegment {
                wire: wire(Point::new(0.0, 0.0), Point::new(10.0, 0.0)),
            })
            .expect("place trunk");

        let junctions = &engine.document().junctions;
        assert_eq!(junctions.len(), 1, "{junctions:?}");
        assert_eq!(junctions[0].position, Point::new(5.0, 0.0));
    }

    /// The negative twin: a trunk merely *crossing* another wire's interior is
    /// not a connection, so it must not mint a dot.
    #[test]
    fn a_trunk_crossing_another_wires_interior_gets_no_junction() {
        let mut document = test_sheet();
        document
            .wires
            .push(wire(Point::new(5.0, -5.0), Point::new(5.0, 5.0)));
        let mut engine = Engine::new(document).expect("engine");

        engine
            .execute(Command::PlaceWireSegment {
                wire: wire(Point::new(0.0, 0.0), Point::new(10.0, 0.0)),
            })
            .expect("place trunk");

        assert!(
            engine.document().junctions.is_empty(),
            "{:?}",
            engine.document().junctions
        );
    }

    /// Dragging a stub onto a trunk is at least as ordinary as drawing through
    /// one, and it produced the identical defect: `MoveSelection` mutates wire
    /// coordinates but reconciled no junctions, so the drag landed a real
    /// junction-less T the netlist reads as disconnected (issues #107, #402).
    /// Fixing only `PlaceWireSegment` left this sibling caller broken.
    #[test]
    fn dragging_a_stub_onto_a_trunks_interior_gets_a_junction() {
        let mut document = test_sheet();
        document
            .wires
            .push(wire(Point::new(0.0, 0.0), Point::new(10.0, 0.0)));
        let stub = wire(Point::new(5.0, 3.0), Point::new(5.0, 10.0));
        let stub_uuid = stub.uuid;
        document.wires.push(stub);
        let mut engine = Engine::new(document).expect("engine");

        let result = engine
            .execute(Command::MoveSelection {
                items: vec![SelectedItem {
                    kind: SelectedKind::Wire,
                    uuid: stub_uuid,
                }],
                dx: 0.0,
                dy: -3.0,
            })
            .expect("drag stub");

        let junctions = &engine.document().junctions;
        assert_eq!(junctions.len(), 1, "{junctions:?}");
        assert_eq!(junctions[0].position, Point::new(5.0, 0.0));
        assert!(
            result
                .patch_pair
                .expect("changed command carries a patch")
                .document
                .contains(DocumentPatch::JUNCTIONS),
            "a minted dot must be in the patch or the canvas never redraws it"
        );
    }

    /// The negative twin of the drag: sliding a stub so it merely *crosses* the
    /// trunk is not a connection either.
    #[test]
    fn dragging_a_stub_across_a_trunk_gets_no_junction() {
        let mut document = test_sheet();
        document
            .wires
            .push(wire(Point::new(0.0, 0.0), Point::new(10.0, 0.0)));
        let stub = wire(Point::new(5.0, 3.0), Point::new(5.0, 13.0));
        let stub_uuid = stub.uuid;
        document.wires.push(stub);
        let mut engine = Engine::new(document).expect("engine");

        engine
            .execute(Command::MoveSelection {
                items: vec![SelectedItem {
                    kind: SelectedKind::Wire,
                    uuid: stub_uuid,
                }],
                dx: 0.0,
                dy: -8.0,
            })
            .expect("drag stub");

        assert!(
            engine.document().junctions.is_empty(),
            "{:?}",
            engine.document().junctions
        );
    }

    /// A dot the netlist will not honour is worse than no dot: it asserts a
    /// connection the derivation refuses to make, with a reassuring visual.
    ///
    /// The stub's endpoint sits 5 µm off the trunk — inside the 0.01 mm float
    /// tolerance the geometry helpers use, but *not* exactly collinear in the
    /// netlist's 1 µm key space, so `SheetConnectivity` would drop the dot and
    /// leave the two wires on separate nets. Mint nothing rather than lie.
    #[test]
    fn an_endpoint_off_the_trunk_in_key_space_gets_no_lying_junction() {
        let mut document = test_sheet();
        document
            .wires
            .push(wire(Point::new(0.0, 0.0), Point::new(10.0, 0.0)));
        let mut engine = Engine::new(document).expect("engine");

        engine
            .execute(Command::PlaceWireSegment {
                wire: wire(Point::new(5.0, 0.005), Point::new(5.0, 10.0)),
            })
            .expect("place off-grid stub");

        assert!(
            engine.document().junctions.is_empty(),
            "dot minted where the netlist would not honour it: {:?}",
            engine.document().junctions
        );
    }
}
