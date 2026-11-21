//! The parts by variants matrix of the Variants section (#1293): one row per
//! part, a column for Base and one per variant definition. The parts are the
//! BOM's: every placement of every sheet, power ports left out, the units of
//! one part grouped once, each showing the value the BOM lists (an `=` value
//! evaluated on its sheet, #1986).

use signex_types::designator::compare_references;
use signex_types::project::is_base_variant;
use signex_types::schematic::{PartKey, Symbol, group_into_parts, normalized_lib_id};

use super::part_is_fitted;
use crate::ExportContext;
use crate::bom::shown_parts;

/// Which parts each variant fits, read from a project's sheets.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VariantMatrix {
    /// The variant definitions the columns after Base stand for, as read.
    pub variants: Vec<String>,
    /// One row per part, in designator order.
    pub rows: Vec<VariantRow>,
}

/// One part of a [`VariantMatrix`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariantRow {
    pub designator: String,
    /// The value the BOM lists: an `=` value evaluated on its sheet.
    pub value: String,
    /// The value as written, which the parts filter also matches.
    pub source_value: String,
    pub footprint: String,
    /// Fitted per column: Base first, then each of [`VariantMatrix::variants`].
    pub fitted: Vec<bool>,
}

impl VariantRow {
    /// Whether Base fits the part.
    pub fn base_fitted(&self) -> bool {
        self.fitted.first().copied().unwrap_or(true)
    }

    /// Whether the variant in `column` fits the part differently from Base.
    pub fn differs_at(&self, column: usize) -> bool {
        column > 0
            && self
                .fitted
                .get(column)
                .is_some_and(|&fitted| fitted != self.base_fitted())
    }

    /// Whether any variant fits the part differently from Base.
    pub fn differs(&self) -> bool {
        (1..self.fitted.len()).any(|column| self.differs_at(column))
    }

    /// Whether the row passes the parts filter: terms split at commas and
    /// spaces, any of which matches the designator, the value as written or
    /// the value shown (evaluated), ignoring case. A term with `*` is a wildcard pattern over the whole text
    /// (`U*`); a term without one matches anywhere in it (`10k`). An empty
    /// filter passes every row.
    pub fn matches(&self, filter: &str) -> bool {
        let mut terms = filter
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|term| !term.is_empty())
            .peekable();
        if terms.peek().is_none() {
            return true;
        }
        terms.any(|term| {
            term_matches(term, &self.designator)
                || term_matches(term, &self.source_value)
                || term_matches(term, &self.value)
        })
    }
}

impl VariantMatrix {
    /// The column of `variant`: 0 for Base (`None` or Base's name), else its
    /// place after Base among the variants read (ignoring ASCII case);
    /// `None` for a variant this matrix was not read for.
    pub fn column(&self, variant: Option<&str>) -> Option<usize> {
        match variant.filter(|name| !is_base_variant(name)) {
            None => Some(0),
            Some(name) => self
                .variants
                .iter()
                .position(|read| read.eq_ignore_ascii_case(name.trim()))
                .map(|index| index + 1),
        }
    }

    /// How many parts the variant in `column` fits.
    pub fn fitted_count(&self, column: usize) -> usize {
        self.rows
            .iter()
            .filter(|row| row.fitted.get(column).copied().unwrap_or(false))
            .count()
    }

    /// How many parts the variant in `column` fits differently from Base.
    pub fn changes(&self, column: usize) -> usize {
        self.rows
            .iter()
            .filter(|row| row.differs_at(column))
            .count()
    }

    /// How many parts some variant fits differently from Base.
    pub fn differing_rows(&self) -> usize {
        self.rows.iter().filter(|row| row.differs()).count()
    }

    /// How many cells differ from Base, over every variant.
    pub fn differing_cells(&self) -> usize {
        (1..=self.variants.len())
            .map(|column| self.changes(column))
            .sum()
    }
}

/// The matrix of the parts `ctx` places, for Base and each of `variants`.
pub fn variant_matrix(ctx: &ExportContext, variants: &[String]) -> VariantMatrix {
    let shown = shown_parts(ctx);
    let placed: Vec<(&Symbol, &str)> = shown
        .iter()
        .map(|part| (&part.symbol, part.designator.as_str()))
        .collect();
    let parts = group_into_parts(placed.iter().map(|(sym, designator)| PartKey {
        reference: designator,
        lib_id: normalized_lib_id(&sym.lib_id),
        unit: sym.unit,
    }));
    let columns: Vec<Option<&str>> = std::iter::once(None)
        .chain(variants.iter().map(|name| Some(name.as_str())))
        .collect();
    let mut rows: Vec<VariantRow> = parts
        .iter()
        .map(|part| {
            let units: Vec<&Symbol> = part.iter().map(|&member| placed[member].0).collect();
            VariantRow {
                designator: placed[part[0]].1.to_string(),
                value: first_set(units.iter().map(|unit| unit.value.as_str())),
                source_value: first_set(
                    part.iter()
                        .map(|&member| shown[member].source_value.as_str()),
                ),
                footprint: first_set(units.iter().map(|unit| unit.footprint.as_str())),
                fitted: columns
                    .iter()
                    .map(|&variant| part_is_fitted(&units, variant))
                    .collect(),
            }
        })
        .collect();
    rows.sort_by(|a, b| compare_references(&a.designator, &b.designator));
    VariantMatrix {
        variants: variants.to_vec(),
        rows,
    }
}

/// The first value a unit sets, as the BOM merges units.
fn first_set<'a>(values: impl Iterator<Item = &'a str>) -> String {
    values
        .map(str::trim)
        .find(|value| !value.is_empty())
        .unwrap_or_default()
        .to_string()
}

/// `term` against `text`, ignoring case: a `*` pattern over the whole text,
/// or a plain term anywhere in it.
fn term_matches(term: &str, text: &str) -> bool {
    let term = term.to_lowercase();
    let text = text.to_lowercase();
    if !term.contains('*') {
        return text.contains(&term);
    }
    let pieces: Vec<&str> = term.split('*').collect();
    let Some((first, rest)) = pieces.split_first() else {
        return true;
    };
    let Some(mut left) = text.strip_prefix(first) else {
        return false;
    };
    let Some((last, middle)) = rest.split_last() else {
        return true;
    };
    for piece in middle {
        match left.find(piece) {
            Some(at) => left = &left[at + piece.len()..],
            None => return false,
        }
    }
    left.ends_with(last)
}
