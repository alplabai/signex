//! The Graphite Selection Filter's All / None control sets the whole
//! filter set, whatever it held before (the legacy toggle flips it, which
//! turns a partial set all on rather than off).

use super::super::super::*;
use crate::active_bar::{ActiveBarMsg, SelectionFilter};

fn app_with_filters(filters: &[SelectionFilter]) -> Signex {
    let (mut app, _task) = Signex::new();
    app.interaction_state.selection_filters = filters.iter().copied().collect();
    app
}

#[test]
fn none_clears_a_partial_filter_set() {
    let mut app = app_with_filters(&[SelectionFilter::Wires, SelectionFilter::Texts]);
    let _ = app.update(Message::ActiveBar(ActiveBarMsg::NoFiltersPicked));
    assert!(app.interaction_state.selection_filters.is_empty());
    assert!(app.document_state.panel_ctx.selection_filters.is_empty());
}

#[test]
fn all_turns_every_category_on_and_is_idempotent() {
    let mut app = app_with_filters(&[SelectionFilter::Wires]);
    for _ in 0..2 {
        let _ = app.update(Message::ActiveBar(ActiveBarMsg::AllFiltersPicked));
        assert_eq!(
            app.interaction_state.selection_filters.len(),
            SelectionFilter::ALL.len()
        );
    }
    assert_eq!(
        app.document_state.panel_ctx.selection_filters,
        app.interaction_state.selection_filters
    );
}

#[test]
fn none_twice_stays_empty() {
    let mut app = app_with_filters(SelectionFilter::ALL);
    for _ in 0..2 {
        let _ = app.update(Message::ActiveBar(ActiveBarMsg::NoFiltersPicked));
        assert!(app.interaction_state.selection_filters.is_empty());
    }
}
