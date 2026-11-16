//! Copy Attributes (#1667): arm placing another of the object under the
//! cursor with its look. A part arms another of the same part (body, value,
//! footprint, fields) with the next free designator; a net label or port
//! arms the label tool with its text, shape and text style; a
//! text note arms the text tool with its text and style. The add-vertex half
//! of the same key, while a wire or a shape is being drawn, is not here.

use std::path::Path;

use iced::Task;
use signex_types::schematic::{
    Label, LabelType, Point, SCHEMATIC_PT_TO_MM, SelectedItem, SelectedKind, TextNote,
};

use crate::app::state::PlacementKind;
use crate::app::{Message, Signex, Tool};
use crate::panels::{PrePlacementKind, PrePlacementStyle};
use crate::toolbar::ToolMessage;

/// What a placement tool takes over from the object it copies.
struct CopiedLook {
    tool: Tool,
    port: Option<(LabelType, String)>,
    kind: PrePlacementKind,
    text: String,
    style: PrePlacementStyle,
    /// The copy the cursor shows until the click.
    ghost: CopiedGhost,
}

/// The ghost of a copied label or text note, at the cursor origin.
enum CopiedGhost {
    Label(Label),
    Text(TextNote),
}

/// The form's text size in points for a text height in mm (10 pt when the
/// object stores none).
fn size_pt(font_size_mm: f64) -> u32 {
    if font_size_mm > 0.0 {
        (font_size_mm / SCHEMATIC_PT_TO_MM).round().max(1.0) as u32
    } else {
        10
    }
}

/// The form's font: the object's own, else the sheet's default font.
fn font_name(family: Option<&String>, sheet_default: &str) -> String {
    family.cloned().unwrap_or_else(|| sheet_default.to_string())
}

impl Signex {
    pub(super) fn copy_attributes_at(&mut self, at: Option<(f64, f64)>) -> Task<Message> {
        if self.interaction_state.wire_drawing
            || self.interaction_state.active_canvas().drawing_mode
        {
            crate::diagnostics::log_info(
                "Copy attributes: finish or cancel the drawing first; adding a vertex while drawing is not available yet.",
            );
            return Task::none();
        }
        let Some(target) = self.fragment_target("Copy attributes") else {
            return Task::none();
        };
        let Some(item) = self.copy_attributes_source(at) else {
            crate::diagnostics::log_info(
                "Copy attributes: put the cursor on a part, a label or a text note.",
            );
            return Task::none();
        };
        if matches!(
            item.kind,
            SelectedKind::Symbol | SelectedKind::SymbolRefField | SelectedKind::SymbolValField
        ) {
            self.arm_part_copy(&target, item.uuid);
            return Task::none();
        }
        match self.copied_look(item, &target) {
            Some(look) => self.arm_copied_look(look),
            None => {
                crate::diagnostics::log_info(
                    "Copy attributes: there is nothing to copy from that object; put the cursor on a part, a label or a text note.",
                );
                Task::none()
            }
        }
    }

    /// The object under the point `at`, else under the cursor, else the
    /// one selected object.
    fn copy_attributes_source(&self, at: Option<(f64, f64)>) -> Option<SelectedItem> {
        let under = at
            .or_else(|| self.cursor_world_now())
            .and_then(|(x, y)| self.item_hit_at(x, y));
        under.or_else(
            || match self.interaction_state.active_canvas().selected.as_slice() {
                [only] => Some(*only),
                _ => None,
            },
        )
    }

    /// Arm another of the part `id` on `target`, numbered as a library part.
    fn arm_part_copy(&mut self, target: &std::path::Path, id: uuid::Uuid) {
        let Some(engine) = self.document_state.engines.get(target) else {
            return;
        };
        let Some(source) = engine.document().symbols.iter().find(|s| s.uuid == id) else {
            return;
        };
        let prefix = signex_types::designator::placement_prefix(&source.reference);
        let reference = self.next_designator_in_sheet(target, &prefix);
        let Some(payload) = self
            .document_state
            .engines
            .get(target)
            .and_then(|engine| engine.part_copy_payload(id, reference))
        else {
            return;
        };
        self.interaction_state.placement_epoch += 1;
        self.arm_placement_session(payload, target, PlacementKind::Part { prefix });
    }

