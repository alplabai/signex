//! A hierarchical project for tests that need sheets placed by sheet symbols
//! (#1618): `top -> mid -> leaf` plus the loose `other`, all open in memory.

use std::path::PathBuf;

use crate::app::handlers::menu::export::tests::{app_workspace, open_with, sheet_with_net};

pub(crate) const DIR: &str = "/w/tree-pins";
pub(crate) const LISTED: [&str; 4] = ["top.snxsch", "other.snxsch", "mid.snxsch", "leaf.snxsch"];

/// `top` -> `mid` -> `leaf`, plus the loose `other`; every sheet open.
pub(crate) fn hierarchical_app() -> crate::app::Signex {
    let mut app = app_workspace(DIR, &LISTED);
    let path = |name: &str| PathBuf::from(DIR).join(name);
    for (name, reference, children) in [
        ("top.snxsch", "R_TOP", &["mid.snxsch"][..]),
        ("other.snxsch", "R_OTHER", &[][..]),
        ("mid.snxsch", "R_MID", &["leaf.snxsch"][..]),
        ("leaf.snxsch", "R_LEAF", &[][..]),
    ] {
        open_with(
            &mut app.document_state,
            &path(name),
            sheet_with_net(reference, &format!("NET_{reference}"), children),
        );
    }
    app.document_state.active_path = Some(path("top.snxsch"));
    app
}
