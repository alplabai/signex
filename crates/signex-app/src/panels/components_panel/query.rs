//! Value-aware search of the Components panel (mockup row 52).
//!
//! A query is split into words and a part must match every word. A number
//! with a prefix or unit (`100n`, `0.1u`, `0.1 µF`, `4k7`, `16V`) matches a
//! measurement parameter of the same base value within
//! [`approx_equal`] (0.5 %); prefixes are case-sensitive (`m` milli,
//! `M` mega) and a unit limits the match to that unit. Such a word also
//! matches the part's names and description written exactly so. A bare
//! number (`100`) matches any measurement that displays that number, in any
//! prefix, and the part's names. Every other word is a case-folded
//! substring of the names and description.

use std::collections::BTreeMap;

use signex_i18n::I18n;
use signex_library::ComponentRow;
use signex_library::range_filter::approx_equal;
use signex_types::quantity::{Quantity, SiPrefix, Unit, ValueStyle, format_quantity};

/// A number with a prefix or unit, as a base value.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ValueWord {
    pub base: f64,
    /// The unit typed, if any; `None` matches every unit.
    pub unit: Option<Unit>,
}

/// What a query word asks for.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum WordKind {
    /// `100n`, `4k7`, `16V`: a base-value comparison.
    Value(ValueWord),
    /// `100`: the displayed number, in any prefix.
    Bare(f64),
    /// Anything else: a case-folded substring.
    Text,
}

/// One word of a query.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct QueryWord {
    /// As typed (`0.1 µF` joined to `0.1µF`).
    pub text: String,
    /// Case-folded for the substring match.
    pub folded: String,
    pub kind: WordKind,
}

/// A parsed Components panel query; empty matches every part.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct PanelQuery {
    words: Vec<QueryWord>,
}

impl PanelQuery {
    pub(crate) fn parse(i18n: &I18n, query: &str) -> Self {
        let raw: Vec<&str> = query.split_whitespace().collect();
        let mut words = Vec::with_capacity(raw.len());
        let mut i = 0;
        while i < raw.len() {
            // "0.1 µF" arrives as two words: a bare number takes the unit
            // word after it.
            let text = match raw.get(i + 1) {
                Some(next) if is_bare_number(raw[i]) && is_unit_tail(next) => {
                    i += 1;
                    format!("{}{next}", raw[i - 1])
                }
                _ => raw[i].to_owned(),
            };
            i += 1;
            words.push(QueryWord {
                folded: i18n.fold_for_search(&text),
                kind: word_kind(&text),
                text,
            });
        }
        Self { words }
    }

    pub(crate) fn words(&self) -> &[QueryWord] {
        &self.words
    }

    /// The bare-number words, for the match summary.
    pub(crate) fn bare_numbers(&self) -> impl Iterator<Item = (&str, f64)> {
        self.words.iter().filter_map(|w| match w.kind {
            WordKind::Bare(n) => Some((w.text.as_str(), n)),
            _ => None,
        })
    }
}

impl ValueWord {
    /// `text` read as a query word reads it (`10k`, `4k7`, `100n`, `16V`,
    /// `1%`; a bare `100` has no unit), for `search_parts`' parameter
    /// operands (#1297). `None` for text that is no number.
    pub(crate) fn parse(text: &str) -> Option<Self> {
        let compact: String = text.split_whitespace().collect();
        match word_kind(&compact) {
            WordKind::Value(value) => Some(value),
            WordKind::Bare(base) => Some(Self { base, unit: None }),
            WordKind::Text => None,
        }
    }

    /// Whether `q` has this unit (any, when none was typed).
    pub(crate) fn same_unit(&self, q: &Quantity) -> bool {
        self.unit.as_ref().is_none_or(|unit| unit == q.unit())
    }

    /// Whether `q` is this value, within the range filter's tolerance.
    pub(crate) fn equals(&self, q: &Quantity) -> bool {
        self.same_unit(q) && approx_equal(q.base(), self.base)
    }
}

/// What a part offers the search, built once per data change.
#[derive(Debug, Clone, Default)]
pub(crate) struct SearchKeys {
    /// Part number, MPN and manufacturer, folded, one per line.
    names: String,
    /// Description, folded.
    description: String,
    /// Names and description as written, for value words.
    written: String,
    /// Measurement parameters as base-unit quantities.
    measurements: Vec<Quantity>,
}

impl SearchKeys {
    pub(crate) fn of(i18n: &I18n, row: &ComponentRow) -> Self {
        let written_names = [
            row.internal_pn.as_str(),
            row.primary_mpn.mpn.as_str(),
            row.primary_mpn.manufacturer.as_str(),
        ]
        .join("\n");
        Self {
            names: i18n.fold_for_search(&written_names),
            description: i18n.fold_for_search(&row.description),
            written: format!("{written_names}\n{}", row.description),
            measurements: row
                .parameters
                .values()
                .filter_map(|v| v.as_quantity())
                .collect(),
        }
    }

    /// Whether the part matches `word`.
    pub(crate) fn matches(&self, word: &QueryWord) -> bool {
        match &word.kind {
            WordKind::Value(v) => {
                self.measurements.iter().any(|q| {
                    v.unit.as_ref().is_none_or(|u| u == q.unit()) && approx_equal(q.base(), v.base)
                }) || self.written.contains(&word.text)
            }
            WordKind::Bare(n) => {
                self.displayed_hits(*n).next().is_some() || self.names.contains(&word.folded)
            }
            WordKind::Text => {
                self.names.contains(&word.folded) || self.description.contains(&word.folded)
            }
        }
    }

