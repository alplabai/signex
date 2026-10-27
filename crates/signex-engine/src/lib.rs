mod command;
mod error;
mod patch;

mod annotation;
mod history;
mod net_class_table;
mod project_general;
mod selection;
mod sheet;
mod sheet_identity;
mod sheet_placement;
mod transform;

mod exec;

#[cfg(test)]
pub(crate) mod test_support;

#[cfg(test)]
mod annotation_tests;

#[cfg(test)]
mod rubber_band_power_tests;
#[cfg(test)]
mod rubber_band_tests;
#[cfg(test)]
mod rubber_band_turn_tests;

#[cfg(test)]
mod sheet_identity_tests;

use std::path::{Path, PathBuf};

pub use annotation::AnnotateScope;
pub use command::{
    AnnotateMode, Command, CommandKind, MirrorAxis, ReorderDirection, SelectionTransform,
    SheetPort, SymbolTextField, TextTarget,
};
pub use error::EngineError;
pub use exec::pin_changes::{SwitchBlockers, switch_blockers};
use history::HistoryEntry;
pub use history::UndoGroupId;
pub use net_class_table::{
    HISTORY_LIMIT as NET_CLASS_HISTORY_LIMIT, NetClassCommand, NetClassError, NetClassHistory,
    StepId as NetClassStepId, apply_net_class_command,
};
pub use patch::{CommandResult, DocumentPatch, PatchPair, SemanticPatch};
pub use project_general::{
    HISTORY_LIMIT as PROJECT_GENERAL_HISTORY_LIMIT, ProjectGeneralCommand, ProjectGeneralHistory,
};
pub use selection::{
    ClipboardSelection, SelectionAnchor, SelectionDetails, info_keys, partition_cuttable,
};
pub use sheet::{
    blank_port_label_count, exposed_sheet_ports, ports_for_pin_sync, unplaced_sheet_ports,
};
pub use sheet_placement::{
    DEFAULT_CHILD_SHEET_STROKE_MM, MIN_CHILD_SHEET_EDGE_MM, child_sheet_from_corners,
    unused_sheet_stem,
};
use signex_types::schematic::{SchematicSheet, SelectedItem};
pub use transform::rubber_band::{
    BeforeNets, Follow, Guard, PinIdent, PinTip, Policy, before_nets, follow_moved_pins,
    moved_pin_tips,
};

const JUNCTION_TOLERANCE_MM: f64 = 0.01;

#[derive(Debug)]
pub struct Engine {
    document: SchematicSheet,
    path: Option<PathBuf>,
    history: std::collections::VecDeque<HistoryEntry>,
    redo_stack: Vec<HistoryEntry>,
    /// Entries evicted from the front of `history` by the cap, so
    /// `undo_position` keeps counting up past `MAX_HISTORY_ENTRIES`.
    history_evicted: usize,
    /// Set for the duration of an `execute_batch` loop so the commands in
    /// the batch do not each land their own history entry — the batch
    /// records one coalesced entry when the loop finishes. Always `false`
    /// outside that loop, so every other path behaves exactly as before.
    /// See `exec/batch.rs`.
    history_suppressed: bool,
    /// Re-drawn from a process-wide counter by every entry point that can
    /// change `document` (and at construction), so an off-thread save can
    /// tell whether the document moved on while it ran, even if the engine
    /// was replaced by a fresh one meanwhile.
    revision: u64,
    /// The value `revision` held before the most recent edit entry point
    /// (a batch counts as one). A render cache stamped with this value was
    /// in step with the document just before that edit, so it may copy only
    /// the collections the edit's patch reports.
    base_revision: u64,
    /// Re-drawn only by an edit that can change which wires, junctions and
    /// pins make up a net or what a net is called (see
    /// [`DocumentPatch::ELECTRICAL`]); a cache of anything derived from
    /// connectivity may keep its answer while this holds.
    connectivity_revision: u64,
    /// Fixed for this engine's lifetime and unique across engines: tells a
    /// reopened sheet from the one an off-thread job started on.
    instance: u64,
    /// Notices raised by the last commands (rubber-band pins that could
    /// not be kept connected); the app drains them into the Messages panel.
    warnings: Vec<String>,
    /// The pins behind the last notice, one "R1 pin 2" each.
    unkept_pins: Vec<String>,
    /// Pre-move nets computed ahead of time (drag previews).
    preset_before: Option<BeforeNets>,
}

/// Process-wide revision source: values are never reused across engines.
pub(crate) fn next_revision() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// A sheet prepared for repeated drag previews: the pre-move nets are derived
/// once, not for every cursor step.
#[derive(Debug)]
pub struct MovePreview {
    sheet: SchematicSheet,
    before: BeforeNets,
}

impl MovePreview {
    #[must_use]
    pub fn new(sheet: &SchematicSheet) -> Self {
        Self {
            before: before_nets(sheet),
            sheet: sheet.clone(),
        }
    }

