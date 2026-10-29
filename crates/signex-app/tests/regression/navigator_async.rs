//! #1470 — the Navigator's sheet hierarchy is assembled off the UI thread:
//! a message handled by `update` leaves the disk alone, and the hierarchy
//! arrives with the assembly task (`settle_navigator` runs it inline).

use crate::project_tab_support::{fixture, open_tab};

#[test]
fn update_assembles_no_hierarchy_the_task_does() {
    let mut f = fixture();
    open_tab(&mut f.app);
    assert!(
        f.app.document_state.panel_ctx.navigator.hierarchy.is_none(),
        "update must not read the project's sheets from disk"
    );

    f.app.settle_navigator();

    let navigator = &f.app.document_state.panel_ctx.navigator;
    assert!(navigator.hierarchy.is_some(), "the assembly task delivered");
    assert!(!navigator.fingerprint.is_empty());
}
