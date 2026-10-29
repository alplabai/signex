//! #1253 — a label placed again and again counts up (D0, D1, D2) by the
//! "Increment step", and Repeat last item arms the last label again with the
//! next value.

use signex_app::app::{EditMsg, Message, Signex, Tool};
use signex_app::panels::{PrePlacementData, PrePlacementKind};
use signex_app::toolbar::ToolMessage;

use crate::library_place_symbol::{app_with_empty_sheet, click, doc};

fn select(app: &mut Signex, tool: Tool) {
    let _ = app.update(Message::Tool(ToolMessage::SelectTool(tool)));
}

/// What the Tab form leaves behind: the label tool with its text typed.
fn arm_label(app: &mut Signex, text: &str) {
    select(app, Tool::Label);
    app.document_state.panel_ctx.pre_placement = Some(PrePlacementData {
        tool_name: "Net Label".to_owned(),
        kind: PrePlacementKind::NetLabel,
        label_text: text.to_owned(),
        designator: String::new(),
        rotation: 0.0,
        font: "Iosevka".to_owned(),
        font_size_pt: 10,
        justify_h: Default::default(),
        justify_v: Default::default(),
        bold: false,
        italic: false,
        underline: false,
        cursor_x_mm: 0.0,
        cursor_y_mm: 0.0,
        shape_width_mm: 0.0,
        shape_fill: signex_types::schematic::FillType::None,
    });
}

fn label_texts(app: &Signex) -> Vec<String> {
    doc(app).labels.iter().map(|l| l.text.clone()).collect()
}

fn click_n(app: &mut Signex, n: usize) {
    for i in 0..n {
        click(app, (25.4 + 5.08 * i as f64, 38.1));
    }
}

#[test]
fn a_label_placed_repeatedly_counts_up() {
    let mut app = app_with_empty_sheet();
    arm_label(&mut app, "D0");
    click_n(&mut app, 3);
    assert_eq!(label_texts(&app), ["D0", "D1", "D2"]);
}

#[test]
fn the_increment_step_preference_sets_the_stride() {
    let mut app = app_with_empty_sheet();
    app.ui_state.increment_step = 5;
    arm_label(&mut app, "D0");
    click_n(&mut app, 3);
    assert_eq!(label_texts(&app), ["D0", "D5", "D10"]);

    let mut down = app_with_empty_sheet();
    down.ui_state.increment_step = -2;
    arm_label(&mut down, "A10");
    click_n(&mut down, 3);
    assert_eq!(label_texts(&down), ["A10", "A8", "A6"]);
}

#[test]
fn a_label_without_a_trailing_number_repeats_unchanged() {
    let mut app = app_with_empty_sheet();
    arm_label(&mut app, "CLK");
    click_n(&mut app, 2);
    assert_eq!(label_texts(&app), ["CLK", "CLK"]);
}

#[test]
fn repeat_last_item_arms_the_label_again_with_the_next_value() {
    let mut app = app_with_empty_sheet();
    arm_label(&mut app, "D0");
    click_n(&mut app, 1);
    // Esc: the tool and its typed text are gone.
    select(&mut app, Tool::Select);
    assert!(app.document_state.panel_ctx.pre_placement.is_none());

    let _ = app.update(Message::Edit(EditMsg::RepeatLastItem));
    assert_eq!(app.interaction_state.current_tool, Tool::Label);
    click_n(&mut app, 2);
    assert_eq!(label_texts(&app), ["D0", "D1", "D2"]);
}

#[test]
fn repeat_last_item_with_nothing_placed_says_so_and_changes_nothing() {
    let _ = signex_app::diagnostics::init_logging();
    let mut app = app_with_empty_sheet();
    let _ = app.update(Message::Edit(EditMsg::RepeatLastItem));
    assert_eq!(app.interaction_state.current_tool, Tool::Select);
    assert!(doc(&app).labels.is_empty());
    assert!(
        signex_app::diagnostics::recent_entries()
            .iter()
            .any(|e| e.message.contains("Repeat last item: no label or text")),
        "the Messages panel explains why nothing happened"
    );
}

#[test]
fn repeat_last_item_repeats_a_text_note_with_the_next_value() {
    let mut app = app_with_empty_sheet();
    select(&mut app, Tool::Text);
    app.document_state.panel_ctx.pre_placement = Some(PrePlacementData {
        tool_name: "Text".to_owned(),
        kind: PrePlacementKind::TextNote,
        label_text: "Note 1".to_owned(),
        designator: String::new(),
        rotation: 0.0,
        font: "Iosevka".to_owned(),
        font_size_pt: 10,
        justify_h: Default::default(),
        justify_v: Default::default(),
        bold: false,
        italic: false,
        underline: false,
        cursor_x_mm: 0.0,
        cursor_y_mm: 0.0,
        shape_width_mm: 0.0,
        shape_fill: signex_types::schematic::FillType::None,
    });
    click_n(&mut app, 1);
    let _ = app.update(Message::Edit(EditMsg::RepeatLastItem));
    assert_eq!(app.interaction_state.current_tool, Tool::Text);
    click_n(&mut app, 1);
    let notes: Vec<_> = doc(&app)
        .text_notes
        .iter()
        .map(|n| n.text.clone())
        .collect();
    assert_eq!(notes, ["Note 1", "Note 2"]);
}

#[test]
fn repeat_last_item_restores_the_look_the_form_gave_the_label() {
    let mut app = app_with_empty_sheet();
    arm_label(&mut app, "D0");
    if let Some(pre) = app.document_state.panel_ctx.pre_placement.as_mut() {
        pre.rotation = 90.0;
        pre.font_size_pt = 14;
        pre.justify_h = signex_types::schematic::HAlign::Right;
    }
    click_n(&mut app, 1);
    select(&mut app, Tool::Select);
    let _ = app.update(Message::Edit(EditMsg::RepeatLastItem));
    click_n(&mut app, 1);

    let labels = &doc(&app).labels;
    assert_eq!(labels[1].text, "D1");
    assert_eq!(labels[1].rotation, labels[0].rotation);
    assert_eq!(labels[1].rotation, 90.0);
    assert_eq!(labels[1].font_size, labels[0].font_size);
    assert_eq!(labels[1].justify, labels[0].justify);
}
