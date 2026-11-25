//! Mockup row 75, decided 2026-11-24 (docs/DECISIONS.md): the Annotate
//! dialog keeps the read-only Fixed options as a collapsed group, and every
//! dialog session opens it collapsed.

use crate::app::{AnnotateMsg, Message, Signex};

fn annotate(app: &mut Signex, msg: AnnotateMsg) {
    let _ = app.update(Message::Annotate(msg));
}

fn fixed_options_open(app: &Signex) -> bool {
    app.ui_state.annotate_session.fixed_options_open
}

#[test]
fn the_annotate_fixed_options_group_opens_collapsed_every_session() {
    let (mut app, _boot) = Signex::new();
    annotate(&mut app, AnnotateMsg::OpenDialog);
    assert!(!fixed_options_open(&app), "collapsed when the dialog opens");
    annotate(&mut app, AnnotateMsg::FixedOptionsToggled);
    assert!(fixed_options_open(&app), "a click unfolds it");

    annotate(&mut app, AnnotateMsg::CloseDialog);
    annotate(&mut app, AnnotateMsg::OpenDialog);
    assert!(
        !fixed_options_open(&app),
        "the next session starts collapsed"
    );
}
