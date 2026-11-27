//! The stale-reference check the export runs before it writes (mockup row
//! 202): every part, pin and net the notebook names is looked up in the
//! design, and one that is no longer there is stale.

use std::collections::BTreeSet;

use serde::Serialize;

use crate::reference::Reference;
use crate::sources::NotebookSources;
use crate::syntax::distinct_references;

/// The names the design has now, as far as the caller could read them.
/// A list left `None` was not read, and nothing is called stale against it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DesignNames {
    /// Every designator.
    pub components: Option<BTreeSet<String>>,
    /// Every pin, as `designator.pin`.
    pub pins: Option<BTreeSet<String>>,
    /// Every net name.
    pub nets: Option<BTreeSet<String>>,
}

impl DesignNames {
    /// Whether `reference` names something the design still has. A pin
    /// needs its part; with no pin list, the part alone decides.
    pub fn has(&self, reference: &Reference) -> bool {
        let in_list = |list: &Option<BTreeSet<String>>, name: &str| {
            list.as_ref().is_none_or(|names| names.contains(name))
        };
        match reference {
            Reference::Component(name) => in_list(&self.components, name),
            Reference::Net(name) => in_list(&self.nets, name),
            Reference::Pin { component, .. } => {
                in_list(&self.components, component) && in_list(&self.pins, &reference.name())
            }
        }
    }
}

/// What the check found.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct StaleCheck {
    /// How many distinct references the notebook makes.
    pub references: usize,
    /// The stale ones, file by file in path order, each file's in the
    /// order it first names them.
    pub stale: Vec<Reference>,
}

impl StaleCheck {
    /// Check every distinct reference of the Typst files of `sources`
    /// with `is_live`.
    pub fn run(sources: &NotebookSources, is_live: impl Fn(&Reference) -> bool) -> Self {
        let mut distinct: Vec<Reference> = Vec::new();
        for path in sources.paths() {
            let is_typst = path.ends_with(".snxnbk") || path.ends_with(".typ");
            let Some(bytes) = sources.get(path).filter(|_| is_typst) else {
                continue;
            };
            for reference in distinct_references(&String::from_utf8_lossy(bytes)) {
                if !distinct.contains(&reference) {
                    distinct.push(reference);
                }
            }
        }
        let stale = distinct.iter().filter(|r| !is_live(r)).cloned().collect();
        Self {
            references: distinct.len(),
            stale,
        }
    }

    /// The names of the stale references, as they print (`R7`, `U3.14`).
    pub fn stale_names(&self) -> Vec<String> {
        self.stale.iter().map(Reference::name).collect()
    }
}
