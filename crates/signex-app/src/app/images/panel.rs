//! Properties ▸ Image (#1116, mockup row 204): the handlers of its rows and
//! buttons. Every row ends in [`Signex::edit_image`], the method
//! `set_image` runs, so a typed value and a CLI call are one edit and one
//! undo step. Relink…, Replace… and Show in folder start their file work in
//! a `Task`. What the panel shows is in `panels/element_properties/image.rs`.

use iced::Task;
use signex_engine::Command;
use signex_i18n::{ids, t};
use signex_types::schematic::{
    DrawingPlacement, ImageEdit, SchDrawing, SelectedItem, SelectedKind,
};
use uuid::Uuid;

use super::edit::ImageFieldEdit;
use super::relink::RelinkScope;
use crate::app::{Message, Signex};
use crate::panels::{ImagePanelField, ImagePanelState};

/// What happened in Properties ▸ Image.
#[derive(Debug, Clone)]
pub enum ImagePanelMsg {
    /// A keystroke in a box: kept as a draft until Enter.
    FieldTyped(ImagePanelField, String),
    /// Enter in a box: the draft becomes one edit.
    FieldSubmitted(ImagePanelField),
    KeepAspectToggled(bool),
    BorderPicked(bool),
    PlacementPicked(DrawingPlacement),
    ResetToFileSizePressed,
    ReplacePressed,
    RelinkPressed,
    ShowInFolderPressed,
    RemovePressed,
}

/// The edit a submitted box gives.
fn field_edit(field: ImagePanelField, text: &str) -> Result<ImageFieldEdit, String> {
    let key = match field {
        ImagePanelField::Scale => "scale",
        ImagePanelField::Width => "width",
        ImagePanelField::Height => "height",
        ImagePanelField::Opacity => "opacity",
    };
    super::edit::parse_image_field(key, text)
}

impl Signex {
    /// The image selected on the active sheet.
    pub(crate) fn selected_image_id(&self) -> Option<Uuid> {
        let ctx = &self.document_view.panel_ctx;
        if ctx.selected_kind != Some(SelectedKind::Drawing) {
            return None;
        }
        match &ctx.selected_drawing {
            Some(SchDrawing::Image(image)) if Some(image.uuid) == ctx.selected_uuid => {
                Some(image.uuid)
            }
            _ => None,
        }
    }

    /// What Properties ▸ Image shows of the selected image's file: whether it
    /// is missing (the canvas read or the last asset scan says so), its
    /// format and size; and the draft kept while it stays selected.
    pub(crate) fn image_panel_state(&self) -> ImagePanelState {
        let ctx = &self.document_view.panel_ctx;
        let Some(SchDrawing::Image(image)) = &ctx.selected_drawing else {
            return ImagePanelState::default();
        };
        let draft = ctx
            .annotation
            .image
            .draft
            .clone()
            .filter(|(id, _, _)| *id == image.uuid);
        let cache = &self.ui_state.image_cache;
        let unread = matches!(
            cache.entry(&super::ImageKey::of(image)),
            Some(super::ImageEntry::Missing)
        );
        let entry = self
            .document_state
            .active_path
            .as_deref()
            .map(|sheet| self.image_root(sheet))
            .and_then(|root| {
                cache
                    .assets
                    .report(&root)
                    .and_then(|r| r.entry(&image.file).cloned())
            });
        ImagePanelState {
            draft,
            missing: unread || entry.as_ref().is_some_and(|e| e.missing),
            bytes: entry.as_ref().and_then(|e| e.bytes),
            format: entry.and_then(|e| e.format),
        }
    }

    /// Run `edit` on the selected image; a refusal is said in Messages.
    fn panel_image_edit(&mut self, id: Uuid, edit: ImageFieldEdit) {
        if let Err(reason) = self.edit_image(id, &edit) {
            crate::diagnostics::log_warning(
                t!(self.ui_state.i18n, ids::IMAGES_EDIT_REFUSED, "reason" => reason).into_string(),
            );
        }
    }

