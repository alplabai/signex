//! Pairing the pins of one library symbol with the pins of another, for
//! swapping a placed part for a different one (#1260).

use super::LibPin;

/// How an old pin found its new one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinMatchedBy {
    /// Same non-empty pin number.
    Number,
    /// No number match left; same non-empty pin name.
    Name,
}

/// One old pin paired with the new pin that takes over its connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinPair {
    /// Key of the old pin ([`pin_key`]).
    pub old: String,
    /// Key of the new pin ([`pin_key`]).
    pub new: String,
    pub by: PinMatchedBy,
}

/// The outcome of [`map_pins`].
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PinMapping {
    pub pairs: Vec<PinPair>,
    /// Keys of the old pins no new pin took over.
    pub unmatched: Vec<String>,
}

/// Reads the label one matching pass compares.
type PinLabel = fn(&LibPin) -> &str;

/// A pin's identity in reports: its number, else its name.
pub fn pin_key(lp: &LibPin) -> &str {
    if lp.pin.number.is_empty() {
        &lp.pin.name
    } else {
        &lp.pin.number
    }
}

/// Pair every old pin with a new one: by pin number first, then, among the
/// pins still free on both sides, by pin name. A new pin takes over at most
/// one old pin.
pub fn map_pins(old: &[&LibPin], new: &[&LibPin]) -> PinMapping {
    let mut taken = vec![false; new.len()];
    let mut found: Vec<Option<(usize, PinMatchedBy)>> = vec![None; old.len()];
    let passes: [(PinMatchedBy, PinLabel); 2] = [
        (PinMatchedBy::Number, |lp| lp.pin.number.as_str()),
        (PinMatchedBy::Name, |lp| lp.pin.name.as_str()),
    ];
    for (by, label) in passes {
        for (i, old_pin) in old.iter().enumerate() {
            let wanted = label(old_pin);
            if found[i].is_some() || wanted.is_empty() {
                continue;
            }
            let free = new
                .iter()
                .enumerate()
                .position(|(j, np)| !taken[j] && label(np) == wanted);
            if let Some(j) = free {
                taken[j] = true;
                found[i] = Some((j, by));
            }
        }
    }
    let mut mapping = PinMapping::default();
    for (old_pin, hit) in old.iter().zip(found) {
        match hit {
            Some((j, by)) => mapping.pairs.push(PinPair {
                old: pin_key(old_pin).to_string(),
                new: pin_key(new[j]).to_string(),
                by,
            }),
            None => mapping.unmatched.push(pin_key(old_pin).to_string()),
        }
    }
    mapping
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schematic::{Pin, PinDirection, PinShapeStyle, Point};

    fn lp(number: &str, name: &str) -> LibPin {
        LibPin {
            unit: 1,
            body_style: 1,
            pin: Pin {
                direction: PinDirection::Passive,
                shape_style: PinShapeStyle::Plain,
                position: Point::new(0.0, 0.0),
                rotation: 0.0,
                length: 2.54,
                name: name.to_string(),
                number: number.to_string(),
                visible: true,
                name_visible: true,
                number_visible: true,
            },
        }
    }

    fn map(old: &[LibPin], new: &[LibPin]) -> PinMapping {
        let old: Vec<&LibPin> = old.iter().collect();
        let new: Vec<&LibPin> = new.iter().collect();
        map_pins(&old, &new)
    }

    fn pair(old: &str, new: &str, by: PinMatchedBy) -> PinPair {
        PinPair {
            old: old.into(),
            new: new.into(),
            by,
        }
    }

    #[test]
    fn pins_with_the_same_number_pair_up() {
        let m = map(&[lp("1", "A"), lp("2", "B")], &[lp("2", "X"), lp("1", "Y")]);
        assert_eq!(
            m.pairs,
            vec![
                pair("1", "1", PinMatchedBy::Number),
                pair("2", "2", PinMatchedBy::Number)
            ]
        );
        assert!(m.unmatched.is_empty());
    }

    #[test]
    fn a_pin_without_a_number_match_falls_back_to_its_name() {
        // Old "3"/VCC has no number 3 on the new part, which calls it pin 8.
        let m = map(
            &[lp("1", "IN"), lp("3", "VCC")],
            &[lp("1", "IN"), lp("8", "VCC")],
        );
        assert_eq!(
            m.pairs,
            vec![
                pair("1", "1", PinMatchedBy::Number),
                pair("3", "8", PinMatchedBy::Name)
            ]
        );
        assert!(m.unmatched.is_empty());
    }

    #[test]
    fn a_number_match_beats_a_name_match_for_the_same_pin() {
        // New pin "2" is called "A" but old pin "1" is also called "A":
        // old "2" takes new "2" by number, so old "1" cannot reuse it by name.
        let m = map(&[lp("1", "A"), lp("2", "B")], &[lp("2", "A")]);
        assert_eq!(m.pairs, vec![pair("2", "2", PinMatchedBy::Number)]);
        assert_eq!(m.unmatched, vec!["1".to_string()]);
    }

    #[test]
    fn pins_with_neither_number_nor_name_in_common_are_unmatched() {
        let m = map(&[lp("1", "A"), lp("2", "B")], &[lp("5", "C")]);
        assert!(m.pairs.is_empty());
        assert_eq!(m.unmatched, vec!["1".to_string(), "2".to_string()]);
    }

    #[test]
    fn empty_names_never_match_each_other() {
        let m = map(&[lp("1", "")], &[lp("9", "")]);
        assert!(m.pairs.is_empty());
        assert_eq!(m.unmatched, vec!["1".to_string()]);
    }

    #[test]
    fn a_new_pin_serves_one_old_pin_only() {
        let m = map(&[lp("1", "P"), lp("2", "P")], &[lp("7", "P")]);
        assert_eq!(m.pairs, vec![pair("1", "7", PinMatchedBy::Name)]);
        assert_eq!(m.unmatched, vec!["2".to_string()]);
    }

    #[test]
    fn a_numberless_pin_is_reported_by_its_name() {
        let m = map(&[lp("", "CLK")], &[lp("4", "OTHER")]);
        assert_eq!(m.unmatched, vec!["CLK".to_string()]);
    }
}
