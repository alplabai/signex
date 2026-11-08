//! Re-keying waivers when a sheet's document id changes (#2172).
//!
//! A net-level waiver names its net on a sheet by that sheet's document id
//! (`net:<name>@<id>`, `pins:<pins>@<id>`). A Save As gives the copy a new id
//! and a repaired duplicate gets one too, so the waivers filed under the old
//! id move to the new one, or the findings they accepted would show again.

use signex_types::project::ErcWaiver;
use uuid::Uuid;

/// Move every sheet-scoped object of `waivers` from `old` to `new`, keeping
/// each waiver's objects sorted as the key builds them. Returns how many
/// waivers changed.
pub fn rekey_sheet(waivers: &mut [ErcWaiver], old: Uuid, new: Uuid) -> usize {
    let (from, to) = (format!("@{old}"), format!("@{new}"));
    let scoped = |object: &str| {
        (object.starts_with("net:") || object.starts_with("pins:")) && object.ends_with(&from)
    };
    let mut changed = 0;
    for waiver in waivers {
        if !waiver.objects.iter().any(|o| scoped(o)) {
            continue;
        }
        for object in waiver.objects.iter_mut().filter(|o| scoped(o)) {
            object.truncate(object.len() - from.len());
            object.push_str(&to);
        }
        waiver.objects.sort();
        changed += 1;
    }
    changed
}

/// Add, beside every waiver filed under `old`, a copy filed under `new`. For
/// a repaired duplicate: two sheets shared the id, so which of them a waiver
/// was made on is unknown, and both keep what was accepted. Returns how many
/// copies were added.
pub fn copy_sheet_waivers(waivers: &mut Vec<ErcWaiver>, old: Uuid, new: Uuid) -> usize {
    let mut copies: Vec<ErcWaiver> = waivers
        .iter()
        .filter(|waiver| {
            let tail = format!("@{old}");
            waiver
                .objects
                .iter()
                .any(|o| (o.starts_with("net:") || o.starts_with("pins:")) && o.ends_with(&tail))
        })
        .cloned()
        .collect();
    rekey_sheet(&mut copies, old, new);
    let added = copies.len();
    waivers.extend(copies);
    added
}
