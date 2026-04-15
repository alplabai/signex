//! Undo/Redo system using command pattern.
//!
//! Each edit operation produces an `EditCommand` that can be undone and redone.
//! The `UndoStack` maintains history with a configurable max depth.

use signex_types::schematic::*;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandOrigin {
    Legacy,
    EngineMirrored,
}

#[derive(Debug, Clone)]
enum HistoryEntry {
    Legacy(EditCommand),
    EngineMarker { steps: usize },
}

impl HistoryEntry {
    fn origin(&self) -> CommandOrigin {
        match self {
            HistoryEntry::Legacy(_) => CommandOrigin::Legacy,
            HistoryEntry::EngineMarker { .. } => CommandOrigin::EngineMirrored,
        }
    }

    fn engine_steps(&self) -> usize {
        match self {
            HistoryEntry::Legacy(_) => 0,
            HistoryEntry::EngineMarker { steps } => *steps,
        }
    }
}

/// A reversible edit operation on a schematic.
#[derive(Debug, Clone)]
pub enum EditCommand {
    /// Add a wire segment.
    AddWire(Wire),
    /// Add a bus segment.
    AddBus(Bus),
    /// Add a junction.
    AddJunction(Junction),
    /// Add a label.
    AddLabel(Label),
    /// Add a symbol instance.
    AddSymbol(Symbol),
    /// Add a no-connect marker.
    AddNoConnect(NoConnect),
    /// Add a text note.
    AddTextNote(TextNote),
    /// Add a bus entry.
    #[allow(dead_code)]
    AddBusEntry(BusEntry),
    /// Add a child sheet (hierarchical sheet symbol).
    #[allow(dead_code)]
    AddChildSheet(ChildSheet),

    /// Remove element by UUID and kind, storing the removed element for undo.
    RemoveWire(Wire),
    RemoveBus(Bus),
    RemoveJunction(Junction),
    RemoveLabel(Label),
    RemoveSymbol(Symbol),
    RemoveNoConnect(NoConnect),
    RemoveTextNote(TextNote),
    #[allow(dead_code)]
    RemoveBusEntry(BusEntry),
    #[allow(dead_code)]
    RemoveChildSheet(ChildSheet),

    /// Move element(s) by a delta offset.
    #[allow(dead_code)]
    MoveElements {
        items: Vec<SelectedItem>,
        dx: f64,
        dy: f64,
    },

    /// Rotate a symbol.
    #[allow(dead_code)]
    RotateSymbol {
        uuid: Uuid,
        old_rotation: f64,
        new_rotation: f64,
    },

    /// Mirror a symbol.
    #[allow(dead_code)]
    MirrorSymbol {
        uuid: Uuid,
        axis: MirrorAxis,
        #[allow(dead_code)]
        old_mirror_x: bool,
        #[allow(dead_code)]
        old_mirror_y: bool,
    },

    /// Update a symbol's string field (designator, value, footprint).
    #[allow(dead_code)]
    UpdateSymbolField {
        uuid: Uuid,
        field: SymbolField,
        old_value: String,
        new_value: String,
    },

    /// Update a label's text.
    UpdateLabelText {
        uuid: Uuid,
        old_text: String,
        new_text: String,
    },

    /// Update a text note's text.
    UpdateTextNoteText {
        uuid: Uuid,
        old_text: String,
        new_text: String,
    },

    /// Batch of commands (for compound operations).
    Batch(Vec<EditCommand>),
}

/// Which symbol field is being updated.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum SymbolField {
    Designator,
    Value,
    Footprint,
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum MirrorAxis {
    X,
    Y,
}

