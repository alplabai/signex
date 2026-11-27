//! Length Matching (#2797, mockup row 107): a board's length-matching
//! groups as a document tab (`app/length_matching_tab`) and as the compact
//! dock panel beside the board. One model, two pure views (DECISIONS
//! 2026-11-18, "Tab + compact panel pair"): both read the rows the app
//! copies from the board ([`LengthMatchingFacts`],
//! `app/handlers/length_matching`) and the view state in
//! [`LengthMatchingUi`], and both send [`LengthMatchingMsg`]. Every edit is
//! a `set_length_group` or `remove_length_group` step on the board.

mod model;
#[cfg(test)]
mod tests;
mod view;

pub use model::{
    LengthField, LengthFilter, LengthGroupRow, LengthMatchingFacts, LengthMatchingMsg,
    LengthMatchingUi, LengthMemberRow,
};
pub use view::{panel_view, tab_view};

use super::PanelMsg;

fn msg(event: LengthMatchingMsg) -> PanelMsg {
    PanelMsg::LengthMatching(event)
}
