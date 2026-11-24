//! The sheet laid out: the header names the profile and the scope; a
//! toolbar holds the filter field and the scope switch; the list is two
//! columns of categories (or the no-match state) with a count under it; the
//! footer names the sheet's key and offers Preferences and Close.

use iced::font::Weight;
use iced::widget::{Column, Row, Space, column, container, row, scrollable};
use iced::{Alignment, Background, Border, Color, Element, Font, Length, Padding, Theme, Widget};
use signex_i18n::{I18n, Localized, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{Label, button, raw, rich_text, segmented, span, text, text_input};

use super::filter::{
    Listing, ShownSection, cap_matches, column_split, label_runs, listing, trigger_caps,
};
use super::{ShortcutScope, ShortcutSection, ShortcutSheet};
use crate::about_modal::{GLYPH_THEME, px};
use crate::app::{Message, OverlayMsg};
use crate::keymap::{CompiledKeymap, KeymapEditorRow, ShortcutProfileSet, category_title};
use crate::styles::{self, ChromeMetrics, DialogButtonKind, TypeScale, ti};

const TS: TypeScale = TypeScale::DEFAULT;

const CARD_W: f32 = px(720.0);
const CARD_H: f32 = px(560.0);
const SCOPE_W: f32 = px(220.0);
const ROW_H: f32 = px(26.0);
const COLUMN_GAP: f32 = px(28.0);

/// What the sheet reads besides its rows: the active profile's name (the
/// header subtitle) and the key that opens and closes it (the footer).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetFacts {
    pub profile_name: String,
    pub sheet_key: String,
}

impl SheetFacts {
    /// The facts of the active profile in `profiles`, compiled as `keymap`.
    pub fn read(profiles: &ShortcutProfileSet, keymap: &CompiledKeymap, i18n: &I18n) -> Self {
        Self {
            profile_name: profiles.active_profile().name.clone(),
            sheet_key: super::sheet_key(keymap, i18n),
        }
    }
}

/// The sheet as a card in the main window, sized to itself: the overlay
/// centres it over the click-outside layer.
pub fn view<'a>(
    i18n: &I18n,
    tokens: &'a ThemeTokens,
    sections: &[ShortcutSection],
    sheet: &'a ShortcutSheet,
    facts: &SheetFacts,
) -> Element<'a, Message> {
    build(i18n, tokens, sections, sheet, facts, false)
}

/// The sheet as the content of its own OS window: no header, filling it.
pub fn window_view<'a>(
    i18n: &I18n,
    tokens: &'a ThemeTokens,
    sections: &[ShortcutSection],
    sheet: &'a ShortcutSheet,
    facts: &SheetFacts,
) -> Element<'a, Message> {
    build(i18n, tokens, sections, sheet, facts, true)
}

fn build<'a>(
    i18n: &I18n,
    tokens: &'a ThemeTokens,
    sections: &[ShortcutSection],
    sheet: &'a ShortcutSheet,
    facts: &SheetFacts,
    windowed: bool,
) -> Element<'a, Message> {
    let shown = listing(i18n, sections, sheet);
    let toolbar = toolbar(i18n, tokens, sheet);
    let body = body(i18n, tokens, sheet, &shown);
    let footer = footer(i18n, tokens, facts);
    if windowed {
        // The OS title bar names the sheet only; the subtitle line keeps
        // the profile it lists in view.
        let caption = container(
            text(subtitle(i18n, facts, sheet.scope))
                .size(TS.caption)
                .color(styles::tokens::resolve(tokens).ink4),
        )
        .padding(Padding {
            top: px(8.0),
            right: px(16.0),
            bottom: 0.0,
            left: px(16.0),
        })
        .width(Length::Fill);
        return container(column![caption, toolbar, body, footer])
            .width(Length::Fill)
            .height(Length::Fill)
            .style(styles::modal_card(tokens))
            .boxed();
    }
    let header = crate::app::view::dialogs::graphite_header(
        crate::keymap::command_menu_text(i18n, "show_current_command_hotkeys"),
        Some(subtitle(i18n, facts, sheet.scope).into_string()),
        None,
        Message::Overlay(OverlayMsg::CloseKeyboardShortcuts),
        crate::app::view::dialogs::DIALOG_HEADER_H,
        tokens,
    );
    container(column![header, toolbar, body, footer])
        .width(Length::Fixed(CARD_W))
        .height(Length::Fixed(CARD_H))
        .clip(true)
        .style(styles::modal_card(tokens))
        .boxed()
}