/// Apply an edit command to a schematic sheet.
pub fn apply(sheet: &mut SchematicSheet, cmd: &EditCommand) {
    match cmd {
        EditCommand::AddWire(w) => sheet.wires.push(w.clone()),
        EditCommand::AddBus(b) => sheet.buses.push(b.clone()),
        EditCommand::AddJunction(j) => sheet.junctions.push(j.clone()),
        EditCommand::AddLabel(l) => sheet.labels.push(l.clone()),
        EditCommand::AddSymbol(s) => sheet.symbols.push(s.clone()),
        EditCommand::AddNoConnect(nc) => sheet.no_connects.push(nc.clone()),
        EditCommand::AddTextNote(tn) => sheet.text_notes.push(tn.clone()),
        EditCommand::AddBusEntry(be) => sheet.bus_entries.push(be.clone()),
        EditCommand::AddChildSheet(cs) => sheet.child_sheets.push(cs.clone()),

        EditCommand::RemoveWire(w) => sheet.wires.retain(|x| x.uuid != w.uuid),
        EditCommand::RemoveBus(b) => sheet.buses.retain(|x| x.uuid != b.uuid),
        EditCommand::RemoveJunction(j) => sheet.junctions.retain(|x| x.uuid != j.uuid),
        EditCommand::RemoveLabel(l) => sheet.labels.retain(|x| x.uuid != l.uuid),
        EditCommand::RemoveSymbol(s) => sheet.symbols.retain(|x| x.uuid != s.uuid),
        EditCommand::RemoveNoConnect(nc) => sheet.no_connects.retain(|x| x.uuid != nc.uuid),
        EditCommand::RemoveTextNote(tn) => sheet.text_notes.retain(|x| x.uuid != tn.uuid),
        EditCommand::RemoveBusEntry(be) => sheet.bus_entries.retain(|x| x.uuid != be.uuid),
        EditCommand::RemoveChildSheet(cs) => sheet.child_sheets.retain(|x| x.uuid != cs.uuid),

        EditCommand::MoveElements { items, dx, dy } => {
            for item in items {
                move_element(sheet, item, *dx, *dy);
            }
        }

        EditCommand::RotateSymbol {
            uuid, new_rotation, ..
        } => {
            if let Some(sym) = sheet.symbols.iter_mut().find(|s| s.uuid == *uuid) {
                sym.rotation = *new_rotation;
            }
        }

        EditCommand::MirrorSymbol { uuid, axis, .. } => {
            if let Some(sym) = sheet.symbols.iter_mut().find(|s| s.uuid == *uuid) {
                match axis {
                    MirrorAxis::X => sym.mirror_x = !sym.mirror_x,
                    MirrorAxis::Y => sym.mirror_y = !sym.mirror_y,
                }
            }
        }

        EditCommand::UpdateSymbolField {
            uuid,
            field,
            new_value,
            ..
        } => {
            if let Some(sym) = sheet.symbols.iter_mut().find(|s| s.uuid == *uuid) {
                match field {
                    SymbolField::Designator => sym.reference = new_value.clone(),
                    SymbolField::Value => sym.value = new_value.clone(),
                    SymbolField::Footprint => sym.footprint = new_value.clone(),
                }
            }
        }

        EditCommand::UpdateLabelText {
            uuid, new_text, ..
        } => {
            if let Some(lbl) = sheet.labels.iter_mut().find(|l| l.uuid == *uuid) {
                lbl.text = new_text.clone();
            }
        }

        EditCommand::UpdateTextNoteText {
            uuid, new_text, ..
        } => {
            if let Some(tn) = sheet.text_notes.iter_mut().find(|t| t.uuid == *uuid) {
                tn.text = new_text.clone();
            }
        }

        EditCommand::Batch(cmds) => {
            for c in cmds {
                apply(sheet, c);
            }
        }
    }
}

