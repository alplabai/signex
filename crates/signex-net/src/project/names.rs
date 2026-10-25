//! Every name a stitched net carries, with where each one sits (#1061).
//!
//! The stitcher picks one exported name per net; this module keeps the rest of
//! the evidence so ERC can say which names a net has across sheets and which
//! one won. The naming rule is in `docs/NET_NAMING.md`.

use signex_types::schematic::{LabelType, Point};
use uuid::Uuid;

use super::{Analysis, L2, Occ, SheetKey};
use crate::build::pt_key;

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
    /// The label's uuid; `None` for a power-port pin.
    pub label: Option<Uuid>,
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
    /// The name the netlist export uses for this net.
    pub exported: String,
    /// Every name source, sorted by name, sheet path, then position.
    pub sources: Vec<NetNameSource>,
}

/// The name sources of one level-2 group, or empty when it has fewer than two
/// different names.
pub(super) fn multi_name_sources(
    members: &[L2],
    occs: &[Occ],
    analyses: &[Analysis],
) -> Vec<NetNameSource> {
    let mut out: Vec<NetNameSource> = Vec::new();
    for &(oid, root) in members {
        let occ = &occs[oid];
        let source = |name: &str, label_type, at, label| NetNameSource {
            name: name.to_string(),
            label_type,
            sheet: occ.key.clone(),
            sheet_path: occ.name_chain.clone(),
            at,
            label,
        };
        for l in analyses[oid].net_labels.get(&root).into_iter().flatten() {
            if !l.text.is_empty() {
                out.push(source(
                    &l.text,
                    Some(l.label_type),
                    l.position,
                    Some(l.uuid),
                ));
            }
        }
        for (v, tip) in analyses[oid].power_by_root.get(&root).into_iter().flatten() {
            if !v.is_empty() {
                out.push(source(v, None, *tip, None));
            }
        }
    }
    out.sort_by(|a, b| {
        (&a.name, &a.sheet_path, pt_key(&a.at)).cmp(&(&b.name, &b.sheet_path, pt_key(&b.at)))
    });
    let mut texts: Vec<&str> = out.iter().map(|s| s.name.as_str()).collect();
    texts.dedup();
    if texts.len() < 2 {
        return Vec::new();
    }
    out
}
