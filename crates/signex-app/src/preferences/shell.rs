//! Graphite Preferences shell (mockup row 143): 52 px header with a "Group / Page"
//! subtitle, the broken-file banner across the full width, a 216 px page
//! list with an accent dot on every page holding unsaved changes, and the
//! footer: a note, Cancel, Apply and OK. Cancel, Apply and OK are the
//! discard, save and save-and-close actions; on Rule Profiles, whose edits
//! save at once, Cancel reads Close and closes as the X does (mockup row
//! 148). Restore defaults has no backend yet, so it is not drawn.

use super::graphite_fields::{graphite_label, group_messages, nav_matches, nav_search};
use super::setting_row::{hairline, medium};
use super::{PrefMsg, PrefNav, PrefsView, SP, TS, build_content, build_prefs_file_banner};
use crate::app::view::dialogs::{PREFERENCES_HEADER_H, graphite_header};
use crate::styles::{
    self, DialogButtonKind, PillTone, Radii, chrome_px, dialog_button, ti, tokens,
};
use iced::widget::{Column, Space, column, container, row, scrollable};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, MessageId, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, text, tooltip};

/// Navigation width (216 px, the mock's `.pnav`).
pub(super) const GRAPHITE_NAV_W: f32 = chrome_px(216.0 * 11.0 / 12.0);
/// Height of a nav row (28 px).
const NAV_ROW_H: f32 = chrome_px(28.0 * 11.0 / 12.0);
/// Side of the unsaved-changes dot (6 px).
const DIRTY_DOT: f32 = chrome_px(6.0 * 11.0 / 12.0);
/// Height of the footer's Unsaved changes pill (20 px).
const FOOTER_PILL_H: f32 = chrome_px(20.0 * 11.0 / 12.0);
/// Card size of the in-window dialog.
pub(super) const GRAPHITE_DLG_W: f32 = 980.0;
pub(super) const GRAPHITE_DLG_H: f32 = 680.0;

/// The whole dialog: header (in-window only), banner, nav | content, footer.
pub(super) fn graphite_dialog<'a>(v: PrefsView<'a>, with_header: bool) -> Element<'a, PrefMsg> {
    let t = v.panel_tokens;
    let mut items: Vec<Element<'a, PrefMsg>> = Vec::new();
    if with_header {
        let subtitle = t!(
            v.i18n,
            ids::PREFERENCES_SUBTITLE,
            "group" => t!(v.i18n, group_messages(v.nav).0).into_string(),
            "page" => t!(v.i18n, graphite_label(v.nav)).into_string()
        )
        .into_string();
        items.push(graphite_header(
            t!(v.i18n, ids::DIALOG_TITLE_PREFERENCES).into_string(),
            Some(subtitle),
            None,
            PrefMsg::Close,
            PREFERENCES_HEADER_H,
            t,
        ));
    }
    if let Some(banner) = build_prefs_file_banner(v.prefs_load_error, v.prefs_status, v.i18n, t) {
        items.push(banner);
    }
    let divider = {
        let line = tokens::resolve(t).line_soft;
        container(Space::new())
            .width(1)
            .height(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(line)),
                ..container::Style::default()
            })
            .boxed()
    };
    items.push(
        row![graphite_nav(&v), divider, build_content(v)]
            .width(Length::Fill)
            .height(Length::Fill)
            .boxed(),
    );
    items.push(hairline(t));
    items.push(graphite_footer(&v));

    container(Column::with_children(items).boxed())
        .width(Length::Fill)
        .height(Length::Fill)
        .style(styles::modal_card(t))
        .clip(true)
        .boxed()
}

