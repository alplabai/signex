//! The unit menu of a measurement slot (mockup row 139): the slot's right
//! segment, opening a list of unit glyphs with their names as muted
//! captions. The current entry is an accent row with a check mark.

use iced::widget::{Column, Space, container, row, scrollable};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget as _};
use iced_aw::DropDown;
use iced_aw::drop_down::Alignment as DropAlignment;
use signex_i18n::{I18n, MessageId, UserText, ids, t};
use signex_types::quantity::Unit;
use signex_types::theme::ThemeTokens;
use signex_widgets::chevron::chevron;
use signex_widgets::press_outside::PressOutside;
use signex_widgets::{button, text, theme_ext};

use super::unit_picker::{UnitChoice, unit_choices};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// Width of the open list, in em of the body size.
const LIST_WIDTH_EM: f32 = 11.5;

/// The muted name of a menu entry. `None` for a unit whose glyph already
/// reads as its name (`%`, `ppm/°C`, `V/µs`, `AWG`).
pub(crate) fn unit_caption(choice: Option<&Unit>) -> Option<MessageId> {
    match choice {
        None => Some(ids::LIBRARY_UNIT_NAME_NONE),
        Some(Unit::Ohm) => Some(ids::LIBRARY_UNIT_NAME_OHM),
        Some(Unit::Farad) => Some(ids::LIBRARY_UNIT_NAME_FARAD),
        Some(Unit::Henry) => Some(ids::LIBRARY_UNIT_NAME_HENRY),
        Some(Unit::Volt) => Some(ids::LIBRARY_UNIT_NAME_VOLT),
        Some(Unit::Ampere) => Some(ids::LIBRARY_UNIT_NAME_AMPERE),
        Some(Unit::Watt) => Some(ids::LIBRARY_UNIT_NAME_WATT),
        Some(Unit::Hertz) => Some(ids::LIBRARY_UNIT_NAME_HERTZ),
        Some(_) => None,
    }
}

/// What the menu shows, and its open and dismiss events. Esc is not wired
/// here: the app's Esc ladder closes the menu, so the same key cannot also
/// cancel the dialog behind it.
pub(crate) struct UnitMenu<M> {
    pub selected: Option<Unit>,
    pub is_open: bool,
    pub on_open: M,
    pub on_dismiss: M,
}

