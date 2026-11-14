//! The words of the Update PCB dialog: a group's heading, a row, why a row
//! is blocked and a note, each from the catalog.

use signex_engine::pcb::eco::{ChangeReason, EcoBlock, EcoChange, EcoGroup, EcoNote};
use signex_i18n::{I18n, Localized, MessageId, ids, t};
use signex_types::pcb::PadNet;

pub(super) fn group_id(group: EcoGroup) -> MessageId {
    match group {
        EcoGroup::AddFootprints => ids::UPDATE_PCB_GROUP_ADD_FOOTPRINTS,
        EcoGroup::RemoveFootprints => ids::UPDATE_PCB_GROUP_REMOVE_FOOTPRINTS,
        EcoGroup::ChangeFootprints => ids::UPDATE_PCB_GROUP_CHANGE_FOOTPRINTS,
        EcoGroup::Designators => ids::UPDATE_PCB_GROUP_DESIGNATORS,
        EcoGroup::Values => ids::UPDATE_PCB_GROUP_VALUES,
        EcoGroup::Links => ids::UPDATE_PCB_GROUP_LINKS,
        EcoGroup::Nets => ids::UPDATE_PCB_GROUP_NETS,
        EcoGroup::PadNets => ids::UPDATE_PCB_GROUP_PAD_NETS,
    }
}

fn net_name(i18n: &I18n, net: &Option<PadNet>) -> String {
    match net {
        Some(net) => net.name.clone(),
        None => t!(i18n, ids::UPDATE_PCB_NO_NET).into_string(),
    }
}

/// What one row does.
pub(super) fn change(i18n: &I18n, change: &EcoChange) -> Localized {
    match change {
        EcoChange::AddFootprint { footprint } => t!(
            i18n,
            ids::UPDATE_PCB_ROW_ADD,
            "designator" => footprint.reference.clone(),
            "value" => footprint.value.clone(),
            "footprint" => footprint.footprint_id.clone()
        ),
        EcoChange::RemoveFootprint { reference, .. } => {
            t!(i18n, ids::UPDATE_PCB_ROW_REMOVE, "designator" => reference.clone())
        }
        EcoChange::ChangeFootprint {
            reference,
            from,
            reason,
            with,
            ..
        } => t!(
            i18n,
            match reason {
                ChangeReason::Footprint => ids::UPDATE_PCB_ROW_CHANGE,
                ChangeReason::OtherVersion => ids::UPDATE_PCB_ROW_CHANGE_VERSION,
            },
            "designator" => reference.clone(),
            "from" => from.clone(),
            "to" => with.footprint_id.clone()
        ),
        EcoChange::SetDesignator { from, to, .. } => t!(
            i18n,
            ids::UPDATE_PCB_ROW_DESIGNATOR,
            "from" => from.clone(),
            "to" => to.clone()
        ),
        EcoChange::SetValue {
            reference,
            from,
            to,
            ..
        } => t!(
            i18n,
            ids::UPDATE_PCB_ROW_VALUE,
            "designator" => reference.clone(),
            "from" => from.clone(),
            "to" => to.clone()
        ),
        EcoChange::Link { reference, .. } => {
            t!(i18n, ids::UPDATE_PCB_ROW_LINK, "designator" => reference.clone())
        }
        EcoChange::AddNet { name, .. } => {
            t!(i18n, ids::UPDATE_PCB_ROW_ADD_NET, "net" => name.clone())
        }
        EcoChange::RemoveNet { name, .. } => {
            t!(i18n, ids::UPDATE_PCB_ROW_REMOVE_NET, "net" => name.clone())
        }
        EcoChange::RenameNet { from, to, .. } => t!(
            i18n,
            ids::UPDATE_PCB_ROW_RENAME_NET,
            "from" => from.clone(),
            "to" => to.clone()
        ),
        EcoChange::PadNet {
            reference,
            pad_number,
            from,
            to,
            locked,
            ..
        } => t!(
            i18n,
            if *locked {
                ids::UPDATE_PCB_ROW_PAD_NET_LOCKED
            } else {
                ids::UPDATE_PCB_ROW_PAD_NET
            },
            "designator" => reference.clone(),
            "pad" => pad_number.clone(),
            "from" => net_name(i18n, from),
            "to" => net_name(i18n, to)
        ),
    }
}

/// Why a row cannot be ticked.
pub(super) fn blocked(i18n: &I18n, why: &EcoBlock) -> Localized {
    match why {
        EcoBlock::MissingFootprint(error) => t!(
            i18n,
            ids::UPDATE_PCB_BLOCKED_MISSING_FOOTPRINT,
            "why" => error.to_string()
        ),
        EcoBlock::NeedsAnnotation => t!(i18n, ids::UPDATE_PCB_BLOCKED_ANNOTATE),
        EcoBlock::DuplicateDesignator(designator) => t!(
            i18n,
            ids::UPDATE_PCB_BLOCKED_DUPLICATE,
            "designator" => designator.clone()
        ),
        EcoBlock::PadsCannotCarry { number } => {
            t!(i18n, ids::UPDATE_PCB_BLOCKED_PADS, "pad" => number.clone())
        }
        EcoBlock::Unplaceable(why) => {
            t!(i18n, ids::UPDATE_PCB_BLOCKED_UNPLACEABLE, "why" => why.clone())
        }
        EcoBlock::PartNotRead => t!(i18n, ids::UPDATE_PCB_BLOCKED_NOT_READ),
    }
}

/// Whether a blocked row is fixed by annotating the schematic.
pub(super) fn needs_annotate(why: &EcoBlock) -> bool {
    matches!(
        why,
        EcoBlock::NeedsAnnotation | EcoBlock::DuplicateDesignator(_)
    )
}

/// One note under the rows.
pub(super) fn note(i18n: &I18n, note: &EcoNote) -> Localized {
    match note {
        EcoNote::UnreadableSheet(_) => t!(i18n, ids::UPDATE_PCB_NOTE_UNREADABLE_SHEET),
        EcoNote::PadLosesLock {
            reference,
            pad_number,
            ..
        } => t!(
            i18n,
            ids::UPDATE_PCB_NOTE_PAD_LOSES_LOCK,
            "designator" => reference.clone(),
            "pad" => pad_number.clone()
        ),
        EcoNote::PadLosesNet {
            reference,
            pad_number,
            net,
            ..
        } => t!(
            i18n,
            ids::UPDATE_PCB_NOTE_PAD_LOSES_NET,
            "designator" => reference.clone(),
            "pad" => pad_number.clone(),
            "net" => net.clone()
        ),
        EcoNote::PartNotRead { designator } => t!(
            i18n,
            ids::UPDATE_PCB_NOTE_PART_NOT_READ,
            "designator" => designator.clone()
        ),
        EcoNote::FootprintUnresolved {
            reference, error, ..
        } => t!(
            i18n,
            ids::UPDATE_PCB_NOTE_FOOTPRINT_UNRESOLVED,
            "designator" => reference.clone(),
            "why" => error.to_string()
        ),
        EcoNote::KeptUnlinked { reference, .. } => t!(
            i18n,
            ids::UPDATE_PCB_NOTE_KEPT_UNLINKED,
            "designator" => reference.clone()
        ),
        EcoNote::ExtraLink {
            reference,
            designator,
            ..
        } => t!(
            i18n,
            ids::UPDATE_PCB_NOTE_EXTRA_LINK,
            "reference" => reference.clone(),
            "designator" => designator.clone()
        ),
        EcoNote::Net(text) => t!(i18n, ids::UPDATE_PCB_NOTE_NET, "text" => text.clone()),
    }
}
