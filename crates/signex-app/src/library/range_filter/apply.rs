//! Filtering a row list with the active range filters.

use std::collections::BTreeMap;

use signex_library::{ParamValue, RangeFilter, RangeVerdict};

/// Keep the items every filter in `filters` accepts. Returns, per column,
/// how many items that filter excluded because their cell is not a
/// measurement in the column's unit (each filter counts independently, so
/// the numbers say what that one filter hides). Items that are measured
/// but outside a range are dropped without being counted: the user asked
/// for that.
pub(crate) fn retain_in_range<'p, T>(
    items: &mut Vec<T>,
    filters: &[(String, RangeFilter)],
    param: impl Fn(&T, &str) -> Option<&'p ParamValue>,
) -> BTreeMap<String, usize> {
    let mut excluded: BTreeMap<String, usize> = BTreeMap::new();
    if filters.is_empty() {
        return excluded;
    }
    items.retain(|item| {
        let mut keep = true;
        for (key, filter) in filters {
            match filter.verdict(param(item, key)) {
                RangeVerdict::Matches => {}
                RangeVerdict::Outside => keep = false,
                RangeVerdict::Excluded => {
                    *excluded.entry(key.clone()).or_default() += 1;
                    keep = false;
                }
            }
        }
        keep
    });
    excluded
}
