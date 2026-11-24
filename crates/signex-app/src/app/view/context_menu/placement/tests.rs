use super::*;

use crate::app::ContextMenuState;
use crate::layout_probe::{LaidOutText, find, lay_out_texts, messages_on_click_at};
use iced::Widget as _;
use signex_types::theme::{ThemeId, theme_tokens};
use signex_widgets::active_bar_dropdown::DropdownItem;

const WINDOW: (f32, f32) = (1000.0, 800.0);
const MENU: (f32, f32) = (200.0, 100.0);

/// `layers` stacked over an empty window, as `view` stacks overlays.
fn stacked(layers: Vec<Element<'_, Message>>) -> Element<'_, Message> {
    let mut stack: iced::widget::Stack<Element<'_, Message>> = iced::widget::Stack::new()
        .width(iced::Length::Fill)
        .height(iced::Length::Fill)
        .push(
            iced::widget::Space::new()
                .width(iced::Length::Fill)
                .height(iced::Length::Fill)
                .boxed(),
        );
    for layer in layers {
        stack = stack.push(layer);
    }
    stack.boxed()
}

/// Every label of `layers` laid out in a window of `window`.
pub(in crate::app::view::context_menu) fn lay_out_layers(
    layers: Vec<Element<'_, Message>>,
    window: iced::Size,
) -> Vec<LaidOutText> {
    lay_out_texts(stacked(layers), window)
}

#[test]
fn an_anchor_with_room_is_kept() {
    assert_eq!(clamp_menu_origin((10.0, 20.0), MENU, WINDOW), (10.0, 20.0));
}

#[test]
fn a_menu_crossing_the_right_edge_shifts_left_of_it() {
    let (x, y) = clamp_menu_origin((950.0, 20.0), MENU, WINDOW);
    assert_eq!(x, WINDOW.0 - MENU.0 - EDGE_MARGIN);
    assert_eq!(y, 20.0);
}

/// The card moves up only as far as the bottom edge needs; it no longer
/// flips a whole card height above the cursor (the maintainer asked for
/// the origin at the cursor, clamped only at the window edge).
#[test]
fn a_menu_crossing_the_bottom_edge_moves_up_only_to_the_edge() {
    let (x, y) = clamp_menu_origin((10.0, 780.0), MENU, WINDOW);
    assert_eq!(x, 10.0);
    assert_eq!(y, WINDOW.1 - MENU.1 - EDGE_MARGIN);
}

#[test]
fn a_menu_larger_than_the_window_stays_on_screen() {
    assert_eq!(
        clamp_menu_origin((5.0, 5.0), (2000.0, 2000.0), WINDOW),
        (0.0, 0.0)
    );
}

#[test]
fn a_flyout_opens_right_of_its_parent_at_the_given_offset() {
    let (x, y) = submenu_origin((100.0, 50.0), 200.0, (200.0, 300.0), 64.0, WINDOW);
    assert_eq!((x, y), (300.0, 114.0));
}

#[test]
fn a_flyout_that_would_cross_the_right_edge_opens_left() {
    let (x, _) = submenu_origin((700.0, 0.0), 200.0, (200.0, 100.0), 0.0, WINDOW);
    assert_eq!(x, 500.0);
}

#[test]
fn a_flyout_that_would_cross_the_bottom_edge_moves_up_to_it() {
    let (_, y) = submenu_origin((100.0, 600.0), 200.0, (200.0, 300.0), 100.0, WINDOW);
    assert_eq!(y, WINDOW.1 - 300.0 - EDGE_MARGIN);
}

fn plain_card(tokens: &signex_types::theme::ThemeTokens) -> Element<'_, Message> {
    let entries = ["Alpha", "Bravo", "Charlie"]
        .into_iter()
        .map(|label| DropdownEntry::Item(DropdownItem::new(label, Message::Ui(UiMsg::Noop))))
        .collect();
    menu_card(entries, tokens, "").element
}

