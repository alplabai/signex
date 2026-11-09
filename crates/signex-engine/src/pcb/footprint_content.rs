//! Rewrite what a footprint is, keeping where it is: swap in a new library
//! footprint, set its designator and value, set its schematic link. These are
//! the forward-annotation edits of Update PCB from schematic (#1312).

use signex_types::pcb::{Footprint, PcbBoard, SymbolLink};
use uuid::Uuid;

use super::command::{Applied, BoardCommand, BoardPatch, BoardPatchKind};
use super::error::BoardCommandError;
use super::items::{self, BoardItemId};

const REFERENCE_KEY: &str = "Reference";
const VALUE_KEY: &str = "Value";

fn find(board: &mut PcbBoard, id: Uuid) -> Result<&mut Footprint, BoardCommandError> {
    board
        .footprints
        .iter_mut()
        .find(|fp| fp.uuid == id)
        .ok_or(BoardCommandError::FootprintNotFound(id))
}

fn patch(kind: BoardPatchKind, id: Uuid) -> BoardPatch {
    BoardPatch {
        kind,
        items: vec![BoardItemId::Footprint(id)],
    }
}

/// Swap footprint `id` for `template`, a library footprint in its top-side
/// form. The board keeps the old footprint's uuid, position, rotation, side,
/// lock, schematic link, designator, value and preserved unknown data; the
/// template brings the footprint name, pads, graphics and properties. A
/// bottom-side footprint flips the template with `pcb_geom::flip_footprint`.
/// Each new pad takes the net and net lock of the old pad with its number, so
/// a locked net survives; a number the old footprint lacks gets no net.
/// Refused for a template that is not top-side or whose pad uuids another
/// footprint already uses.
pub(super) fn replace(
    board: &mut PcbBoard,
    id: Uuid,
    template: Footprint,
) -> Result<Applied, BoardCommandError> {
    if pcb_geom::Side::of_layer(&template.layer) != pcb_geom::Side::Top {
        return Err(BoardCommandError::TemplateNotTopSide(template.uuid));
    }
    find(board, id)?;
    items::check_pad_uuids(board, template.pads.iter(), Some(id))?;
    let old = find(board, id)?;
    let mut new = template;
    carry_pad_nets(old, &mut new);
    if pcb_geom::Side::of_layer(&old.layer) == pcb_geom::Side::Bottom {
        pcb_geom::flip_footprint(&mut new);
    }
    new.uuid = old.uuid;
    new.position = old.position;
    new.rotation = old.rotation;
    new.locked = old.locked;
    new.schematic_link = old.schematic_link.clone();
    new.reference = old.reference.clone();
    new.value = old.value.clone();
    new.unknown = old.unknown.clone();
    new.unknown_columns = old.unknown_columns.clone();
    sync_text_properties(&mut new);
    let previous = std::mem::replace(old, new);
    Ok(Applied {
        inverse: BoardCommand::RestoreFootprint {
            footprint: Box::new(previous),
        },
        patch: patch(BoardPatchKind::FootprintReplaced, id),
    })
}

/// Put `footprint` back over the board footprint with its uuid, whole; the
/// inverse of [`BoardCommand::ReplaceFootprint`].
pub(super) fn restore(
    board: &mut PcbBoard,
    footprint: Footprint,
) -> Result<Applied, BoardCommandError> {
    let id = footprint.uuid;
    let slot = find(board, id)?;
    let previous = std::mem::replace(slot, footprint);
    Ok(Applied {
        inverse: BoardCommand::RestoreFootprint {
            footprint: Box::new(previous),
        },
        patch: patch(BoardPatchKind::FootprintReplaced, id),
    })
}

/// Set the designator and value, and the Reference and Value property texts
/// that show them.
pub(super) fn set_text(
    board: &mut PcbBoard,
    id: Uuid,
    reference: String,
    value: String,
) -> Result<Applied, BoardCommandError> {
    let fp = find(board, id)?;
    let before = FootprintText::of(fp);
    fp.reference = reference;
    fp.value = value;
    sync_text_properties(fp);
    Ok(Applied {
        inverse: BoardCommand::RestoreFootprintText {
            footprint: id,
            text: before,
        },
        patch: patch(BoardPatchKind::FootprintTextSet, id),
    })
}

pub(super) fn restore_text(
    board: &mut PcbBoard,
    id: Uuid,
    text: FootprintText,
) -> Result<Applied, BoardCommandError> {
    let fp = find(board, id)?;
    if !text.fits(fp) {
        return Err(BoardCommandError::TextMismatch(id));
    }
    let current = FootprintText::of(fp);
    text.write(fp);
    Ok(Applied {
        inverse: BoardCommand::RestoreFootprintText {
            footprint: id,
            text: current,
        },
        patch: patch(BoardPatchKind::FootprintTextSet, id),
    })
}

pub(super) fn set_link(
    board: &mut PcbBoard,
    id: Uuid,
    link: Option<SymbolLink>,
) -> Result<Applied, BoardCommandError> {
    let fp = find(board, id)?;
    let previous = std::mem::replace(&mut fp.schematic_link, link);
    Ok(Applied {
        inverse: BoardCommand::SetFootprintLink {
            footprint: id,
            link: previous,
        },
        patch: patch(BoardPatchKind::FootprintLinkSet, id),
    })
}

/// Give each pad of `new` the net and lock of the first pad of `old` with its
/// number; a pad whose number `old` lacks has none.
fn carry_pad_nets(old: &Footprint, new: &mut Footprint) {
    for pad in &mut new.pads {
        let from = old.pads.iter().find(|p| p.number == pad.number);
        pad.net = from.and_then(|p| p.net.clone());
        pad.net_locked = from.is_some_and(|p| p.net_locked);
    }
}

fn sync_text_properties(fp: &mut Footprint) {
    for property in &mut fp.properties {
        if property.key == REFERENCE_KEY {
            property.value.clone_from(&fp.reference);
        } else if property.key == VALUE_KEY {
            property.value.clone_from(&fp.value);
        }
    }
}

/// A footprint's designator, value and every property text, as a text edit
/// found them. Undo writes them back as they were, even where a property did
/// not match the designator before the edit.
#[derive(Debug, Clone, PartialEq)]
pub struct FootprintText {
    reference: String,
    value: String,
    property_values: Vec<String>,
}

impl FootprintText {
    fn of(fp: &Footprint) -> Self {
        Self {
            reference: fp.reference.clone(),
            value: fp.value.clone(),
            property_values: fp.properties.iter().map(|p| p.value.clone()).collect(),
        }
    }

    /// Whether the footprint still has the properties this text was taken
    /// from.
    fn fits(&self, fp: &Footprint) -> bool {
        self.property_values.len() == fp.properties.len()
    }

    fn write(self, fp: &mut Footprint) {
        fp.reference = self.reference;
        fp.value = self.value;
        for (property, value) in fp.properties.iter_mut().zip(self.property_values) {
            property.value = value;
        }
    }
}
