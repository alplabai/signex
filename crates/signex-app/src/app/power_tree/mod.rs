//! The power tree of a project (#1313, mockup row 171): the run behind
//! `query_power_tree`, which stitches the project off the UI thread and
//! answers with its rails, converters, loads and the power budget per sheet
//! (the budget of the block that stands for the sheet). The tree itself is
//! built in `signex_net::power_tree`. The block diagram tab's Power tree
//! view (mockup row 171) is to read the same run; the tab is not built yet.
//! Rules: `docs/POWER_TREE.md`.

mod answer;
mod job;
