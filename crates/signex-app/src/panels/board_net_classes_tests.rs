//! #2400: the board Net Classes panel's table, chips and buttons.

use signex_types::net_class::NetClass;

use super::*;
use crate::layout_probe::{lay_out_texts, messages_on_click};

fn context(view: Option<BoardNetClassView>) -> PanelContext {
    let (app, _boot) = crate::app::Signex::new();
    let mut ctx = app.document_state.panel_ctx.clone();
    ctx.net_classes.board = view;
    ctx
}

fn power() -> NetClass {
    NetClass {
        nets: vec!["VCC".into()],
        trace_width: 0.5,
        clearance: 0.3,
        ..NetClass::named("Power")
    }
}

fn table() -> BoardNetClassView {
    BoardNetClassView {
        in_project: true,
        classes: vec![power()],
        nets: vec!["GND".into(), "VCC".into()],
        ..BoardNetClassView::default()
    }
}

const WINDOW: iced::Size = iced::Size::new(2000.0, 800.0);

fn texts(ctx: &PanelContext) -> Vec<String> {
    lay_out_texts(view(ctx), WINDOW)
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
}

#[test]
fn a_cell_shows_the_stored_value_and_an_unset_rule_empty() {
    let class = power();
    assert_eq!(BoardNetClassField::Width.shown(&class), "0.5");
    assert_eq!(BoardNetClassField::Clearance.shown(&class), "0.3");
    assert_eq!(BoardNetClassField::ViaDrill.shown(&class), "");
    assert_eq!(BoardNetClassField::Name.shown(&class), "Power");
}

#[test]
fn the_table_has_its_headings_and_the_members_as_chips() {
    let shown = texts(&context(Some(table())));
    for heading in ["Name", "Width", "Clearance", "Via", "Drill", "Nets"] {
        assert!(shown.iter().any(|t| t == heading), "{heading}: {shown:?}");
    }
    assert!(shown.iter().any(|t| t == "VCC"), "{shown:?}");
}

#[test]
fn a_chip_takes_its_net_out_and_add_class_adds() {
    let ctx = context(Some(table()));
    let pressed = messages_on_click(view(&ctx), WINDOW, "VCC");
    assert!(
        matches!(
            pressed.as_slice(),
            [PanelMsg::BoardNetClass(BoardNetClassMsg::NetRemoved { class, net })]
                if class == "Power" && net == "VCC"
        ),
        "{pressed:?}"
    );
    let pressed = messages_on_click(view(&ctx), WINDOW, "Add Class");
    assert!(
        matches!(
            pressed.as_slice(),
            [PanelMsg::BoardNetClass(BoardNetClassMsg::AddRequested)]
        ),
        "{pressed:?}"
    );
}

#[test]
fn without_a_board_or_a_project_the_panel_says_why() {
    let shown = texts(&context(None));
    assert!(
        shown.iter().any(|t| t.starts_with("Open a board")),
        "{shown:?}"
    );
    let loose = BoardNetClassView {
        in_project: false,
        ..BoardNetClassView::default()
    };
    let shown = texts(&context(Some(loose)));
    assert!(
        shown.iter().any(|t| t.contains("in no open project")),
        "{shown:?}"
    );
}