    /// The tool, text and style that place another of the label or text
    /// note `item`; `None` for any other object.
    fn copied_look(&self, item: SelectedItem, target: &Path) -> Option<CopiedLook> {
        let doc = self.document_state.engines.get(target)?.document();
        let default_font = self.document_state.schematic_default_font_for(target);
        match item.kind {
            SelectedKind::Label => {
                let label = doc.labels.iter().find(|l| l.uuid == item.uuid)?;
                let (port, kind) = match label.label_type {
                    LabelType::Net => (None, PrePlacementKind::NetLabel),
                    LabelType::Global => (
                        Some((LabelType::Global, label.shape.clone())),
                        PrePlacementKind::GlobalPort,
                    ),
                    LabelType::Hierarchical => (
                        Some((LabelType::Hierarchical, label.shape.clone())),
                        PrePlacementKind::HierPort,
                    ),
                    LabelType::Power => return None,
                };
                Some(CopiedLook {
                    tool: Tool::Label,
                    port,
                    kind,
                    text: label.text.clone(),
                    style: PrePlacementStyle {
                        rotation: label.rotation,
                        font: font_name(label.font_family.as_ref(), &default_font),
                        font_size_pt: size_pt(label.font_size),
                        justify_h: label.justify,
                        justify_v: label.justify_v,
                    },
                    ghost: CopiedGhost::Label(Label {
                        uuid: uuid::Uuid::now_v7(),
                        position: Point::new(0.0, 0.0),
                        locked: false,
                        ..label.clone()
                    }),
                })
            }
            SelectedKind::TextNote => {
                let note = doc.text_notes.iter().find(|t| t.uuid == item.uuid)?;
                Some(CopiedLook {
                    tool: Tool::Text,
                    port: None,
                    kind: PrePlacementKind::TextNote,
                    text: note.text.clone(),
                    style: PrePlacementStyle {
                        rotation: note.rotation,
                        font: font_name(note.font_family.as_ref(), &default_font),
                        font_size_pt: size_pt(note.font_size),
                        justify_h: note.justify_h,
                        justify_v: note.justify_v,
                    },
                    ghost: CopiedGhost::Text(TextNote {
                        uuid: uuid::Uuid::now_v7(),
                        position: Point::new(0.0, 0.0),
                        locked: false,
                        ..note.clone()
                    }),
                })
            }
            _ => None,
        }
    }

    /// Arm `look.tool` with the copied text and style in the pre-placement
    /// form, as the Tab form would leave it.
    fn arm_copied_look(&mut self, look: CopiedLook) -> Task<Message> {
        let armed = self.update(Message::Tool(ToolMessage::SelectTool(look.tool)));
        self.interaction_state.pending_port = look.port;
        let tool_name = format!("{}", self.interaction_state.current_tool);
        let cursor_mm = (self.ui_state.cursor_x, self.ui_state.cursor_y);
        let mut pre = match self.document_state.panel_ctx.pre_placement.take() {
            Some(pre) => pre,
            None => self.document_state.new_pre_placement(
                tool_name.clone(),
                look.kind,
                look.text.clone(),
                cursor_mm,
            ),
        };
        pre.kind = look.kind;
        pre.tool_name = tool_name;
        pre.label_text = look.text;
        pre.set_style(look.style);
        self.document_state.panel_ctx.pre_placement = Some(pre);
        let canvas = self.interaction_state.active_canvas_mut();
        match look.ghost {
            CopiedGhost::Label(label) => canvas.ghost_label = Some(label),
            CopiedGhost::Text(note) => canvas.ghost_text = Some(note),
        }
        armed
    }
}
