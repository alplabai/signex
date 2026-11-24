//! Mockup rows 89 and 90: the domain bar keeps Table, Text and Run DRC in
//! view when the docks leave the tab a narrow width.

use iced::Size;

use super::RulesTabMsg;
use super::matrix_tests::{app_with_board, send};
use crate::layout_probe::lay_out_texts;

#[test]
fn a_narrow_tab_keeps_run_drc_and_every_domain_in_view() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_board(&dir);
    // Off: the message gives the project its tab state, on the table view.
    send(
        &mut app,
        RulesTabMsg::TextViewToggled {
            project_id: id,
            on: false,
        },
    );
    let width = 860.0;
    let laid = lay_out_texts(app.view_rules_tab(id), Size::new(width, 800.0));
    let at = |label: &str| {
        laid.iter()
            .find(|t| t.text == label)
            .unwrap_or_else(|| panic!("{label} is not drawn"))
            .bounds
    };
    for label in ["Clearance", "Placement", "Table", "Text", "Run DRC"] {
        let bounds = at(label);
        assert!(
            bounds.x + bounds.width <= width,
            "{label} ends at {} of {width}",
            bounds.x + bounds.width
        );
    }
    assert!(
        (at("Table").y - at("Text").y).abs() < 1.0,
        "the switch stays on one line"
    );
}
