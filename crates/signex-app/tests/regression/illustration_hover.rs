//! #2523: the field / callout hover of the Properties illustration is panel
//! state. A field-row hover routed through `PanelMsg::Illustration` lights
//! that field for the subject on show, its leave clears it, and a hover
//! left from another subject reads as none.

use signex_app::app::{Message, Signex, Tool};
use signex_app::dock::DockMessage;
use signex_app::panels::{HoverSubject, PanelMsg};
use signex_app::toolbar::ToolMessage;
use signex_widgets::property_illustration::{FieldId, IllustrationMsg};

use crate::library_place_symbol::app_with_empty_sheet;

const WIDTH: FieldId = FieldId::new("wire-width");

fn hover(app: &mut Signex, msg: IllustrationMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::Illustration(
        msg,
    ))));
}

/// Whether the panel shows the row of `field` lit now.
fn is_lit(app: &Signex, field: &FieldId) -> bool {
    let ctx = &app.document_view.panel_ctx;
    ctx.illustration_hover
        .for_subject(HoverSubject::of(ctx))
        .is_some_and(|h| h.is_field_highlighted(field))
}

#[test]
fn a_field_hover_lights_its_row_until_it_leaves() {
    let mut app = app_with_empty_sheet();
    let _ = app.update(Message::Tool(ToolMessage::SelectTool(Tool::Wire)));
    let _ = app.update(Message::Tool(ToolMessage::PrePlacementTab));
    assert!(app.document_view.panel_ctx.pre_placement.is_some());

    hover(&mut app, IllustrationMsg::FieldHovered(WIDTH));
    assert!(is_lit(&app, &WIDTH), "the hovered field is lit");

    hover(&mut app, IllustrationMsg::FieldLeft(WIDTH));
    assert!(!is_lit(&app, &WIDTH), "leaving the row clears it");
}

#[test]
fn a_hover_of_another_subject_reads_as_none() {
    let mut app = app_with_empty_sheet();
    let _ = app.update(Message::Tool(ToolMessage::SelectTool(Tool::Wire)));
    let _ = app.update(Message::Tool(ToolMessage::PrePlacementTab));
    hover(&mut app, IllustrationMsg::FieldHovered(WIDTH));
    assert!(is_lit(&app, &WIDTH));

    // The Tab form closes: the panel shows another subject.
    app.document_view.panel_ctx.pre_placement = None;
    assert!(!is_lit(&app, &WIDTH), "a stale hover lights nothing");
}
