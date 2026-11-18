//! Unique names for the whole-job exports (IPC-2581, ODB++). Each format
//! maps a name to the characters it allows, which can turn two names into
//! one; the second then takes `_2`, the third `_3`, and so on.

use std::collections::{BTreeMap, BTreeSet};

/// The names handed out so far.
#[derive(Debug, Default)]
pub(crate) struct Namer {
    taken: BTreeSet<String>,
}

impl Namer {
    /// `base`, or the first of `base_2`, `base_3`, ... no earlier name took.
    pub(crate) fn claim(&mut self, base: String) -> String {
        if self.taken.insert(base.clone()) {
            return base;
        }
        let mut n = 2;
        loop {
            let candidate = format!("{base}_{n}");
            if self.taken.insert(candidate.clone()) {
                return candidate;
            }
            n += 1;
        }
    }
}

/// Each distinct name of `names` mapped by `map` and made unique: the name
/// every reference to it is written with. A name the mapping leaves alone
/// keeps it; the others follow in sorted order.
pub(crate) fn unique_names<'a>(
    names: impl IntoIterator<Item = &'a str>,
    map: fn(&str) -> String,
) -> BTreeMap<String, String> {
    let distinct: BTreeSet<&str> = names.into_iter().collect();
    let (kept, changed): (Vec<&str>, Vec<&str>) =
        distinct.into_iter().partition(|name| map(name) == *name);
    let mut namer = Namer::default();
    kept.into_iter()
        .chain(changed)
        .map(|name| (name.to_string(), namer.claim(map(name))))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn underscore(name: &str) -> String {
        name.replace([' ', ':'], "_")
    }

    #[test]
    fn names_a_mapping_joins_stay_apart() {
        let names = unique_names(["A:B", "A_B", "A B", "A_B"], underscore);
        assert_eq!(names.len(), 3);
        assert_eq!(names["A_B"], "A_B");
        assert_eq!(names["A B"], "A_B_2");
        assert_eq!(names["A:B"], "A_B_3");
        let mut namer = Namer::default();
        assert_eq!(namer.claim("R1".into()), "R1");
        assert_eq!(namer.claim("R1_2".into()), "R1_2");
        assert_eq!(namer.claim("R1".into()), "R1_3");
    }
}
