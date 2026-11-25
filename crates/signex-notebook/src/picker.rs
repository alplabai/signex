//! The reference picker (mockup row 195): typing `@` in the editor opens a
//! list of the project's parts and nets, filtered as you type, and Enter
//! inserts the stored call (`#component("R7")`). The caller gathers the
//! candidates from the project; this module filters and orders them.

use crate::reference::Reference;

/// One part or net the picker offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub reference: Reference,
    /// What the row shows after the name: a part's value and footprint, a
    /// net's pin count.
    pub detail: String,
    /// The sheet it is on, as a file name.
    pub sheet: String,
}

/// The most rows the picker lists; the header says how many matched.
pub const MAX_ROWS: usize = 12;

/// The candidates whose name matches `query`, best first: an exact name,
/// then names that start with it, then names that contain it, each group
/// in natural designator order (`R2` before `R10`). Case is ignored; an
/// empty query lists everything.
pub fn matching<'a>(candidates: &'a [Candidate], query: &str) -> Vec<&'a Candidate> {
    let query = query.trim().to_lowercase();
    let mut found: Vec<(u8, &Candidate)> = candidates
        .iter()
        .filter_map(|candidate| {
            let name = candidate.reference.name().to_lowercase();
            let rank = if name == query {
                0
            } else if name.starts_with(&query) {
                1
            } else if name.contains(&query) {
                2
            } else {
                return None;
            };
            Some((rank, candidate))
        })
        .collect();
    found.sort_by(|(a_rank, a), (b_rank, b)| {
        a_rank
            .cmp(b_rank)
            .then_with(|| natural_key(&a.reference.name()).cmp(&natural_key(&b.reference.name())))
    });
    found.into_iter().map(|(_, candidate)| candidate).collect()
}

/// `R10` -> (`R`, 10, ``): a designator's letters, its number and the rest.
fn natural_key(name: &str) -> (String, u64, String) {
    let letters: String = name.chars().take_while(|c| !c.is_ascii_digit()).collect();
    let rest = &name[letters.len()..];
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    let number = digits.parse().unwrap_or(0);
    (
        letters.to_lowercase(),
        number,
        rest[digits.len()..].to_lowercase(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn part(name: &str) -> Candidate {
        Candidate {
            reference: Reference::Component(name.into()),
            detail: String::new(),
            sheet: "power.snxsch".into(),
        }
    }

    fn names(found: &[&Candidate]) -> Vec<String> {
        found.iter().map(|c| c.reference.name()).collect()
    }

    #[test]
    fn exact_then_prefix_then_contains_in_designator_order() {
        let all = [part("R10"), part("R2"), part("UR1"), part("R"), part("C1")];
        assert_eq!(names(&matching(&all, "r")), ["R", "R2", "R10", "UR1"]);
    }

    #[test]
    fn case_is_ignored_and_an_empty_query_lists_everything() {
        let all = [part("R7"), part("C15")];
        assert_eq!(names(&matching(&all, "c1")), ["C15"]);
        assert_eq!(names(&matching(&all, "  ")), ["C15", "R7"]);
    }

    #[test]
    fn nets_match_by_their_name() {
        let net = Candidate {
            reference: Reference::Net("+3V3".into()),
            detail: "3 pins".into(),
            sheet: "power.snxsch".into(),
        };
        let all = [net, part("R3")];
        assert_eq!(names(&matching(&all, "3v")), ["+3V3"]);
    }
}