/// A menu opens with its corner at the cursor wherever it has room, in any
/// window: the clamp reads the window it is laid out in, so an undocked
/// window clamps to its own size.
#[test]
fn a_menu_opens_at_the_cursor_and_clamps_only_at_its_own_windows_edge() {
    for id in [ThemeId::GraphiteLight] {
        let tokens = theme_tokens(id);
        let metrics = MenuMetrics::GRAPHITE;
        let card_h = 3.0 * metrics.item_height(false) + 2.0 * metrics.panel_pad;
        for window in [(1400.0_f32, 1000.0_f32), (500.0, 300.0)] {
            let size = iced::Size::new(window.0, window.1);
            let at = (120.0, 90.0);
            let texts = lay_out_texts(
                stacked(Signex::menu_overlay(
                    Message::Ui(UiMsg::Noop),
                    plain_card(&tokens),
                    at,
                )),
                size,
            );
            let first = find(&texts, "Alpha").bounds;
            let row_top = first.y + first.height / 2.0 - metrics.item_height(false) / 2.0;
            assert!(
                (row_top - (at.1 + metrics.panel_pad)).abs() < 0.6,
                "{id:?} {window:?}: first row at {row_top}, cursor at {}",
                at.1
            );
            assert!(first.x > at.0 && first.x < at.0 + 40.0, "{id:?}: {first:?}");

            // At the far corner the card stays inside that window.
            let corner = (window.0 - 5.0, window.1 - 5.0);
            let texts = lay_out_texts(
                stacked(Signex::menu_overlay(
                    Message::Ui(UiMsg::Noop),
                    plain_card(&tokens),
                    corner,
                )),
                size,
            );
            let last = find(&texts, "Charlie").bounds;
            let card_bottom =
                last.y + last.height / 2.0 + metrics.item_height(false) / 2.0 + metrics.panel_pad;
            assert!(
                (card_bottom - (window.1 - EDGE_MARGIN)).abs() < 1.0,
                "{id:?} {window:?}: card bottom {card_bottom}, height {card_h}"
            );
            assert!(
                last.x + last.width < window.0,
                "{id:?} {window:?}: {last:?}"
            );
        }
    }
}

/// The app as the maintainer ran it: Graphite, a tall window.
fn graphite_app() -> (Signex, iced::window::Id) {
    let (mut app, _boot) = Signex::new();
    app.document_state.panel_ctx.tokens = theme_tokens(ThemeId::GraphiteLight);
    let main = app
        .ui_state
        .main_window_id
        .expect("the app opens its main window");
    (app, main)
}

fn open_canvas_menu(app: &mut Signex, at: (f32, f32)) {
    app.interaction_state.context_menu = Some(ContextMenuState {
        x: at.0,
        y: at.1,
        world: None,
        window: None,
    });
}

/// A right-click in the middle of the canvas opens the menu at the cursor.
/// It used to open over the menu bar: the placement assumed 22 icon rows,
/// taller than the room below the click, so it flipped the card above the
/// cursor and clamped it to the top of the window.
#[test]
fn a_canvas_right_click_opens_the_menu_at_the_cursor() {
    let (mut app, main) = graphite_app();
    // The sheet menu follows each command (#2419) and is taller than it was:
    // the window leaves room for all of it below the click.
    let window = iced::Size::new(1400.0, 1200.0);
    let at = (600.0, 450.0);
    open_canvas_menu(&mut app, at);
    let metrics = MenuMetrics::GRAPHITE;
    let texts = lay_out_layers(app.context_menu_overlay(main), window);
    // The muted context line comes first, then Find (Graphite mockup #62).
    let first = find(
        &texts,
        &super::super::items::command_text(signex_i18n::I18n::en_us_ref(), "find"),
    )
    .bounds;
    let row_top = first.y + first.height / 2.0 - metrics.item_height(true) / 2.0;
    let expected = at.1 + metrics.panel_pad + metrics.header_row_height();
    assert!(
        (row_top - expected).abs() < 0.6,
        "first row at {row_top}, expected {expected} below the cursor at {}",
        at.1
    );
    assert!(first.x > at.0 && first.x < at.0 + 60.0, "{first:?}");
}

