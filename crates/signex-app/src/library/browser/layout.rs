//! Library Browser — grid column layout.
//!
//! One list of slots (data columns plus the Graphite Lifecycle pill) and one
//! width per slot. The header and every body row read the same width from
//! here, so the cells line up and never overlap.

use super::columns::{ColumnKind, GridColumn};
use super::look::LIFECYCLE_PILL_W;

/// Horizontal padding inside a text cell, each side.
pub(super) const CELL_PAD_X: f32 = 6.0;

/// Average glyph advance as a fraction of the font size, used to decide
/// how many characters fit before the ellipsis.
const GLYPH_ADVANCE_EM: f32 = 0.56;

/// One horizontal slot of the grid.
#[derive(Debug, Clone, Copy)]
pub(super) enum Slot<'a> {
    Column(&'a GridColumn),
    /// The Graphite Lifecycle pill column.
    Lifecycle,
}

impl Slot<'_> {
    /// The slot's own width; the only width header and cells use.
    pub(super) fn width(&self) -> f32 {
        match self {
            Slot::Column(c) => c.width,
            Slot::Lifecycle => LIFECYCLE_PILL_W,
        }
    }
}

/// Position of a column in the Graphite order (mockup row 118): Internal
/// PN, the Sym
/// and Fpt status columns, Value, MPN, Manufacturer, Description, Lifecycle,
/// Price, Updated, then the other parameters and Tags.
fn graphite_rank(kind: &ColumnKind) -> u8 {
    match kind {
        ColumnKind::InternalPn => 0,
        ColumnKind::Symbol => 1,
        ColumnKind::Footprint => 2,
        ColumnKind::Parameter(k) if k == "value" => 3,
        ColumnKind::Mpn => 4,
        ColumnKind::Manufacturer => 5,
        ColumnKind::Description => 6,
        ColumnKind::Rev => 7,
        ColumnKind::Price => 9,
        ColumnKind::Updated => 10,
        // Other parametric columns and tags follow Updated: the pill and the
        // date stay in view.
        ColumnKind::Parameter(_) => 11,
        ColumnKind::Tags => 12,
    }
}

/// Width of the narrow Sym and Fpt status columns: room for the header and
/// the amber dot.
pub(super) const STATUS_COL_W: f32 = 40.0;

/// Widest the Description column gets; a longer text ends in an ellipsis
/// and shows whole in its tooltip.
pub(super) const DESCRIPTION_MAX_W: f32 = 200.0;

/// The mock's column set and widths: Rev shows in the preview pane instead
/// (as before mockup row 118; hiding Rev and Tags by default is an open decision),
/// Symbol and Footprint are narrow status columns, and the rest are compact
/// so the pill and Updated are in view.
pub(super) fn graphite_columns(columns: &[GridColumn]) -> Vec<GridColumn> {
    columns
        .iter()
        .filter(|c| !matches!(c.kind, ColumnKind::Rev))
        .map(|c| {
            let width = match &c.kind {
                ColumnKind::InternalPn => 140.0,
                ColumnKind::Symbol | ColumnKind::Footprint => STATUS_COL_W,
                ColumnKind::Description => DESCRIPTION_MAX_W,
                ColumnKind::Parameter(k) if k == "value" => 64.0,
                ColumnKind::Manufacturer | ColumnKind::Mpn => 96.0,
                ColumnKind::Updated => 84.0,
                _ => c.width.min(96.0),
            };
            GridColumn { width, ..c.clone() }
        })
        .collect()
}

/// Rank of the Lifecycle pill: after Description, before Price.
const LIFECYCLE_RANK: u8 = 8;

/// The slots to draw. Legacy themes keep the derived order and no pill
/// column; Graphite reorders to the mock and adds the Lifecycle pill.
pub(super) fn slots(columns: &[GridColumn], graphite: bool) -> Vec<Slot<'_>> {
    if !graphite {
        return columns
            .iter()
            .filter(|c| !c.kind.graphite_only())
            .map(Slot::Column)
            .collect();
    }
    let mut ranked: Vec<(u8, Slot<'_>)> = columns
        .iter()
        .map(|c| (graphite_rank(&c.kind), Slot::Column(c)))
        .collect();
    ranked.push((LIFECYCLE_RANK, Slot::Lifecycle));
    ranked.sort_by_key(|(rank, _)| *rank);
    ranked.into_iter().map(|(_, s)| s).collect()
}

