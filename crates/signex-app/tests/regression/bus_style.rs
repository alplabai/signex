//! #2813, mockup row 43: the Properties Style section edits a placed bus's
//! width and colour through undoable commands, and a save keeps them.

use signex_app::app::{EditMsg, Message, Signex};
use signex_app::dock::DockMessage;
use signex_app::panels::PanelMsg;
use signex_types::format::SnxSchematic;
use signex_types::schematic::{Bus, Point, WireWidthPreset};
use signex_types::theme::Color;
use uuid::Uuid;

use crate::library_place_symbol::{app_with_empty_sheet, doc};

const RED: Color = Color::new(255, 0, 0, 255);

fn panel(app: &mut Signex, msg: PanelMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(msg)));
}

fn app_with_bus() -> (Signex, Uuid) {
    let mut app = app_with_empty_sheet();
    let id = Uuid::now_v7();
    let bus = Bus {
        uuid: id,
        start: Point::new(10.0, 20.0),
        end: Point::new(30.0, 20.0),
        ..Bus::default()
    };
    app.document_state
        .engines
        .values_mut()
        .next()
        .expect("engine")
        .execute(signex_engine::Command::PlaceBus { bus })
        .expect("bus");
    (app, id)
}

fn bus(app: &Signex) -> &Bus {
    &doc(app).buses[0]
}

#[test]
fn width_and_colour_of_a_placed_bus_edit_and_undo() {
    let (mut app, id) = app_with_bus();
    panel(&mut app, PanelMsg::EditBusWidth(id, WireWidthPreset::Large));
    panel(&mut app, PanelMsg::EditBusColor(id, Some(RED)));
    assert_eq!(bus(&app).stroke_width, 1.0);
    assert_eq!(bus(&app).color, Some(RED));

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(bus(&app).color, None, "colour undoes first");
    assert_eq!(bus(&app).stroke_width, 1.0);
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(bus(&app).stroke_width, 0.0);

    let _ = app.update(Message::Edit(EditMsg::Redo));
    let _ = app.update(Message::Edit(EditMsg::Redo));
    assert_eq!(bus(&app).stroke_width, 1.0);
    assert_eq!(bus(&app).color, Some(RED));

    // Back to the theme colour is an edit too.
    panel(&mut app, PanelMsg::EditBusColor(id, None));
    assert_eq!(bus(&app).color, None);
}

#[test]
fn an_edited_bus_style_survives_a_save_and_load() {
    let (mut app, id) = app_with_bus();
    panel(
        &mut app,
        PanelMsg::EditBusWidth(id, WireWidthPreset::Medium),
    );
    panel(&mut app, PanelMsg::EditBusColor(id, Some(RED)));
    let text = SnxSchematic::new(doc(&app).clone())
        .write_string()
        .expect("save");
    assert!(text.starts_with("format = \"snxsch/1.4\""), "{text}");
    let back = SnxSchematic::parse(&text).expect("load").sheet;
    assert_eq!(back.buses[0].stroke_width, 0.5);
    assert_eq!(back.buses[0].color, Some(RED));
}
