//! The unit menu of a measurement slot (mockup row 139): the slot's right
//! segment, opening a list of unit glyphs with their names as muted
//! captions, under the unit's prefixes when it takes them. A current entry
//! is an accent row with a check mark.

use iced::widget::{Column, Space, container, row, scrollable};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget as _};
use iced_aw::DropDown;
use iced_aw::drop_down::Alignment as DropAlignment;
use signex_i18n::{I18n, MessageId, UserText, ids, t};
use signex_types::quantity::{SiPrefix, Unit};
use signex_types::theme::ThemeTokens;
use signex_widgets::chevron::chevron;
use signex_widgets::press_outside::PressOutside;
use signex_widgets::{button, text, theme_ext};

use super::prefix_picker::{effective_prefix, prefix_choices, prefixed_glyph};
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
    let current = UnitChoice(menu.selected.clone());
    let label = current.to_string();
    let body = Column::new().extend(unit_rows(&current, &on_pick, tokens, i18n));
    dropdown(menu, label, body, tokens)
}

/// The one control of a measurement (mockup row 139). A unit that takes
/// prefixes shows them joined on the face (`kΩ`), and its list holds the
/// prefixes, then the units. Any other unit gets the plain [`unit_menu`].
pub(crate) fn measurement_menu<'a, M: Clone + 'a>(
    menu: UnitMenu<M>,
    prefix: SiPrefix,
    on_prefix: impl Fn(SiPrefix) -> M,
    on_pick: impl Fn(String) -> M,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, M> {
    let Some(unit) = menu.selected.clone().filter(Unit::takes_prefix) else {
        return unit_menu(menu, on_pick, tokens, i18n);
    };
    let prefix = effective_prefix(&unit, prefix);
    let body = prefix_and_unit_rows(&unit, prefix, &on_prefix, &on_pick, tokens, i18n);
    dropdown(menu, prefixed_glyph(prefix, &unit), body, tokens)
}

/// The face labelled `label` over the open list of `body`.
fn dropdown<'a, M: Clone + 'a>(
    menu: UnitMenu<M>,
    label: String,
    body: Column<Element<'a, M>>,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    let face = face(label, menu.is_open, menu.on_open, tokens);
    let list = list(body, tokens);
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

/// The closed control: the label and a down chevron, with the accent ring
/// while open. The slot draws the border otherwise.
fn face<'a, M: Clone + 'a>(
    label: String,
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
        text(UserText::new(label))
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

/// A muted section heading of the open list.
fn heading<'a, M: 'a>(id: MessageId, tokens: &ThemeTokens, i18n: &I18n) -> Element<'a, M> {
    let ink3 = crate::styles::tokens::resolve(tokens).ink3;
    container(text(t!(i18n, id)).size(TS.caption).color(ink3))
        .padding([SP.xs, SP.s + SP.xs])
        .boxed()
}

/// The "Unit" heading and its entries, the current one marked.
fn unit_rows<'a, M: Clone + 'a>(
    current: &UnitChoice,
    on_pick: &impl Fn(String) -> M,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Vec<Element<'a, M>> {
    let rows = unit_choices(true).into_iter().map(|choice| {
        let token = choice.0.as_ref().map_or("", Unit::token).to_string();
        let caption = unit_caption(choice.0.as_ref());
        let is_current = choice == *current;
        let look = EntryLook {
            label: choice.to_string(),
            caption,
            is_current,
        };
        entry(look, on_pick(token), tokens, i18n)
    });
    std::iter::once(heading(ids::LIBRARY_EDIT_ROW_UNIT, tokens, i18n))
        .chain(rows)
        .collect()
}

/// The "Prefix" section, then the "Unit" section, each marking its current
/// entry. A prefix reads joined to the unit, so "no prefix" is a plain `Ω`.
fn prefix_and_unit_rows<'a, M: Clone + 'a>(
    unit: &Unit,
    prefix: SiPrefix,
    on_prefix: &impl Fn(SiPrefix) -> M,
    on_pick: &impl Fn(String) -> M,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Column<Element<'a, M>> {
    let prefixes = prefix_choices(unit).into_iter().map(|choice| {
        let look = EntryLook {
            label: choice.to_string(),
            caption: None,
            is_current: choice.prefix == prefix,
        };
        entry(look, on_prefix(choice.prefix), tokens, i18n)
    });
    let current = UnitChoice(Some(unit.clone()));
    Column::new()
        .push(heading(ids::LIBRARY_EDIT_ROW_PREFIX, tokens, i18n))
        .extend(prefixes)
        .extend(unit_rows(&current, on_pick, tokens, i18n))
}

/// The open list around its sections.
fn list<'a, M: Clone + 'a>(body: Column<Element<'a, M>>, tokens: &ThemeTokens) -> Element<'a, M> {
    let body = body.width(Length::Fill);
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

/// What one list entry shows.
struct EntryLook {
    label: String,
    caption: Option<MessageId>,
    is_current: bool,
}

/// One entry: glyph on the left; muted name and the current mark right.
fn entry<'a, M: Clone + 'a>(
    look: EntryLook,
    on_press: M,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, M> {
    let EntryLook {
        label,
        caption,
        is_current,
    } = look;
    let r = crate::styles::tokens::resolve(tokens);
    let ink = if is_current {
        r.accent_soft_ink
    } else {
        theme_ext::text_primary(tokens)
    };
    let mut line = row![
        text(UserText::new(label))
            .size(TS.body)
            .font(mono())
            .color(ink),
        Space::new().width(Length::Fill),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center);
    if let Some(id) = caption {
        line = line.push(text(t!(i18n, id)).size(TS.caption).color(r.ink3).boxed());
    }
    if is_current {
        line = line.push(
            text(UserText::new("\u{2713}"))
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
        click_through_overlays(page, y)
    }

    /// What a click at height `y` publishes, open overlays included.
    fn click_through_overlays<M>(page: Element<'_, M>, y: f32) -> Vec<M> {
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

    #[derive(Debug, Clone, PartialEq)]
    enum Pick {
        Prefix(SiPrefix),
        Unit(String),
    }

    const TALL: Size = Size::new(400.0, 1200.0);

    fn kilo_ohm_list(i18n: &I18n) -> Element<'static, Pick> {
        let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteLight);
        let body = prefix_and_unit_rows(
            &Unit::Ohm,
            SiPrefix::Kilo,
            &Pick::Prefix,
            &Pick::Unit,
            &tokens,
            i18n,
        );
        list(body, &tokens)
    }

    /// Mockup row 139: prefixes, then units, under two headings, with the
    /// unit captions and one mark on the prefix and one on the unit.
    #[test]
    fn the_list_holds_prefixes_then_units_with_both_marked() {
        let i18n = I18n::en_us();
        let texts: Vec<String> = crate::layout_probe::lay_out_texts(kilo_ohm_list(&i18n), TALL)
            .into_iter()
            .map(|t| t.text)
            .collect();
        let at = |wanted: &str| texts.iter().position(|t| t == wanted).expect(wanted);
        assert_eq!(at("Prefix"), 0, "{texts:?}");
        assert_eq!(&texts[1..3], ["mΩ", "Ω"], "{texts:?}");
        assert_eq!(texts[3], "kΩ", "{texts:?}");
        assert_eq!(texts[4], "\u{2713}", "the current prefix: {texts:?}");
        assert!(at("GΩ") < at("Unit"), "{texts:?}");
        assert_eq!(texts[at("Unit") + 1], "\u{2014}", "{texts:?}");
        assert_eq!(texts[at("ohm") - 1], "Ω", "{texts:?}");
        assert_eq!(texts[at("ohm") + 1], "\u{2713}", "the unit: {texts:?}");
        assert!(texts.contains(&"farad".to_string()), "{texts:?}");
        let marks = texts.iter().filter(|t| *t == "\u{2713}").count();
        assert_eq!(marks, 2, "{texts:?}");
    }

    #[test]
    fn a_prefix_and_a_unit_entry_each_send_their_own_pick() {
        let i18n = I18n::en_us();
        let prefix = crate::layout_probe::messages_on_click(kilo_ohm_list(&i18n), TALL, "MΩ");
        assert_eq!(prefix, vec![Pick::Prefix(SiPrefix::Mega)]);
        let unit = crate::layout_probe::messages_on_click(kilo_ohm_list(&i18n), TALL, "farad");
        assert_eq!(unit, vec![Pick::Unit("F".into())]);
    }

    /// The menu's list is the plain unit list for a unit with no prefixes.
    #[test]
    fn a_unit_with_no_prefix_has_no_prefix_section() {
        let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteLight);
        let i18n = I18n::en_us();
        let current = UnitChoice(Some(Unit::Percent));
        let rows = unit_rows(&current, &Pick::Unit, &tokens, &i18n);
        let element = list(Column::new().extend(rows), &tokens);
        let texts = crate::layout_probe::lay_out_texts(element, TALL);
        assert!(texts.iter().all(|t| t.text != "Prefix"));
        assert_eq!(texts[0].text, "Unit");
    }

    /// The longer joined list still opens above a low row, in reach.
    #[test]
    fn the_joined_menu_of_a_low_row_opens_above_it() {
        let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteLight);
        let i18n = I18n::en_us();
        let menu = UnitMenu {
            selected: Some(Unit::Ohm),
            is_open: true,
            on_open: Pick::Unit("opened".into()),
            on_dismiss: Pick::Unit("dismissed".into()),
        };
        let menu = measurement_menu(
            menu,
            SiPrefix::Kilo,
            Pick::Prefix,
            Pick::Unit,
            &tokens,
            &i18n,
        );
        let gap = Space::new().height(Length::Fill).boxed();
        let page: Element<'_, Pick> = Column::new().push(gap).push(menu).boxed();
        let hits = click_through_overlays(page, WINDOW.height - 120.0);
        assert!(
            matches!(hits.as_slice(), [Pick::Prefix(_)] | [Pick::Unit(_)])
                && hits != [Pick::Unit("dismissed".into())],
            "{hits:?}"
        );
    }
}
