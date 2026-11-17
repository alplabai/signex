//! Cross-sheet net navigation messages (#1270, #1271, #1272), carried under
//! `UiMsg::NetNav`.

use std::sync::Arc;

use crate::app::state::net_nav::IndexOutcome;

#[derive(Debug, Clone)]
pub enum NetNavMsg {
    /// Go to net partner: from a port, global or hierarchical label or sheet
    /// entry to the next one on the same net.
    PartnerJumpRequested,
    SheetBackRequested,
    SheetForwardRequested,
    /// Highlight the net under the cursor on every sheet.
    NetHighlightRequested,
    /// Highlight the net at the world point `at` on every sheet, such as where
    /// a context menu was opened (#2419); the selection's net when nothing on
    /// a net is there.
    NetHighlightAtRequested {
        at: (f64, f64),
    },
    /// Highlight the net of the selection and select what is on it here.
    RelatedNetHighlightRequested,
    HighlightStepRequested {
        forward: bool,
    },
    HighlightClearRequested,
    /// The dwell timer fired while the cursor rests on an object.
    HoverDwellTicked,
    /// A background build of a project's net index finished.
    IndexResolved(Arc<IndexOutcome>),
}