/// `(start, end)` x-offsets of every slot after a `lead` of fixed space.
#[cfg(test)]
pub(super) fn spans(slots: &[Slot<'_>], lead: f32) -> Vec<(f32, f32)> {
    let mut x = lead;
    slots
        .iter()
        .map(|s| {
            let span = (x, x + s.width());
            x = span.1;
            span
        })
        .collect()
}

/// `text` cut with an ellipsis so it fits a cell of `width` at `size`.
pub(super) fn ellipsize(text: String, width: f32, size: f32) -> String {
    let room = (width - 2.0 * CELL_PAD_X).max(0.0);
    let budget = (room / (size * GLYPH_ADVANCE_EM)).floor() as usize;
    if text.chars().count() <= budget {
        return text;
    }
    let keep = budget.saturating_sub(1);
    let mut cut: String = text.chars().take(keep).collect();
    cut.push('…');
    cut
}

#[cfg(test)]
mod tests {
    use super::*;

    fn col(kind: ColumnKind, width: f32) -> GridColumn {
        GridColumn {
            label: String::new(),
            kind,
            width,
        }
    }

    fn sample() -> Vec<GridColumn> {
        vec![
            col(ColumnKind::InternalPn, 130.0),
            col(ColumnKind::Manufacturer, 120.0),
            col(ColumnKind::Mpn, 130.0),
            col(ColumnKind::Description, 200.0),
            col(ColumnKind::Rev, 80.0),
            col(ColumnKind::Symbol, 140.0),
            col(ColumnKind::Footprint, 140.0),
            col(ColumnKind::Updated, 90.0),
            col(ColumnKind::Parameter("value".into()), 110.0),
        ]
    }

    #[test]
    fn no_two_columns_overlap() {
        let cols = sample();
        for graphite in [false, true] {
            let slots = slots(&cols, graphite);
            let spans = spans(&slots, 16.0);
            for pair in spans.windows(2) {
                assert!(pair[0].1 <= pair[1].0, "overlap: {pair:?}");
            }
            for (s, w) in spans.iter().zip(&slots) {
                assert!((s.1 - s.0 - w.width()).abs() < f32::EPSILON);
            }
        }
    }

    #[test]
    fn graphite_follows_the_mock_order_with_the_pill_before_updated() {
        let cols = sample();
        let order: Vec<u8> = slots(&cols, true)
            .iter()
            .map(|s| match s {
                Slot::Column(c) => graphite_rank(&c.kind),
                Slot::Lifecycle => LIFECYCLE_RANK,
            })
            .collect();
        let mut sorted = order.clone();
        sorted.sort_unstable();
        assert_eq!(order, sorted);
        assert_eq!(order.first(), Some(&0));
        assert_eq!(order.last(), Some(&graphite_rank(&ColumnKind::Updated)));
        assert!(order.contains(&LIFECYCLE_RANK));
        assert!(LIFECYCLE_RANK < graphite_rank(&ColumnKind::Updated));
    }

    /// Mockup row 118: Internal PN, then the narrow Sym and Fpt status columns, then
    /// Value, MPN, Manufacturer, Description, Lifecycle and Updated; Rev
    /// stays out of the Graphite grid (the preview pane shows it).
    #[test]
    fn the_status_columns_follow_the_internal_pn() {
        let compact = graphite_columns(&sample());
        let names: Vec<String> = slots(&compact, true)
            .iter()
            .map(|s| match s {
                Slot::Column(c) => match &c.kind {
                    ColumnKind::Parameter(k) => k.clone(),
                    other => format!("{other:?}"),
                },
                Slot::Lifecycle => "Lifecycle".into(),
            })
            .collect();
        assert_eq!(
            names,
            [
                "InternalPn",
                "Symbol",
                "Footprint",
                "value",
                "Mpn",
                "Manufacturer",
                "Description",
                "Lifecycle",
                "Updated",
            ]
        );
        for c in &compact {
            match c.kind {
                ColumnKind::Symbol | ColumnKind::Footprint => assert_eq!(c.width, STATUS_COL_W),
                ColumnKind::Description => assert_eq!(c.width, DESCRIPTION_MAX_W),
                _ => {}
            }
        }
    }

    #[test]
    fn legacy_keeps_derived_order_without_graphite_columns() {
        let cols = sample();
        let slots = slots(&cols, false);
        assert_eq!(slots.len(), cols.len() - 2);
        assert!(slots.iter().all(|s| matches!(s, Slot::Column(_))));
        assert!(matches!(slots[0], Slot::Column(c) if matches!(c.kind, ColumnKind::InternalPn)));
    }

    #[test]
    fn ellipsize_stays_inside_the_cell() {
        let long = "Resistor (IEC rectangle). Chip-0805 footprint".to_string();
        let cut = ellipsize(long, 200.0, 12.0);
        assert!(cut.ends_with('…'));
        let room = 200.0 - 2.0 * CELL_PAD_X;
        assert!(cut.chars().count() as f32 * 12.0 * GLYPH_ADVANCE_EM <= room);
        assert_eq!(ellipsize("0.0.1".into(), 80.0, 12.0), "0.0.1");
    }

    /// #2262: an Internal PN of 17 mono characters ("STOCK-C_Polarized")
    /// fits its editable cell (cell inset and input padding included), so it
    /// is not cut mid-word.
    #[test]
    fn the_internal_pn_column_holds_a_seventeen_character_pn() {
        const MONO_ADVANCE_EM: f32 = 0.6;
        let sp = crate::styles::Spacing::DEFAULT;
        let size = crate::styles::TypeScale::DEFAULT.body;
        let pn = graphite_columns(&sample())
            .into_iter()
            .find(|c| matches!(c.kind, ColumnKind::InternalPn))
            .expect("Internal PN column");
        let need = 17.0 * MONO_ADVANCE_EM * size + 2.0 * (sp.xs + CELL_PAD_X);
        assert!(pn.width >= need, "{} < {need}", pn.width);
    }
}