/// Undo an edit command (apply the inverse).
pub fn undo(sheet: &mut SchematicSheet, cmd: &EditCommand) {
    match cmd {
        EditCommand::AddWire(w) => sheet.wires.retain(|x| x.uuid != w.uuid),
        EditCommand::AddBus(b) => sheet.buses.retain(|x| x.uuid != b.uuid),
        EditCommand::AddJunction(j) => sheet.junctions.retain(|x| x.uuid != j.uuid),
        EditCommand::AddLabel(l) => sheet.labels.retain(|x| x.uuid != l.uuid),
        EditCommand::AddSymbol(s) => sheet.symbols.retain(|x| x.uuid != s.uuid),
        EditCommand::AddNoConnect(nc) => sheet.no_connects.retain(|x| x.uuid != nc.uuid),
        EditCommand::AddTextNote(tn) => sheet.text_notes.retain(|x| x.uuid != tn.uuid),
        EditCommand::AddBusEntry(be) => sheet.bus_entries.retain(|x| x.uuid != be.uuid),
        EditCommand::AddChildSheet(cs) => sheet.child_sheets.retain(|x| x.uuid != cs.uuid),

        EditCommand::RemoveWire(w) => sheet.wires.push(w.clone()),
        EditCommand::RemoveBus(b) => sheet.buses.push(b.clone()),
        EditCommand::RemoveJunction(j) => sheet.junctions.push(j.clone()),
        EditCommand::RemoveLabel(l) => sheet.labels.push(l.clone()),
        EditCommand::RemoveSymbol(s) => sheet.symbols.push(s.clone()),
        EditCommand::RemoveNoConnect(nc) => sheet.no_connects.push(nc.clone()),
        EditCommand::RemoveTextNote(tn) => sheet.text_notes.push(tn.clone()),
        EditCommand::RemoveBusEntry(be) => sheet.bus_entries.push(be.clone()),
        EditCommand::RemoveChildSheet(cs) => sheet.child_sheets.push(cs.clone()),

        EditCommand::MoveElements { items, dx, dy } => {
            for item in items {
                move_element(sheet, item, -*dx, -*dy);
            }
        }

        EditCommand::RotateSymbol {
            uuid, old_rotation, ..
        } => {
            if let Some(sym) = sheet.symbols.iter_mut().find(|s| s.uuid == *uuid) {
                sym.rotation = *old_rotation;
            }
        }

        EditCommand::MirrorSymbol { uuid, axis, .. } => {
            if let Some(sym) = sheet.symbols.iter_mut().find(|s| s.uuid == *uuid) {
                match axis {
                    MirrorAxis::X => sym.mirror_x = !sym.mirror_x,
                    MirrorAxis::Y => sym.mirror_y = !sym.mirror_y,
                }
            }
        }

        EditCommand::UpdateSymbolField {
            uuid,
            field,
            old_value,
            ..
        } => {
            if let Some(sym) = sheet.symbols.iter_mut().find(|s| s.uuid == *uuid) {
                match field {
                    SymbolField::Designator => sym.reference = old_value.clone(),
                    SymbolField::Value => sym.value = old_value.clone(),
                    SymbolField::Footprint => sym.footprint = old_value.clone(),
                }
            }
        }

        EditCommand::UpdateLabelText {
            uuid, old_text, ..
        } => {
            if let Some(lbl) = sheet.labels.iter_mut().find(|l| l.uuid == *uuid) {
                lbl.text = old_text.clone();
            }
        }

        EditCommand::UpdateTextNoteText {
            uuid, old_text, ..
        } => {
            if let Some(tn) = sheet.text_notes.iter_mut().find(|t| t.uuid == *uuid) {
                tn.text = old_text.clone();
            }
        }

        EditCommand::Batch(cmds) => {
            for c in cmds.iter().rev() {
                undo(sheet, c);
            }
        }
    }
}

fn move_element(sheet: &mut SchematicSheet, item: &SelectedItem, dx: f64, dy: f64) {
    match item.kind {
        SelectedKind::Symbol => {
            if let Some(sym) = sheet.symbols.iter_mut().find(|s| s.uuid == item.uuid) {
                sym.position.x += dx;
                sym.position.y += dy;
                if let Some(ref mut rt) = sym.ref_text {
                    rt.position.x += dx;
                    rt.position.y += dy;
                }
                if let Some(ref mut vt) = sym.val_text {
                    vt.position.x += dx;
                    vt.position.y += dy;
                }
            }
        }
        SelectedKind::Wire => {
            if let Some(w) = sheet.wires.iter_mut().find(|w| w.uuid == item.uuid) {
                w.start.x += dx;
                w.start.y += dy;
                w.end.x += dx;
                w.end.y += dy;
            }
        }
        SelectedKind::Bus => {
            if let Some(b) = sheet.buses.iter_mut().find(|b| b.uuid == item.uuid) {
                b.start.x += dx;
                b.start.y += dy;
                b.end.x += dx;
                b.end.y += dy;
            }
        }
        SelectedKind::Label => {
            if let Some(l) = sheet.labels.iter_mut().find(|l| l.uuid == item.uuid) {
                l.position.x += dx;
                l.position.y += dy;
            }
        }
        SelectedKind::Junction => {
            if let Some(j) = sheet.junctions.iter_mut().find(|j| j.uuid == item.uuid) {
                j.position.x += dx;
                j.position.y += dy;
            }
        }
        SelectedKind::NoConnect => {
            if let Some(nc) = sheet.no_connects.iter_mut().find(|n| n.uuid == item.uuid) {
                nc.position.x += dx;
                nc.position.y += dy;
            }
        }
        SelectedKind::TextNote => {
            if let Some(tn) = sheet.text_notes.iter_mut().find(|t| t.uuid == item.uuid) {
                tn.position.x += dx;
                tn.position.y += dy;
            }
        }
        SelectedKind::ChildSheet => {
            if let Some(cs) = sheet.child_sheets.iter_mut().find(|c| c.uuid == item.uuid) {
                cs.position.x += dx;
                cs.position.y += dy;
            }
        }
        SelectedKind::BusEntry => {
            if let Some(be) = sheet.bus_entries.iter_mut().find(|b| b.uuid == item.uuid) {
                be.position.x += dx;
                be.position.y += dy;
            }
        }
        SelectedKind::Drawing => {} // TODO
        SelectedKind::SymbolRefField => {
            // UUID = symbol UUID; only move the ref_text anchor, not the whole symbol
            if let Some(sym) = sheet.symbols.iter_mut().find(|s| s.uuid == item.uuid) {
                let (field_dx, field_dy) = inverse_field_display_delta(sym, dx, dy);
                if let Some(ref mut rt) = sym.ref_text {
                    rt.position.x += field_dx;
                    rt.position.y += field_dy;
                }
            }
        }
        SelectedKind::SymbolValField => {
            // UUID = symbol UUID; only move the val_text anchor
            if let Some(sym) = sheet.symbols.iter_mut().find(|s| s.uuid == item.uuid) {
                let (field_dx, field_dy) = inverse_field_display_delta(sym, dx, dy);
                if let Some(ref mut vt) = sym.val_text {
                    vt.position.x += field_dx;
                    vt.position.y += field_dy;
                }
            }
        }
    }
}

