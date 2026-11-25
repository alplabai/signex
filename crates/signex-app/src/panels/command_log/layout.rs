//! How the panel fits its width (mockup row 161): the table columns, the
//! detail pane and the filter bar. Rules: `docs/COMMAND_LOG.md`, "Panel width".

use super::{DETAIL_WIDTH, TS};

/// Column widths of the mock.
const TIME_W: f32 = TS.body * 100.0 / 12.0;
const SOURCE_W: f32 = TS.body * 82.0 / 12.0;
const COMMAND_W: f32 = TS.body * 206.0 / 12.0;
const ARGUMENTS_W: f32 = TS.body * 150.0 / 12.0;
const RESULT_W: f32 = TS.body * 196.0 / 12.0;
const TOOK_W: f32 = TS.body * 64.0 / 12.0;
/// The least the three text columns shrink to; their cells clip.
const COMMAND_MIN: f32 = TS.body * 10.0;
const ARGUMENTS_MIN: f32 = TS.body * 6.0;
const RESULT_MIN: f32 = TS.body * 100.0 / 12.0;
/// The search field's least width. The filter bar measures its other parts
/// at layout time (`toolbar::bar`); the search field gives up width first,
/// down to this, before Follow newest leaves the filters' line.
pub(super) const SEARCH_MIN: f32 = TS.body * 10.0;
/// The search field's width in the mock, the most it takes.
pub(super) const SEARCH_W: f32 = TS.body * 250.0 / 12.0;

/// Less than this over the width is float rounding, not overflow.
const ROUNDING: f32 = 0.5;

/// The width of each table column.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct TableColumns {
    pub time: f32,
    pub source: f32,
    pub command: f32,
    pub arguments: f32,
    pub result: f32,
    pub took: f32,
}

impl TableColumns {
    pub(super) fn total(&self) -> f32 {
        self.time + self.source + self.command + self.arguments + self.result + self.took
    }

    /// Whether the columns need more than `width`; float rounding is not.
    pub(super) fn overflow(&self, width: f32) -> bool {
        self.total() > width + ROUNDING
    }
}

/// The columns for a table `width` wide. Command, Arguments and Result shrink
/// together to their least widths; past that the table scrolls sideways, so
/// no column is ever lost. Extra width goes to Arguments.
pub(super) fn columns(width: f32) -> TableColumns {
    let fixed = TIME_W + SOURCE_W + TOOK_W;
    let least = COMMAND_MIN + ARGUMENTS_MIN + RESULT_MIN;
    let full = COMMAND_W + ARGUMENTS_W + RESULT_W;
    let share = ((width - fixed - least) / (full - least)).clamp(0.0, 1.0);
    let between = |min: f32, max: f32| min + (max - min) * share;
    let command = between(COMMAND_MIN, COMMAND_W);
    let result = between(RESULT_MIN, RESULT_W);
    let arguments = between(ARGUMENTS_MIN, ARGUMENTS_W).max(width - fixed - command - result);
    TableColumns {
        time: TIME_W,
        source: SOURCE_W,
        command,
        arguments,
        result,
        took: TOOK_W,
    }
}

/// The detail pane's width: the mock's, but never more than half the panel,
/// so the table stays in a side dock.
pub(super) fn detail_width(panel: f32) -> f32 {
    DETAIL_WIDTH.min(panel / 2.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bottom dock of the audit capture, at the 12 px body.
    const DOCK: f32 = 890.0;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.01
    }

    #[test]
    fn all_six_columns_fit_beside_the_detail_pane_in_the_bottom_dock() {
        let table = DOCK - detail_width(DOCK);
        let c = columns(table);
        assert!(close(c.total(), table), "{} != {table}", c.total());
        assert!(c.arguments >= ARGUMENTS_MIN);
        assert!(c.result >= RESULT_MIN);
        assert!(c.took > 0.0);
    }

    #[test]
    fn a_wide_table_keeps_the_mock_widths_and_gives_the_rest_to_arguments() {
        let c = columns(1200.0);
        assert!(close(c.command, COMMAND_W));
        assert!(close(c.result, RESULT_W));
        assert!(c.arguments > ARGUMENTS_W);
        assert!(close(c.total(), 1200.0));
    }

    #[test]
    fn a_narrow_table_keeps_every_column_at_its_least_width() {
        let c = columns(200.0);
        assert!(close(c.command, COMMAND_MIN));
        assert!(close(c.arguments, ARGUMENTS_MIN));
        assert!(close(c.result, RESULT_MIN));
        assert!(c.overflow(200.0), "the table scrolls sideways");
    }

    #[test]
    fn a_table_that_fits_does_not_scroll_sideways() {
        for width in [538.0, 570.0, 613.3, 798.0, 1200.0] {
            assert!(!columns(width).overflow(width), "at {width}");
        }
    }

    #[test]
    fn the_detail_pane_never_takes_more_than_half_the_panel() {
        assert!(close(detail_width(DOCK), DETAIL_WIDTH));
        assert!(close(detail_width(290.0), 145.0));
    }
}
