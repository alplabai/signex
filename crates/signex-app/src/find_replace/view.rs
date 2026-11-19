use super::{FindReplaceMsg, FindReplaceState, FindScope};
use iced::widget::{Space, column, container, row, scrollable};
use iced::{Background, Border, Element, Length, Theme, Widget};
use signex_i18n::{I18n, Localized, MessageId, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, raw, text, text_input};

use crate::styles::{Spacing, TypeScale};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

const DIALOG_W: f32 = 440.0;
const DIALOG_H: f32 = 400.0;

/// The dialog painted as a centred card on a scrim in the main window.
pub fn view<'a>(
    state: &'a FindReplaceState,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, FindReplaceMsg> {
    build(state, tokens, i18n, false)
}

/// The dialog as the content of its own OS window: no header (the native
/// title bar has the title and the close), filling the window.
pub fn window_view<'a>(
    state: &'a FindReplaceState,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, FindReplaceMsg> {
    build(state, tokens, i18n, true)
}

/// One result row's two lines: what kind of text it is, and the text.
type ResultLines = (MessageId, String);

/// The catalog name of a sheet hit's kind.
fn find_kind_label(kind: signex_engine::FindKind) -> MessageId {
    use signex_engine::FindKind;
    match kind {
        FindKind::NetLabel => signex_i18n::command_ids::CMD_MENU_PLACE_NET_LABEL,
        FindKind::TextNote => ids::FIND_KIND_TEXT_NOTE,
        FindKind::Designator => ids::PROPERTIES_DESIGNATOR,
        FindKind::Value => ids::PROPERTIES_VALUE,
    }
}

/// The catalog name of a board hit's kind.
fn board_find_kind_label(kind: signex_engine::pcb::find::BoardFindKind) -> MessageId {
    use signex_engine::pcb::find::BoardFindKind;
    match kind {
        BoardFindKind::Designator => ids::PROPERTIES_DESIGNATOR,
        BoardFindKind::Value => ids::PROPERTIES_VALUE,
        BoardFindKind::Footprint => ids::PROPERTIES_FOOTPRINT,
        BoardFindKind::Net => ids::PROPERTIES_NET,
    }
}

/// The rows the list shows: a board's hits on a board (#2403), else the
/// sheet results with the placement a designator was read at.
fn result_lines(state: &FindReplaceState) -> Vec<ResultLines> {
    if let Some(board) = &state.board {
        return board
            .hits
            .iter()
            .map(|hit| (board_find_kind_label(hit.kind), hit.text.clone()))
            .collect();
    }
    state
        .matches
        .iter()
        .map(|hit| {
            let text = match &hit.placement_label {
                Some(label) => format!("{} · {label}", hit.hit.text),
                None => hit.hit.text.clone(),
            };
            (find_kind_label(hit.hit.kind), text)
        })
        .collect()
}