/// Convert a drag delta measured in displayed field coordinates back to the
/// stored field-coordinate delta before symbol TRANSFORM is applied.
fn inverse_field_display_delta(_sym: &Symbol, dx: f64, dy: f64) -> (f64, f64) {
    (dx, dy)
}

/// Undo history stack with configurable depth.
pub struct UndoStack {
    history: Vec<HistoryEntry>,
    position: usize,
    max_depth: usize,
}

impl UndoStack {
    pub fn new(max_depth: usize) -> Self {
        Self {
            history: Vec::new(),
            position: 0,
            max_depth,
        }
    }

    /// Execute a command and push it onto the stack.
    pub fn execute(&mut self, sheet: &mut SchematicSheet, cmd: EditCommand) {
        apply(sheet, &cmd);
        self.record(HistoryEntry::Legacy(cmd));
    }

    pub fn record_engine_marker(&mut self, steps: usize) {
        if steps == 0 {
            return;
        }

        self.record(HistoryEntry::EngineMarker { steps });
    }

    fn record(&mut self, entry: HistoryEntry) {
        // Truncate any redo history
        self.history.truncate(self.position);
        self.history.push(entry);
        self.position += 1;
        // Trim oldest if over max depth
        if self.history.len() > self.max_depth {
            let excess = self.history.len() - self.max_depth;
            self.history.drain(0..excess);
            self.position -= excess;
        }
    }

    /// Undo the last command. Returns true if something was undone.
    pub fn undo(&mut self, sheet: &mut SchematicSheet) -> bool {
        if self.position == 0 {
            return false;
        }
        self.position -= 1;
        let HistoryEntry::Legacy(command) = &self.history[self.position] else {
            self.position += 1;
            return false;
        };
        undo(sheet, &command.clone());
        true
    }

    /// Redo the next command. Returns true if something was redone.
    pub fn redo(&mut self, sheet: &mut SchematicSheet) -> bool {
        if self.position >= self.history.len() {
            return false;
        }
        let HistoryEntry::Legacy(command) = &self.history[self.position] else {
            return false;
        };
        apply(sheet, &command.clone());
        self.position += 1;
        true
    }

    pub fn peek_undo_origin(&self) -> Option<CommandOrigin> {
        (self.position > 0).then(|| self.history[self.position - 1].origin())
    }

    pub fn peek_redo_origin(&self) -> Option<CommandOrigin> {
        (self.position < self.history.len()).then(|| self.history[self.position].origin())
    }

    pub fn peek_undo_engine_steps(&self) -> Option<usize> {
        (self.position > 0).then(|| self.history[self.position - 1].engine_steps())
    }

    pub fn peek_redo_engine_steps(&self) -> Option<usize> {
        (self.position < self.history.len()).then(|| self.history[self.position].engine_steps())
    }

    pub fn step_back(&mut self) -> bool {
        if self.position == 0 {
            return false;
        }
        self.position -= 1;
        true
    }

    pub fn step_forward(&mut self) -> bool {
        if self.position >= self.history.len() {
            return false;
        }
        self.position += 1;
        true
    }

    #[allow(dead_code)]
    pub fn can_undo(&self) -> bool {
        self.position > 0
    }

    #[allow(dead_code)]
    pub fn can_redo(&self) -> bool {
        self.position < self.history.len()
    }
}
