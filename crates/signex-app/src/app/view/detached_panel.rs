//! A dock panel in its own OS window (mockup row 8). The window is borderless where the
//! dialog windows are: the Graphite title strip (panel name, the main
//! window's active document in `ink4`, the panel's own actions, Dock back and
//! the close) moves it, and its edges resize it. The
//! body is the panel itself, following the main window's active document.
//! Dock back and the close both close the window, which re-docks the panel
//! in its home region (`WindowMsg::SecondaryWindowClosed`).

use super::*;
use crate::panels::PanelKind;
use crate::styles::{Radii, chrome_px, ti, tokens};
use iced::widget::text::{Ellipsis, Wrapping};
use iced::widget::{column, container, mouse_area, row, scrollable, stack, svg};
use iced::{Background, Border, Color};
use signex_types::theme::{ThemeId, ThemeTokens};
use signex_widgets::{button, text, tooltip};

/// Height of the title strip (32 px at the default base).
pub(super) const STRIP_H: f32 = chrome_px(32.0 * 11.0 / 12.0);
/// Panel name: 13 px at the default base, weight 500, as in a dock title.
const NAME_SIZE: f32 = chrome_px(13.0 * 11.0 / 12.0);
/// Side of the Dock back icon button and of its glyph.
const ICON_BUTTON: f32 = chrome_px(24.0 * 11.0 / 12.0);
const ICON: f32 = chrome_px(14.0 * 11.0 / 12.0);
/// Width of the close button.
const CLOSE_W: f32 = chrome_px(40.0 * 11.0 / 12.0);

/// What the title strip shows and sends.
pub(super) struct StripFacts {
    pub(super) window: iced::window::Id,
    pub(super) panel_name: signex_i18n::Localized,
    pub(super) document: Option<String>,
    /// The panel's title-bar actions (Refresh, Create Union, Recompile, ...).
    pub(super) actions: Vec<Element<'static, Message>>,
    pub(super) dock_back_tip: signex_i18n::Localized,
    pub(super) theme: ThemeId,
}

/// The Dock back button: no fill at rest, `hover` while hovered or pressed.
fn icon_button_style(hover: Color) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |_, status| button::Style {
        background: matches!(status, button::Status::Hovered | button::Status::Pressed)
            .then_some(Background::Color(hover)),
        border: Border {
            radius: Radii::GRAPHITE.small.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

/// A strip glyph in `ink`.
fn glyph(handle: svg::Handle, ink: Color) -> Element<'static, Message> {
    svg(handle)
        .width(ICON)
        .height(ICON)
        .style(move |_: &iced::Theme, _| svg::Style {
            color: Some(ink),
            ..Default::default()
        })
        .boxed()
}

fn centred(content: Element<'static, Message>) -> Element<'static, Message> {
    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .boxed()
}

/// The Graphite title strip of a detached panel window: the `chrome` band
/// with a bottom hairline, the name, the document, Dock back, the close.
pub(super) fn title_strip(
    facts: StripFacts,
    theme_tokens: &ThemeTokens,
) -> Element<'static, Message> {
    let r = tokens::resolve(theme_tokens);
    let (ink, ink2) = (ti(theme_tokens.text), ti(theme_tokens.text_secondary));
    let (hover, close_hover) = (ti(theme_tokens.hover), ti(theme_tokens.error));
    let window = facts.window;
    let mut line = row![
        text(facts.panel_name)
            .size(NAME_SIZE)
            .color(ink)
            .font(iced::Font {
                weight: iced::font::Weight::Medium,
                ..signex_widgets::ui_font::regular()
            })
            .wrapping(Wrapping::None)
            .boxed(),
    ]
    .spacing(SP.m)
    .align_y(iced::Alignment::Center);
    let document = facts.document.unwrap_or_default();
    line = line.push(
        text(signex_i18n::UserText::new(document))
            .size(TS.caption)
            .color(r.ink4)
            .wrapping(Wrapping::None)
            .ellipsis(Ellipsis::End)
            .width(Length::Fill)
            .boxed(),
    );
    for action in facts.actions {
        line = line.push(action);
    }
    let dock_back = button(centred(glyph(
        crate::icons::icon_chrome_dock_back(facts.theme),
        ink2,
    )))
    .width(ICON_BUTTON)
    .height(ICON_BUTTON)
    .padding(0)
    .on_press(Message::Window(WindowMsg::DetachedPanelDockBackPressed(
        window,
    )))
    .style(icon_button_style(hover))
    .boxed();
    line = line.push(
        tooltip(
            dock_back,
            container(text(facts.dock_back_tip).size(TS.body)).padding([SP.xs, SP.m]),
            tooltip::Position::Bottom,
        )
        .style(crate::styles::context_menu(theme_tokens))
        .boxed(),
    );
    let close = button(centred(glyph(
        crate::icons::icon_chrome_window_close(facts.theme),
        ink2,
    )))
    .width(CLOSE_W)
    .height(Length::Fill)
    .padding(0)
    .on_press(Message::Window(WindowMsg::DialogCloseClicked(window)))
    .style(move |_: &iced::Theme, status| button::Style {
        background: matches!(status, button::Status::Hovered | button::Status::Pressed)
            .then_some(Background::Color(close_hover)),
        ..button::Style::default()
    });
    line = line.push(close.boxed());
    let (band, rule) = (ti(theme_tokens.toolbar_bg), ti(theme_tokens.border));
    let strip = container(line)
        .padding(iced::Padding {
            left: SP.l,
            ..iced::Padding::ZERO
        })
        .width(Length::Fill)
        .height(STRIP_H)
        .align_y(iced::alignment::Vertical::Center)
        .style(move |_: &iced::Theme| container::Style {
            background: Some(Background::Color(band)),
            border: Border {
                bottom: iced::border::Side::default().color(rule).width(1.0),
                ..Border::default()
            },
            ..container::Style::default()
        });
    // The buttons take their own presses; the rest of the strip moves the
    // window.
    mouse_area(strip)
        .on_press(Message::Window(WindowMsg::DialogTitleBarPressed(window)))
        .boxed()
}

/// The panel's actions in a window the OS decorates: there is no title strip
/// to hold them, so they get a right-aligned row over the body.
fn native_action_row(
    actions: Vec<Element<'static, Message>>,
    background: Color,
) -> Element<'static, Message> {
    let mut line: iced::widget::Row<Element<'static, Message>> =
        iced::widget::Row::new().align_y(iced::Alignment::Center);
    for action in actions {
        line = line.push(action);
    }
    container(line)
        .padding(SP.xs)
        .width(Length::Fill)
        .align_x(iced::alignment::Horizontal::Right)
        .style(move |_: &iced::Theme| container::Style {
            background: Some(Background::Color(background)),
            ..container::Style::default()
        })
        .boxed()
}

