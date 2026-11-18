//! Rewrite what a footprint is, keeping where it is: swap in a new library
//! footprint, set its designator and value, set its schematic link. These are
//! the forward-annotation edits of Update PCB from schematic (#1312). One
//! property's value or visibility is the Properties panel's edit (#2354).

use std::collections::{HashMap, HashSet};

use signex_types::format::parse_pcb_layer;
use signex_types::pcb::{ComponentAttributes, Footprint, Pad, PadNet, PcbBoard, SymbolLink};
use signex_types::pcb_layer::BoardSide;
use signex_types::property::PcbProperty;
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
/// lock, schematic link, designator, value and preserved unknown data, and its
/// library link when the template has none; the template brings the footprint
/// name, pads, graphics and property values ([`merge_properties`]). A
/// bottom-side footprint flips the template with `pcb_geom::flip_footprint`.
/// The n-th new pad of a number takes the net and net lock of the n-th old pad
/// of that number, so a locked net survives; a number the old footprint lacks
/// gets no net. Refused for a template not on a readable top-side layer, with
/// a pad off the number line, with a pad uuid already used on the board, or
/// with a different count of a pad number the old footprint also has.
pub(super) fn replace(
    board: &mut PcbBoard,
    id: Uuid,
    template: Footprint,
) -> Result<Applied, BoardCommandError> {
    // A layer this build cannot read is not taken for the top.
    let side = parse_pcb_layer(&template.layer).and_then(|layer| layer.side());
    if side != Some(BoardSide::Top) {
        return Err(BoardCommandError::TemplateNotTopSide(template.uuid));
    }
    items::check_pads_finite(&template.pads)?;
    find(board, id)?;
    items::check_pad_uuids(board, template.pads.iter(), Some(id), &[])?;
    let old = find(board, id)?;
    let mut new = template;
    carry_pad_nets(old, &mut new)?;
    if pcb_geom::Side::of_layer(&old.layer) == pcb_geom::Side::Bottom {
        pcb_geom::flip_footprint(&mut new);
    }
    new.uuid = old.uuid;
    new.position = old.position;
    new.rotation = old.rotation;
    new.locked = old.locked;
    new.component = Box::new(carried_component(
        &old.component,
        std::mem::take(&mut *new.component),
    ));
    new.schematic_link = old.schematic_link.clone();
    new.reference = old.reference.clone();
    new.value = old.value.clone();
    new.unknown = old.unknown.clone();
    new.unknown_columns = old.unknown_columns.clone();
    if new.library_ref.is_none() {
        new.library_ref = old.library_ref.clone();
    }
    new.properties = merge_properties(&old.properties, std::mem::take(&mut new.properties));
    sync_text_properties(&mut new);
    let previous = std::mem::replace(old, new);
    Ok(Applied {
        inverse: BoardCommand::RestoreFootprint {
            footprint: Box::new(previous),
        },
        patch: patch(BoardPatchKind::FootprintReplaced, id),
    })
}

