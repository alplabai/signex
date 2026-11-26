//! Document Options with nothing selected: a hovered General or Page
//! Options row lights that row alone, and a late leave of the row the
//! pointer came from does not clear the row it is on now.

use signex_app::app::{Message, Signex};
use signex_app::dock::DockMessage;
use signex_app::panels::{DocumentOptionsMsg, PanelMsg};
use signex_widgets::property_illustration::{FieldId, IllustrationMsg};

const VISIBLE_GRID: FieldId = FieldId::new("doc-visible-grid");
const SNAP_GRID: FieldId = FieldId::new("doc-snap-grid");
const HOTSPOTS: FieldId = FieldId::new("doc-snap-hotspots");
const ORIGIN: FieldId = FieldId::new("doc-origin");
const BORDER: FieldId = FieldId::new("doc-show-border");
const ZONES: FieldId = FieldId::new("doc-zones");
const TITLE_BLOCK: FieldId = FieldId::new("doc-show-title-block");

const ROWS: [FieldId; 7] = [
    VISIBLE_GRID,
    SNAP_GRID,
    HOTSPOTS,
    ORIGIN,
    BORDER,
    ZONES,
    TITLE_BLOCK,
];

fn hover(app: &mut Signex, msg: IllustrationMsg) {
    let msg = PanelMsg::DocumentOptions(DocumentOptionsMsg::Illustration(msg));
    let _ = app.update(Message::Dock(DockMessage::Panel(msg)));
}

/// The rows lit now, in panel order.
fn lit(app: &Signex) -> Vec<FieldId> {
    let state = &app.document_view.panel_ctx.document_options.hover;
    ROWS.into_iter()
        .filter(|row| state.is_field_highlighted(row))
        .collect()
}

#[test]
fn page_options_rows_light_one_at_a_time_and_a_late_leave_keeps_the_new_row() {
    let (mut app, _boot) = Signex::new();
    hover(&mut app, IllustrationMsg::FieldHovered(ORIGIN));
    assert_eq!(lit(&app), vec![ORIGIN]);
    // Moving down: the Show Border header is entered before Origin's leave.
    hover(&mut app, IllustrationMsg::FieldHovered(BORDER));
    assert_eq!(lit(&app), vec![BORDER]);
    hover(&mut app, IllustrationMsg::FieldLeft(ORIGIN));
    assert_eq!(lit(&app), vec![BORDER], "a late leave keeps the new row");
    hover(&mut app, IllustrationMsg::FieldLeft(BORDER));
    hover(&mut app, IllustrationMsg::FieldHovered(ZONES));
    assert_eq!(
        lit(&app),
        vec![ZONES],
        "Zones lights alone, not with Show Border"
    );
    hover(&mut app, IllustrationMsg::FieldLeft(ZONES));
    assert_eq!(lit(&app), Vec::<FieldId>::new());
}

#[test]
fn general_rows_light_one_at_a_time_and_a_late_leave_keeps_the_new_row() {
    let (mut app, _boot) = Signex::new();
    hover(&mut app, IllustrationMsg::FieldHovered(VISIBLE_GRID));
    assert_eq!(lit(&app), vec![VISIBLE_GRID]);
    hover(&mut app, IllustrationMsg::FieldHovered(SNAP_GRID));
    hover(&mut app, IllustrationMsg::FieldLeft(VISIBLE_GRID));
    assert_eq!(lit(&app), vec![SNAP_GRID], "a late leave keeps the new row");
    hover(&mut app, IllustrationMsg::FieldLeft(SNAP_GRID));
    assert_eq!(lit(&app), Vec::<FieldId>::new());
}