impl Signex {
    /// The whole detached panel window `window` showing `kind`: the title
    /// strip over the panel body, inside the resize edges.
    pub(super) fn view_detached_panel_window(
        &self,
        window: iced::window::Id,
        kind: PanelKind,
    ) -> Element<'_, Message> {
        let ctx = &self.document_state.panel_ctx;
        let i18n = &self.ui_state.i18n;
        let panel = match kind {
            PanelKind::Components => crate::panels::components_panel::view(
                &self.library,
                ctx,
                &ctx.tokens,
                self.sheet_grid(),
            )
            .map(crate::dock::DockMessage::Library)
            .boxed(),
            // The same surface as the docked and floating Library panel.
            PanelKind::Library | PanelKind::SchLibrary | PanelKind::FootprintLibrary => {
                crate::library::panel::view(&self.library, ctx)
            }
            other => crate::panels::view_panel(other, ctx)
                .map(crate::dock::DockMessage::Panel)
                .boxed(),
        }
        .map(Message::Dock);
        // Sent like the body's messages: a detached panel acts on the main
        // window's document.
        let actions: Vec<Element<'static, Message>> = crate::dock::title_actions(kind, ctx)
            .into_iter()
            .map(|action| action.map(Message::Dock))
            .collect();
        let document = &self.document_state;
        let borderless = self.dialog_window_is_borderless(window);
        let (strip_actions, native_actions) = if borderless {
            (actions, Vec::new())
        } else {
            (Vec::new(), actions)
        };
        let facts = StripFacts {
            window,
            panel_name: signex_i18n::t!(i18n, kind.message()),
            document: document
                .tabs
                .get(document.active_tab)
                .map(|tab| tab.title.clone()),
            actions: strip_actions,
            dock_back_tip: signex_i18n::t!(i18n, signex_i18n::menu_ids::CTX_DOCK_BACK),
            theme: self.ui_state.effective_theme_id(),
        };
        let panel_bg = ti(ctx.tokens.panel_bg);
        let body = container(scrollable(panel))
            .padding(SP.m)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_: &iced::Theme| container::Style {
                background: Some(Background::Color(panel_bg)),
                ..container::Style::default()
            });
        // A window the OS decorates (native macOS controls) keeps its own
        // title bar, as the dialog windows do.
        if !borderless {
            if native_actions.is_empty() {
                return body.boxed();
            }
            return column![native_action_row(native_actions, panel_bg), body]
                .width(Length::Fill)
                .height(Length::Fill)
                .boxed();
        }
        stack![
            column![title_strip(facts, &ctx.tokens), body]
                .width(Length::Fill)
                .height(Length::Fill),
            Self::resize_edges_overlay_with(move |direction| {
                Message::Window(WindowMsg::DialogEdgePressed(window, direction))
            }),
        ]
        .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(document: Option<&str>) -> StripFacts {
        let en = signex_i18n::I18n::en_us();
        StripFacts {
            window: iced::window::Id::unique(),
            panel_name: signex_i18n::t!(en, PanelKind::Projects.message()),
            document: document.map(str::to_owned),
            actions: Vec::new(),
            dock_back_tip: signex_i18n::t!(en, signex_i18n::menu_ids::CTX_DOCK_BACK),
            theme: ThemeId::GraphiteLight,
        }
    }

    /// #2764: a detached panel keeps its header actions, in the borderless
    /// strip and in a window the OS decorates, and a click reaches the panel.
    #[test]
    fn a_detached_panel_window_keeps_its_header_actions() {
        use crate::app::state::WindowKind;
        use crate::dock::DockMessage;
        use crate::panels::{MessagesPanelMsg, PanelMsg};
        const WINDOW: iced::Size = iced::Size::new(600.0, 400.0);
        let (mut app, _boot) = Signex::new();
        let id = iced::window::Id::unique();
        app.ui_state
            .windows
            .insert(id, WindowKind::DetachedPanel(PanelKind::Messages));
        let clears = |messages: Vec<Message>| {
            messages.iter().any(|m| {
                matches!(
                    m,
                    Message::Dock(DockMessage::Panel(PanelMsg::MessagesView(
                        MessagesPanelMsg::Cleared
                    )))
                )
            })
        };

        // Clear is the last action, just inside the row's right edge.
        let native = iced::Point::new(WINDOW.width - SP.xs - 4.0, SP.xs + 4.0);
        let sent = crate::layout_probe::messages_on_click_at(app.view(id), WINDOW, None, native);
        assert!(clears(sent), "no Clear in the OS-decorated window");

        app.ui_state.borderless_dialog_windows.insert(id);
        let right = WINDOW.width - CLOSE_W - SP.m - ICON_BUTTON - SP.m;
        let strip = iced::Point::new(right - 4.0, STRIP_H / 2.0);
        let sent = crate::layout_probe::messages_on_click_at(app.view(id), WINDOW, None, strip);
        assert!(clears(sent), "no Clear in the title strip");
    }

    /// The strip names the panel and the active document, in Graphite
    /// Light and Dark, and is exactly the strip height.
    #[test]
    fn the_strip_names_the_panel_and_the_document() {
        use iced::advanced::Widget as _;
        for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
            let t = signex_types::theme::theme_tokens(id);
            let strip = title_strip(facts(Some("e1m-carrier.snxpcb")), &t);
            assert_eq!(strip.size().height, Length::Fixed(STRIP_H));
            let texts = crate::layout_probe::lay_out_texts(
                title_strip(facts(Some("e1m-carrier.snxpcb")), &t),
                iced::Size::new(300.0, STRIP_H),
            );
            for want in ["Projects", "e1m-carrier.snxpcb"] {
                assert!(
                    texts.iter().any(|x| x.text == want),
                    "{want} missing: {texts:#?}"
                );
            }
        }
        let _ = title_strip(
            facts(None),
            &signex_types::theme::theme_tokens(ThemeId::GraphiteDark),
        );
    }

    /// Dock back closes the panel's window; once the OS reports it gone the
    /// panel is docked again, in its home region, as after the close.
    #[test]
    fn dock_back_closes_the_window_and_the_panel_docks_again() {
        use crate::app::state::WindowKind;
        let (mut app, _boot) = Signex::new();
        let kind = PanelKind::Projects;
        app.document_state.dock.close_panel(kind);
        assert!(app.document_state.dock.locate(kind).is_none());
        let id = iced::window::Id::unique();
        app.ui_state
            .windows
            .insert(id, WindowKind::DetachedPanel(kind));

        let close = app.update(Message::Window(WindowMsg::DetachedPanelDockBackPressed(id)));
        assert_ne!(close.units(), 0, "Dock back closes the window");
        let _ = app.update(Message::Window(WindowMsg::SecondaryWindowClosed(id)));
        assert!(!app.ui_state.windows.contains_key(&id));
        assert!(app.document_state.dock.locate(kind).is_some(), "re-docked");

        // Dock back for a window that is not a detached panel does nothing.
        let other = iced::window::Id::unique();
        let none = app.update(Message::Window(WindowMsg::DetachedPanelDockBackPressed(
            other,
        )));
        assert_eq!(none.units(), 0);
    }

    #[test]
    fn the_dock_back_tooltip_is_from_the_catalog() {
        let en = signex_i18n::I18n::en_us();
        assert_eq!(
            signex_i18n::t!(en, signex_i18n::menu_ids::CTX_DOCK_BACK).into_string(),
            "Dock back"
        );
    }
}
