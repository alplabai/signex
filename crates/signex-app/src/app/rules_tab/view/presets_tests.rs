//! Mockup row 91: the Presets panel keeps Cancel and Apply at the foot of
//! the preview card, and the card in view, however long the list of changes.

use iced::Size;

use super::{CHANGES_MAX_EM, TS};
use crate::app::Message;
use crate::app::rules_tab::matrix_tests::{app_with_board, send};
use crate::app::rules_tab::{RulesPanel, RulesTabMsg};
use crate::layout_probe::lay_out_texts;

#[test]
fn cancel_and_apply_stay_in_view_at_the_foot_of_a_long_preview() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_board(&dir);
    let _ = app.update(Message::Menu(crate::menu_bar::MenuMessage::command(
        "open_rules",
    )));
    send(
        &mut app,
        RulesTabMsg::PanelPicked {
            project_id: id,
            panel: Some(RulesPanel::Presets),
        },
    );
    // The Advanced tier adds seven rules: more than the card shows at once.
    send(
        &mut app,
        RulesTabMsg::PresetPicked {
            project_id: id,
            preset: "tier:advanced".into(),
        },
    );
    // The default 1400 x 900 window less the shell's menu, tab and status
    // bars.
    let window = Size::new(1400.0, 760.0);
    let texts = lay_out_texts(app.view_rules_tab(id), window);
    assert!(
        texts.iter().any(|t| t.text == "fab_silk"),
        "the preview lists the tier's rules"
    );
    let title = texts
        .iter()
        .find(|t| t.text.starts_with("Applying"))
        .expect("the preview card's title is drawn");
    let title_bottom = title.bounds.y + title.bounds.height;
    // The title, the capped list and the button row's padding.
    let card_reach = TS.body * (CHANGES_MAX_EM + 4.0);
    for label in ["Cancel", "Apply"] {
        let button = texts
            .iter()
            .rfind(|t| t.text == label)
            .unwrap_or_else(|| panic!("{label} is not drawn"));
        let bottom = button.bounds.y + button.bounds.height;
        assert!(
            button.bounds.y >= 0.0 && bottom <= window.height,
            "{label} spans {}..{bottom} of {}",
            button.bounds.y,
            window.height
        );
        assert!(
            button.bounds.y > title_bottom && button.bounds.y - title_bottom <= card_reach,
            "{label} at {} sits at the foot of the card titled at {title_bottom}",
            button.bounds.y
        );
    }
}
