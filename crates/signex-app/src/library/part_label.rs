//! How a placed part's library and value read on the hover card.

use std::collections::HashMap;

use uuid::Uuid;

use super::state::OpenLibrary;

/// Shown where a part has no value worth displaying.
const NO_VALUE: &str = "\u{2014}";

/// Hex digits kept of each uuid in the short fallback.
const SHORT_ID_LEN: usize = 8;

/// A readable name for a placed symbol's `"<library uuid>:<symbol uuid>"` id:
/// the drift scan's `Library/Symbol`, else what the mounted libraries know,
/// else a short form of the two uuids.
pub fn part_library_label(
    lib_id: &str,
    drift_names: &HashMap<String, String>,
    libs: &[OpenLibrary],
) -> String {
    if let Some(name) = drift_names.get(lib_id) {
        return name.clone();
    }
    let Some((library, symbol)) = lib_id.split_once(':') else {
        return lib_id.to_owned();
    };
    let (Ok(library_id), Ok(symbol_id)) = (Uuid::parse_str(library), Uuid::parse_str(symbol))
    else {
        return lib_id.to_owned();
    };
    if let Some(lib) = libs.iter().find(|l| l.library_id == library_id) {
        return match lib.cached_symbols.iter().find(|s| s.uuid == symbol_id) {
            Some(s) => format!("{}/{}", lib.display_name, s.name),
            None => lib.display_name.clone(),
        };
    }
    format!("{}:{}", short_id(library), short_id(symbol))
}

fn short_id(id: &str) -> &str {
    id.get(..SHORT_ID_LEN).unwrap_or(id)
}

/// The value the card shows: empty and the `*` "unset" placeholder read as a dash.
pub fn card_value(value: &str) -> &str {
    match value.trim() {
        "" | "*" => NO_VALUE,
        _ => value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_library::{PrimitiveKind, PrimitiveSummary};
    use uuid::Uuid;

    fn library(name: &str, id: Uuid, symbols: Vec<PrimitiveSummary>) -> OpenLibrary {
        OpenLibrary {
            root: std::path::PathBuf::from("/tmp/x.snxlib"),
            display_name: name.into(),
            library_id: id,
            tables: HashMap::new(),
            cached_components: Vec::new(),
            cached_symbols: symbols,
            cached_footprints: Vec::new(),
            cached_sims: Vec::new(),
            cached_classes: Vec::new(),
            observed_stamp: None,
            dangling: Default::default(),
            pricing: Default::default(),
            own_edit_stamp: None,
            display: Default::default(),
        }
    }

    fn symbol(id: Uuid, name: &str) -> PrimitiveSummary {
        PrimitiveSummary {
            uuid: id,
            name: name.into(),
            kind: PrimitiveKind::Symbol,
            used_by_count: 0,
        }
    }

    #[test]
    fn a_drift_name_wins() {
        let names = HashMap::from([("a:b".to_owned(), "Stock/R".to_owned())]);
        assert_eq!(part_library_label("a:b", &names, &[]), "Stock/R");
    }

    #[test]
    fn a_mounted_library_names_the_part() {
        let (lib_id, sym_id) = (Uuid::now_v7(), Uuid::now_v7());
        let libs = [library("Stock", lib_id, vec![symbol(sym_id, "R")])];
        let id = format!("{lib_id}:{sym_id}");
        assert_eq!(part_library_label(&id, &HashMap::new(), &libs), "Stock/R");
    }

    #[test]
    fn a_mounted_library_without_the_symbol_names_the_library() {
        let lib_id = Uuid::now_v7();
        let libs = [library("Stock", lib_id, Vec::new())];
        let id = format!("{lib_id}:{}", Uuid::now_v7());
        assert_eq!(part_library_label(&id, &HashMap::new(), &libs), "Stock");
    }

    #[test]
    fn an_unmounted_pair_of_uuids_gets_a_short_form() {
        let id = "0190aaaa-1111-7000-8000-000000000001:0190bbbb-2222-7000-8000-000000000002";
        let label = part_library_label(id, &HashMap::new(), &[]);
        assert_eq!(label, "0190aaaa:0190bbbb");
        assert!(label.len() < id.len());
    }

    #[test]
    fn a_non_uuid_id_is_kept_as_is() {
        assert_eq!(part_library_label("Mine", &HashMap::new(), &[]), "Mine");
    }

    #[test]
    fn unset_values_read_as_a_dash() {
        for unset in ["*", "", "  "] {
            assert_eq!(card_value(unset), "\u{2014}", "{unset:?}");
        }
        assert_eq!(card_value("10k"), "10k");
    }
}
