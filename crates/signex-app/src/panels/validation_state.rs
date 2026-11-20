//! The view state the ERC, DRC and Messages panels share in the Model: what
//! each list shows, the cached user identity a waiver's Author starts as,
//! and what the newest clean ERC run checked.

use super::{DrcView, ErcChecked, ErcPanelView, MessagesView, UserIdentity};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ValidationPanels {
    pub erc: ErcPanelView,
    /// The DRC panel's tiles, folded groups and row hover and selection.
    pub drc: DrcView,
    pub messages: MessagesView,
    /// The user identity, read once off the update loop (AV-13).
    pub identity: UserIdentity,
    /// What the newest ERC run checked.
    pub erc_checked: Option<ErcChecked>,
    /// The Run ERC key as the active keymap writes it; set on every panel
    /// refresh, empty when the keymap binds none.
    pub run_erc_key: String,
}
