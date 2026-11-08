//! An open menu-bar dropdown covers what is drawn under it. The vendored
//! iced_aw drew the first dropdown into the window's last layer, where text,
//! icons and canvas meshes draw after quads, so the dock's labels showed
//! through the menu (see `vendor/README.md`).

use iced::widget::{column, container, text};
use iced::{Color, Element, Length, Point, Size, Theme, Widget as _, mouse, window};
use signex_types::theme::{ThemeId, theme_tokens};

use super::{MenuContext, MenuMessage};

const WINDOW: Size = Size::new(900.0, 600.0);
const INK: Color = Color::from_rgb(1.0, 0.0, 0.0);

/// The menu bar over a block of red text, where a dock's labels would be.
fn scene(tokens: &signex_types::theme::ThemeTokens) -> Element<'static, MenuMessage> {
    let block = (0..8).fold(column![].spacing(0), |col, _| {
        col.push(
            text("M".repeat(40))
                .size(crate::styles::TypeScale::DEFAULT.glyph)
                .color(INK)
                .boxed(),
        )
    });
    column![
        super::view(tokens, "Inter", MenuContext::default()),
        container(block).width(Length::Fill).height(Length::Fill),
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .boxed()
}

/// Red pixels of the RGBA `pixels` inside `area`.
fn red_in(pixels: &[u8], area: iced::Rectangle) -> usize {
    let w = WINDOW.width as usize;
    let mut n = 0;
    for y in area.y as usize..(area.y + area.height) as usize {
        for x in area.x as usize..(area.x + area.width) as usize {
            let i = (y * w + x) * 4;
            let (r, g, b) = (pixels[i], pixels[i + 1], pixels[i + 2]);
            if r > 200 && g < 80 && b < 80 {
                n += 1;
            }
        }
    }
    n
}

/// Draw the scene, after clicking `click` when given, as RGBA pixels.
fn draw(click: Option<Point>) -> Vec<u8> {
    use iced::advanced::renderer::Headless as _;
    let tokens = theme_tokens(ThemeId::GraphiteLight);
    let mut renderer = crate::layout_probe::headless_renderer();
    let mut ui = iced_runtime::UserInterface::build(
        scene(&tokens),
        WINDOW,
        iced_runtime::user_interface::Cache::default(),
        &mut renderer,
    );
    let mut published = iced::advanced::shell::Bus::<MenuMessage>::new();
    let cursor = click.map_or(mouse::Cursor::Unavailable, mouse::Cursor::Available);
    let mut events = Vec::new();
    if let Some(at) = click {
        events.extend([
            iced::Event::Mouse(mouse::Event::CursorMoved { position: at }),
            iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
        ]);
    }
    let _ = ui.update(
        &window::Headless,
        &iced::advanced::shell::Waker::noop(),
        &events,
        cursor,
        &mut renderer,
        &mut published,
    );
    let mut ui =
        iced_runtime::UserInterface::build(scene(&tokens), WINDOW, ui.into_cache(), &mut renderer);
    let redraw = iced::Event::Window(window::Event::RedrawRequested(std::time::Instant::now()));
    let _ = ui.update(
        &window::Headless,
        &iced::advanced::shell::Waker::noop(),
        &[redraw],
        cursor,
        &mut renderer,
        &mut published,
    );
    ui.draw(
        &mut renderer,
        &Theme::Light,
        &iced::advanced::renderer::Style {
            text_color: Color::BLACK,
        },
        cursor,
    );
    let iced::Renderer::Secondary(software) = &mut renderer else {
        panic!("the probe renderer is the software one");
    };
    software.screenshot(
        Size::new(WINDOW.width as u32, WINDOW.height as u32),
        1.0,
        Color::WHITE,
    )
}

#[test]
fn an_open_dropdown_hides_the_text_drawn_under_it() {
    let tokens = theme_tokens(ThemeId::GraphiteLight);
    let texts = crate::layout_probe::lay_out_texts(scene(&tokens), WINDOW);
    let file = crate::layout_probe::find(&texts, "File").bounds;
    // A patch inside the File dropdown's rows, under its first few rows.
    let bar = super::MENU_BAR_HEIGHT;
    let inside = iced::Rectangle::new(
        Point::new(file.x + 20.0, bar + 30.0),
        Size::new(120.0, 60.0),
    );
    assert!(
        red_in(&draw(None), inside) > 100,
        "the closed menu shows the red block there (the probe sees it)"
    );
    let open = draw(Some(file.center()));
    assert_eq!(
        red_in(&open, inside),
        0,
        "the red text under the open dropdown shows through it"
    );
}