    /// The sheet as `MoveEach` would leave it: the same pass the commit runs
    /// (rubber-banding, junction reconcile and the net guard included), on a
    /// throwaway engine. Warnings are dropped.
    #[must_use]
    pub fn moved(&self, moves: &[(SelectedItem, f64, f64)], keep_wires: bool) -> SchematicSheet {
        let Ok(mut engine) = Engine::new(self.sheet.clone()) else {
            return self.sheet.clone();
        };
        engine.preset_before = Some(self.before.clone());
        let done = engine.execute(Command::MoveEach {
            moves: moves.to_vec(),
            keep_wires,
        });
        if done.is_err() {
            return self.sheet.clone();
        }
        engine.document().clone()
    }
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
            history: std::collections::VecDeque::new(),
            redo_stack: Vec::new(),
            history_evicted: 0,
            history_suppressed: false,
            revision: crate::next_revision(),
            base_revision: 0,
            connectivity_revision: crate::next_revision(),
            instance: crate::next_revision(),
            warnings: Vec::new(),
            unkept_pins: Vec::new(),
            preset_before: None,
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
            history: std::collections::VecDeque::new(),
            redo_stack: Vec::new(),
            history_evicted: 0,
            history_suppressed: false,
            revision: crate::next_revision(),
            base_revision: 0,
            connectivity_revision: crate::next_revision(),
            instance: crate::next_revision(),
            warnings: Vec::new(),
            unkept_pins: Vec::new(),
            preset_before: None,
        })
    }

    /// The pins the last commands could not keep connected (drained).
    pub fn take_unkept_pins(&mut self) -> Vec<String> {
        std::mem::take(&mut self.unkept_pins)
    }

    /// Drain the notices raised since the last call.
    pub fn take_warnings(&mut self) -> Vec<String> {
        std::mem::take(&mut self.warnings)
    }

    pub fn save(&mut self) -> Result<(), EngineError> {
        let Some(path) = self.path.clone() else {
            return Err(EngineError::MissingPath);
        };

        self.save_as(&path)
    }

    /// The current document encoded as `.snxsch` text: the consistent
    /// snapshot an off-thread save writes. Pair it with [`Self::revision`].
    pub fn serialize(&self) -> Result<String, EngineError> {
        Self::encode(self.document.clone())
    }

    fn encode(sheet: SchematicSheet) -> Result<String, EngineError> {
        signex_types::format::SnxSchematic::new(sheet)
            .write_string()
            // The typed `FormatError` stays reachable through the io error's
            // `get_ref()`, so the app can list the objects a refusal names.
            .map_err(|error| EngineError::SaveFailed(std::io::Error::other(error)))
    }

    /// Changes whenever the document may have changed (any command, undo,
    /// redo, annotate or replace). Equal revisions mean an unchanged document;
    /// revisions are unique across engine instances.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// The revision the document had before the most recent edit entry
    /// point ran; `0` before any edit.
    pub fn base_revision(&self) -> u64 {
        self.base_revision
    }

    /// Changes whenever the document's connectivity may have changed: the
    /// same entry points as [`Self::revision`], minus the edits whose patch
    /// touches nothing a net is derived from (a text note, a drawing, the
    /// paper). Unique across engine instances.
    pub fn connectivity_revision(&self) -> u64 {
        self.connectivity_revision
    }

    pub(crate) fn bump_connectivity_revision(&mut self) {
        self.connectivity_revision = crate::next_revision();
    }

    /// Move [`Self::connectivity_revision`] after a command, unless its patch
    /// proves it left connectivity alone. An error counts as a change: the
    /// command may have run part-way before it failed.
    pub(crate) fn note_command_result(&mut self, result: &Result<CommandResult, EngineError>) {
        let touches = match result {
            Ok(done) => {
                done.changed
                    && done
                        .patch_pair
                        .is_none_or(|pair| pair.document.intersects(DocumentPatch::ELECTRICAL))
            }
            Err(_) => true,
        };
        if touches {
            self.bump_connectivity_revision();
        }
    }

    pub(crate) fn bump_revision(&mut self) {
        self.base_revision = self.revision;
        self.revision = crate::next_revision();
    }

    /// Identity of this engine instance; unlike [`Self::revision`] it does
    /// not move on edits.
    pub fn instance_id(&self) -> u64 {
        self.instance
    }

    /// [`Self::serialize`] with `colors` as the sheet's net colour overrides.
    /// The app keeps the live overrides outside the document (they are not
    /// part of the undo history), so a save passes them in. Overrides whose
    /// wire or junction is no longer in the sheet are dropped here.
    pub fn serialize_with_net_colors(
        &self,
        colors: &std::collections::HashMap<uuid::Uuid, signex_types::theme::Color>,
    ) -> Result<String, EngineError> {
        let mut sheet = self.document.clone();
        let live: std::collections::HashSet<uuid::Uuid> = sheet
            .wires
            .iter()
            .map(|w| w.uuid)
            .chain(sheet.junctions.iter().map(|j| j.uuid))
            .collect();
        sheet.net_colors = colors
            .iter()
            .filter(|(id, _)| live.contains(id))
            .map(|(id, color)| (*id, *color))
            .collect();
        Self::encode(sheet)
    }

    /// [`Self::save`] writing `colors` as the net colour overrides; see
    /// [`Self::serialize_with_net_colors`].
    pub fn save_with_net_colors(
        &mut self,
        colors: &std::collections::HashMap<uuid::Uuid, signex_types::theme::Color>,
    ) -> Result<(), EngineError> {
        let Some(path) = self.path.clone() else {
            return Err(EngineError::MissingPath);
        };
        let content = self.serialize_with_net_colors(colors)?;
        signex_types::atomic_io::atomic_write(&path, content.as_bytes())
            .map_err(EngineError::SaveFailed)
    }

    pub fn save_as(&mut self, path: &Path) -> Result<(), EngineError> {
        let content = self.serialize()?;
        // HI-6: atomic write — a crash mid-save no longer truncates the
        // destination. The user's prior file stays intact until the
        // rename succeeds.
        signex_types::atomic_io::atomic_write(path, content.as_bytes())
            .map_err(EngineError::SaveFailed)?;
        self.path = Some(path.to_path_buf());
        Ok(())
    }

    pub fn execute(&mut self, cmd: Command) -> Result<CommandResult, EngineError> {
        self.bump_revision();
        let before = self.document.clone();

        let result = match cmd {
            Command::ReplaceDocument { .. }
            | Command::UpdateText { .. }
            | Command::UpdateLabelProps { .. }
            | Command::SetSymbolRotation { .. }
            | Command::SetSymbolLocked { .. }
            | Command::SetSymbolDnp { .. }
            | Command::UpdateSymbolTextSize { .. }
            | Command::UpdateSymbolLibId { .. }
            | Command::UpdateSymbolFootprint { .. }
            | Command::SetSymbolField { .. }
            | Command::UpdateSymbolFields { .. }
            | Command::SetTextFontFamily { .. }
            | Command::SetSymbolDesignatorIn { .. } => self.exec_edits(before, cmd),
            Command::DeleteSelection { .. }
            | Command::MoveSelection { .. }
            | Command::MoveEach { .. }
            | Command::RotateSelection { .. }
            | Command::MirrorSelection { .. }
            | Command::TransformSelection { .. }
            | Command::PlaceBus { .. }
            | Command::PlaceLabel { .. }
            | Command::PlaceSymbol { .. }
            | Command::RegisterLibSymbol { .. }
            | Command::ReplaceLibSymbol { .. }
            | Command::PlaceChildSheet { .. }
            | Command::PlaceWireSegment { .. }
            | Command::PlaceJunction { .. }
            | Command::PlaceNoConnect { .. }
            | Command::PlaceNoErcDirective { .. }
            | Command::PlaceBusEntry { .. }
            | Command::PlaceTextNote { .. }
            | Command::PlaceSchDrawing { .. } => self.exec_place(before, cmd),
            Command::UpdateSchDrawing { .. }
            | Command::UpdateChildSheetStyle { .. }
            | Command::AnnotateAll { .. }
            | Command::MoveSymbolAbsolute { .. }
            | Command::ReorderObjects { .. }
            | Command::ReconcileChildSheetPins { .. }
            | Command::RenameChildSheetFile { .. }
            | Command::ModifyChildSheet { .. }
            | Command::SetPaperSize { .. }
            | Command::SetSheetFrame { .. }
            | Command::SetTitleBlockField { .. }
            | Command::SetDocumentFontFamily { .. } => self.exec_structure(before, cmd),
        };
        self.note_command_result(&result);
        result
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
        self.bump_revision();
        self.bump_connectivity_revision();
        self.document = document;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::test_sheet;
    use signex_types::schematic::{
        BusEntry, ChildSheet, FillType, GRID_MM, Label, LabelType, Point, SelectedItem,
        SelectedKind, SheetPin,
    };

    #[test]
    fn every_mutating_entry_point_moves_the_revision() {
        use crate::command::AnnotateMode;
        use signex_types::schematic::AnnotateOrder;
        let mut engine = Engine::new(test_sheet()).unwrap();
        let mut last = engine.revision();
        let mut moved = |engine: &Engine, what: &str| {
            assert_ne!(engine.revision(), last, "{what} must move the revision");
            last = engine.revision();
        };
        engine
            .execute(Command::SetPaperSize {
                paper_size: "A3".into(),
            })
            .unwrap();
        moved(&engine, "execute");
        engine
            .execute_batch([Command::SetPaperSize {
                paper_size: "A2".into(),
            }])
            .unwrap();
        moved(&engine, "execute_batch");
        engine.undo().unwrap();
        moved(&engine, "undo");
        engine.redo().unwrap();
        moved(&engine, "redo");
        engine
            .annotate_with_seed(
                AnnotateMode::Incremental,
                &mut Default::default(),
                AnnotateOrder::UpThenAcross,
            )
            .unwrap();
        moved(&engine, "annotate");
        engine.set_document(test_sheet());
        moved(&engine, "set_document");
    }

    #[test]
    fn only_electrical_edits_move_the_connectivity_revision() {
        use signex_types::schematic::Wire;
        let mut sheet = test_sheet();
        let wire = Wire {
            uuid: uuid::Uuid::from_u128(7),
            start: Point::new(0.0, 0.0),
            end: Point::new(10.0, 0.0),
            ..Default::default()
        };
        sheet.wires.push(wire.clone());
        let mut engine = Engine::new(sheet).unwrap();
        let start = engine.connectivity_revision();

        engine
            .execute(Command::SetPaperSize {
                paper_size: "A3".into(),
            })
            .unwrap();
        assert_ne!(engine.revision(), 0);
        assert_eq!(
            engine.connectivity_revision(),
            start,
            "paper size is not electrical"
        );
        engine.undo().unwrap();
        assert_eq!(engine.connectivity_revision(), start, "nor is its undo");

        engine
            .execute(Command::DeleteSelection {
                items: vec![SelectedItem::new(wire.uuid, SelectedKind::Wire)],
            })
            .unwrap();
        let after_delete = engine.connectivity_revision();
        assert_ne!(after_delete, start, "deleting a wire changes connectivity");
        engine.undo().unwrap();
        assert_ne!(
            engine.connectivity_revision(),
            after_delete,
            "undoing it changes connectivity back"
        );
        let before_set = engine.connectivity_revision();
        engine.set_document(test_sheet());
        assert_ne!(engine.connectivity_revision(), before_set);
    }

    #[test]
    fn revisions_are_unique_across_engine_instances() {
        let a = Engine::new(test_sheet()).unwrap();
        let b = Engine::new(test_sheet()).unwrap();
        assert_ne!(a.revision(), b.revision());
    }

    #[test]
    fn collect_exposed_sheet_ports_prefers_hierarchical_over_global() {
        let mut document = test_sheet();
        document.labels.push(Label {
            unknown_columns: Default::default(),
            uuid: uuid::Uuid::new_v4(),
            text: "ALERT".to_string(),
            position: Point::new(0.0, 0.0),
            rotation: 0.0,
            label_type: LabelType::Global,
            shape: "output".to_string(),
            font_size: 1.27,
            justify: signex_types::schematic::HAlign::Left,
            justify_v: signex_types::schematic::VAlign::Bottom,
            font_family: None,
        });
        document.labels.push(Label {
            unknown_columns: Default::default(),
            uuid: uuid::Uuid::new_v4(),
            text: "ALERT".to_string(),
            position: Point::new(1.0, 1.0),
            rotation: 0.0,
            label_type: LabelType::Hierarchical,
            shape: "input".to_string(),
            font_size: 1.27,
            justify: signex_types::schematic::HAlign::Left,
            justify_v: signex_types::schematic::VAlign::Bottom,
            font_family: None,
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
        assert_eq!(pin.direction, "input", "reconcile never rewrites a pin");
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
                keep_wires: false,
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

    fn test_sheet_pin(name: &str) -> SheetPin {
        SheetPin {
            uuid: uuid::Uuid::new_v4(),
            name: name.to_string(),
            direction: "input".to_string(),
            position: Point::new(10.0, 22.0),
            rotation: 0.0,
            auto_generated: false,
            user_moved: false,
        }
    }

    fn test_child_sheet(pins: Vec<SheetPin>) -> ChildSheet {
        ChildSheet {
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
            pins,
            instances: Vec::new(),
        }
    }

    #[test]
    fn delete_selection_removes_child_sheet_and_its_pins() {
        let mut document = test_sheet();
        let sheet = test_child_sheet(vec![test_sheet_pin("SDA"), test_sheet_pin("SCL")]);
        let sheet_uuid = sheet.uuid;
        document.child_sheets.push(sheet);

        let mut engine = Engine::new(document).unwrap();
        let result = engine
            .execute(Command::DeleteSelection {
                items: vec![SelectedItem::new(sheet_uuid, SelectedKind::ChildSheet)],
            })
            .unwrap();

        assert!(result.changed);
        assert!(engine.document().child_sheets.is_empty());
    }

    #[test]
    fn delete_selection_removes_only_the_targeted_sheet_pin() {
        let mut document = test_sheet();
        let keep_pin = test_sheet_pin("SDA");
        let keep_uuid = keep_pin.uuid;
        let doomed_pin = test_sheet_pin("SCL");
        let doomed_uuid = doomed_pin.uuid;
        let sheet = test_child_sheet(vec![keep_pin, doomed_pin]);
        let sheet_uuid = sheet.uuid;
        document.child_sheets.push(sheet);

        let mut engine = Engine::new(document).unwrap();
        let result = engine
            .execute(Command::DeleteSelection {
                items: vec![SelectedItem::new(doomed_uuid, SelectedKind::SheetPin)],
            })
            .unwrap();

        assert!(result.changed);
        assert_eq!(engine.document().child_sheets.len(), 1);
        assert_eq!(engine.document().child_sheets[0].uuid, sheet_uuid);
        let pins = &engine.document().child_sheets[0].pins;
        assert_eq!(pins.len(), 1);
        assert_eq!(pins[0].uuid, keep_uuid);
    }

    #[test]
    fn delete_selection_of_child_sheet_and_sheet_pin_are_undoable() {
        let mut document = test_sheet();
        let sheet = test_child_sheet(vec![test_sheet_pin("SDA"), test_sheet_pin("SCL")]);
        let sheet_uuid = sheet.uuid;
        let pin_uuid = sheet.pins[0].uuid;
        document.child_sheets.push(sheet);

        let mut engine = Engine::new(document).unwrap();

        // Deleting the pin lands a history entry that undo() reverts.
        engine
            .execute(Command::DeleteSelection {
                items: vec![SelectedItem::new(pin_uuid, SelectedKind::SheetPin)],
            })
            .unwrap();
        assert_eq!(engine.document().child_sheets[0].pins.len(), 1);
        assert!(engine.can_undo());
        engine.undo().unwrap();
        assert_eq!(engine.document().child_sheets[0].pins.len(), 2);

        // Deleting the sheet itself is likewise undoable.
        engine
            .execute(Command::DeleteSelection {
                items: vec![SelectedItem::new(sheet_uuid, SelectedKind::ChildSheet)],
            })
            .unwrap();
        assert!(engine.document().child_sheets.is_empty());
        engine.undo().unwrap();
        assert_eq!(engine.document().child_sheets.len(), 1);
        assert_eq!(engine.document().child_sheets[0].uuid, sheet_uuid);
        assert_eq!(engine.document().child_sheets[0].pins.len(), 2);
    }

    #[test]
    fn has_selected_items_recognizes_child_sheet_and_pin() {
        // Regression for review finding #3 on PR fixing #341:
        // `contains_selected_item` gates `has_selected_items`, which the
        // app uses to decide whether Delete is even armed
        // (handlers/editing_commands.rs). None of the DeleteSelection
        // tests above route through it — `Command::execute` deletes
        // unconditionally — so a refactor could drop the ChildSheet /
        // SheetPin arms with every other test still green. This asserts
        // the gate itself.
        let mut document = test_sheet();
        let pin = test_sheet_pin("SDA");
        let pin_uuid = pin.uuid;
        let sheet = test_child_sheet(vec![pin]);
        let sheet_uuid = sheet.uuid;
        document.child_sheets.push(sheet);

        let engine = Engine::new(document).unwrap();

        assert!(
            engine.has_selected_items(&[SelectedItem::new(sheet_uuid, SelectedKind::ChildSheet)])
        );
        assert!(engine.has_selected_items(&[SelectedItem::new(pin_uuid, SelectedKind::SheetPin)]));
    }

    #[test]
    fn delete_selection_removes_bus_entry() {
        // Review finding #2: BusEntry was selectable, hit-testable and
        // movable but fell through both `contains_selected_item` and
        // `remove_selected_item`'s `_ => false` — the same silent-no-op
        // bug #341 reports for sheets, just for a different kind.
        let mut document = test_sheet();
        let bus_entry = BusEntry {
            uuid: uuid::Uuid::new_v4(),
            position: Point::new(5.0, 5.0),
            size: (2.54, 2.54),
        };
        let bus_entry_uuid = bus_entry.uuid;
        document.bus_entries.push(bus_entry);

        let mut engine = Engine::new(document).unwrap();
        assert!(
            engine.has_selected_items(&[SelectedItem::new(bus_entry_uuid, SelectedKind::BusEntry)])
        );

        let result = engine
            .execute(Command::DeleteSelection {
                items: vec![SelectedItem::new(bus_entry_uuid, SelectedKind::BusEntry)],
            })
            .unwrap();

        assert!(result.changed);
        assert!(engine.document().bus_entries.is_empty());
    }

    #[test]
    fn partition_cuttable_keeps_child_sheet_and_pin_out_of_cut() {
        // Review finding #1: Cut (copy + delete) must not destroy a kind
        // `collect_selection_clipboard` silently drops. `partition_cuttable`
        // is what `handle_selection_cut_requested` (signex-app) uses to
        // keep the two in sync.
        let symbol_item = SelectedItem::new(uuid::Uuid::new_v4(), SelectedKind::Symbol);
        let sheet_item = SelectedItem::new(uuid::Uuid::new_v4(), SelectedKind::ChildSheet);
        let pin_item = SelectedItem::new(uuid::Uuid::new_v4(), SelectedKind::SheetPin);

        let (cuttable, kept) =
            crate::selection::partition_cuttable(&[symbol_item, sheet_item, pin_item]);

        assert_eq!(cuttable, vec![symbol_item]);
        assert_eq!(kept, vec![sheet_item, pin_item]);
    }

    #[test]
    fn deleting_a_port_backed_sheet_pin_does_not_survive_reconcile() {
        // Review finding #4 (comment correction): documents the actual
        // interaction between SheetPin delete and
        // `reconcile_child_sheet_pins` (sheet.rs; not yet wired into the
        // app — #359). Reconcile derives pins from currently-exposed
        // ports by name, so deleting a pin whose port is still exposed
        // does not stick — the next reconcile recreates it with a fresh
        // uuid.
        let pin = test_sheet_pin("SDA");
        let original_uuid = pin.uuid;
        let sheet = test_child_sheet(vec![pin]);
        let sheet_uuid = sheet.uuid;
        let mut document = test_sheet();
        document.child_sheets.push(sheet);

        let mut engine = Engine::new(document).unwrap();
        engine
            .execute(Command::DeleteSelection {
                items: vec![SelectedItem::new(original_uuid, SelectedKind::SheetPin)],
            })
            .unwrap();
        assert!(engine.document().child_sheets[0].pins.is_empty());

        // "SDA" is still an exposed port (that comes from the child
        // sheet's own hierarchical labels, unrelated to the pin delete),
        // so reconcile regenerates it.
        let mut child = engine.document().child_sheets[0].clone();
        assert_eq!(child.uuid, sheet_uuid);
        let ports = [SheetPort {
            name: "SDA".to_string(),
            direction: "input".to_string(),
        }];
        let changed = crate::sheet::reconcile_child_sheet_pins(
            &mut child,
            &ports,
            &signex_net::SheetPinContacts::default(),
        );

        assert!(changed);
        assert_eq!(child.pins.len(), 1);
        assert_ne!(
            child.pins[0].uuid, original_uuid,
            "delete does not survive reconcile once #359 wires it in"
        );
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

    #[test]
    fn set_sheet_frame_clamps_no_ops_and_undoes() {
        use signex_types::schematic::{MAX_ZONES, SheetFrameSettings};
        let mut engine = Engine::new(test_sheet()).expect("engine");
        let wanted = SheetFrameSettings {
            show_border: false,
            horizontal_zones: u8::MAX,
            ..SheetFrameSettings::default()
        };

        let result = engine
            .execute(Command::SetSheetFrame { settings: wanted })
            .expect("set frame");
        assert!(result.changed);
        assert_eq!(
            result.patch_pair.expect("patch").document,
            DocumentPatch::PAPER
        );
        assert!(!engine.document().frame.show_border);
        assert_eq!(engine.document().frame.horizontal_zones, MAX_ZONES);

        // The same settings again, clamped or not, change nothing.
        let result = engine
            .execute(Command::SetSheetFrame { settings: wanted })
            .expect("no-op set");
        assert!(!result.changed);

        engine.undo().expect("undo");
        assert_eq!(engine.document().frame, SheetFrameSettings::default());
    }

    #[test]
    fn set_title_block_field_persists_no_ops_clears_and_undoes() {
        let mut engine = Engine::new(test_sheet()).expect("engine");
        let set = |engine: &mut Engine, value: &str| {
            engine
                .execute(Command::SetTitleBlockField {
                    key: "drawn_by".to_string(),
                    value: value.to_string(),
                })
                .expect("set title block field")
        };
        let field = |engine: &Engine| engine.document().title_block.get("drawn_by").cloned();

        let result = set(&mut engine, "A. Engineer");
        assert!(result.changed);
        assert_eq!(
            result.patch_pair.expect("patch").document,
            DocumentPatch::PAPER
        );
        assert_eq!(field(&engine).as_deref(), Some("A. Engineer"));

        // Same value again is a no-op, and so is clearing an absent key.
        assert!(!set(&mut engine, "A. Engineer").changed);

        // An empty value removes the key, so the file stays clean.
        assert!(set(&mut engine, "").changed);
        assert_eq!(field(&engine), None);
        assert!(!set(&mut engine, "").changed);

        // Undo walks back through both edits.
        engine.undo().expect("undo");
        assert_eq!(field(&engine).as_deref(), Some("A. Engineer"));
        engine.undo().expect("undo");
        assert_eq!(field(&engine), None);
    }

    fn wire(a: Point, b: Point) -> signex_types::schematic::Wire {
        signex_types::schematic::Wire {
            uuid: uuid::Uuid::new_v4(),
            start: a,
            end: b,
            ..Default::default()
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
                keep_wires: false,
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
                keep_wires: false,
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

    /// Issue #422: the old add-only reconcile left a minted dot in place once
    /// the T that justified it lost its stub — the dot then sat electrically
    /// inert on the trunk alone until some unrelated wire was later dragged
    /// to merely cross the same point, at which point the stale dot silently
    /// merged two nets the user never connected. Dragging the stub away must
    /// remove the dot immediately, not wait for a crossing wire to expose it.
    #[test]
    fn dragging_a_minted_stub_off_the_trunk_removes_the_stale_dot() {
        let mut document = test_sheet();
        document
            .wires
            .push(wire(Point::new(0.0, 0.0), Point::new(10.0, 0.0)));
        let stub = wire(Point::new(5.0, 3.0), Point::new(5.0, 10.0));
        let stub_uuid = stub.uuid;
        document.wires.push(stub);
        let mut engine = Engine::new(document).expect("engine");

        // Drag the stub onto the trunk — mints a dot at (5, 0), same as
        // `dragging_a_stub_onto_a_trunks_interior_gets_a_junction` above.
        engine
            .execute(Command::MoveSelection {
                items: vec![SelectedItem {
                    kind: SelectedKind::Wire,
                    uuid: stub_uuid,
                }],
                dx: 0.0,
                dy: -3.0,
                keep_wires: false,
            })
            .expect("drag stub onto trunk");
        assert_eq!(engine.document().junctions.len(), 1);
        assert!(engine.document().junctions[0].minted);

        // Drag the same stub away again: the T it justified is gone, so the
        // now-stale dot must be removed rather than left to assert a
        // connection the wires no longer make.
        let result = engine
            .execute(Command::MoveSelection {
                items: vec![SelectedItem {
                    kind: SelectedKind::Wire,
                    uuid: stub_uuid,
                }],
                dx: 0.0,
                dy: 20.0,
                keep_wires: false,
            })
            .expect("drag stub away");

        assert!(
            engine.document().junctions.is_empty(),
            "{:?}",
            engine.document().junctions
        );
        assert!(
            result
                .patch_pair
                .expect("changed command carries a patch")
                .document
                .contains(DocumentPatch::JUNCTIONS),
            "a removed dot must be in the patch or the canvas keeps drawing it"
        );
    }

    /// The other half of #422: a user-placed dot is user data. It must never
    /// be swept up by the same removal pass just because it doesn't happen to
    /// sit on a wire meeting.
    #[test]
    fn a_user_placed_junction_survives_reconcile_even_when_unjustified() {
        let mut document = test_sheet();
        document
            .wires
            .push(wire(Point::new(0.0, 0.0), Point::new(10.0, 0.0)));
        let mut engine = Engine::new(document).expect("engine");

        let user_junction_uuid = uuid::Uuid::new_v4();
        engine
            .execute(Command::PlaceJunction {
                junction: signex_types::schematic::Junction {
                    uuid: user_junction_uuid,
                    position: Point::new(50.0, 50.0),
                    ..Default::default()
                },
            })
            .expect("place user junction");

        // Any wire-geometry command runs reconcile. The user's dot sits
        // nowhere near a wire meeting and would fail the mint test outright,
        // but `minted == false` must keep it out of the removal pass.
        let trunk_uuid = engine.document().wires[0].uuid;
        engine
            .execute(Command::MoveSelection {
                items: vec![SelectedItem {
                    kind: SelectedKind::Wire,
                    uuid: trunk_uuid,
                }],
                dx: 1.0,
                dy: 1.0,
                keep_wires: false,
            })
            .expect("move trunk");

        assert!(
            engine
                .document()
                .junctions
                .iter()
                .any(|j| j.uuid == user_junction_uuid),
            "user-placed dot must never be auto-removed"
        );
    }

    /// Issue #422's root-cause gap: the stale-dot removal pass ran only via
    /// `reconciled_patch()` for move/rotate/mirror. `DeleteSelection` and
    /// `PlaceWireSegment` never reconciled, so a dot minted for a genuine T
    /// could outlive the wire that justified it, then get silently
    /// "reactivated" by an unrelated wire merely crossing the same point —
    /// merging two nets the user never connected (a FAB short).
    #[test]
    fn deleting_a_stub_removes_the_stale_dot_so_a_later_crossing_wire_cannot_merge_nets() {
        let mut engine = Engine::new(test_sheet()).expect("engine");

        engine
            .execute(Command::PlaceWireSegment {
                wire: wire(Point::new(0.0, 0.0), Point::new(10.0, 0.0)),
            })
            .expect("place trunk");

        let stub = wire(Point::new(5.0, 0.0), Point::new(5.0, 10.0));
        let stub_uuid = stub.uuid;
        engine
            .execute(Command::PlaceWireSegment { wire: stub })
            .expect("place stub");
        assert_eq!(
            engine.document().junctions.len(),
            1,
            "{:?}",
            engine.document().junctions
        );
        assert_eq!(
            engine.document().junctions[0].position,
            Point::new(5.0, 0.0)
        );

        engine
            .execute(Command::DeleteSelection {
                items: vec![SelectedItem::new(stub_uuid, SelectedKind::Wire)],
            })
            .expect("delete stub");
        assert!(
            engine.document().junctions.is_empty(),
            "delete must reconcile the now-orphaned dot: {:?}",
            engine.document().junctions
        );

        engine
            .execute(Command::PlaceWireSegment {
                wire: wire(Point::new(5.0, -5.0), Point::new(5.0, 5.0)),
            })
            .expect("place crossing wire");

        assert!(
            engine.document().junctions.is_empty(),
            "a wire merely crossing the old dot's location must not silently \
             merge the trunk and crossing-wire nets: {:?}",
            engine.document().junctions
        );
    }

    #[test]
    fn delete_child_sheet_and_undo() {
        let mut document = test_sheet();
        let child_sheet_uuid = uuid::Uuid::new_v4();
        document.child_sheets.push(ChildSheet {
            uuid: child_sheet_uuid,
            name: "ChildSheet".to_string(),
            filename: "child.snxsch".to_string(),
            position: Point::new(10.0, 20.0),
            size: (30.0, 30.0),
            stroke_width: 0.12,
            fill: FillType::None,
            stroke_color: None,
            fill_color: None,
            fields_autoplaced: false,
            pins: Vec::new(),
            instances: Vec::new(),
        });

        let mut engine = Engine::new(document).unwrap();
        assert_eq!(engine.document().child_sheets.len(), 1);
        assert_eq!(engine.document().child_sheets[0].uuid, child_sheet_uuid);

        // Delete the child sheet
        let result = engine.execute(Command::DeleteSelection {
            items: vec![SelectedItem::new(
                child_sheet_uuid,
                SelectedKind::ChildSheet,
            )],
        });
        assert!(result.unwrap().changed);
        assert_eq!(engine.document().child_sheets.len(), 0);

        // Undo the deletion
        engine.undo().expect("undo");
        assert_eq!(engine.document().child_sheets.len(), 1);
        assert_eq!(engine.document().child_sheets[0].uuid, child_sheet_uuid);
        // Ensure the restored sheet matches the original
        assert_eq!(engine.document().child_sheets[0].name, "ChildSheet");
        assert_eq!(engine.document().child_sheets[0].filename, "child.snxsch");
    }

    #[test]
    fn delete_sheet_pin_and_undo() {
        let mut document = test_sheet();
        let child_sheet_uuid = uuid::Uuid::new_v4();
        let pin_uuid = uuid::Uuid::new_v4();
        document.child_sheets.push(ChildSheet {
            uuid: child_sheet_uuid,
            name: "ChildSheet".to_string(),
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
                name: "Pin1".to_string(),
                direction: "input".to_string(),
                position: Point::new(15.0, 25.0),
                rotation: 0.0,
                auto_generated: false,
                user_moved: false,
            }],
            instances: Vec::new(),
        });

        let mut engine = Engine::new(document).unwrap();
        assert_eq!(engine.document().child_sheets.len(), 1);
        assert_eq!(engine.document().child_sheets[0].pins.len(), 1);
        assert_eq!(engine.document().child_sheets[0].pins[0].uuid, pin_uuid);

        // Delete the sheet pin
        let result = engine.execute(Command::DeleteSelection {
            items: vec![SelectedItem::new(pin_uuid, SelectedKind::SheetPin)],
        });
        assert!(result.unwrap().changed);
        assert_eq!(engine.document().child_sheets.len(), 1);
        assert_eq!(engine.document().child_sheets[0].pins.len(), 0);

        // Undo the deletion
        engine.undo().expect("undo");
        assert_eq!(engine.document().child_sheets.len(), 1);
        assert_eq!(engine.document().child_sheets[0].pins.len(), 1);
        assert_eq!(engine.document().child_sheets[0].pins[0].uuid, pin_uuid);
        // Ensure the restored pin matches the original
        assert_eq!(engine.document().child_sheets[0].pins[0].name, "Pin1");
        assert_eq!(engine.document().child_sheets[0].pins[0].direction, "input");
        assert_eq!(
            engine.document().child_sheets[0].pins[0].position,
            Point::new(15.0, 25.0)
        );
    }

    fn new_child_sheet(filename: &str, name: &str) -> ChildSheet {
        ChildSheet {
            uuid: uuid::Uuid::new_v4(),
            name: name.to_string(),
            filename: filename.to_string(),
            position: Point::new(10.0, 20.0),
            size: (30.0, 20.0),
            stroke_width: 0.12,
            fill: FillType::None,
            stroke_color: None,
            fill_color: None,
            fields_autoplaced: false,
            pins: vec![SheetPin {
                uuid: uuid::Uuid::new_v4(),
                name: "SDA".to_string(),
                direction: "input".to_string(),
                position: Point::new(10.0, 25.0),
                rotation: 0.0,
                auto_generated: false,
                user_moved: false,
            }],
            instances: Vec::new(),
        }
    }

    #[test]
    fn place_child_sheet_appends_marks_child_sheets_and_round_trips_undo_redo() {
        let mut engine = Engine::new(test_sheet()).unwrap();
        let sheet = new_child_sheet("child.snxsch", "Child");
        let uuid = sheet.uuid;

        let result = engine
            .execute(Command::PlaceChildSheet { child_sheet: sheet })
            .unwrap();

        assert!(result.changed);
        assert!(
            result
                .patch_pair
                .expect("patch")
                .document
                .contains(DocumentPatch::CHILD_SHEETS)
        );
        let placed = &engine.document().child_sheets;
        assert_eq!(placed.len(), 1);
        assert_eq!(placed[0].uuid, uuid);
        assert_eq!(placed[0].filename, "child.snxsch");
        assert_eq!(placed[0].pins.len(), 1);
        assert_eq!(placed[0].pins[0].name, "SDA");

        engine.undo().unwrap();
        assert!(engine.document().child_sheets.is_empty());
        engine.redo().unwrap();
        assert_eq!(engine.document().child_sheets.len(), 1);
        assert_eq!(engine.document().child_sheets[0].uuid, uuid);
    }

    #[test]
    fn place_child_sheet_rejects_blank_filename_without_mutating() {
        let mut engine = Engine::new(test_sheet()).unwrap();
        for filename in ["", "   "] {
            let err = engine
                .execute(Command::PlaceChildSheet {
                    child_sheet: new_child_sheet(filename, "Child"),
                })
                .unwrap_err();
            assert!(matches!(err, EngineError::EmptyChildSheetFilename));
        }
        assert!(engine.document().child_sheets.is_empty());
        // Nothing was recorded, so there is nothing to undo.
        assert!(engine.undo().unwrap().is_none());
    }

    #[test]
    fn place_child_sheet_allows_an_empty_name() {
        let mut engine = Engine::new(test_sheet()).unwrap();
        engine
            .execute(Command::PlaceChildSheet {
                child_sheet: new_child_sheet("child.snxsch", ""),
            })
            .unwrap();
        assert_eq!(engine.document().child_sheets.len(), 1);
    }
}