/// The page list: the search field, then each group's heading in capitals
/// and its pages.
fn graphite_nav<'a>(v: &PrefsView<'a>) -> Element<'a, PrefMsg> {
    let (t, i18n, query) = (v.panel_tokens, v.i18n, v.nav_search);
    let mut col = column![].padding(SP.m).width(Length::Fill);
    col = col.push(
        container(nav_search(query, t, i18n))
            .padding(iced::Padding {
                bottom: SP.xs,
                ..iced::Padding::ZERO
            })
            .boxed(),
    );
    let mut last_group = "";
    for &item in PrefNav::ALL
        .iter()
        .filter(|&&n| nav_matches(i18n, n, query))
    {
        let group = item.group();
        if group != last_group {
            last_group = group;
            col = col.push(
                container(
                    text(t!(i18n, group_messages(item).1))
                        .size(TS.section)
                        .font(medium())
                        .style(styles::section_label(t)),
                )
                .padding(iced::Padding {
                    top: SP.l,
                    right: SP.m,
                    bottom: SP.s,
                    left: SP.m,
                })
                .boxed(),
            );
        }
        let dirty = v.dirty_pages.contains(&item);
        col = col.push(nav_row(item, item == v.nav, dirty, t, i18n));
    }
    let bg = ti(t.panel_bg);
    container(
        scrollable(col)
            .width(Length::Fill)
            .style(super::graphite_fields::scroll_style(t))
            .boxed(),
    )
    .width(Length::Fixed(GRAPHITE_NAV_W))
    .height(Length::Fill)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(bg)),
        ..container::Style::default()
    })
    .boxed()
}

/// One page row: 28 px, control radius; the selection fill and weight 500
/// when open, the hover overlay otherwise; an accent dot at the right when
/// the page holds unsaved changes.
fn nav_row<'a>(
    item: PrefNav,
    active: bool,
    dirty: bool,
    t: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, PrefMsg> {
    let (ink, selected, hover) = (
        ti(t.text),
        ti(t.selection),
        tokens::overlay(t, tokens::Overlay::Hover),
    );
    let font = if active {
        medium()
    } else {
        signex_widgets::ui_font::regular()
    };
    let mut line = row![
        text(t!(i18n, graphite_label(item)))
            .size(TS.label)
            .font(font)
            .color(ink)
            .width(Length::Fill)
    ]
    .spacing(SP.s + SP.xs)
    .align_y(Alignment::Center);
    if dirty {
        line = line.push(dirty_dot(t, i18n));
    }
    button(
        container(line)
            .padding([0.0, SP.m])
            .height(Length::Fill)
            .align_y(iced::alignment::Vertical::Center)
            .width(Length::Fill),
    )
    .padding(0)
    .width(Length::Fill)
    .height(Length::Fixed(NAV_ROW_H))
    .on_press(PrefMsg::Nav(item))
    .style(move |_: &Theme, status: button::Status| button::Style {
        background: if active {
            Some(Background::Color(selected))
        } else if matches!(status, button::Status::Hovered | button::Status::Pressed) {
            Some(Background::Color(hover))
        } else {
            None
        },
        text_color: ink,
        border: Border {
            radius: Radii::GRAPHITE.control.into(),
            ..Border::default()
        },
        ..button::Style::default()
    })
    .boxed()
}

/// The 6 px accent dot of a page with unsaved changes, titled "Unsaved
/// changes".
fn dirty_dot<'a>(t: &ThemeTokens, i18n: &I18n) -> Element<'a, PrefMsg> {
    let accent = ti(t.accent);
    let dot = container(Space::new())
        .width(Length::Fixed(DIRTY_DOT))
        .height(Length::Fixed(DIRTY_DOT))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(accent)),
            border: Border {
                radius: Radii::GRAPHITE.pill.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    tooltip(
        dot,
        container(text(t!(i18n, ids::PREFERENCES_UNSAVED)).size(TS.caption))
            .padding([SP.xs, SP.s + SP.xs])
            .style(styles::context_menu(t)),
        tooltip::Position::Bottom,
    )
    .boxed()
}

/// What the footer says left of its buttons, in the mock's order: the
/// Language page says when the language changes; otherwise unsaved changes
/// show the warn pill; otherwise the Appearance page says themes preview
/// live and the Rule Profiles page that its edits save at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FooterNote {
    LanguageApplies,
    Unsaved,
    ThemePreviews,
    RuleProfilesSave,
    Nothing,
}