/// The component settings of a replaced footprint (#2365): the type, height
/// and description come with the new library footprint; the opacity, locks,
/// swapping and channel offset are board choices and stay.
fn carried_component(
    old: &ComponentAttributes,
    template: ComponentAttributes,
) -> ComponentAttributes {
    ComponentAttributes {
        component_type: template.component_type,
        height_mm: template.height_mm,
        description: template.description,
        ..old.clone()
    }
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

/// Replace property `index` of footprint `id` with `property`, see
/// [`BoardCommand::SetFootprintProperty`]. Every check runs before the write.
pub(super) fn set_property(
    board: &mut PcbBoard,
    id: Uuid,
    index: usize,
    property: PcbProperty,
) -> Result<Applied, BoardCommandError> {
    check_property_finite(&property)?;
    let fp = find(board, id)?;
    let Some(old) = fp.properties.get(index) else {
        return Err(BoardCommandError::PropertyNotFound {
            footprint: id,
            index,
        });
    };
    if old.key != property.key {
        return Err(BoardCommandError::PropertyKeyChanged {
            footprint: id,
            index,
        });
    }
    if is_text_key(&old.key) && old.value != property.value {
        return Err(BoardCommandError::PropertyTextOwned {
            footprint: id,
            key: old.key.clone(),
        });
    }
    if fp.component.lock_strings && moves_text(old, &property) {
        return Err(BoardCommandError::StringsLocked(id));
    }
    let previous = std::mem::replace(&mut fp.properties[index], property);
    Ok(Applied {
        inverse: BoardCommand::SetFootprintProperty {
            footprint: id,
            index,
            property: previous,
        },
        patch: patch(BoardPatchKind::FootprintPropertySet, id),
    })
}

/// Whether `new` puts the text of `old` somewhere else: another position,
/// angle, layer, size or justification. Lock Strings refuses exactly these
/// (#2365, #2368).
fn moves_text(old: &PcbProperty, new: &PcbProperty) -> bool {
    old.position != new.position
        || old.rotation != new.rotation
        || old.layer != new.layer
        || old.font_size != new.font_size
        || old.justify != new.justify
}

/// Insert a parameter, see [`BoardCommand::InsertFootprintProperty`].
pub(super) fn insert_property(
    board: &mut PcbBoard,
    id: Uuid,
    index: usize,
    property: PcbProperty,
) -> Result<Applied, BoardCommandError> {
    check_property_finite(&property)?;
    if property.key.trim().is_empty() {
        return Err(BoardCommandError::PropertyKeyEmpty);
    }
    if is_text_key(&property.key) {
        return Err(BoardCommandError::PropertyTextOwned {
            footprint: id,
            key: property.key,
        });
    }
    let fp = find(board, id)?;
    if index > fp.properties.len() {
        return Err(BoardCommandError::PropertyNotFound {
            footprint: id,
            index,
        });
    }
    fp.properties.insert(index, property);
    Ok(Applied {
        inverse: BoardCommand::RemoveFootprintProperty {
            footprint: id,
            index,
        },
        patch: patch(BoardPatchKind::FootprintPropertySet, id),
    })
}

/// Remove a parameter, see [`BoardCommand::RemoveFootprintProperty`].
pub(super) fn remove_property(
    board: &mut PcbBoard,
    id: Uuid,
    index: usize,
) -> Result<Applied, BoardCommandError> {
    let fp = find(board, id)?;
    let Some(old) = fp.properties.get(index) else {
        return Err(BoardCommandError::PropertyNotFound {
            footprint: id,
            index,
        });
    };
    if is_text_key(&old.key) {
        return Err(BoardCommandError::PropertyTextOwned {
            footprint: id,
            key: old.key.clone(),
        });
    }
    let removed = fp.properties.remove(index);
    Ok(Applied {
        inverse: BoardCommand::InsertFootprintProperty {
            footprint: id,
            index,
            property: removed,
        },
        patch: patch(BoardPatchKind::FootprintPropertySet, id),
    })
}

/// Whether a property of `key` shows the designator or the value.
pub(super) fn is_text_key(key: &str) -> bool {
    pcb_geom::is_board_text_key(key)
}

/// The key of the property that shows `field` on the board.
pub(super) fn text_key(field: super::designators::FootprintTextField) -> &'static str {
    match field {
        super::designators::FootprintTextField::Designator => REFERENCE_KEY,
        super::designators::FootprintTextField::Value => VALUE_KEY,
    }
}

fn check_property_finite(property: &PcbProperty) -> Result<(), BoardCommandError> {
    let position_ok = property
        .position
        .is_none_or(|p| p.x.is_finite() && p.y.is_finite());
    if !position_ok {
        return Err(BoardCommandError::NonFinite {
            field: "property.position",
        });
    }
    if !property.rotation.is_finite() {
        return Err(BoardCommandError::NonFinite {
            field: "property.rotation",
        });
    }
    if property.font_size.is_some_and(|size| !size.is_finite()) {
        return Err(BoardCommandError::NonFinite {
            field: "property.font_size",
        });
    }
    Ok(())
}

