//! The words of the Update PCB dialog: a group's heading, a row's action,
//! detail and whole sentence, why a row is blocked (short and in full) and
//! a note, each from the catalog. A row's object is the model's
//! (`app/update_pcb/table.rs`), since the filter field searches it.

use signex_engine::pcb::eco::{ChangeReason, EcoBlock, EcoChange, EcoNote};
use signex_i18n::{I18n, Localized, MessageId, ids, t};
use signex_types::pcb::PadNet;

use crate::app::update_pcb::{RowAction, TableGroup};

/// The value a part shows when it has none of its own.
const PLACEHOLDER_VALUE: &str = "*";

pub(super) fn group_id(group: TableGroup) -> MessageId {
    match group {
        TableGroup::Components => ids::UPDATE_PCB_TABLE_COMPONENTS,
        TableGroup::Nets => ids::UPDATE_PCB_TABLE_NETS,
        TableGroup::PadNets => ids::UPDATE_PCB_TABLE_PAD_NETS,
    }
}

/// The word of an action, for its pill.
pub(super) fn action(action: RowAction) -> MessageId {
    match action {
        RowAction::Add => ids::UPDATE_PCB_ACTION_ADD,
        RowAction::Change => ids::UPDATE_PCB_ACTION_CHANGE,
        RowAction::Remove => ids::UPDATE_PCB_ACTION_REMOVE,
    }
}

fn net_name(i18n: &I18n, net: &Option<PadNet>) -> String {
    match net {
        Some(net) => net.name.clone(),
        None => t!(i18n, ids::UPDATE_PCB_NO_NET).into_string(),
    }
}

/// The row's Detail column: the footprint (with the value unless it is the
/// placeholder), or what changes from what to what.
pub(super) fn detail(i18n: &I18n, change: &EcoChange) -> String {
    let from_to = |id: MessageId, from: &str, to: &str| {
        t!(i18n, id, "from" => from.to_string(), "to" => to.to_string()).into_string()
    };
    match change {
        EcoChange::AddFootprint { footprint } => {
            let value = footprint.value.trim();
            if value.is_empty() || value == PLACEHOLDER_VALUE {
                footprint.footprint_id.clone()
            } else {
                format!("{value} · {}", footprint.footprint_id)
            }
        }
        EcoChange::RemoveFootprint { .. }
        | EcoChange::AddNet { .. }
        | EcoChange::RemoveNet { .. } => String::new(),
        EcoChange::ChangeFootprint {
            from, reason, with, ..
        } => from_to(
            match reason {
                ChangeReason::Footprint => ids::UPDATE_PCB_DETAIL_FROM_TO,
                ChangeReason::OtherVersion => ids::UPDATE_PCB_DETAIL_OTHER_VERSION,
            },
            from,
            &with.footprint_id,
        ),
        EcoChange::SetDesignator { from, to, .. } => {
            from_to(ids::UPDATE_PCB_DETAIL_DESIGNATOR, from, to)
        }
        EcoChange::SetValue { from, to, .. } => from_to(ids::UPDATE_PCB_DETAIL_VALUE, from, to),
        EcoChange::Link { .. } => t!(i18n, ids::UPDATE_PCB_DETAIL_LINK).into_string(),
        EcoChange::RenameNet { from, to, .. } => from_to(ids::UPDATE_PCB_DETAIL_FROM_TO, from, to),
        EcoChange::PadNet {
            from, to, locked, ..
        } => {
            let (from, to) = (net_name(i18n, from), net_name(i18n, to));
            let id = if *locked {
                ids::UPDATE_PCB_DETAIL_LOCKED
            } else {
                ids::UPDATE_PCB_DETAIL_FROM_TO
            };
            from_to(id, &from, &to)
        }
    }
}

/// Why a row cannot be ticked, in a few words for its Status column.
pub(super) fn problem(i18n: &I18n, why: &EcoBlock) -> Localized {
    match why {
        EcoBlock::MissingFootprint(_) => t!(i18n, ids::UPDATE_PCB_PROBLEM_NO_FOOTPRINT),
        EcoBlock::NeedsAnnotation => t!(i18n, ids::UPDATE_PCB_PROBLEM_NOT_ANNOTATED),
        EcoBlock::DuplicateDesignator(designator) => t!(
            i18n,
            ids::UPDATE_PCB_PROBLEM_DUPLICATE,
            "designator" => designator.clone()
        ),
        EcoBlock::PadsCannotCarry { .. } => t!(i18n, ids::UPDATE_PCB_PROBLEM_PADS),
        EcoBlock::Unplaceable(_) => t!(i18n, ids::UPDATE_PCB_PROBLEM_UNPLACEABLE),
        EcoBlock::PartNotRead => t!(i18n, ids::UPDATE_PCB_PROBLEM_NOT_READ),
    }
}

/// What one row does, as a sentence (the row's tooltip).
pub(super) fn change(i18n: &I18n, change: &EcoChange) -> Localized {
    match change {
        EcoChange::AddFootprint { footprint } => t!(
            i18n,
            ids::UPDATE_PCB_ROW_ADD,
            "designator" => footprint.reference.clone(),
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

/// Why a row cannot be ticked. Compare Board with Schematic says it too.
pub(in crate::app::view::dialogs) fn blocked(i18n: &I18n, why: &EcoBlock) -> Localized {
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

/// One note of the Notes group. Compare Board with Schematic shows it too.
pub(in crate::app::view::dialogs) fn note(i18n: &I18n, note: &EcoNote) -> Localized {
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