fn build<'a>(
    state: &'a FindReplaceState,
    tokens: &ThemeTokens,
    i18n: &I18n,
    windowed: bool,
) -> Element<'a, FindReplaceMsg> {
    let text_primary = crate::styles::ti(tokens.text);
    let text_secondary = crate::styles::ti(tokens.text_secondary);
    let border = crate::styles::ti(tokens.border);
    let hover = crate::styles::ti(tokens.hover);
    let accent = crate::styles::ti(tokens.accent);
    let title = t!(
        i18n,
        if state.replace_mode {
            ids::DIALOG_TITLE_FIND_REPLACE
        } else {
            ids::FIND_TITLE
        }
    );

    // The shared dialog header (title, neutral close).
    let header: Element<'a, FindReplaceMsg> = crate::app::view::dialogs::modal_title_header(
        title.into_string(),
        FindReplaceMsg::Close,
        tokens,
    );

    let query_row = row![
        text(t!(i18n, ids::FIND_LABEL))
            .size(TS.body)
            .color(text_secondary)
            .width(80),
        text_input(t!(i18n, ids::PLACEHOLDER_FIND_QUERY), &state.query)
            .style(crate::panels::form_look::text_input_style)
            .on_input(FindReplaceMsg::QueryChanged)
            .padding([6.0, SP.m])
            .size(TS.label)
            .width(Length::Fill),
    ]
    .align_y(iced::Alignment::Center)
    .spacing(10);

    let scope_button = |label: MessageId, scope: FindScope| {
        let chosen = state.scope == scope;
        button(text(t!(i18n, label)).size(TS.body).color(if chosen {
            accent
        } else {
            text_secondary
        }))
        .on_press(FindReplaceMsg::ScopeChanged(scope))
        .style(crate::styles::menu_item(tokens))
    };
    let scope_row = row![
        text(t!(i18n, ids::FIND_SCOPE))
            .size(TS.body)
            .color(text_secondary)
            .width(80),
        scope_button(ids::FIND_SCOPE_SHEET, FindScope::Sheet),
        scope_button(ids::FIND_SCOPE_PROJECT, FindScope::Project),
    ]
    .align_y(iced::Alignment::Center)
    .spacing(10);

    // A board is searched whole (#2403): no scope to pick.
    let mut form = column![query_row].spacing(10);
    if state.board.is_none() {
        form = form.push(Widget::boxed(scope_row));
    }
    if state.replace_mode {
        form = form.push(Widget::boxed(
            row![
                text(t!(i18n, ids::FIND_REPLACE_LABEL))
                    .size(TS.body)
                    .color(text_secondary)
                    .width(80),
                text_input(
                    t!(i18n, ids::PLACEHOLDER_FIND_REPLACEMENT),
                    &state.replacement
                )
                .style(crate::panels::form_look::text_input_style)
                .on_input(FindReplaceMsg::ReplacementChanged)
                .padding([6.0, SP.m])
                .size(TS.label)
                .width(Length::Fill),
            ]
            .align_y(iced::Alignment::Center)
            .spacing(10),
        ));
    }
    let count = count_line(
        i18n,
        state.replace_job.is_some(),
        state.scanning,
        state.result_count(),
    );
    form = form.push(
        container(text(count).size(TS.caption).color(text_secondary))
            .padding([SP.xs, 0.0])
            .boxed(),
    );
    if let Some(note) = &state.skipped_note {
        form = form.push(
            // raw: the skipped-sheets note is worded by the project scan (#2061).
            text(raw(note.clone()))
                .size(TS.caption)
                .color(text_secondary)
                .boxed(),
        );
    }

    let lines = result_lines(state);
    let results: Vec<Element<'a, FindReplaceMsg>> = if lines.is_empty() {
        vec![
            container(
                text(t!(i18n, ids::FIND_NO_MATCHES))
                    .size(TS.body)
                    .color(text_secondary),
            )
            .padding([10.0, SP.l])
            .width(Length::Fill)
            .boxed(),
        ]
    } else {
        let result_row = |idx: usize, (kind, found): ResultLines| -> Element<'a, FindReplaceMsg> {
            let is_active = state.selected_index == Some(idx);
            let row_bg = if is_active {
                Some(Background::Color(hover))
            } else {
                None
            };
            button(
                row![
                    column![
                        text(t!(i18n, kind)).size(TS.body).color(text_primary),
                        text(UserText::new(found))
                            .size(TS.caption)
                            .color(text_secondary),
                    ]
                    .spacing(SP.xs)
                    .width(Length::Fill),
                ]
                .width(Length::Fill),
            )
            .width(Length::Fill)
            .padding([SP.m, 10.0])
            .on_press(FindReplaceMsg::SelectResult(idx))
            .style(move |_: &Theme, status: button::Status| {
                let bg = match status {
                    button::Status::Hovered => Some(Background::Color(hover)),
                    _ => row_bg,
                };
                button::Style {
                    background: bg,
                    border: Border {
                        width: if is_active { 1.0 } else { 0.0 },
                        radius: 4.0.into(),
                        color: if is_active { accent } else { border },
                        ..Border::default()
                    },
                    text_color: text_primary,
                    ..button::Style::default()
                }
            })
            .boxed()
        };
        let mut rows: Vec<Element<'a, FindReplaceMsg>> = Vec::new();
        let mut current_sheet: Option<&std::path::Path> = None;
        for (idx, line) in lines.into_iter().enumerate() {
            let sheet = state.matches.get(idx).and_then(|hit| hit.sheet.as_deref());
            if state.board.is_none() && state.scope == FindScope::Project && sheet != current_sheet
            {
                current_sheet = sheet;
                let name = current_sheet
                    .and_then(|p| p.file_name())
                    .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                rows.push(
                    container(
                        text(signex_i18n::UserText::new(name))
                            .size(TS.body)
                            .color(accent),
                    )
                    .padding([6.0, SP.s])
                    .boxed(),
                );
            }
            rows.push(result_row(idx, line));
        }
        rows
    };

    let replace_current: Element<'a, FindReplaceMsg> =
        if state.replace_mode && state.selected_index.is_some() && !state.is_busy() {
            button(
                text(t!(i18n, ids::FIND_REPLACE_CURRENT))
                    .size(TS.body)
                    .color(text_primary),
            )
            .on_press(FindReplaceMsg::ReplaceCurrent)
            .style(crate::styles::menu_item(tokens))
            .boxed()
        } else {
            container(
                text(t!(i18n, ids::FIND_REPLACE_CURRENT))
                    .size(TS.body)
                    .color(text_secondary),
            )
            .padding([6, 10])
            .boxed()
        };

    let replace_all: Element<'a, FindReplaceMsg> =
        if state.replace_mode && state.result_count() > 0 && !state.is_busy() {
            button(
                text(t!(i18n, ids::FIND_REPLACE_ALL))
                    .size(TS.body)
                    .color(text_primary),
            )
            .on_press(FindReplaceMsg::ReplaceAll)
            .style(crate::styles::menu_item(tokens))
            .boxed()
        } else {
            container(
                text(t!(i18n, ids::FIND_REPLACE_ALL))
                    .size(TS.body)
                    .color(text_secondary),
            )
            .padding([6, 10])
            .boxed()
        };

    let targets = if state.board.is_some() {
        text(t!(i18n, ids::BOARD_FIND_TARGETS))
    } else {
        text(t!(i18n, ids::FIND_TARGETS))
    };
    let footer = row![
        container(targets.size(TS.caption).color(text_secondary)),
        Space::new().width(Length::Fill),
        replace_current,
        replace_all,
    ]
    .align_y(iced::Alignment::Center)
    .spacing(SP.m);

    let header: Element<'a, FindReplaceMsg> = if windowed {
        Space::new().height(0).boxed()
    } else {
        header
    };
    let (dialog_w, dialog_h) = if windowed {
        (Length::Fill, Length::Fill)
    } else {
        (Length::Fixed(DIALOG_W), Length::Fixed(DIALOG_H))
    };
    let dialog = container(
        column![
            header,
            container(form).padding([SP.l, 14.0]),
            container(scrollable(column(results).spacing(SP.s)).height(Length::Fill))
                .padding([10, 10]),
            container(footer).padding([10, 14]),
        ]
        .width(dialog_w)
        .height(dialog_h),
    )
    .style(crate::styles::modal_card(tokens));
    if windowed {
        return dialog.boxed();
    }

    container(
        column![
            Space::new().height(Length::Fill),
            row![
                Space::new().width(Length::Fill),
                dialog,
                Space::new().width(Length::Fill),
            ],
            Space::new().height(Length::Fill),
        ]
        .width(Length::Fill)
        .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(crate::app::view::dialogs::scrim_style(tokens))
    .boxed()
}

