//! The block diagram messages: reads and writes landing, File ▸ New, and
//! what happens on a diagram's tab. An edit from the toolbar, the canvas or
//! Properties builds the same [`DiagramEdit`] its `block_*` command does
//! and runs it through [`Signex::edit_block_diagram`]; a refusal is named
//! in Messages.

use std::path::{Path, PathBuf};

use iced::Task;
use signex_i18n::{Arg, MessageId, ids};
use signex_types::block_diagram::{BlockFields, BlockPatch, Interface, PortDirection, PortSide};
use uuid::Uuid;

use super::commands::next_port_offset;
use super::edit::DiagramEdit;
use super::fields::{parse_mm, parse_rails, parse_specs};
use super::refusal::Refusal;
use super::state::{Camera, DiagramItem, PropertyField};
use super::{BlockDiagramMsg, TabEvent};
use crate::app::{Message, Signex};

impl Signex {
    /// Route one block diagram message.
    pub(crate) fn dispatch_block_diagram_message(&mut self, msg: BlockDiagramMsg) -> Task<Message> {
        match msg {
            BlockDiagramMsg::NewRequested => {
                if let Err(refusal) = self.new_block_diagram(None) {
                    log_refusal(&refusal);
                }
                Task::none()
            }
            BlockDiagramMsg::Read { path, result } => self.block_diagram_read(path, result),
            BlockDiagramMsg::Saved {
                path,
                written,
                hash,
                result,
            } => self.block_diagram_saved(path, *written, hash, result),
            BlockDiagramMsg::ReadForCommand {
                path,
                result,
                rerun,
            } => self.block_diagram_read_for_command(path, result, *rerun),
            BlockDiagramMsg::PowerTreeRead(landed) => self.power_tree_landed(*landed),
            BlockDiagramMsg::Tab { path, event } => self.block_diagram_event(path, event),
        }
    }

    fn block_diagram_event(&mut self, path: PathBuf, event: TabEvent) -> Task<Message> {
        if !self
            .document_state
            .block_diagrams
            .editors
            .contains_key(&path)
        {
            return Task::none();
        }
        let result = match event {
            TabEvent::UndoPressed => {
                self.step_block_diagram(&path, true);
                Ok(())
            }
            TabEvent::RedoPressed => {
                self.step_block_diagram(&path, false);
                Ok(())
            }
            TabEvent::SavePressed => match self.begin_block_diagram_save(&path) {
                Ok(task) => return task,
                Err(refusal) => Err(refusal),
            },
            TabEvent::BlockOpened(block) => return self.open_block_sheet(&path, block),
            TabEvent::ViewPicked(view) => return self.pick_diagram_view(&path, view),
            TabEvent::RailPicked(rail) => {
                self.pick_power_rail(&path, rail);
                Ok(())
            }
            TabEvent::PartOpened(part) => return self.open_power_part(&path, part),
            TabEvent::VariantPicked(variant) => return self.pick_power_variant(&path, variant),
            TabEvent::Review(event) => return self.block_diagram_review_event(&path, event),
            TabEvent::ShowGroundToggled(on) => {
                if let Some(editor) = self.document_state.block_diagrams.editors.get_mut(&path) {
                    editor.show_ground = on;
                }
                Ok(())
            }
            TabEvent::ResyncPressed => return self.resync_block_diagram(),
            TabEvent::Export(event) => return self.block_diagram_export_event(&path, event),
            TabEvent::EndPicked(end) => self.pick_connection_end(&path, end),
            TabEvent::BlockMoved { block, to } => {
                let patch = BlockPatch {
                    position: Some(to),
                    ..BlockPatch::default()
                };
                self.gui_edit(&path, DiagramEdit::SetBlock(block, patch))
            }
            TabEvent::AddBlockPressed => self.add_block_from_toolbar(&path),
            TabEvent::AddPortPressed => self.add_port_from_toolbar(&path),
            TabEvent::GroupPressed => self.group_from_toolbar(&path),
            TabEvent::DeletePressed => {
                let items = self.diagram_editor(&path).map(|e| e.selection.clone());
                match items {
                    Some(items) if !items.is_empty() => {
                        self.gui_edit(&path, DiagramEdit::RemoveItems(items))
                    }
                    _ => Ok(()),
                }
            }
            TabEvent::FieldCommitted(field) => self.commit_field(&path, field),
            TabEvent::ShapePicked(shape) => self.patch_selected_block(&path, |patch| {
                patch.shape = Some(shape);
            }),
            TabEvent::GroupPicked(group) => self.patch_selected_block(&path, |patch| {
                patch.group = Some(group);
            }),
            TabEvent::InterfacePicked(token) => {
                self.set_selected_connection(&path, Some(Interface::from_token(token)), None)
            }
            TabEvent::ColorPicked(color) => self.color_selection(&path, color),
            session @ (TabEvent::ToolPicked(_)
            | TabEvent::ItemPicked { .. }
            | TabEvent::ViewChanged(_)
            | TabEvent::FitPressed
            | TabEvent::FieldEdited(..)) => {
                self.session_event(&path, session);
                Ok(())
            }
        };
        if let Err(refusal) = result {
            log_refusal(&refusal);
        }
        Task::none()
    }