    pub(crate) fn handle_image_panel(&mut self, msg: ImagePanelMsg) -> Task<Message> {
        let Some(id) = self.selected_image_id() else {
            return Task::none();
        };
        match msg {
            ImagePanelMsg::FieldTyped(field, text) => {
                self.document_view.panel_ctx.annotation.image.draft = Some((id, field, text));
            }
            ImagePanelMsg::FieldSubmitted(field) => {
                let draft = self.document_view.panel_ctx.annotation.image.draft.take();
                let Some((_, _, text)) = draft.filter(|(d, f, _)| *d == id && *f == field) else {
                    return Task::none();
                };
                match field_edit(field, &text) {
                    Ok(edit) => self.panel_image_edit(id, edit),
                    Err(reason) => crate::diagnostics::log_warning(
                        t!(self.ui_state.i18n, ids::IMAGES_EDIT_REFUSED, "reason" => reason)
                            .into_string(),
                    ),
                }
            }
            ImagePanelMsg::KeepAspectToggled(on) => {
                self.panel_image_edit(id, ImageFieldEdit::Image(ImageEdit::KeepAspect(on)));
            }
            ImagePanelMsg::BorderPicked(on) => {
                self.panel_image_edit(id, ImageFieldEdit::Image(ImageEdit::Border(on)));
            }
            ImagePanelMsg::PlacementPicked(placement) => {
                self.panel_image_edit(id, ImageFieldEdit::Placement(placement));
            }
            ImagePanelMsg::ResetToFileSizePressed => {
                self.panel_image_edit(id, ImageFieldEdit::Image(ImageEdit::Scale(100.0)));
            }
            ImagePanelMsg::ReplacePressed => return self.relink_dialog(RelinkScope::Image(id)),
            ImagePanelMsg::RelinkPressed => {
                let Ok(image) = self.image_on_edit_sheet(id) else {
                    return Task::none();
                };
                return self.relink_dialog(RelinkScope::File(image.file));
            }
            ImagePanelMsg::ShowInFolderPressed => return self.show_image_in_folder(id),
            ImagePanelMsg::RemovePressed => self.remove_image(id),
        }
        Task::none()
    }

    /// Remove image `id` from the active sheet as one undo step (the
    /// missing-file banner's Remove image). `delete_selection ids=` is the
    /// same edit for a CLI.
    fn remove_image(&mut self, id: Uuid) {
        let command = Command::DeleteSelection {
            items: vec![SelectedItem::new(id, SelectedKind::Drawing)],
        };
        let refused = self
            .document_state
            .edit_engine()
            .and_then(|engine| engine.lock_refusal(&command));
        if let Some(reason) = refused {
            crate::diagnostics::log_warning(
                t!(self.ui_state.i18n, ids::IMAGES_EDIT_REFUSED, "reason" => reason).into_string(),
            );
            return;
        }
        if self.apply_engine_command(command, true, true) {
            self.interaction_state
                .active_canvas_mut()
                .selected
                .retain(|item| item.uuid != id);
        }
    }

    /// A handle drag dropped the image `id` with this box: one edit.
    pub(super) fn image_handle_dropped(
        &mut self,
        id: Uuid,
        position: signex_types::schematic::Point,
        size: (f64, f64),
    ) {
        self.panel_image_edit(
            id,
            ImageFieldEdit::Image(ImageEdit::Bounds { position, size }),
        );
    }

    /// The Projects tree's `assets/images/` row of `file` was pressed.
    pub(crate) fn pick_project_asset(&mut self, file: &str) -> Task<Message> {
        self.update(
            super::ImagesMsg::AssetPicked {
                file: file.to_string(),
            }
            .into(),
        )
    }

    /// A row of the Projects tree's `assets/images/` was picked: read the
    /// file and put it on the cursor over the active sheet, as a file picked
    /// in the Place Image dialog is.
    pub(super) fn asset_picked(&mut self, file: &str) -> Task<Message> {
        let Some(target) = self.image_target() else {
            return Task::none();
        };
        let Ok(relative) = signex_types::image_asset::checked_asset_path(file) else {
            return Task::none();
        };
        let path = self.image_root(&target).join(relative);
        self.ui_state.image_cache.dialog = None;
        self.read_picked_file(Some(path))
    }
}