/// Graphite menu rows are 28 px apart, and the card is fitted between 200
/// and 320 px.
#[test]
fn the_canvas_menu_has_28_px_rows_and_a_fitted_width() {
    let (mut app, main) = graphite_app();
    open_canvas_menu(&mut app, (100.0, 100.0));
    let width = crate::layout_probe::root_size(
        app.view_context_menu(main).element,
        iced::Size::new(1400.0, 1000.0),
    )
    .width;
    assert!((200.0..=320.0).contains(&width), "{width}");
    let texts = lay_out_layers(
        app.context_menu_overlay(main),
        iced::Size::new(1400.0, 1000.0),
    );
    let y = |id: &str| {
        let b = find(
            &texts,
            &super::super::items::command_text(signex_i18n::I18n::en_us_ref(), id),
        )
        .bounds;
        b.y + b.height / 2.0
    };
    let pitch = y("smart_paste") - y("paste");
    assert!((pitch - 28.0).abs() < 0.6, "rows {pitch} px apart");
}

/// The click lands on the row that is drawn there: a press in the gap
/// right of a label, one pixel inside the row's top edge, runs that row and
/// does not close the menu. The Place launcher is live on an empty spot
/// (Graphite mockup #62); the command rows there follow the sheet's facts (#2419).
#[test]
fn a_click_anywhere_on_a_drawn_row_runs_that_row() {
    let (mut app, main) = graphite_app();
    open_canvas_menu(&mut app, (300.0, 200.0));
    let window = iced::Size::new(1400.0, 1000.0);
    let metrics = MenuMetrics::GRAPHITE;
    let texts = lay_out_layers(app.context_menu_overlay(main), window);
    let label = find(&texts, "Place").bounds;
    let gap_x = label.x + label.width + 20.0;
    let row_top = label.y + label.height / 2.0 - metrics.item_height(true) / 2.0;
    let point = iced::Point::new(gap_x, row_top + 1.0);
    let messages =
        messages_on_click_at(stacked(app.context_menu_overlay(main)), window, None, point);
    assert!(
        messages.iter().any(|m| matches!(
            m,
            Message::ContextMenu(ContextMenuMsg::SubmenuOpen(ContextSubmenu::Place))
        )),
        "a press at {point:?} sent {messages:?}"
    );
    assert!(
        !messages
            .iter()
            .any(|m| matches!(m, Message::ContextMenu(ContextMenuMsg::Close))),
        "the press fell through to the dismiss layer: {messages:?}"
    );
}

/// The open submenu's first row sits level with its launcher, right of the
/// card (#2136), while the window has room below; in a short window the
/// flyout moves up only as far as its bottom edge needs.
#[test]
fn the_place_flyout_lines_up_with_its_launcher() {
    let (mut app, main) = graphite_app();
    open_canvas_menu(&mut app, (200.0, 150.0));
    app.interaction_state.context_submenu = Some(ContextSubmenu::Place);
    let mid = |b: iced::Rectangle| b.y + b.height / 2.0;
    let metrics = MenuMetrics::GRAPHITE;

    let tall = lay_out_layers(
        app.context_menu_overlay(main),
        iced::Size::new(1400.0, 1600.0),
    );
    let launcher = find(&tall, "Place").bounds;
    // The flyout opens on its Wiring subhead (Graphite mockup #62): its top meets the
    // launcher's top.
    let head = find(&tall, "Wiring").bounds;
    let launcher_top = mid(launcher) - metrics.item_height(true) / 2.0;
    let head_top = mid(head) - metrics.header_row_height() / 2.0;
    assert!(
        (head_top - launcher_top).abs() < 0.6,
        "flyout subhead at {head:?}, launcher at {launcher:?}"
    );
    let first = find(&tall, "Wire").bounds;
    assert!(
        first.x > launcher.x + 150.0,
        "the flyout opens right of the card"
    );

    let short = lay_out_layers(
        app.context_menu_overlay(main),
        iced::Size::new(1400.0, 700.0),
    );
    let head = find(&short, "Wiring").bounds;
    let flyout_top = mid(head) - metrics.header_row_height() / 2.0 - metrics.panel_pad;
    assert!(
        flyout_top >= 0.0,
        "the flyout runs above the window: {first:?}"
    );
    assert!(
        flyout_top < mid(launcher),
        "a short window lifts the flyout to keep it inside: {first:?}"
    );
}