/// The name of `scope` on the switch, in the subtitle and in the notes.
fn scope_label(i18n: &I18n, scope: ShortcutScope) -> Localized {
    match scope {
        ShortcutScope::Schematic => t!(i18n, ids::DIALOG_SHORTCUTS_SCOPE_SCHEMATIC),
        ShortcutScope::Pcb => t!(i18n, ids::DIALOG_SHORTCUTS_SCOPE_PCB),
        ShortcutScope::All => t!(i18n, ids::DIALOG_SHORTCUTS_SCOPE_ALL),
    }
}

/// "Classic profile · Schematic": the header subtitle, and the caption
/// line of the sheet in its own window.
fn subtitle(i18n: &I18n, facts: &SheetFacts, scope: ShortcutScope) -> Localized {
    t!(
        i18n,
        ids::DIALOG_SHORTCUTS_SUBTITLE,
        "profile" => facts.profile_name.as_str(),
        "scope" => scope_label(i18n, scope).into_string()
    )
}

fn mono() -> Font {
    crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
}

/// A key cap: mono caption text on the sunken key-cap face. `hit` (the cap
/// matches the filter) draws the accent border and ink.
pub(crate) fn key_cap<'a, M: 'a>(
    label: impl Into<Label<'a>>,
    hit: bool,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    let base = styles::key_cap(tokens);
    let (accent, accent_ink) = (
        ti(tokens.accent),
        styles::tokens::resolve(tokens).accent_text,
    );
    container(
        text(label)
            .size(TS.caption)
            .font(mono())
            .wrapping(iced::widget::text::Wrapping::None),
    )
    .padding([px(1.5), px(5.0)])
    .style(move |theme: &Theme| {
        let mut style = base(theme);
        if hit {
            style.border.color = accent;
            style.text_color = Some(accent_ink);
        }
        style
    })
    .boxed()
}

/// A container style with a hairline on its lower edge.
fn bottom_rule(tokens: &ThemeTokens) -> impl Fn(&Theme) -> container::Style + 'static {
    let rule = styles::tokens::resolve(tokens).line_soft;
    move |_| {
        let mut border = Border {
            color: rule,
            ..Border::default()
        };
        border.bottom = iced::border::Side::default().width(1.0);
        container::Style {
            border,
            ..container::Style::default()
        }
    }
}

/// The filter field (search glyph, input, clear button while it holds
/// text) and the Schematic / PCB / All switch.
fn toolbar<'a>(
    i18n: &I18n,
    tokens: &ThemeTokens,
    sheet: &'a ShortcutSheet,
) -> Element<'a, Message> {
    let r = styles::tokens::resolve(tokens);
    let (ink, ink4, accent) = (ti(tokens.text), r.ink4, ti(tokens.accent));
    let input = text_input(
        t!(i18n, ids::DIALOG_SHORTCUTS_FILTER_PLACEHOLDER),
        sheet.filter.as_str(),
    )
    .on_input(|filter| Message::Overlay(OverlayMsg::ShortcutsFilterChanged(filter)))
    .padding(Padding::ZERO)
    .size(TS.body)
    .font(mono())
    .width(Length::Fill)
    .style(move |_: &Theme, _| iced::widget::text_input::Style {
        background: Background::Color(Color::TRANSPARENT),
        border: Border::default(),
        placeholder: ink4,
        value: ink,
        selection: Color { a: 0.4, ..accent },
    });
    let mut field_line: Row<Element<'a, Message>> = Row::new()
        .spacing(px(6.0))
        .align_y(Alignment::Center)
        .push(styles::tinted_glyph(
            crate::icons::icon_chrome_search(GLYPH_THEME),
            px(14.0),
            ink4,
        ))
        .push(input.boxed());
    if !sheet.filter.is_empty() {
        field_line = field_line.push(
            button(styles::tinted_glyph(
                crate::icons::icon_close(GLYPH_THEME),
                px(12.0),
                ink4,
            ))
            .padding(px(3.0))
            .on_press(Message::Overlay(OverlayMsg::ShortcutsFilterCleared))
            .style(styles::ghost_button(tokens))
            .boxed(),
        );
    }
    let field_h = ChromeMetrics::DEFAULT.field;
    let field = container(field_line)
        .padding([0.0, px(8.0)])
        .height(field_h)
        .center_y(field_h)
        .width(Length::Fill)
        .style(styles::field(tokens, sheet.is_filtered()));

    let style = r.segmented(TS.body).fill_width(true);
    let options = ShortcutScope::ALL.map(|scope| (scope, Label::from(scope_label(i18n, scope))));
    let scope = container(segmented(options, sheet.scope, style, |scope| {
        Message::Overlay(OverlayMsg::ShortcutsScopePicked(scope))
    }))
    .width(SCOPE_W);

    container(
        row![field, scope]
            .spacing(px(8.0))
            .align_y(Alignment::Center),
    )
    .padding([px(10.0), px(16.0)])
    .width(Length::Fill)
    .style(bottom_rule(tokens))
    .boxed()
}