    /// The events that change only the tab's own state: no undo step, no
    /// dirty mark.
    fn session_event(&mut self, path: &Path, event: TabEvent) {
        let Some(editor) = self.document_state.block_diagrams.editors.get_mut(path) else {
            return;
        };
        match event {
            TabEvent::ToolPicked(tool) => {
                editor.tool = tool;
                editor.connect_from = None;
            }
            TabEvent::ItemPicked { item, add } => editor.pick(item, add),
            TabEvent::ViewChanged(camera) => editor.camera = camera,
            TabEvent::FitPressed => editor.camera = Camera::default(),
            TabEvent::FieldEdited(field, text) => {
                editor.drafts.insert(field, text);
            }
            _ => {}
        }
    }

    pub(super) fn diagram_editor(&self, path: &Path) -> Option<&super::BlockDiagramEditor> {
        self.document_state.block_diagrams.editors.get(path)
    }

    /// Run a tab edit; the answer is for commands, the tab reads the diagram.
    fn gui_edit(&mut self, path: &Path, edit: DiagramEdit) -> Result<(), Refusal> {
        self.edit_block_diagram(path, edit).map(|_| ())
    }

    /// A default name, "Block 3", in the person's language.
    fn numbered_name(&self, id: MessageId, number: usize) -> String {
        self.ui_state
            .i18n
            .message_plain(id, &[("number", Arg::from(number))])
    }

    fn add_block_from_toolbar(&mut self, path: &Path) -> Result<(), Refusal> {
        let Some(diagram) = self.diagram_editor(path).map(|e| &e.diagram) else {
            return Ok(());
        };
        let name = self.numbered_name(ids::BLOCK_DIAGRAM_NEW_BLOCK, diagram.blocks.len() + 1);
        let fields = BlockFields::named(&name, diagram.next_free_position());
        let answer = self.edit_block_diagram(path, DiagramEdit::AddBlock(fields))?;
        self.select_answered(path, &answer, DiagramItem::Block);
        Ok(())
    }

    fn add_port_from_toolbar(&mut self, path: &Path) -> Result<(), Refusal> {
        let Some(editor) = self.diagram_editor(path) else {
            return Ok(());
        };
        let Some(DiagramItem::Block(block)) = editor.single_selection() else {
            return Ok(());
        };
        let Some(owner) = editor.diagram.block(block) else {
            return Ok(());
        };
        let name = self.numbered_name(ids::BLOCK_DIAGRAM_NEW_PORT, owner.ports.len() + 1);
        let edit = DiagramEdit::AddPort {
            block,
            name,
            side: PortSide::Left,
            offset_mm: next_port_offset(owner, PortSide::Left),
            direction: PortDirection::Bidirectional,
        };
        self.gui_edit(path, edit)
    }

    fn group_from_toolbar(&mut self, path: &Path) -> Result<(), Refusal> {
        let Some(editor) = self.diagram_editor(path) else {
            return Ok(());
        };
        let blocks = editor.selected_blocks();
        if blocks.is_empty() {
            return Ok(());
        }
        let name = self.numbered_name(
            ids::BLOCK_DIAGRAM_NEW_GROUP,
            editor.diagram.groups.len() + 1,
        );
        let edit = DiagramEdit::AddGroup {
            name,
            color: None,
            blocks,
        };
        let answer = self.edit_block_diagram(path, edit)?;
        self.select_answered(path, &answer, DiagramItem::Group);
        Ok(())
    }

    /// Select what an edit made, by the `id` its answer names.
    fn select_answered(
        &mut self,
        path: &Path,
        answer: &serde_json::Value,
        item: fn(Uuid) -> DiagramItem,
    ) {
        let id = answer
            .get("id")
            .and_then(|id| id.as_str())
            .and_then(|id| Uuid::parse_str(id).ok());
        if let (Some(id), Some(editor)) =
            (id, self.document_state.block_diagrams.editors.get_mut(path))
        {
            editor.pick(Some(item(id)), false);
        }
    }

    /// The Connect tool: the first pick is held, the second connects.
    fn pick_connection_end(
        &mut self,
        path: &Path,
        end: signex_types::block_diagram::Endpoint,
    ) -> Result<(), Refusal> {
        let Some(editor) = self.document_state.block_diagrams.editors.get_mut(path) else {
            return Ok(());
        };
        let Some(from) = editor.connect_from.take() else {
            editor.connect_from = Some(end);
            return Ok(());
        };
        if from == end {
            return Ok(());
        }
        let edit = DiagramEdit::Connect {
            from,
            to: end,
            interface: Interface::Signal,
            label: String::new(),
        };
        let answer = self.edit_block_diagram(path, edit)?;
        self.select_answered(path, &answer, DiagramItem::Connection);
        Ok(())
    }

