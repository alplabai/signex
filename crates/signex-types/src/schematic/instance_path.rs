//! Per-placement designators (#829).
//!
//! A child sheet file placed twice is two occurrences of every symbol in it.
//! An occurrence is named by the chain of [`ChildSheet`](super::ChildSheet)
//! uuids walked from the root page down to it; a symbol's
//! [`SymbolInstance`](super::SymbolInstance) entries map such a chain to the
//! designator it takes there.

use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Symbol;

/// The `ChildSheet.uuid` chain from the walk root to one occurrence of a sheet.
/// A root page is the empty chain.
///
/// String form (the `.snxsch` `path` convention): `/` followed by the uuids
/// joined with `/`, e.g. `/<uuid-a>/<uuid-b>`. The root chain is `/`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct InstancePath(Vec<Uuid>);

impl InstancePath {
    /// The empty chain: a root page.
    pub fn root() -> Self {
        Self(Vec::new())
    }

    pub fn from_chain(chain: Vec<Uuid>) -> Self {
        Self(chain)
    }

    pub fn chain(&self) -> &[Uuid] {
        &self.0
    }

    pub fn is_root(&self) -> bool {
        self.0.is_empty()
    }

    /// This path extended by one child placement.
    pub fn child(&self, child_sheet: Uuid) -> Self {
        let mut chain = self.0.clone();
        chain.push(child_sheet);
        Self(chain)
    }

    /// The placement one level up: this path without its last child
    /// placement. `None` for the root chain, which has no parent.
    pub fn parent(&self) -> Option<Self> {
        let (_, up) = self.0.split_last()?;
        Some(Self(up.to_vec()))
    }

    /// Parse the `/`-joined form. Only `"/"` is the root; an empty string is
    /// an unset path (a default `SymbolInstance`), not a placement. Anything
    /// malformed (no leading `/`, empty or non-uuid segment) is `None`.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        if text == "/" {
            return Some(Self::root());
        }
        let rest = text.strip_prefix('/')?;
        let chain = rest
            .split('/')
            .map(|seg| Uuid::parse_str(seg).ok())
            .collect::<Option<Vec<_>>>()?;
        Some(Self(chain))
    }
}

impl fmt::Display for InstancePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_empty() {
            return f.write_str("/");
        }
        for id in &self.0 {
            write!(f, "/{id}")?;
        }
        Ok(())
    }
}

impl Symbol {
    /// The designator this symbol carries at `path`: the first
    /// [`SymbolInstance`](super::SymbolInstance) entry whose parsed path equals
    /// `path` and whose designator is non-blank (returned trimmed), else
    /// `reference`. The rule does not depend on how many times the file is
    /// placed, so removing one placement never changes what the others resolve
    /// to. Entries with an unparseable path or a blank designator never match.
    /// `SymbolInstance.unit` and `SymbolInstance.project` are ignored in this
    /// version: the symbol's own `unit` stands, and the project name is not
    /// compared.
    pub fn designator_in(&self, path: &InstancePath) -> &str {
        self.instance_entry_in(path)
            .unwrap_or(self.reference.as_str())
    }

    /// Whether [`designator_in`](Self::designator_in) at `path` comes from an
    /// instance entry rather than the `reference` fallback.
    pub fn has_instance_entry_in(&self, path: &InstancePath) -> bool {
        self.instance_entry_in(path).is_some()
    }

    /// Whether the entry at `path` gives this placement a designator that
    /// differs from `reference`; an entry that merely mirrors `reference`
    /// changes nothing.
    pub fn retargets_designator_in(&self, path: &InstancePath) -> bool {
        self.designator_in(path) != self.reference
    }

    /// Every instance entry the resolver honours: a parseable path and a
    /// non-blank designator (trimmed), in file order. The one place that rule
    /// lives; [`designator_in`](Self::designator_in) and the expression
    /// tables both read through it.
    pub fn valid_instance_entries(&self) -> impl Iterator<Item = (InstancePath, &str)> {
        self.instances.iter().filter_map(|e| {
            let reference = e.reference.trim();
            if reference.is_empty() {
                return None;
            }
            Some((InstancePath::parse(&e.path)?, reference))
        })
    }