/// The two columns and the count under them, or the no-match state;
/// scrolls when it outgrows the card.
fn body<'a>(
    i18n: &I18n,
    tokens: &ThemeTokens,
    sheet: &ShortcutSheet,
    shown: &Listing<'_>,
) -> Element<'a, Message> {
    let content: Element<'a, Message> = if shown.sections.is_empty() {
        no_match(i18n, tokens, sheet)
    } else {
        let (left, right) = shown.sections.split_at(column_split(&shown.sections));
        let count = if sheet.is_filtered() {
            t!(i18n, ids::DIALOG_SHORTCUTS_COUNT_FILTERED, "shown" => shown.shown, "total" => shown.total)
        } else {
            t!(
                i18n,
                ids::DIALOG_SHORTCUTS_COUNT_ALL,
                "total" => shown.total,
                "scope" => scope_label(i18n, sheet.scope).into_string()
            )
        };
        column![
            row![
                section_column(i18n, tokens, left, &sheet.filter),
                section_column(i18n, tokens, right, &sheet.filter),
            ]
            .spacing(COLUMN_GAP)
            .width(Length::Fill),
            Space::new().height(px(16.0)),
            text(count)
                .size(TS.caption)
                .color(styles::tokens::resolve(tokens).ink4),
        ]
        .width(Length::Fill)
        .boxed()
    };
    container(
        scrollable(container(content).padding(px(16.0)).width(Length::Fill))
            .direction(styles::thin_vertical())
            .style(styles::scrollbar_style(tokens))
            .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .boxed()
}

/// One column: each category's uppercase heading over its rows.
fn section_column<'a>(
    i18n: &I18n,
    tokens: &ThemeTokens,
    sections: &[ShownSection<'_>],
    filter: &str,
) -> Element<'a, Message> {
    let mut list: Column<Element<'a, Message>> = Column::new().width(Length::Fill);
    for section in sections {
        let heading = i18n.upper_for_display(&category_title(i18n, section.category));
        list = list.push(
            // raw: the category title, already rendered from the catalog.
            container(
                text(raw(heading))
                    .size(TS.section)
                    .style(styles::section_label(tokens)),
            )
            .padding(Padding {
                top: px(12.0),
                bottom: px(4.0),
                ..Padding::ZERO
            })
            .boxed(),
        );
        for row in &section.rows {
            list = list.push(shortcut_row(i18n, tokens, row, filter));
        }
    }
    list.boxed()
}

