//! The category picker's entries: All parts, Recently placed, then each
//! component class the mounted libraries hold, with its part count.

use std::collections::BTreeMap;
use std::fmt;

use signex_i18n::{I18n, ids};

use super::state::CategoryKey;
use crate::library::range_filter::parameter_caption;
use crate::library::state::OpenLibrary;

/// One picker entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategoryEntry {
    pub key: CategoryKey,
    /// "Capacitor (12)", ready to draw.
    pub label: String,
}

impl fmt::Display for CategoryEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

/// All parts first, then Recently placed (`recent` parts), then the
/// classes by caption in display order. A class is listed while at least
/// one mounted part has it.
pub fn categories_of(libraries: &[OpenLibrary], recent: usize, i18n: &I18n) -> Vec<CategoryEntry> {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    let mut total = 0;
    for row in libraries
        .iter()
        .flat_map(|lib| lib.tables.values())
        .flatten()
    {
        *counts.entry(row.class.as_str()).or_default() += 1;
        total += 1;
    }
    let entry = |key: CategoryKey, name: String, count: usize| CategoryEntry {
        key,
        label: i18n.message_plain(
            ids::PANELS_COMPONENTS_CATEGORY,
            &[("name", name.into()), ("count", count.into())],
        ),
    };
    let mut classes: Vec<(String, &str, usize)> = counts
        .into_iter()
        .map(|(class, n)| (parameter_caption(i18n, class), class, n))
        .collect();
    classes.sort_by(|a, b| i18n.compare_for_display(&a.0, &b.0).then(a.1.cmp(b.1)));
    let all = i18n.message_plain(ids::PANELS_COMPONENTS_ALL_PARTS, &[]);
    let recently = i18n.message_plain(ids::PANELS_COMPONENTS_RECENT_CATEGORY, &[]);
    [
        entry(CategoryKey::AllParts, all, total),
        entry(CategoryKey::Recent, recently, recent),
    ]
    .into_iter()
    .chain(
        classes
            .into_iter()
            .map(|(name, class, n)| entry(CategoryKey::Class(class.to_owned()), name, n)),
    )
    .collect()
}
