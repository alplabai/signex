//! Every name a stitched net carries, with where each one sits (#1061).
//!
//! The stitcher picks one exported name per net; this module keeps the rest of
//! the evidence so ERC can say which names a net has across sheets and which
//! one won. The naming rule is in `docs/NET_NAMING.md`.

use signex_types::schematic::{LabelType, Point};
use uuid::Uuid;

use super::{Analysis, L2, Occ, SheetKey};
use crate::build::pt_key;
use signex_types::net::Net;

/// One place a net gets a name from.
#[derive(Debug, Clone, PartialEq)]
pub struct NetNameSource {
    /// The text as written on the sheet (not qualified).
    pub name: String,
    /// `None` for a power-port pin, which names the net like a Power label.
    pub label_type: Option<LabelType>,
    /// The sheet the name sits on.
    pub sheet: SheetKey,
    /// The `ChildSheet.name` chain from the root; empty for a root sheet.
    pub sheet_path: Vec<String>,
    /// Position on that sheet.
    pub at: Point,
    /// The label's uuid, or the power-port symbol's uuid when `label_type` is
    /// `None`.
    pub uuid: Uuid,
}

impl NetNameSource {
    /// The name as the export would write it if this source won: a
    /// Hierarchical or local name off the root is prefixed by its sheet chain.
    pub fn exported_text(&self) -> String {
        let scoped = matches!(
            self.label_type,
            Some(LabelType::Hierarchical | LabelType::Net)
        );
        if scoped && !self.sheet_path.is_empty() {
            format!("{}/{}", self.sheet_path.join("/"), self.name)
        } else {
            self.name.clone()
        }
    }
}

/// A net with two or more different names, in the project netlist.
#[derive(Debug, Clone, PartialEq)]
pub struct MultiNamedNet {
    /// The name the netlist export uses for this net, dedup suffix included;
    /// for a net with no terminal, the name it would get (no net is exported).
    pub exported: String,
    /// `false` for a net with no terminal: the export writes no net for it.
    pub has_terminals: bool,
    /// Every name source, sorted by name, sheet path, then position.
    pub sources: Vec<NetNameSource>,
    /// Index into `sources` of the name that won, before any dedup suffix.
    pub winner: usize,
}

/// The name sources of one group, with the index of the winning one.
pub(super) struct NameReport {
    sources: Vec<NetNameSource>,
    winner: usize,
}

impl NameReport {
    fn into_net(self, exported: String, has_terminals: bool) -> MultiNamedNet {
        MultiNamedNet {
            exported,
            has_terminals,
            sources: self.sources,
            winner: self.winner,
        }
    }
}

/// The reports of the exported nets (`multi`: index into `nets`, read after the
/// dedup pass so a suffixed name is reported as the export shows it), then the
/// terminal-less groups (`pinless`: the name each would get), in a fixed order.
pub(super) fn collect_reports(
    multi: Vec<(usize, NameReport)>,
    mut pinless: Vec<(String, NameReport)>,
    nets: &[Net],
) -> Vec<MultiNamedNet> {
    let mut out: Vec<MultiNamedNet> = multi
        .into_iter()
        .map(|(idx, r)| r.into_net(nets[idx].name.clone(), true))
        .collect();
    // Group iteration order is a hash order; pin it.
    pinless.sort_by_key(|(name, r)| {
        let first = &r.sources[0];
        (name.clone(), first.sheet.clone(), pt_key(&first.at))
    });
    out.extend(pinless.into_iter().map(|(name, r)| r.into_net(name, false)));
    out
}

/// The name sources of one level-2 group whose winning name (as qualified by
/// the stitcher) is `winner`, or `None` when it has fewer than two different
/// names.
pub(super) fn multi_name_sources(
    members: &[L2],
    occs: &[Occ],
    analyses: &[Analysis],
    winner: Option<&str>,
) -> Option<NameReport> {
    let mut out: Vec<NetNameSource> = Vec::new();
    for &(oid, root) in members {
        let occ = &occs[oid];
        let source = |name: &str, label_type, at, uuid| NetNameSource {
            name: name.to_string(),
            label_type,
            sheet: occ.key.clone(),
            sheet_path: occ.name_chain.clone(),
            at,
            uuid,
        };
        for l in analyses[oid].net_labels.get(&root).into_iter().flatten() {
            if !l.text.is_empty() {
                out.push(source(&l.text, Some(l.label_type), l.position, l.uuid));
            }
        }
        for (v, tip, id) in analyses[oid].power_by_root.get(&root).into_iter().flatten() {
            if !v.is_empty() {
                out.push(source(v, None, *tip, *id));
            }
        }
    }
    // A child placed twice reports the same name at the same spot once.
    let mut seen = std::collections::HashSet::new();
    out.retain(|s| seen.insert((s.name.clone(), s.sheet.clone(), pt_key(&s.at))));
    out.sort_by(|a, b| {
        (&a.name, &a.sheet_path, pt_key(&a.at)).cmp(&(&b.name, &b.sheet_path, pt_key(&b.at)))
    });
    let mut texts: Vec<&str> = out.iter().map(|s| s.name.as_str()).collect();
    texts.dedup();
    if texts.len() < 2 {
        return None;
    }
    let winner = winner
        .and_then(|w| out.iter().position(|s| s.exported_text() == w))
        .unwrap_or(0);
    Some(NameReport {
        sources: out,
        winner,
    })
}