/// One row: the action, its match on the accent-soft fill, and the key caps.
fn shortcut_row<'a>(
    i18n: &I18n,
    tokens: &ThemeTokens,
    row: &KeymapEditorRow,
    filter: &str,
) -> Element<'a, Message> {
    let r = styles::tokens::resolve(tokens);
    let ink = ti(tokens.text);
    let mark = Border {
        radius: styles::RADIUS.into(),
        ..Border::default()
    };
    let spans: Vec<_> = label_runs(i18n, &row.label, filter)
        .into_iter()
        .map(|(run, hit)| {
            // raw: runs of the row's label, already rendered, split at the match.
            let piece = span::<()>(raw(run));
            if hit {
                piece
                    .color(r.accent_soft_ink)
                    .background(r.accent_soft)
                    .border(mark)
            } else {
                piece.color(ink)
            }
        })
        .collect();
    let mut line: Row<Element<'a, Message>> =
        Row::new().spacing(px(8.0)).align_y(Alignment::Center).push(
            rich_text::<(), Message, Theme>(spans)
                .size(TS.body)
                // A word longer than the column breaks between glyphs
                // rather than run under the key caps (mockup row 13).
                .wrapping(iced::widget::text::Wrapping::WordOrGlyph)
                .width(Length::Fill)
                .boxed(),
        );
    for cap in trigger_caps(row) {
        // raw: a key or gesture as the keymap renders it.
        line = line.push(key_cap(
            raw(cap.to_string()),
            cap_matches(i18n, cap, filter),
            tokens,
        ));
    }
    // At least a row tall; a long action wraps clear of its key caps.
    container(row![Space::new().height(ROW_H), line].align_y(Alignment::Center))
        .width(Length::Fill)
        .style(bottom_rule(tokens))
        .boxed()
}

/// Nothing matches: says so, and offers All when another scope is shown.
fn no_match<'a>(i18n: &I18n, tokens: &ThemeTokens, sheet: &ShortcutSheet) -> Element<'a, Message> {
    let r = styles::tokens::resolve(tokens);
    let headline = text(t!(
        i18n,
        ids::DIALOG_SHORTCUTS_NO_MATCH,
        "query" => sheet.filter.trim(),
        "scope" => scope_label(i18n, sheet.scope).into_string()
    ))
    .size(TS.body)
    .font(Font {
        weight: Weight::Medium,
        ..signex_widgets::ui_font::regular()
    })
    .color(ti(tokens.text));
    let next: Element<'a, Message> = if sheet.scope == ShortcutScope::All {
        text(t!(i18n, ids::DIALOG_SHORTCUTS_NO_MATCH_ALL))
            .size(TS.body)
            .color(r.ink3)
            .boxed()
    } else {
        row![
            text(t!(i18n, ids::DIALOG_SHORTCUTS_TRY))
                .size(TS.body)
                .color(r.ink3),
            button(
                text(scope_label(i18n, ShortcutScope::All))
                    .size(TS.caption)
                    .color(r.accent_text)
            )
            .padding([px(3.0), px(8.0)])
            .on_press(Message::Overlay(OverlayMsg::ShortcutsScopePicked(
                ShortcutScope::All
            )))
            .style(styles::ghost_button(tokens)),
        ]
        .spacing(px(4.0))
        .align_y(Alignment::Center)
        .boxed()
    };
    container(
        column![headline, next]
            .spacing(px(4.0))
            .align_x(Alignment::Center),
    )
    .padding([px(24.0), px(8.0)])
    .width(Length::Fill)
    .center_x(Length::Fill)
    .boxed()
}