pub(super) fn footer_note(nav: PrefNav, dirty: bool) -> FooterNote {
    match (nav, dirty) {
        (PrefNav::Language, _) => FooterNote::LanguageApplies,
        (_, true) => FooterNote::Unsaved,
        (PrefNav::Appearance, false) => FooterNote::ThemePreviews,
        (PrefNav::RuleProfiles, false) => FooterNote::RuleProfilesSave,
        _ => FooterNote::Nothing,
    }
}

/// The footer's dismiss button, label and message (mockup row 148). Rule
/// Profiles edits are already in their files, so the button is Close and
/// takes the guarded close of the X and Esc: unsaved drafts of another
/// page keep the dialog open. Every draft page offers Cancel, which
/// discards the drafts and closes.
pub(super) fn dismiss_button(nav: PrefNav) -> (MessageId, PrefMsg) {
    match nav {
        PrefNav::RuleProfiles => (ids::DIALOG_BUTTON_CLOSE, PrefMsg::Close),
        PrefNav::Appearance
        | PrefNav::Language
        | PrefNav::General
        | PrefNav::Annotation
        | PrefNav::Editing
        | PrefNav::SchematicDisplay
        | PrefNav::PcbDefaults
        | PrefNav::Keyboard
        | PrefNav::LibraryDistributors
        | PrefNav::ComponentClasses => (ids::DIALOG_BUTTON_CANCEL, PrefMsg::DiscardAndClose),
    }
}