/// The bottom-left pixel of `layers` drawn over a white window: far from
/// every card these tests open, so only a scrim can change it.
fn corner_over_white(layers: Vec<Element<'_, Message>>, window: iced::Size) -> [u8; 3] {
    let white = iced::widget::container(iced::widget::Space::new())
        .width(iced::Length::Fill)
        .height(iced::Length::Fill)
        .style(|_: &iced::Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(iced::Color::WHITE)),
            ..iced::widget::container::Style::default()
        });
    let mut stack: iced::widget::Stack<Element<'_, Message>> = iced::widget::Stack::new()
        .width(iced::Length::Fill)
        .height(iced::Length::Fill)
        .push(white.boxed());
    for layer in layers {
        stack = stack.push(layer);
    }
    let pixels = crate::layout_probe::draw_pixels(stack.boxed(), window);
    let at = ((window.height as usize - 4) * window.width as usize + 4) * 4;
    [pixels[at], pixels[at + 1], pixels[at + 2]]
}

/// The window point the scrim tests press: the corner they read.
fn corner(window: iced::Size) -> iced::Point {
    iced::Point::new(4.0, window.height - 4.0)
}

/// How many menus [`app_with_menu`] opens.
const APP_MENUS: usize = 4;

/// The app with the canvas menu, a Components row menu or the schematic
/// Active Bar's Align menu open at (100, 100) in `window`, or the board
/// Active Bar's Selection Filter flyout.
fn app_with_menu(which: usize, window: iced::Size) -> (Signex, iced::window::Id) {
    use crate::library::state::{ComponentsPanelRowKey, ComponentsRowMenu};
    let (mut app, main) = graphite_app();
    app.ui_state.window_size = (window.width, window.height);
    match which {
        0 => open_canvas_menu(&mut app, (100.0, 100.0)),
        1 => {
            app.library.components_panel.row_menu = Some(ComponentsRowMenu {
                key: ComponentsPanelRowKey {
                    library_path: "lib.snxlib".into(),
                    table: "t".into(),
                    row_id: signex_library::RowId::from_uuid(uuid::Uuid::from_u128(1)),
                },
                x: 100.0,
                y: 100.0,
                window: None,
            });
        }
        2 => app.interaction_state.active_bar_menu = Some(crate::active_bar::ActiveBarMenu::Align),
        _ => app.interaction_state.pcb_bar_menu = Some(crate::app::PcbBarMenu::Filter),
    }
    (app, main)
}

/// The open menu's overlay layers, for [`app_with_menu`]'s `which`.
fn menu_layers(app: &Signex, which: usize, main: iced::window::Id) -> Vec<Element<'_, Message>> {
    match which {
        0 => app.context_menu_overlay(main),
        1 => app.components_row_context_menu_overlay(main),
        2 => app.active_bar_menu_overlay(main),
        _ => vec![app.pcb_active_bar(true)],
    }
}

/// Mockup rows 53 and 61: the schematic canvas menu, a panel row menu and
/// the schematic and board Active Bar menus all dim the window behind
/// them, in one scrim.
#[test]
fn every_menu_dims_the_window_behind_it_alike() {
    let window = iced::Size::new(1400.0, 900.0);
    let dims: Vec<[u8; 3]> = (0..APP_MENUS)
        .map(|which| {
            let (app, main) = app_with_menu(which, window);
            let layers = menu_layers(&app, which, main);
            assert!(!layers.is_empty(), "menu {which} is not open");
            corner_over_white(layers, window)
        })
        .collect();
    for (which, rgb) in dims.iter().enumerate() {
        assert!(
            rgb.iter().all(|&c| c < 200),
            "menu {which} leaves the window undimmed: {rgb:?}"
        );
    }
    assert!(
        dims.windows(2).all(|pair| pair[0] == pair[1]),
        "the menus dim the window differently: {dims:?}"
    );
}

