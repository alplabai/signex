//! Mockup row 91, the text view: typed text is held until Apply, which is
//! one undo step; Discard drops it.

use std::sync::Arc;

use iced::widget::text_editor::{Action, Edit};
use signex_app::app::RulesTabMsg;

use super::{Rig, SIG_RULE};

const TYPED: &str = "rule sig on clearance when net == \"SIG\" require clearance >= 0.3mm \
                     then error\nrule wide on track require width >= 0.4mm then error\n";
const BROKEN: &str = "rule broken on clearance require clearance >= then error\n";

fn text_rig() -> Rig {
    let mut rig = Rig::new(Some(SIG_RULE), &[]);
    let project_id = rig.id;
    rig.send(RulesTabMsg::TextViewToggled {
        project_id,
        on: true,
    });
    rig
}

/// Select everything in the editor and paste `text` over it.
fn replace_text(rig: &mut Rig, text: &str) {
    let project_id = rig.id;
    rig.send(RulesTabMsg::TextActed {
        project_id,
        action: Action::SelectAll,
    });
    rig.send(RulesTabMsg::TextActed {
        project_id,
        action: Action::Edit(Edit::Paste(Arc::new(text.to_string()))),
    });
}

fn view_text(rig: &Rig) -> String {
    rig.tab()
        .text
        .as_ref()
        .expect("the text view is open")
        .content
        .text()
}

fn is_edited(rig: &Rig) -> bool {
    rig.tab().text.as_ref().is_some_and(|text| text.edited)
}

#[test]
fn the_text_view_opens_on_the_stored_rules_and_writes_nothing() {
    let rig = text_rig();
    assert_eq!(view_text(&rig).trim(), SIG_RULE.trim());
    assert!(!is_edited(&rig));
    rig.assert_untouched(SIG_RULE);
}

#[test]
fn typed_text_is_held_until_apply_stores_it_as_one_undo_step() {
    let mut rig = text_rig();
    let project_id = rig.id;
    replace_text(&mut rig, TYPED);
    assert!(is_edited(&rig));
    rig.assert_untouched(SIG_RULE);

    rig.send(RulesTabMsg::TextApplied { project_id });
    assert!(rig.table().row("wide").is_some(), "{}", rig.stored());
    assert!(!is_edited(&rig), "applied text is no longer pending");
    assert!(rig.is_dirty());

    rig.undo();
    rig.assert_untouched(SIG_RULE);
    assert_eq!(view_text(&rig).trim(), SIG_RULE.trim(), "the view follows");
}

#[test]
fn discard_drops_the_typed_text_and_shows_the_stored_rules() {
    let mut rig = text_rig();
    let project_id = rig.id;
    replace_text(&mut rig, TYPED);

    rig.send(RulesTabMsg::TextDiscarded { project_id });

    assert_eq!(view_text(&rig).trim(), SIG_RULE.trim());
    assert!(!is_edited(&rig));
    rig.assert_untouched(SIG_RULE);
}

#[test]
fn apply_with_nothing_typed_records_no_step() {
    let mut rig = text_rig();
    let project_id = rig.id;
    rig.send(RulesTabMsg::TextApplied { project_id });
    rig.assert_untouched(SIG_RULE);
}

#[test]
fn a_syntax_error_is_reported_as_typed_and_stores_nothing() {
    let mut rig = text_rig();
    replace_text(&mut rig, BROKEN);

    let checked = &rig.tab().text.as_ref().expect("the text view").checked;
    assert!(
        checked.is_unparsed(),
        "the compiler's error reaches the view"
    );
    rig.assert_untouched(SIG_RULE);
}

#[test]
fn leaving_the_text_view_with_unapplied_text_is_refused_and_keeps_it() {
    let mut rig = text_rig();
    let project_id = rig.id;
    replace_text(&mut rig, TYPED);

    rig.send(RulesTabMsg::TextViewToggled {
        project_id,
        on: false,
    });

    assert!(rig.refusal().contains("not applied"), "{}", rig.refusal());
    assert!(rig.tab().text.is_some(), "the view stays open");
    assert!(
        view_text(&rig).contains("rule wide"),
        "the typed text stays"
    );
    rig.assert_untouched(SIG_RULE);
}

#[test]
fn leaving_the_text_view_after_discard_returns_to_the_table() {
    let mut rig = text_rig();
    let project_id = rig.id;
    replace_text(&mut rig, TYPED);
    rig.send(RulesTabMsg::TextDiscarded { project_id });

    rig.send(RulesTabMsg::TextViewToggled {
        project_id,
        on: false,
    });

    assert!(rig.tab().text.is_none());
    assert_eq!(rig.tab().refused, None);
    rig.assert_untouched(SIG_RULE);
}

#[test]
fn undo_does_not_replace_text_that_is_typed_but_not_applied() {
    let mut rig = text_rig();
    let project_id = rig.id;
    replace_text(&mut rig, TYPED);
    rig.send(RulesTabMsg::TextApplied { project_id });
    let applied = rig.stored();
    replace_text(&mut rig, BROKEN);

    rig.undo();

    assert_eq!(rig.stored(), applied, "the applied step is not undone");
    assert!(
        view_text(&rig).contains("rule broken"),
        "the typed text stays"
    );
    assert!(is_edited(&rig));
}