/// Footer: the note, Cancel, Apply and OK at the right on the chrome fill.
/// Apply needs unsaved changes; OK saves them and closes, or only closes
/// when there are none.
fn graphite_footer<'a>(v: &PrefsView<'a>) -> Element<'a, PrefMsg> {
    let t = v.panel_tokens;
    let ink3 = tokens::resolve(t).ink3;
    let note: Element<'a, PrefMsg> = match footer_note(v.nav, v.dirty) {
        FooterNote::LanguageApplies => text(t!(v.i18n, ids::PREFERENCES_LANGUAGE_FOOTER_NOTE))
            .size(TS.body)
            .color(ink3)
            .boxed(),
        FooterNote::Unsaved => {
            container(text(t!(v.i18n, ids::PREFERENCES_UNSAVED)).size(TS.caption))
                .padding([0.0, SP.m])
                .height(Length::Fixed(FOOTER_PILL_H))
                .align_y(iced::alignment::Vertical::Center)
                .style(styles::pill(t, PillTone::Warn))
                .boxed()
        }
        FooterNote::ThemePreviews => text(t!(v.i18n, ids::PREFERENCES_THEME_PREVIEW_HINT))
            .size(TS.body)
            .color(ink3)
            .boxed(),
        FooterNote::RuleProfilesSave => {
            text(t!(v.i18n, ids::PREFERENCES_RULE_PROFILES_FOOTER_NOTE))
                .size(TS.body)
                .color(ink3)
                .boxed()
        }
        FooterNote::Nothing => Space::new().boxed(),
    };
    // A pitch label holding refused text stays open: Save refuses it, and
    // the plain Close of a clean dialog would drop it.
    let ok_msg = if v.dirty {
        PrefMsg::SaveAndClose
    } else if v.pages_view.snap_label_refused(v.draft_fast_grids, v.i18n) {
        PrefMsg::Save
    } else {
        PrefMsg::Close
    };
    let chrome = ti(t.toolbar_bg);
    let (dismiss_id, dismiss_msg) = dismiss_button(v.nav);
    container(
        row![
            Space::new().width(Length::Fill),
            note,
            dialog_button(
                t!(v.i18n, dismiss_id),
                DialogButtonKind::Cancel,
                Some(dismiss_msg),
                t
            ),
            dialog_button(
                t!(v.i18n, ids::DIALOG_BUTTON_APPLY),
                DialogButtonKind::Secondary,
                v.dirty.then_some(PrefMsg::Save),
                t
            ),
            dialog_button(
                t!(v.i18n, ids::DIALOG_BUTTON_OK),
                DialogButtonKind::Primary,
                Some(ok_msg),
                t
            ),
        ]
        .spacing(SP.m)
        .align_y(Alignment::Center),
    )
    .padding([SP.m, SP.l])
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(chrome)),
        ..container::Style::default()
    })
    .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mockup rows 143 and 144a: the footer note follows the mock's precedence.
    #[test]
    fn the_footer_note_follows_the_mock() {
        assert_eq!(
            footer_note(PrefNav::Language, true),
            FooterNote::LanguageApplies
        );
        assert_eq!(
            footer_note(PrefNav::Language, false),
            FooterNote::LanguageApplies
        );
        assert_eq!(footer_note(PrefNav::Appearance, true), FooterNote::Unsaved);
        assert_eq!(footer_note(PrefNav::Editing, true), FooterNote::Unsaved);
        assert_eq!(
            footer_note(PrefNav::Appearance, false),
            FooterNote::ThemePreviews
        );
        assert_eq!(footer_note(PrefNav::Annotation, false), FooterNote::Nothing);
    }

    /// Mockup row 148: the page says its edits save at once; unsaved changes
    /// of another page still take the footer.
    #[test]
    fn the_rule_profiles_page_says_it_saves_at_once() {
        assert_eq!(
            footer_note(PrefNav::RuleProfiles, false),
            FooterNote::RuleProfilesSave
        );
        assert_eq!(
            footer_note(PrefNav::RuleProfiles, true),
            FooterNote::Unsaved
        );
    }

    /// Mockup row 148: Rule Profiles edits save at once, so the footer
    /// offers Close there and Cancel on the draft pages.
    #[test]
    fn the_rule_profiles_footer_says_close_in_place_of_cancel() {
        let en = I18n::en_us();
        let (close, msg) = dismiss_button(PrefNav::RuleProfiles);
        assert_eq!(t!(en, close).into_string(), "Close");
        assert!(matches!(msg, PrefMsg::Close), "{msg:?}");
        for &nav in PrefNav::ALL {
            if nav != PrefNav::RuleProfiles {
                let (cancel, msg) = dismiss_button(nav);
                assert_eq!(cancel, ids::DIALOG_BUTTON_CANCEL, "{nav:?}");
                assert!(matches!(msg, PrefMsg::DiscardAndClose), "{nav:?}");
            }
        }
    }

    /// Mockup row 148: Close on Rule Profiles is the guarded close, so an
    /// unsaved draft of another page keeps the dialog open and is kept.
    #[test]
    fn the_rule_profiles_close_keeps_another_pages_unsaved_draft() {
        use crate::app::{Message, PreferencesMsg, Signex};
        let send = |app: &mut Signex, msg: PrefMsg| {
            let _ = app.update(Message::Preferences(PreferencesMsg::Inner(msg)));
        };
        let (mut app, _boot) = Signex::new();
        let _ = app.update(Message::Preferences(PreferencesMsg::Open));
        let wires = !app.ui_state.move_keeps_wires;
        send(&mut app, PrefMsg::DraftMoveKeepsWires(wires));
        send(&mut app, PrefMsg::Nav(PrefNav::RuleProfiles));
        assert!(app.ui_state.preferences_dirty);

        send(&mut app, dismiss_button(PrefNav::RuleProfiles).1);
        assert!(app.ui_state.preferences_open);
        assert_eq!(app.ui_state.preferences_draft_move_keeps_wires, wires);
        assert!(app.ui_state.preferences_dirty);
    }

    #[test]
    fn the_nav_and_its_rows_match_the_mock() {
        for (got, mock) in [(GRAPHITE_NAV_W, 216.0), (NAV_ROW_H, 28.0), (DIRTY_DOT, 6.0)] {
            assert!((got - mock).abs() < 1e-3, "{got} != {mock}");
        }
    }
}
