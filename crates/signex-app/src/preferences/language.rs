//! Language page (mockup row 144a): the interface language, a preview of real strings
//! in the drafted language, and a note while the pseudo-locale is drafted.
//! The choice is a draft: Apply or OK switches every window at once, with no
//! restart.

use iced::widget::{Row, column, container, pick_list, row};
use iced::{Alignment, Background, Border, Element, Length, Padding, Theme, Widget as _};
use signex_i18n::{Arg, I18n, LanguagePref, MessageId, ids, menu_ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::text;

use super::setting_row::{self, SettingRow, StatusGlyph};
use super::{PrefMsg, PrefsView};
use crate::choice::{Choice, chosen};
use crate::styles::{self, PillTone, Radii, Spacing, TypeScale, ti, tokens};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The root menus, left to right, as the menu bar names them. A test pins
/// this to `menu_bar::model::root_titles`.
const MENU_TITLES: [MessageId; 8] = [
    ids::MENU_FILE,
    menu_ids::MENU_EDIT,
    menu_ids::MENU_VIEW,
    menu_ids::MENU_PROJECT,
    menu_ids::MENU_PLACE,
    menu_ids::MENU_DESIGN,
    menu_ids::MENU_TOOLS,
    menu_ids::MENU_HELP,
];

/// Three real button labels: Run ERC, Annotate Schematics..., Save All.
const BUTTONS: [MessageId; 3] = [
    ids::DIALOG_ERC_RUN,
    menu_ids::MENU_TOOLS_ANNOTATE,
    ids::DIALOG_BUTTON_SAVE_ALL,
];

/// A real caption: the ERC panel's stale-results banner.
const CAPTION: MessageId = ids::PANELS_ERC_STALE_BANNER;

/// The Language page.
pub(super) fn content_language<'a>(v: PrefsView<'a>) -> Element<'a, PrefMsg> {
    let (i18n, t) = (v.i18n, v.panel_tokens);
    let mut items: Vec<Element<'a, PrefMsg>> = vec![
        setting_row::page_heading(
            t!(i18n, ids::PREFERENCES_LANGUAGE_HEADING),
            t!(i18n, ids::PREFERENCES_LANGUAGE_INTRO),
            t,
        ),
        setting_row::section_header(
            t!(i18n, ids::PREFERENCES_LANGUAGE_SECTION_INTERFACE),
            None,
            t,
        ),
        interface_language_row(i18n, v.draft_language, v.system_language_name, t),
        preview(&v),
    ];
    if v.draft_language.is_pseudo_locale() {
        items.push(
            container(setting_row::banner(
                StatusGlyph::Info,
                PillTone::Info,
                text(t!(i18n, ids::PREFERENCES_LANGUAGE_PSEUDO_NOTE))
                    .size(TS.body)
                    .color(ti(t.text))
                    .width(Length::Fill)
                    .boxed(),
                t,
            ))
            .padding(Padding {
                top: SP.l,
                ..Padding::ZERO
            })
            .boxed(),
        );
    }
    setting_row::page(items)
}

/// The Interface language row. It is a wide row (mockup row 144a, #3003):
/// the value names the language "Follow the system" resolves to, and the
/// standard control column cuts that name off.
fn interface_language_row<'a>(
    i18n: &I18n,
    draft: &LanguagePref,
    system_language_name: &str,
    t: &ThemeTokens,
) -> Element<'a, PrefMsg> {
    let choices = language_choices(i18n, draft, system_language_name);
    let picker = pick_list(
        chosen(&choices, draft),
        choices,
        |c: &Choice<LanguagePref>| c.label.clone(),
    )
    .graphite()
    .on_select(|c: Choice<LanguagePref>| PrefMsg::DraftLanguage(c.value))
    .padding([SP.s, SP.m])
    .width(Length::Fill);
    setting_row::wide_setting_row(
        SettingRow {
            label: t!(i18n, ids::PREFERENCES_LANGUAGE_INTERFACE),
            pills: Vec::new(),
            help: Some(setting_row::help_text(
                t!(i18n, ids::PREFERENCES_LANGUAGE_INTERFACE_HELP),
                t,
            )),
            control: picker.boxed(),
            error: None,
        },
        t,
    )
}

/// The rows of the language list, labelled in the UI language: Follow the
/// system (naming the language it resolves to), each shipped language by its
/// own name, and the pseudo-locale last. A drafted language the list does not
/// show (the pseudo-locale in a release build, a language this build lacks)
/// still gets a row, so the picker shows the setting rather than a blank.
pub(super) fn language_choices(
    i18n: &I18n,
    draft: &LanguagePref,
    system_language_name: &str,
) -> Vec<Choice<LanguagePref>> {
    let mut prefs: Vec<LanguagePref> = i18n
        .language_options()
        .iter()
        .map(|option| option.pref.clone())
        .collect();
    if !prefs.contains(draft) {
        prefs.push(draft.clone());
    }
    // The house dropdown has no group headings, so the pseudo-locale (the
    // mock's "Testing" group) is the last row instead.
    prefs.sort_by_key(LanguagePref::is_pseudo_locale);
    prefs
        .into_iter()
        .map(|pref| Choice {
            label: choice_label(i18n, &pref, system_language_name),
            value: pref,
        })
        .collect()
}

fn choice_label(i18n: &I18n, pref: &LanguagePref, system_language_name: &str) -> String {
    match pref {
        LanguagePref::System => i18n.message_plain(
            ids::PREFERENCES_LANGUAGE_FOLLOW_SYSTEM,
            &[("language", Arg::from(system_language_name))],
        ),
        pseudo if pseudo.is_pseudo_locale() => {
            i18n.message_plain(ids::PREFERENCES_LANGUAGE_PSEUDO, &[])
        }
        pref => i18n.option_for(pref).label,
    }
}