    fn instance_entry_in(&self, path: &InstancePath) -> Option<&str> {
        self.valid_instance_entries()
            .find(|(p, _)| p == path)
            .map(|(_, r)| r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schematic::SymbolInstance;

    fn id(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    fn entry(path: &str, reference: &str) -> SymbolInstance {
        SymbolInstance {
            project: "p".into(),
            path: path.into(),
            reference: reference.into(),
            unit: 1,
        }
    }

    fn symbol(reference: &str, instances: Vec<SymbolInstance>) -> Symbol {
        Symbol {
            reference: reference.into(),
            instances,
            ..Default::default()
        }
    }

    #[test]
    fn the_parent_drops_the_last_placement_and_the_root_has_none() {
        let path = InstancePath::from_chain(vec![id(1), id(2)]);
        assert_eq!(path.parent(), Some(InstancePath::from_chain(vec![id(1)])));
        assert_eq!(
            InstancePath::from_chain(vec![id(1)]).parent(),
            Some(InstancePath::root())
        );
        assert_eq!(InstancePath::root().parent(), None);
    }

    #[test]
    fn display_and_parse_round_trip() {
        let p = InstancePath::from_chain(vec![id(1), id(2)]);
        let text = p.to_string();
        assert_eq!(text, format!("/{}/{}", id(1), id(2)));
        assert_eq!(InstancePath::parse(&text), Some(p));
        assert_eq!(InstancePath::root().to_string(), "/");
        assert_eq!(InstancePath::parse("/"), Some(InstancePath::root()));
        assert_eq!(InstancePath::parse(""), None);
    }

    #[test]
    fn malformed_paths_parse_to_none() {
        for bad in [
            "abc",
            "/not-a-uuid",
            "//",
            &format!("/{}/", id(1)),
            &format!("{}", id(1)),
        ] {
            assert_eq!(InstancePath::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn designator_in_prefers_matching_entry() {
        let a = InstancePath::from_chain(vec![id(1)]);
        let b = InstancePath::from_chain(vec![id(2)]);
        let s = symbol("R1", vec![entry(&b.to_string(), "R101")]);
        assert_eq!(s.designator_in(&b), "R101");
        assert!(s.has_instance_entry_in(&b));
        assert_eq!(s.designator_in(&a), "R1");
        assert!(!s.has_instance_entry_in(&a));
    }

    #[test]
    fn a_default_entry_or_blank_reference_never_resolves_to_blank() {
        let blank_path = SymbolInstance {
            project: "x".into(),
            ..Default::default()
        };
        let blank_ref = entry("/", "  ");
        let s = symbol("R1", vec![blank_path, blank_ref]);
        assert_eq!(s.designator_in(&InstancePath::root()), "R1");
        assert!(!s.has_instance_entry_in(&InstancePath::root()));
    }

    #[test]
    fn the_entry_designator_is_returned_trimmed() {
        let a = InstancePath::from_chain(vec![id(1)]);
        let s = symbol("R1", vec![entry(&a.to_string(), "  R7 ")]);
        assert_eq!(s.designator_in(&a), "R7");
    }

    #[test]
    fn only_an_entry_that_changes_the_designator_retargets() {
        let a = InstancePath::from_chain(vec![id(1)]);
        let b = InstancePath::from_chain(vec![id(2)]);
        let s = symbol(
            "R1",
            vec![entry(&a.to_string(), "R1"), entry(&b.to_string(), "R101")],
        );
        assert!(!s.retargets_designator_in(&a));
        assert!(s.retargets_designator_in(&b));
    }

    #[test]
    fn designator_in_ignores_malformed_entry_and_root_matches_empty_chain() {
        let s = symbol("R1", vec![entry("garbage", "R9"), entry("/", "R7")]);
        assert_eq!(s.designator_in(&InstancePath::root()), "R7");
        assert_eq!(
            s.designator_in(&InstancePath::from_chain(vec![id(1)])),
            "R1"
        );
        assert_eq!(s.instances.len(), 2, "raw entry kept");
    }
}