/// A press on the scrim closes each menu with the message an outside click
/// sent before the scrim was painted.
#[test]
fn a_press_on_the_scrim_closes_each_menu() {
    let window = iced::Size::new(1400.0, 900.0);
    let closed: [fn(&Message) -> bool; APP_MENUS] = [
        |m| matches!(m, Message::ContextMenu(ContextMenuMsg::Close)),
        |m| {
            matches!(
                m,
                Message::Library(
                    crate::library::messages::LibraryMessage::ComponentsPanelRowMenuDismissed
                )
            )
        },
        |m| {
            matches!(
                m,
                Message::ActiveBar(crate::active_bar::ActiveBarMsg::CloseMenus)
            )
        },
        |m| {
            matches!(
                m,
                Message::Pcb(crate::app::PcbMsg::Bar(
                    crate::app::PcbBarMsg::MenuDismissed
                ))
            )
        },
    ];
    for (which, is_close) in closed.iter().enumerate() {
        let (app, main) = app_with_menu(which, window);
        let messages = messages_on_click_at(
            stacked(menu_layers(&app, which, main)),
            window,
            None,
            corner(window),
        );
        assert!(
            messages.iter().any(is_close),
            "menu {which}: a scrim press sent {messages:?}"
        );
    }
}

/// How many menus [`in_canvas_menu`] opens.
const IN_CANVAS_MENUS: usize = 3;

/// The editors the in-canvas menus open over: a footprint and a symbol with
/// their Active Bar's Selection Filter open, and a footprint sketch with a
/// dimension's menu open at (100, 100).
struct Editors {
    footprint: crate::app::FootprintDocument,
    symbol: crate::app::SymbolDocument,
    sketch: crate::app::FootprintDocument,
}

fn editors() -> Editors {
    use crate::library::editor::footprint::{CanvasMenu, FpActiveBarMenu, MenuTarget};
    use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
    use signex_sketch::id::{ConstraintId, SketchEntityId};
    let footprint_doc = |name: &str| {
        let path = std::path::PathBuf::from(format!("{name}.snxfpt"));
        let footprint = signex_library::Footprint::empty(name);
        let file = signex_library::FootprintFile::from_footprint(footprint);
        crate::app::FootprintDocument::new(path, file)
    };
    let mut footprint = footprint_doc("menu");
    footprint.state.active_bar_menu = Some(FpActiveBarMenu::Filter);
    let file = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("menu"));
    let mut symbol = crate::app::SymbolDocument::new("menu.snxsym".into(), file);
    symbol.active_bar_menu = Some(crate::library::editor::symbol::state::SymActiveBarMenu::Filter);
    let mut sketch = footprint_doc("sketch");
    let id = ConstraintId::new();
    let kind = ConstraintKind::DistancePtPt {
        p1: SketchEntityId::new(),
        p2: SketchEntityId::new(),
        target: DimTarget::Literal(1.0),
    };
    sketch.primitive_mut().sketch = Some(signex_sketch::SketchData {
        constraints: vec![Constraint { id, kind }],
        ..signex_sketch::SketchData::default()
    });
    sketch.state.sketch_overlay.menu = Some(CanvasMenu {
        target: MenuTarget::Constraint(id),
        x: 100.0,
        y: 100.0,
    });
    Editors {
        footprint,
        symbol,
        sketch,
    }
}

/// The footprint editor's or the symbol editor's Active Bar dropdown, or
/// the sketch constraint menu, laid over a canvas of `canvas`.
fn in_canvas_menu<'a>(
    editors: &'a Editors,
    tokens: &'a signex_types::theme::ThemeTokens,
    which: usize,
    canvas: iced::Size,
) -> Element<'a, Message> {
    use crate::library::editor::footprint::unified_active_bar::{BarSpan, dropdown_overlay};
    let theme = ThemeId::GraphiteLight;
    let i18n = signex_i18n::I18n::en_us_ref();
    let span = BarSpan {
        x: 0.0,
        width: canvas.width,
        window_width: canvas.width,
    };
    let overlay = match which {
        0 => dropdown_overlay(&editors.footprint, theme, tokens, &[], 40, span, i18n),
        1 => crate::library::editor::symbol::active_bar::dropdown_overlay(
            &editors.symbol,
            theme,
            tokens,
            40,
            i18n,
        ),
        _ => Some(
            crate::library::editor::footprint::sketch_mode::constraint_menu::overlay(
                &editors.sketch,
                tokens,
                i18n,
                "Inter",
            ),
        ),
    };
    overlay
        .unwrap_or_else(|| panic!("in-canvas menu {which} is not open"))
        .map(Message::Library)
        .boxed()
}

