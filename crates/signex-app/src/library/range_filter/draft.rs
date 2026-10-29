//! What the user has set on the range filters of one surface.

use std::collections::BTreeMap;

use signex_library::{RangeFilter, RangeOp};
use signex_types::quantity::{MantissaError, SiPrefix, Unit, parse_mantissa, quantity_from_parts};

use crate::library::messages::{RangeBound, RangeFilterMsg};
use crate::library::prefix_picker::effective_prefix;

/// One end of the range as the user is typing it.
#[derive(Debug, Clone, PartialEq)]
pub struct BoundDraft {
    pub text: String,
    pub prefix: SiPrefix,
    /// Why the last keystroke was refused; cleared by the next accepted
    /// edit.
    pub refusal: Option<String>,
}

impl Default for BoundDraft {
    fn default() -> Self {
        BoundDraft {
            text: String::new(),
            prefix: SiPrefix::One,
            refusal: None,
        }
    }
}

impl BoundDraft {
    fn quantity(&self, unit: &Unit) -> Option<signex_types::quantity::Quantity> {
        let prefix = effective_prefix(unit, self.prefix);
        quantity_from_parts(&self.text, prefix, unit.clone()).ok()
    }

    fn edit(&mut self, text: String) {
        match refusal_reason(&text) {
            Some(reason) => self.refusal = Some(reason),
            None => {
                self.text = text;
                self.refusal = None;
            }
        }
    }
}

/// Why `text` is not acceptable in a number field, if it is not: only
/// digits, a sign and a decimal point are. The reason is the editors' own
/// for letters and a comma. `None` for text that is, or may still become,
/// a number (`-`, `1.`).
fn refusal_reason(text: &str) -> Option<String> {
    if text
        .chars()
        .all(|c| c.is_ascii_digit() || matches!(c, '.' | '+' | '-'))
    {
        return None;
    }
    Some(match parse_mantissa(text) {
        Err(e @ (MantissaError::HasLetters(_) | MantissaError::Comma(_))) => e.to_string(),
        _ => format!("{text:?}: only digits, a sign and a decimal point are accepted"),
    })
}

/// The filter on one column.
#[derive(Debug, Clone, PartialEq)]
pub struct RangeFilterDraft {
    pub op: RangeOp,
    /// The column's unit, fixed when the filter is added.
    pub unit: Unit,
    pub low: BoundDraft,
    pub high: BoundDraft,
}

impl RangeFilterDraft {
    fn new(unit: Unit) -> Self {
        RangeFilterDraft {
            op: RangeOp::AtLeast,
            unit,
            low: BoundDraft::default(),
            high: BoundDraft::default(),
        }
    }

    /// The validated filter, or `None` while a needed number is empty or
    /// unfinished. An inactive filter hides nothing.
    pub fn filter(&self) -> Option<RangeFilter> {
        let low = self.low.quantity(&self.unit)?;
        let high = if self.op.is_between() {
            Some(self.high.quantity(&self.unit)?)
        } else {
            None
        };
        RangeFilter::new(self.op, low, high)
    }

    fn bound_mut(&mut self, bound: RangeBound) -> &mut BoundDraft {
        match bound {
            RangeBound::Low => &mut self.low,
            RangeBound::High => &mut self.high,
        }
    }
}

/// The range filters of one surface, by parameter column key.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RangeFilters {
    drafts: BTreeMap<String, RangeFilterDraft>,
}

impl RangeFilters {
    pub fn is_empty(&self) -> bool {
        self.drafts.is_empty()
    }

    pub fn get(&self, key: &str) -> Option<&RangeFilterDraft> {
        self.drafts.get(key)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &RangeFilterDraft)> {
        self.drafts.iter()
    }

    /// The filters that are complete, by column key: what the row lists
    /// are filtered with.
    pub fn active(&self) -> Vec<(String, RangeFilter)> {
        self.drafts
            .iter()
            .filter_map(|(k, d)| Some((k.clone(), d.filter()?)))
            .collect()
    }

    /// Apply one user action.
    pub fn apply(&mut self, msg: RangeFilterMsg) {
        match msg {
            RangeFilterMsg::ColumnPicked { key, unit } => {
                self.drafts
                    .entry(key)
                    .or_insert_with(|| RangeFilterDraft::new(unit));
            }
            RangeFilterMsg::OperatorPicked { key, op } => {
                if let Some(d) = self.drafts.get_mut(&key) {
                    d.op = op;
                }
            }
            RangeFilterMsg::NumberEdited { key, bound, text } => {
                if let Some(d) = self.drafts.get_mut(&key) {
                    d.bound_mut(bound).edit(text);
                }
            }
            RangeFilterMsg::PrefixPicked { key, bound, prefix } => {
                if let Some(d) = self.drafts.get_mut(&key) {
                    d.bound_mut(bound).prefix = prefix;
                }
            }
            RangeFilterMsg::Cleared { key } => {
                self.drafts.remove(&key);
            }
        }
    }
}
