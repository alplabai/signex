//! The Esc rungs of the editing cards on the canvas: Paste Array, the union
//! popover, Smart Paste and the variant part edit (mockup rows 208, 209).

use super::OpenOverlays;
use crate::app::Message;
use crate::app::view::overlay_id::OverlayId;

impl OpenOverlays {
    /// Esc on an open editing card closes or cancels it.
    pub(super) fn edit_escape(&self, id: OverlayId) -> Option<Message> {
        match id {
            OverlayId::PasteArray => self.paste_array_open.then(|| {
                Message::Edit(crate::app::EditMsg::PasteArray(
                    crate::paste_array::PasteArrayMsg::EscapePressed,
                ))
            }),
            OverlayId::UnionPopover => self.union_popover_open.then(|| {
                Message::Edit(crate::app::EditMsg::Unions(
                    crate::app::unions::UnionsMsg::Popover(
                        crate::app::unions::UnionPopoverMsg::Cancelled,
                    ),
                ))
            }),
            OverlayId::SmartPaste => self.smart_paste_open.then(|| {
                Message::Edit(crate::app::EditMsg::SmartPasteDialog(
                    crate::smart_paste::SmartPasteMsg::Cancelled,
                ))
            }),
            OverlayId::VariantPartEdit => self
                .variant_part_edit_open
                .then(|| Message::from(crate::app::VariantPartEditMsg::CancelPressed)),
            _ => None,
        }
    }
}