/// The menu. `on_pick` gets the unit's canonical token, or an empty string
/// for "no unit".
pub(crate) fn unit_menu<'a, M: Clone + 'a>(
    menu: UnitMenu<M>,
    on_pick: impl Fn(String) -> M,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, M> {
    let current = UnitChoice(menu.selected);
    let face = face(&current, menu.is_open, menu.on_open, tokens);
    let list = list(&current, on_pick, tokens, i18n);
    // A press anywhere off the list closes it, the face included: the face
    // sends nothing while open, so that press cannot reopen it on release.
    let list = PressOutside::new(list, Some(menu.on_dismiss)).boxed();
    DropDown::new(face, list, menu.is_open)
        .width(Length::Shrink)
        .alignment(DropAlignment::Bottom)
        .opens_above_when_short(true)
        .offset(SP.xs)
        .boxed()
}

fn mono() -> iced::Font {
    crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
}

/// The closed control: the glyph and a down chevron, with the accent ring
/// while open. The slot draws the border otherwise.
fn face<'a, M: Clone + 'a>(
    current: &UnitChoice,
    is_open: bool,
    on_open: M,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    let r = crate::styles::tokens::resolve(tokens);
    let (ink, ink3, accent) = (
        theme_ext::text_primary(tokens),
        r.ink3,
        theme_ext::accent(tokens),
    );
    let radius = crate::styles::Radii::GRAPHITE.control;
    let label = row![
        text(UserText::new(current.to_string()))
            .size(TS.body)
            .font(mono())
            .color(ink),
        Space::new().width(Length::Fill),
        chevron(true, TS.body, ink3),
    ]
    .align_y(Alignment::Center);
    button(label)
        .width(Length::Fill)
        .padding([SP.xs + SP.xs, SP.s + SP.xs])
        .on_press_maybe((!is_open).then_some(on_open))
        .style(move |_: &Theme, _| iced::widget::button::Style {
            background: None,
            text_color: ink,
            border: Border {
                width: if is_open { 1.0 } else { 0.0 },
                color: accent,
                radius: iced::border::right(radius),
                ..Border::default()
            },
            ..iced::widget::button::Style::default()
        })
        .boxed()
}

/// The open list under the "Unit" heading.
fn list<'a, M: Clone + 'a>(
    current: &UnitChoice,
    on_pick: impl Fn(String) -> M,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, M> {
    let ink3 = crate::styles::tokens::resolve(tokens).ink3;
    let heading = container(
        text(t!(i18n, ids::LIBRARY_EDIT_ROW_UNIT))
            .size(TS.caption)
            .color(ink3),
    )
    .padding([SP.xs, SP.s + SP.xs])
    .boxed();
    let rows = unit_choices(true).into_iter().map(|choice| {
        let token = choice.0.as_ref().map_or("", Unit::token).to_string();
        let is_current = choice == *current;
        entry(&choice, is_current, on_pick(token), tokens, i18n)
    });
    let body = Column::new().push(heading).extend(rows).width(Length::Fill);
    // Scrolls when neither side of the row has room for the whole list.
    let body = scrollable(body)
        .direction(crate::styles::scrollbar::thin_vertical())
        .style(crate::styles::scrollbar::scrollbar_style(tokens));
    container(body)
        .width(TS.body * LIST_WIDTH_EM)
        .padding(SP.xs)
        .style(crate::styles::context_menu(tokens))
        .boxed()
}

/// One entry: glyph on the left; muted name and the current mark right.
fn entry<'a, M: Clone + 'a>(
    choice: &UnitChoice,
    is_current: bool,
    on_press: M,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, M> {
    let r = crate::styles::tokens::resolve(tokens);
    let ink = if is_current {
        r.accent_soft_ink
    } else {
        theme_ext::text_primary(tokens)
    };
    let mut line = row![
        text(UserText::new(choice.to_string()))
            .size(TS.body)
            .font(mono())
            .color(ink),
        Space::new().width(Length::Fill),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center);
    if let Some(id) = unit_caption(choice.0.as_ref()) {
        line = line.push(text(t!(i18n, id)).size(TS.caption).color(r.ink3).boxed());
    }
    if is_current {
        // A mark, not language.
        let mark = "\u{2713}";
        line = line.push(
            text(UserText::new(mark))
                .size(TS.caption)
                .color(ink)
                .boxed(),
        );
    }
    let soft = r.accent_soft;
    let idle = crate::styles::menu_item(tokens);
    button(line)
        .width(Length::Fill)
        .padding([SP.xs, SP.s + SP.xs])
        .on_press(on_press)
        .style(move |theme: &Theme, status| {
            let mut style = idle(theme, status);
            if is_current {
                style.background = Some(Background::Color(soft));
            }
            style.border.radius = crate::styles::Radii::GRAPHITE.control.into();
            style
        })
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::advanced::shell::{Bus, Waker};
    use iced::{Point, Size, mouse};
    use iced_runtime::UserInterface;
    use iced_runtime::user_interface::Cache;

    /// Mockup row 139: the units with a name carry it as a caption, "—"
    /// reads "no unit", and a glyph that is its own name has none.
    #[test]
    fn the_menu_captions_name_the_units() {
        let i18n = I18n::en_us();
        let name = |u: Option<Unit>| unit_caption(u.as_ref()).map(|id| t!(i18n, id).to_string());
        assert_eq!(name(None).as_deref(), Some("no unit"));
        assert_eq!(name(Some(Unit::Ohm)).as_deref(), Some("ohm"));
        assert_eq!(name(Some(Unit::Farad)).as_deref(), Some("farad"));
        assert_eq!(name(Some(Unit::Henry)).as_deref(), Some("henry"));
        assert_eq!(name(Some(Unit::Volt)).as_deref(), Some("volt"));
        assert_eq!(name(Some(Unit::Ampere)).as_deref(), Some("ampere"));
        assert_eq!(name(Some(Unit::Watt)).as_deref(), Some("watt"));
        assert_eq!(name(Some(Unit::Percent)), None);
        assert_eq!(name(Some(Unit::PpmPerKelvin)), None);
    }

    #[derive(Debug, Clone, PartialEq)]
    enum Hit {
        Opened,
        Dismissed,
        Picked(String),
    }

    const WINDOW: Size = Size::new(400.0, 480.0);

    /// What a click at height `y` publishes with the open menu's face at
    /// the top of the window, or at the bottom when `is_low`.
    fn clicked(is_low: bool, y: f32) -> Vec<Hit> {
        let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteLight);
        let i18n = I18n::en_us();
        let menu = UnitMenu {
            selected: Some(Unit::Ohm),
            is_open: true,
            on_open: Hit::Opened,
            on_dismiss: Hit::Dismissed,
        };
        let menu = unit_menu(menu, Hit::Picked, &tokens, &i18n);
        let gap = Space::new().height(Length::Fill).boxed();
        let page: Element<'_, Hit> = if is_low {
            Column::new().push(gap).push(menu).boxed()
        } else {
            Column::new().push(menu).push(gap).boxed()
        };
        let mut renderer = crate::layout_probe::headless_renderer();
        let mut ui = UserInterface::build(page, WINDOW, Cache::default(), &mut renderer);
        let mut bus = Bus::new();
        let at = Point::new(WINDOW.width / 2.0, y);
        let events = [
            mouse::Event::CursorMoved { position: at },
            mouse::Event::ButtonPressed(mouse::Button::Left),
            mouse::Event::ButtonReleased(mouse::Button::Left),
        ]
        .map(iced::Event::Mouse);
        for event in events {
            let _ = ui.update(
                &iced::window::Headless,
                &Waker::noop(),
                std::slice::from_ref(&event),
                mouse::Cursor::Available(at),
                &mut renderer,
                &mut bus,
            );
        }
        bus.drain().map(|(hit, _)| hit).collect()
    }

    fn is_pick(hits: &[Hit]) -> bool {
        matches!(hits, [Hit::Picked(_)])
    }

    /// On a low row the list opens above its face at full height, where the
    /// room below would leave a sliver.
    #[test]
    fn the_menu_of_a_low_row_opens_above_it() {
        assert!(is_pick(&clicked(true, WINDOW.height - 80.0)));
        assert!(is_pick(&clicked(true, WINDOW.height - 160.0)));
    }

    #[test]
    fn the_menu_of_a_high_row_still_opens_below_it() {
        assert!(is_pick(&clicked(false, 80.0)));
        assert_eq!(clicked(false, WINDOW.height - 10.0), vec![Hit::Dismissed]);
    }
}