    fn patch_selected_block(
        &mut self,
        path: &Path,
        fill: impl FnOnce(&mut BlockPatch),
    ) -> Result<(), Refusal> {
        let Some(DiagramItem::Block(block)) =
            self.diagram_editor(path).and_then(|e| e.single_selection())
        else {
            return Ok(());
        };
        let mut patch = BlockPatch::default();
        fill(&mut patch);
        self.gui_edit(path, DiagramEdit::SetBlock(block, patch))
    }

    fn set_selected_connection(
        &mut self,
        path: &Path,
        interface: Option<Interface>,
        label: Option<String>,
    ) -> Result<(), Refusal> {
        let Some(DiagramItem::Connection(id)) =
            self.diagram_editor(path).and_then(|e| e.single_selection())
        else {
            return Ok(());
        };
        let edit = DiagramEdit::SetConnection {
            id,
            interface,
            label,
        };
        self.gui_edit(path, edit)
    }

    fn color_selection(
        &mut self,
        path: &Path,
        color: Option<signex_types::theme::Color>,
    ) -> Result<(), Refusal> {
        match self.diagram_editor(path).and_then(|e| e.single_selection()) {
            Some(DiagramItem::Block(block)) => {
                let patch = BlockPatch {
                    color: Some(color),
                    ..BlockPatch::default()
                };
                self.gui_edit(path, DiagramEdit::SetBlock(block, patch))
            }
            Some(DiagramItem::Group(id)) => {
                let edit = DiagramEdit::SetGroup {
                    id,
                    name: None,
                    color: Some(color),
                };
                self.gui_edit(path, edit)
            }
            _ => Ok(()),
        }
    }

    /// Commit the text typed in `field` to the selected object. The draft
    /// goes either way; a refusal is named in Messages.
    fn commit_field(&mut self, path: &Path, field: PropertyField) -> Result<(), Refusal> {
        let Some(editor) = self.document_state.block_diagrams.editors.get_mut(path) else {
            return Ok(());
        };
        let Some(text) = editor.drafts.remove(&field) else {
            return Ok(());
        };
        let selected = editor.single_selection();
        let block = editor
            .diagram
            .block(match selected {
                Some(DiagramItem::Block(id)) => id,
                _ => Uuid::nil(),
            })
            .cloned();
        match (field, selected) {
            (PropertyField::ConnectionLabel, Some(DiagramItem::Connection(_))) => {
                self.set_selected_connection(path, None, Some(text))
            }
            (PropertyField::ConnectionInterface, Some(DiagramItem::Connection(_))) => {
                self.set_selected_connection(path, Some(Interface::from_token(&text)), None)
            }
            (PropertyField::GroupName, Some(DiagramItem::Group(id))) => {
                let edit = DiagramEdit::SetGroup {
                    id,
                    name: Some(text),
                    color: None,
                };
                self.gui_edit(path, edit)
            }
            (_, Some(DiagramItem::Block(id))) => {
                let Some(block) = block else {
                    return Ok(());
                };
                let patch = block_field_patch(field, &text, block.position)?;
                self.gui_edit(path, DiagramEdit::SetBlock(id, patch))
            }
            _ => Ok(()),
        }
    }

    /// Double-click on a block: open the sheet it is linked to, by its path
    /// in the project folder.
    fn open_block_sheet(&mut self, path: &Path, block: Uuid) -> Task<Message> {
        let link = self
            .diagram_editor(path)
            .and_then(|e| e.diagram.block(block))
            .and_then(|b| b.sheet.clone());
        let (Some(link), Some(dir)) = (link, path.parent()) else {
            return Task::none();
        };
        let sheet = dir.join(&link.path);
        self.open_path_reported(sheet)
            .unwrap_or_else(|_| Task::none())
    }
}

/// The patch one block field's text makes.
fn block_field_patch(
    field: PropertyField,
    text: &str,
    position: signex_types::schematic::Point,
) -> Result<BlockPatch, Refusal> {
    let mut patch = BlockPatch::default();
    match field {
        PropertyField::BlockName => patch.name = Some(text.to_string()),
        PropertyField::BlockFunction => patch.function = Some(text.to_string()),
        PropertyField::BlockNote => patch.note = Some(text.to_string()),
        PropertyField::BlockRails => patch.rails = Some(parse_rails(text)),
        PropertyField::BlockSpecs => patch.specs = Some(parse_specs(text)?),
        PropertyField::BlockX => {
            patch.position = Some(signex_types::schematic::Point::new(
                parse_mm(text)?,
                position.y,
            ));
        }
        PropertyField::BlockY => {
            patch.position = Some(signex_types::schematic::Point::new(
                position.x,
                parse_mm(text)?,
            ));
        }
        PropertyField::BlockWidth => patch.width_mm = Some(parse_mm(text)?),
        PropertyField::BlockHeight => patch.height_mm = Some(parse_mm(text)?),
        PropertyField::ConnectionLabel
        | PropertyField::ConnectionInterface
        | PropertyField::GroupName => {}
    }
    Ok(patch)
}

/// Name a refused tab edit in Messages.
fn log_refusal(refusal: &Refusal) {
    crate::diagnostics::log_warning(format!("Block diagram: {}", refusal.english()));
}
