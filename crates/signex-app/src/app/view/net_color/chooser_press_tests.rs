//! Mockup row 71, decided 2026-11-22: the chooser closes on any outside
//! press, and a press on the card or on another row keeps its normal action.

use iced::advanced::shell::{Bus, Waker};
use iced::mouse::{self, Cursor};
use iced::{Event, Point, Size};
use iced_runtime::UserInterface;
use iced_runtime::user_interface::Cache;
use signex_types::schematic::{Label, LabelType, Point as SheetPoint, SchematicSheet};

use crate::app::handlers::editing_commands::net_color_driver::add_sheet_with;
use crate::app::state::{ModalId, WindowKind};
use crate::app::{Message, NetColorMsg, Signex};
use crate::layout_probe::{headless_renderer, lay_out_texts};
use crate::styles::Spacing;

const WINDOW: Size = Size::new(360.0, 400.0);

fn palette_with_nets() -> Signex {
    let (mut app, _task) = Signex::new();
    let mut sheet = SchematicSheet::default();
    for (i, net) in ["A", "B", "C"].into_iter().enumerate() {
        let at = SheetPoint::new(0.0, 10.0 * i as f64);
        sheet.labels.push(Label::new(net, at, LabelType::Net));
    }
    add_sheet_with(&mut app, std::path::Path::new("palette.snxsch"), sheet);
    app
}

fn open_row(app: &mut Signex, net: &str) {
    let _ = app.update(Message::NetColor(NetColorMsg::RowOpened(net.into())));
}

fn chooser(app: &Signex) -> Option<&str> {
    app.ui_state.net_color_palette.chooser.as_deref()
}

/// The middle of `net`'s name, which the card at the right edge never covers.
fn name_of(app: &Signex, net: &str) -> Point {
    lay_out_texts(app.view_net_color_palette_body(), WINDOW)
        .into_iter()
        .find(|laid| laid.text == net)
        .map(|laid| laid.bounds.center())
        .expect("the net has a row")
}

/// Inside the open card: its right padding, level with its row. The card
/// stops one edge margin short of the window.
fn on_card(app: &Signex, net: &str) -> Point {
    let inset = Spacing::DEFAULT.s + 2.0;
    Point::new(WINDOW.width - inset, name_of(app, net).y)
}

fn click_events(at: Point) -> [Event; 3] {
    [
        mouse::Event::CursorMoved { position: at },
        mouse::Event::ButtonPressed(mouse::Button::Left),
        mouse::Event::ButtonReleased(mouse::Button::Left),
    ]
    .map(Event::Mouse)
}

/// Feeds `events` to one built interface and applies what they published.
fn deliver(app: &mut Signex, events: &[Event], at: Point, cache: Cache) -> Cache {
    let mut renderer = headless_renderer();
    let mut bus = Bus::new();
    let mut ui = UserInterface::build(
        app.view_net_color_palette_body(),
        WINDOW,
        cache,
        &mut renderer,
    );
    let _ = ui.update(
        &iced::window::Headless,
        &Waker::noop(),
        events,
        Cursor::Available(at),
        &mut renderer,
        &mut bus,
    );
    let cache = ui.into_cache();
    let messages: Vec<Message> = bus.drain().map(|(message, _)| message).collect();
    for message in messages {
        let _ = app.update(message);
    }
    cache
}

/// A click with the view rebuilt after each event, as at normal speed.
fn click(app: &mut Signex, at: Point) {
    let mut cache = Cache::default();
    for event in click_events(at) {
        cache = deliver(app, std::slice::from_ref(&event), at, cache);
    }
}

/// A click whose press and release arrive in one batch, with no rebuild.
fn fast_click(app: &mut Signex, at: Point) {
    let _ = deliver(app, &click_events(at), at, Cache::default());
}

#[test]
fn a_press_on_the_card_keeps_the_chooser_open() {
    for deliver_click in [click, fast_click] {
        let mut app = palette_with_nets();
        open_row(&mut app, "A");
        let at = on_card(&app, "A");
        deliver_click(&mut app, at);
        assert_eq!(chooser(&app), Some("A"));
    }
}

#[test]
fn a_press_on_another_row_moves_the_one_chooser_to_it() {
    for deliver_click in [click, fast_click] {
        let mut app = palette_with_nets();
        open_row(&mut app, "A");
        let at = name_of(&app, "C");
        deliver_click(&mut app, at);
        assert_eq!(chooser(&app), Some("C"));
    }
}

#[test]
fn a_press_on_the_open_row_closes_its_chooser() {
    for deliver_click in [click, fast_click] {
        let mut app = palette_with_nets();
        open_row(&mut app, "A");
        let at = name_of(&app, "A");
        deliver_click(&mut app, at);
        assert_eq!(chooser(&app), None);
    }
}

#[test]
fn a_press_on_empty_palette_space_closes_the_chooser() {
    let mut app = palette_with_nets();
    open_row(&mut app, "A");
    click(&mut app, Point::new(4.0, WINDOW.height - 60.0));
    assert_eq!(chooser(&app), None);
}

/// The pointer leaving and focus going elsewhere are not presses on the card
/// or the rows, so the open card ignores them.
#[test]
fn the_palette_view_losing_focus_leaves_the_chooser_open() {
    let mut app = palette_with_nets();
    open_row(&mut app, "A");
    let at = name_of(&app, "C");
    let events = [
        Event::Mouse(mouse::Event::CursorLeft),
        Event::Window(iced::window::Event::Unfocused),
    ];
    let _ = deliver(&mut app, &events, at, Cache::default());
    assert_eq!(chooser(&app), Some("A"));
}

#[test]
fn a_press_in_another_window_closes_the_chooser_and_one_in_the_palette_does_not() {
    let mut app = palette_with_nets();
    let palette = iced::window::Id::unique();
    app.ui_state
        .windows
        .insert(palette, WindowKind::DetachedModal(ModalId::NetColorPalette));
    open_row(&mut app, "A");
    let _ = app.update(Message::NetColor(NetColorMsg::PressedInWindow(palette)));
    assert_eq!(chooser(&app), Some("A"));
    let main = iced::window::Id::unique();
    let _ = app.update(Message::NetColor(NetColorMsg::PressedInWindow(main)));
    assert_eq!(chooser(&app), None);
}