/// "F1 opens and closes this sheet", Edit in Preferences… and Close.
fn footer<'a>(i18n: &I18n, tokens: &ThemeTokens, facts: &SheetFacts) -> Element<'a, Message> {
    let hint = text(t!(i18n, ids::DIALOG_SHORTCUTS_KEY_HINT, "key" => facts.sheet_key.as_str()))
        .size(TS.caption)
        .color(styles::tokens::resolve(tokens).ink4);
    styles::dialog_footer_band(
        vec![hint.boxed()],
        vec![
            styles::dialog_button(
                t!(i18n, ids::DIALOG_SHORTCUTS_EDIT_IN_PREFERENCES),
                DialogButtonKind::Secondary,
                Some(Message::Overlay(OverlayMsg::ShortcutsPreferencesPressed)),
                tokens,
            ),
            styles::dialog_button(
                t!(i18n, ids::DIALOG_BUTTON_CLOSE),
                DialogButtonKind::Primary,
                Some(Message::Overlay(OverlayMsg::CloseKeyboardShortcuts)),
                tokens,
            ),
        ],
        Padding::from([px(12.0), px(16.0)]),
        tokens,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout_probe::{lay_out_texts, messages_on_click};
    use signex_types::theme::{ThemeId, theme_tokens};

    const WINDOW: iced::Size = iced::Size::new(1400.0, 900.0);

    fn sections() -> Vec<ShortcutSection> {
        let set = ShortcutProfileSet::built_ins().expect("built-ins load");
        super::super::build_sections(I18n::en_us_ref(), &set)
    }

    fn facts() -> SheetFacts {
        SheetFacts {
            profile_name: "Classic".into(),
            sheet_key: "F1".into(),
        }
    }

    fn sheet(filter: &str, scope: ShortcutScope) -> ShortcutSheet {
        ShortcutSheet {
            filter: filter.into(),
            scope,
        }
    }

    fn plain(text: &str) -> String {
        text.replace(['\u{2068}', '\u{2069}'], "")
    }

    fn texts(element: Element<'_, Message>) -> Vec<String> {
        lay_out_texts(element, WINDOW)
            .into_iter()
            .map(|t| plain(&t.text))
            .collect()
    }

    #[test]
    fn the_subtitle_names_the_profile_and_the_scope() {
        let i18n = I18n::en_us_ref();
        let facts = facts();
        assert_eq!(
            plain(&subtitle(i18n, &facts, ShortcutScope::Schematic).into_string()),
            "Classic profile · Schematic"
        );
        assert_eq!(
            plain(&subtitle(i18n, &facts, ShortcutScope::Pcb).into_string()),
            "Classic profile · PCB"
        );
        let tokens = theme_tokens(ThemeId::GraphiteLight);
        let sections = sections();
        let open = sheet("", ShortcutScope::All);
        let shown = texts(view(i18n, &tokens, &sections, &open, &facts));
        assert!(shown.iter().any(|t| t == "Keyboard Shortcuts"), "{shown:?}");
        assert!(
            shown.iter().any(|t| t == "Classic profile · All"),
            "{shown:?}"
        );
    }

    #[test]
    fn the_sheet_shows_its_toolbar_count_and_footer() {
        let i18n = I18n::en_us_ref();
        let sections = sections();
        for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
            let tokens = theme_tokens(id);
            let open = sheet("", ShortcutScope::Schematic);
            let total = listing(i18n, &sections, &open).total;
            let shown = texts(view(i18n, &tokens, &sections, &open, &facts()));
            for expected in [
                "Schematic".to_string(),
                "PCB".to_string(),
                "All".to_string(),
                format!("{total} shortcuts in Schematic."),
                "F1 opens and closes this sheet".to_string(),
                "Edit in Preferences…".to_string(),
                "Close".to_string(),
            ] {
                assert!(
                    shown.contains(&expected),
                    "{id:?}: {expected:?} not in {shown:?}"
                );
            }
            assert!(
                !shown.iter().any(|t| t == "Action" || t == "Shortcut"),
                "the Action | Shortcut header is gone"
            );
        }
    }

    /// Mockup row 13: a label too long for its column wraps, growing the
    /// row, instead of running under the key cap.
    #[test]
    fn a_long_action_label_wraps_clear_of_its_key_cap() {
        let i18n = I18n::en_us_ref();
        let tokens = theme_tokens(ThemeId::GraphiteDark);
        let sections = sections();
        let mut sample = sections[0].rows[0].clone();
        let height = |row: &KeymapEditorRow| {
            let line = shortcut_row(i18n, &tokens, row, "");
            crate::layout_probe::root_size(line, iced::Size::new(260.0, 400.0)).height
        };
        sample.label = "Move".into();
        assert!(
            (height(&sample) - ROW_H).abs() < 0.5,
            "one line keeps the row"
        );
        sample.label = "Show valid shortcuts for the current interactive command".into();
        assert!(height(&sample) > ROW_H, "a long label takes a second line");
        // One word with no break in it still wraps instead of running on
        // one line under the key cap.
        sample.label = "Show_valid_shortcuts_for_the_current_interactive_command".into();
        assert!(
            height(&sample) > ROW_H,
            "an unbroken word takes a second line"
        );
    }

    #[test]
    fn a_filter_counts_its_matches() {
        let i18n = I18n::en_us_ref();
        let tokens = theme_tokens(ThemeId::GraphiteDark);
        let sections = sections();
        let filtered = sheet("undo", ShortcutScope::All);
        let found = listing(i18n, &sections, &filtered);
        assert!(found.shown > 0 && found.shown < found.total);
        let caption = format!(
            "{} of {} shortcuts {} Clear the filter to see all categories.",
            found.shown,
            found.total,
            if found.shown == 1 {
                "matches."
            } else {
                "match."
            }
        );
        let shown = texts(view(i18n, &tokens, &sections, &filtered, &facts()));
        assert!(shown.contains(&caption), "{caption:?} not in {shown:?}");
    }

    #[test]
    fn no_match_offers_all_unless_all_is_shown() {
        let i18n = I18n::en_us_ref();
        let tokens = theme_tokens(ThemeId::GraphiteLight);
        let sections = sections();

        let narrow = sheet("zzzqqq", ShortcutScope::Schematic);
        let shown = texts(view(i18n, &tokens, &sections, &narrow, &facts()));
        assert!(
            shown
                .iter()
                .any(|t| t == "No shortcut matches “zzzqqq” in Schematic."),
            "{shown:?}"
        );
        assert!(shown.iter().any(|t| t == "Try"));
        // The switch's All and the offered All.
        assert_eq!(shown.iter().filter(|t| *t == "All").count(), 2);
        let offered = messages_on_click(no_match(i18n, &tokens, &narrow), WINDOW, "All");
        assert!(offered.iter().any(|m| matches!(
            m,
            Message::Overlay(OverlayMsg::ShortcutsScopePicked(ShortcutScope::All))
        )));

        let wide = sheet("zzzqqq", ShortcutScope::All);
        let shown = texts(view(i18n, &tokens, &sections, &wide, &facts()));
        assert!(
            shown
                .iter()
                .any(|t| t == "Check the spelling, or clear the filter.")
        );
        assert!(!shown.iter().any(|t| t == "Try"));
    }

    #[test]
    fn the_footer_buttons_send_their_messages() {
        let i18n = I18n::en_us_ref();
        let tokens = theme_tokens(ThemeId::GraphiteDark);
        let sections = sections();
        let open = sheet("", ShortcutScope::Pcb);
        let click = |label: &str| {
            messages_on_click(
                view(i18n, &tokens, &sections, &open, &facts()),
                WINDOW,
                label,
            )
        };
        assert!(
            click("Edit in Preferences…")
                .iter()
                .any(|m| matches!(m, Message::Overlay(OverlayMsg::ShortcutsPreferencesPressed)))
        );
        assert!(
            click("Close")
                .iter()
                .any(|m| matches!(m, Message::Overlay(OverlayMsg::CloseKeyboardShortcuts)))
        );
        assert!(click("Schematic").iter().any(|m| matches!(
            m,
            Message::Overlay(OverlayMsg::ShortcutsScopePicked(ShortcutScope::Schematic))
        )));
    }

    #[test]
    fn the_window_form_has_no_header_but_the_toolbar_and_footer() {
        let i18n = I18n::en_us_ref();
        let tokens = theme_tokens(ThemeId::GraphiteLight);
        let sections = sections();
        let open = sheet("", ShortcutScope::Schematic);
        let shown = texts(window_view(i18n, &tokens, &sections, &open, &facts()));
        assert!(!shown.iter().any(|t| t == "Keyboard Shortcuts"));
        assert!(
            shown.iter().any(|t| t == "Classic profile · Schematic"),
            "the window names the profile it lists: {shown:?}"
        );
        assert!(shown.iter().any(|t| t == "PCB"));
        assert!(shown.iter().any(|t| t == "Edit in Preferences…"));
    }
}