    /// The measurements that display `number` in their own prefix.
    pub(crate) fn displayed_hits(&self, number: f64) -> impl Iterator<Item = &Quantity> {
        self.measurements
            .iter()
            .filter(move |q| approx_equal(q.displayed_number(), number))
    }
}

/// What one bare-number word caught: each displayed value with its part
/// count, ascending, then the parts it matched by name only.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct MatchLine {
    pub word: String,
    pub values: Vec<(String, usize)>,
    pub by_name: usize,
}

/// The summary lines of `query` over the parts it matched.
pub(crate) fn match_summary<'k>(
    query: &PanelQuery,
    matched: impl Iterator<Item = &'k SearchKeys> + Clone,
) -> Vec<MatchLine> {
    query
        .bare_numbers()
        .map(|(word, number)| {
            // (unit token, base bits) keeps one entry per value; the
            // BTreeMap below orders them by unit then base value.
            let mut values: BTreeMap<(String, OrderedBase), (String, usize)> = BTreeMap::new();
            let mut by_name = 0;
            for keys in matched.clone() {
                // A part counts once per value, however many parameters
                // carry it.
                let mut seen: Vec<&Quantity> = Vec::new();
                for q in keys.displayed_hits(number) {
                    if seen.contains(&q) {
                        continue;
                    }
                    seen.push(q);
                    let entry = values
                        .entry((q.unit().token().to_owned(), OrderedBase(q.base())))
                        .or_insert_with(|| (format_quantity(q, ValueStyle::Engineering, false), 0));
                    entry.1 += 1;
                }
                if seen.is_empty() {
                    by_name += 1;
                }
            }
            MatchLine {
                word: word.to_owned(),
                values: values.into_values().collect(),
                by_name,
            }
        })
        .collect()
}

/// A base value ordered by `f64::total_cmp`, as a map key.
#[derive(Debug, Clone, Copy)]
struct OrderedBase(f64);

impl PartialEq for OrderedBase {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}

impl Eq for OrderedBase {}

impl PartialOrd for OrderedBase {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OrderedBase {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}

/// The leading `[-]digits[.digits]` of `word` and the rest; `None` when it
/// does not start with a number.
fn split_number(word: &str) -> Option<(&str, &str)> {
    let mut dot = false;
    let end = word
        .char_indices()
        .find(|&(i, c)| {
            let keep = c.is_ascii_digit() || (c == '-' && i == 0) || (c == '.' && !dot);
            dot |= c == '.';
            !keep
        })
        .map_or(word.len(), |(i, _)| i);
    let (lead, rest) = word.split_at(end);
    lead.chars()
        .any(|c| c.is_ascii_digit())
        .then_some((lead, rest))
}

fn is_bare_number(word: &str) -> bool {
    split_number(word).is_some_and(|(_, rest)| rest.is_empty())
}

/// A unit, optionally after a prefix: `F`, `µF`, `kΩ`, `V`.
fn is_unit_tail(word: &str) -> bool {
    Unit::from_alias(word).is_some() || prefix_and_unit(word).is_some_and(|(_, u)| u.is_some())
}

/// A prefix letter (or `R`, the ohm decimal mark) and the unit after it.
fn prefix_and_unit(rest: &str) -> Option<(SiPrefix, Option<Unit>)> {
    let mut chars = rest.chars();
    let letter = chars.next()?;
    let tail = chars.as_str();
    let (prefix, default_unit) = match letter {
        'R' => (SiPrefix::One, Some(Unit::Ohm)),
        c => (SiPrefix::from_char(c)?, None),
    };
    if tail.is_empty() {
        return Some((prefix, default_unit));
    }
    let unit = Unit::from_alias(tail)?;
    if default_unit.as_ref().is_some_and(|d| *d != unit) {
        return None;
    }
    Some((prefix, Some(unit)))
}

fn word_kind(word: &str) -> WordKind {
    let Some((lead, rest)) = split_number(word) else {
        return WordKind::Text;
    };
    if rest.is_empty() {
        return lead.parse().map_or(WordKind::Text, WordKind::Bare);
    }
    value_word(lead, rest).map_or(WordKind::Text, WordKind::Value)
}

/// `lead` (a number) and `rest` (prefix and unit, or an RKM letter and the
/// fraction digits: `4k7`, `4R7`) as a base value.
fn value_word(lead: &str, rest: &str) -> Option<ValueWord> {
    let (mantissa, (prefix, unit)) = match rkm_split(lead, rest) {
        Some((frac, tail)) => (format!("{lead}.{frac}"), prefix_and_unit(&tail)?),
        None => match Unit::from_alias(rest) {
            Some(unit) => (lead.to_owned(), (SiPrefix::One, Some(unit))),
            None => (lead.to_owned(), prefix_and_unit(rest)?),
        },
    };
    let base: f64 = format!("{mantissa}e{}", prefix.exponent()).parse().ok()?;
    base.is_finite().then_some(ValueWord { base, unit })
}

/// For `4k7`: the fraction digits `7` and `k` with the unit after them, as
/// one prefix-and-unit tail (`k` / `kΩ`). `None` when `rest` is no RKM code.
fn rkm_split<'w>(lead: &str, rest: &'w str) -> Option<(&'w str, String)> {
    if lead.contains('.') || lead.starts_with('-') {
        return None;
    }
    let mut chars = rest.chars();
    let letter = chars.next()?;
    let after = chars.as_str();
    if !after.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    let frac_end = after
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(after.len());
    let (frac, unit) = after.split_at(frac_end);
    Some((frac, format!("{letter}{unit}")))
}

#[cfg(test)]
#[path = "query_tests.rs"]
mod tests;