/// The preview block: its label and caption in the UI language, then the
/// root menu titles, three buttons and a caption in the drafted language.
fn preview<'a>(v: &PrefsView<'a>) -> Element<'a, PrefMsg> {
    let (i18n, sample, t) = (v.i18n, v.language_preview, v.panel_tokens);
    let r = tokens::resolve(t);
    let heading = row![
        text(t!(i18n, ids::PREFERENCES_LANGUAGE_PREVIEW))
            .size(TS.section)
            .font(setting_row::medium())
            .style(styles::section_label(t)),
        text(t!(i18n, ids::PREFERENCES_LANGUAGE_PREVIEW_CAPTION))
            .size(TS.caption)
            .color(r.ink4),
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center);

    let menus = Row::with_children(MENU_TITLES.iter().map(|&id| {
        text(t!(sample, id))
            .size(TS.label)
            .color(ti(t.text))
            .boxed()
    }))
    .spacing(SP.l + SP.xs)
    .wrap()
    .vertical_spacing(SP.s);
    let buttons = Row::with_children(
        BUTTONS
            .iter()
            .map(|&id| setting_row::small_button_face(t!(sample, id), t)),
    )
    .spacing(SP.m)
    .align_y(Alignment::Center)
    .wrap()
    .vertical_spacing(SP.m);
    let (fill, line) = (ti(t.toolbar_bg), ti(t.border));
    let samples = container(
        column![
            menus,
            buttons,
            text(t!(sample, CAPTION)).size(TS.caption).color(r.ink3),
        ]
        .spacing(SP.m)
        .width(Length::Fill),
    )
    .padding([SP.l - SP.xs, SP.l])
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(fill)),
        border: Border {
            width: 1.0,
            color: line,
            radius: Radii::GRAPHITE.card.into(),
            ..Border::default()
        },
        ..container::Style::default()
    });

    column![
        container(column![heading, samples].spacing(SP.m).width(Length::Fill)).padding(Padding {
            top: SP.l - SP.xs,
            bottom: SP.l,
            ..Padding::ZERO
        }),
        setting_row::hairline(t),
    ]
    .width(Length::Fill)
    .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(choices: &[Choice<LanguagePref>]) -> Vec<String> {
        choices.iter().map(|c| c.label.clone()).collect()
    }

    /// Mockup row 144a: the language list names what System resolves to, shows each
    /// language by its own name and puts the pseudo-locale last.
    #[test]
    fn the_language_list_follows_the_mock() {
        let en = I18n::en_us();
        let choices = language_choices(&en, &LanguagePref::System, "English (United States)");
        assert_eq!(choices[0].value, LanguagePref::System);
        assert_eq!(
            choices[0].label,
            "Follow the system: English (United States)"
        );
        assert!(
            labels(&choices).contains(&"English (United States)".to_owned()),
            "{:?}",
            labels(&choices)
        );
        if let Some(pseudo) = choices.iter().position(|c| c.value.is_pseudo_locale()) {
            assert_eq!(pseudo, choices.len() - 1);
            assert_eq!(choices[pseudo].label, "Pseudo-locale (en-XA)");
        }
    }

    /// A drafted language the list does not show still gets a row.
    #[test]
    fn a_drafted_language_outside_the_list_keeps_a_row() {
        let en = I18n::en_us();
        let draft = LanguagePref::Locale("tr-TR".to_owned());
        let choices = language_choices(&en, &draft, "English (United States)");
        assert!(chosen(&choices, &draft).is_some());
    }

    /// The width the dropdown asks for to show `label` whole beside its
    /// handle: the same dropdown, sized to its value.
    fn natural_dropdown_width(label: &str) -> f32 {
        let dropdown = pick_list(
            Some(label.to_owned()),
            vec![label.to_owned()],
            |s: &String| s.clone(),
        )
        .graphite()
        .padding([SP.s, SP.m])
        .width(Length::Shrink);
        crate::layout_probe::natural::<()>(dropdown.boxed(), label).width
    }

    /// #3003, mockup row 144a: with Follow the system drafted, the dropdown
    /// is wide enough to show the whole language it resolves to.
    #[test]
    fn the_follow_the_system_value_fits_the_language_dropdown() {
        let en = I18n::en_us();
        let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteLight);
        let row = interface_language_row(
            &en,
            &LanguagePref::System,
            "English (United States)",
            &tokens,
        );
        let label = "Follow the system: English (United States)";

        let texts = crate::layout_probe::lay_out_texts(row, iced::Size::new(900.0, 300.0));

        let field = crate::layout_probe::find(&texts, label).bounds;
        let needed = natural_dropdown_width(label);
        assert!(
            needed <= field.width + 0.5,
            "the value is cut off: it needs {needed} px, the field is {field:?}"
        );
        assert!(
            (field.width - setting_row::CONTROL_COLUMN_WIDE_W).abs() < 0.5,
            "the dropdown fills the wide control column: {field:?}"
        );
    }

    /// The preview names the real root menus, in menu-bar order.
    #[test]
    fn the_preview_lists_the_real_root_menus() {
        let en = I18n::en_us();
        let shown: Vec<String> = MENU_TITLES
            .iter()
            .map(|&id| t!(en, id).into_string())
            .collect();
        assert_eq!(shown, crate::menu_bar::model::root_titles(&en));
    }
}