/// Give the n-th pad of `new` with a number the net and lock of the n-th pad
/// of `old` with that number, so a lock on any of several `EP` pads stays on
/// its pad. A number `old` lacks gets no net. Refused, before anything is
/// written, when `new` has fewer pads of a number than `old` and one of the
/// old pads left over holds a net or a lock ([`carry_would_lose`]); a count
/// difference that loses nothing is carried as far as it goes.
fn carry_pad_nets(old: &Footprint, new: &mut Footprint) -> Result<(), BoardCommandError> {
    if let Some(lost) = carry_would_lose(old, new) {
        return Err(lost);
    }
    let mut old_by_number: HashMap<&str, Vec<&Pad>> = HashMap::new();
    for pad in &old.pads {
        old_by_number.entry(&pad.number).or_default().push(pad);
    }
    let carried: Vec<(Option<PadNet>, bool)> = {
        let mut seen: HashMap<&str, usize> = HashMap::new();
        new.pads
            .iter()
            .map(|pad| {
                let nth = seen.entry(&pad.number).or_default();
                let from = old_by_number
                    .get(pad.number.as_str())
                    .and_then(|olds| olds.get(*nth).copied());
                *nth += 1;
                (
                    from.and_then(|p| p.net.clone()),
                    from.is_some_and(|p| p.net_locked),
                )
            })
            .collect()
    };
    for (pad, (net, locked)) in new.pads.iter_mut().zip(carried) {
        pad.net = net;
        pad.net_locked = locked;
    }
    Ok(())
}

/// The pads of `old` whose number `new` lacks: a replace of `old` by `new`
/// drops them, with their nets and locks. Shared by the ECO notes and the
/// board's footprint swap report.
pub(crate) fn unmatched_pads<'a>(old: &'a Footprint, new: &Footprint) -> Vec<&'a Pad> {
    let numbers: HashSet<&str> = new.pads.iter().map(|p| p.number.as_str()).collect();
    old.pads
        .iter()
        .filter(|pad| !numbers.contains(pad.number.as_str()))
        .collect()
}

/// The refusal a replace of `old` by `new` would meet, if any: `new` has
/// fewer pads of a number `old` also has, and an old pad past the new count
/// holds a net or a lock that would be dropped with it. A number `new` lacks
/// entirely is not a refusal: its pads go, locks included, which the change
/// list reports. Shared with the ECO diff so a change row is blocked before
/// it is run.
pub(crate) fn carry_would_lose(old: &Footprint, new: &Footprint) -> Option<BoardCommandError> {
    let mut old_by_number: HashMap<&str, Vec<&Pad>> = HashMap::new();
    for pad in &old.pads {
        old_by_number.entry(&pad.number).or_default().push(pad);
    }
    let mut new_counts: HashMap<&str, usize> = HashMap::new();
    for pad in &new.pads {
        *new_counts.entry(&pad.number).or_default() += 1;
    }
    let mut numbers: Vec<(&str, usize)> = new_counts.into_iter().collect();
    numbers.sort_unstable();
    for (number, count) in numbers {
        let Some(olds) = old_by_number.get(number) else {
            continue;
        };
        let dropped = olds.iter().skip(count);
        if olds.len() > count && dropped.clone().any(|p| p.net.is_some() || p.net_locked) {
            return Some(BoardCommandError::PadCountMismatch {
                footprint: old.uuid,
                number: number.to_string(),
                old: olds.len(),
                new: count,
            });
        }
    }
    None
}

/// The properties of a replaced footprint: each template property takes the
/// placement (position, rotation, layer, size, hidden flag, justification)
/// of the first old property of its key not yet matched, so a moved or
/// hidden designator stays as the user left it; a template-only key comes as the template has it; an
/// old property left unmatched (a user-added MPN, say) is kept after them.
/// Runs on the board-side template, after any flip.
fn merge_properties(old: &[PcbProperty], template: Vec<PcbProperty>) -> Vec<PcbProperty> {
    let mut unmatched: Vec<Option<&PcbProperty>> = old.iter().map(Some).collect();
    let mut merged: Vec<PcbProperty> = template
        .into_iter()
        .map(|mut property| {
            let previous = unmatched
                .iter_mut()
                .find(|slot| slot.is_some_and(|p| p.key == property.key))
                .and_then(Option::take);
            if let Some(previous) = previous {
                property.position = previous.position;
                property.rotation = previous.rotation;
                property.layer.clone_from(&previous.layer);
                property.font_size = previous.font_size;
                property.hidden = previous.hidden;
                property.justify = previous.justify;
            }
            property
        })
        .collect();
    merged.extend(unmatched.into_iter().flatten().cloned());
    merged
}

/// Write the designator and value into the Reference and Value property texts.
pub(super) fn sync_text_properties(fp: &mut Footprint) {
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