/// The line under the form: a replace or scan in progress, else the count.
fn count_line(i18n: &I18n, replacing: bool, scanning: bool, matches: usize) -> Localized {
    if replacing {
        t!(i18n, ids::FIND_REPLACING)
    } else if scanning {
        t!(i18n, ids::FIND_SEARCHING)
    } else {
        t!(i18n, ids::FIND_MATCHES, "count" => matches)
    }
}

#[cfg(test)]
mod i18n_tests {
    use signex_i18n::{Arg, I18n, LanguagePref, ids};

    use super::count_line;

    fn matches(i18n: &I18n) -> String {
        i18n.message_plain(ids::FIND_MATCHES, &[("count", Arg::from(3_usize))])
    }

    fn line(i18n: &I18n, replacing: bool, scanning: bool, matches: usize) -> String {
        count_line(i18n, replacing, scanning, matches)
            .into_string()
            .replace(['\u{2068}', '\u{2069}'], "")
    }

    /// #2058: the line under the form keeps its English, follows the
    /// language and picks the replace and scan states over the count.
    #[test]
    fn the_count_line_keeps_its_english() {
        let en = I18n::en_us();
        assert_eq!(line(&en, false, false, 0), "0 match(es)");
        assert_eq!(line(&en, false, false, 1), "1 match(es)");
        assert_eq!(line(&en, false, false, 3), "3 match(es)");
        assert_eq!(line(&en, false, true, 3), "Searching...");
        assert_eq!(line(&en, true, true, 3), "Replacing...");
        let xa = I18n::new(&LanguagePref::Locale("en-XA".to_owned()));
        assert_ne!(line(&xa, false, false, 3), "3 match(es)");
        assert_eq!(line(&xa, false, false, 3), matches(&xa));
    }

    /// #2060: every hit kind reads as the engine's `label` and follows the
    /// language.
    #[test]
    fn the_hit_kinds_keep_their_english() {
        use signex_engine::FindKind;
        use signex_engine::pcb::find::BoardFindKind;
        let en = I18n::en_us();
        let xa = I18n::new(&LanguagePref::Locale("en-XA".to_owned()));
        let check = |id, english: &str| {
            assert_eq!(en.message_plain(id, &[]), english);
            assert_ne!(xa.message_plain(id, &[]), english);
        };
        for kind in [
            FindKind::NetLabel,
            FindKind::TextNote,
            FindKind::Designator,
            FindKind::Value,
        ] {
            check(super::find_kind_label(kind), kind.label());
        }
        for kind in [
            BoardFindKind::Designator,
            BoardFindKind::Value,
            BoardFindKind::Footprint,
            BoardFindKind::Net,
        ] {
            check(super::board_find_kind_label(kind), kind.label());
        }
    }

    /// #2058: the find, find-similar and paste-array texts keep their
    /// English and follow the language in force.
    #[test]
    fn the_find_texts_follow_the_language() {
        let en = I18n::en_us();
        assert_eq!(matches(&en), "3 match(es)");
        assert_eq!(
            en.message_plain(ids::PASTE_ARRAY_X_SPACING, &[("unit", Arg::from("mm"))]),
            "X spacing (mm)"
        );
        assert_eq!(
            en.message_plain(ids::FIND_SIMILAR_SEED, &[("seed", Arg::from("Symbol R1"))]),
            "Seed: Symbol R1"
        );
        let xa = I18n::new(&LanguagePref::Locale("en-XA".to_owned()));
        assert_ne!(matches(&xa), matches(&en));
        assert_ne!(
            xa.message_plain(ids::FIND_SIMILAR_TITLE, &[]),
            "Find Similar Objects"
        );
    }
}