/// Mockup rows 53 and 61: the footprint and symbol editors' Active Bar
/// dropdowns and the sketch constraint menu dim the canvas under them in
/// the scrim every other menu draws.
#[test]
fn every_in_canvas_menu_dims_the_canvas_under_it_alike() {
    let canvas = iced::Size::new(1400.0, 900.0);
    let (app, main) = app_with_menu(0, canvas);
    let shared = corner_over_white(menu_layers(&app, 0, main), canvas);
    let editors = editors();
    let tokens = theme_tokens(ThemeId::GraphiteLight);
    for which in 0..IN_CANVAS_MENUS {
        let rgb = corner_over_white(
            vec![in_canvas_menu(&editors, &tokens, which, canvas)],
            canvas,
        );
        assert!(
            rgb.iter().all(|&c| c < 200),
            "in-canvas menu {which} leaves the canvas undimmed: {rgb:?}"
        );
        assert_eq!(rgb, shared, "in-canvas menu {which} dims differently");
    }
}

/// A press on the scrim closes each in-canvas menu with its own close
/// message.
#[test]
fn a_press_on_the_scrim_closes_each_in_canvas_menu() {
    use crate::library::editor::footprint::sketch_mode::SketchCanvasMsg;
    use crate::library::messages::{
        FootprintEditorMsg as Fp, LibraryMessage, PrimitiveEdit, SymbolEditorMsg as Sym,
    };
    let canvas = iced::Size::new(1400.0, 900.0);
    let closed: [fn(&PrimitiveEdit) -> bool; IN_CANVAS_MENUS] = [
        |m| matches!(m, PrimitiveEdit::Footprint(Fp::CloseActiveBarMenu)),
        |m| matches!(m, PrimitiveEdit::Symbol(Sym::CloseActiveBarMenu)),
        |m| {
            matches!(
                m,
                PrimitiveEdit::Footprint(Fp::SketchCanvas(SketchCanvasMsg::MenuClosed))
            )
        },
    ];
    let editors = editors();
    let tokens = theme_tokens(ThemeId::GraphiteLight);
    for (which, is_close) in closed.iter().enumerate() {
        let layer = in_canvas_menu(&editors, &tokens, which, canvas);
        let messages = messages_on_click_at(stacked(vec![layer]), canvas, None, corner(canvas));
        let closes = |m: &Message| match m {
            Message::Library(LibraryMessage::PrimitiveEditorEvent { msg, .. }) => is_close(msg),
            _ => false,
        };
        assert!(
            messages.iter().any(closes),
            "in-canvas menu {which}: a scrim press sent {messages:?}"
        );
    }
}

/// The row height the placement assumes is the one the dropdown really lays
/// out, with and without icons, in both looks; a submenu launcher is exactly
/// one item row tall (#2136).
#[test]
fn rows_and_launchers_lay_out_at_the_metrics_row_height() {
    let icon = iced::widget::svg::Handle::from_memory(
        br#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"/>"#.as_slice(),
    );
    for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        let tokens = theme_tokens(id);
        let metrics = MenuMetrics::GRAPHITE;
        for has_icons in [false, true] {
            let with_icon = |item: DropdownItem<Message>| {
                DropdownEntry::Item(if has_icons {
                    item.icon(icon.clone())
                } else {
                    item
                })
            };
            let entries = vec![
                with_icon(DropdownItem::new("Alpha", Message::Ui(UiMsg::Noop))),
                super::super::items::submenu_launcher(
                    has_icons.then(|| icon.clone()),
                    "Launcher",
                    ContextSubmenu::Align,
                    false,
                ),
                with_icon(DropdownItem::new("Charlie", Message::Ui(UiMsg::Noop))),
            ];
            let menu = signex_widgets::active_bar_dropdown::view(entries, &tokens, Some(240.0));
            let texts = lay_out_texts(menu, iced::Size::new(400.0, 400.0));
            let mid = |label: &str| {
                let b = find(&texts, label).bounds;
                b.y + b.height / 2.0
            };
            let row = metrics.item_height(has_icons);
            for (a, b) in [("Alpha", "Launcher"), ("Launcher", "Charlie")] {
                let pitch = mid(b) - mid(a);
                assert!(
                    (pitch - row).abs() < 0.6,
                    "{id:?} icons {has_icons}: {a} to {b} is {pitch}, rows are {row}"
                );
            }
        }
    }
}
