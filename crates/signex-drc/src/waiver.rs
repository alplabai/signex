//! The name a waiver gives a finding (#2399): the same text for the same
//! finding in every run and wherever the board file lives, so a waiver kept
//! in the board's options finds its finding again. Built from the rule (and
//! the kind, for a written rule), the items and the keepout, as the check
//! report's key is, but without the board path. A finding that names no
//! object (an unconnected link with no pad at either end) is named by where
//! it is instead.

use sha2::{Digest, Sha256};
use signex_types::pcb_options::DrcWaiver;

use crate::Violation;
use crate::findings::{item_label, rule_of};

/// Hex digits of the key: 128 bits of the hash.
const KEY_HEX_DIGITS: usize = 32;

/// The rule id `v` is filed under: a written rule's `user::<name>`, else
/// its kind's `builtin::<kind>`.
pub fn rule_id(v: &Violation) -> &str {
    rule_of(v)
}

/// The waiver key of `v`.
pub fn waiver_key(v: &Violation) -> String {
    let mut hasher = Sha256::new();
    hasher.update(rule_of(v).as_bytes());
    if v.rule.is_some() {
        hasher.update(b"\nkind:");
        hasher.update(v.kind.rule_id().as_bytes());
    }
    for item in &v.items {
        hasher.update(b"\nitem:");
        hasher.update(item_label(item).as_bytes());
    }
    if let Some(keepout) = v.keepout {
        hasher.update(b"\nkeepout:");
        hasher.update(keepout.to_string().as_bytes());
    }
    if v.items.is_empty() && v.keepout.is_none() {
        hasher.update(format!("\nat:{},{}", v.at.x, v.at.y).as_bytes());
    }
    let mut hex = format!("{:x}", hasher.finalize());
    hex.truncate(KEY_HEX_DIGITS);
    hex
}

/// The waiver of `v` among `waivers`, if one names it.
pub fn waiver_of<'a>(waivers: &'a [DrcWaiver], v: &Violation) -> Option<&'a DrcWaiver> {
    let key = waiver_key(v);
    waivers.iter().find(|w| w.key == key)
}

#[cfg(test)]
#[path = "waiver_tests.rs"]
mod tests;
