//! Mockup row 91: the Presets panel keeps Cancel and Apply of a preview in
//! view at a short window, however long the list of changes.

use iced::Size;

use crate::app::Message;
use crate::app::rules_tab::matrix_tests::{app_with_board, send};
use crate::app::rules_tab::{RulesPanel, RulesTabMsg};
use crate::layout_probe::lay_out_texts;

#[test]
fn cancel_and_apply_stay_in_view_under_a_long_preview() {
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
    // The Advanced tier adds seven rules: the list outgrows the panel.
    send(
        &mut app,
        RulesTabMsg::PresetPicked {
            project_id: id,
            preset: "tier:advanced".into(),
        },
    );
    let window = Size::new(1200.0, 480.0);
    let texts = lay_out_texts(app.view_rules_tab(id), window);
    assert!(
        texts.iter().any(|t| t.text == "fab_silk"),
        "the preview lists the tier's rules"
    );
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
    }
}
