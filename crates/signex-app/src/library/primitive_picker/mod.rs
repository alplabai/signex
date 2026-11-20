//! Primitive picker modal (mockup row 131): Pick Symbol / Pick Footprint /
//! Pick Sim Model, listed from the mounted libraries' cached summaries. A
//! symbol outside the row's library opens the copy confirm in place of the
//! list. See docs/architecture/library-dialogs.md, "Primitive picker".

mod copy_confirm;
mod list;

use iced::widget::text::{Ellipsis, Wrapping};
use iced::widget::{Space, column, container, row, scrollable};
use iced::{Background, Border, Element, Length, Padding, Theme, Widget};
use signex_i18n::{I18n, UserText, ids, t};
use signex_library::PrimitiveKind;
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;
use signex_widgets::{Label, button, text};

use super::messages::{LibraryMessage, PrimitivePickerMsg};
use super::modal_chrome::{cancel_btn, footer_strip, header_with_note_closing, notice_banner};
use super::picker::{empty_card, mono, search_field};
use super::state::{LibraryState, PrimitivePickerState};
use crate::styles::{DialogButtonKind, PillTone, Radii, dialog_button};
use list::{Choice, Group, ListItem, list_items, list_window};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

const PICKER_W: f32 = 640.0;
const PICKER_H: f32 = 460.0;
/// Height every list entry (group head or primitive row) is pinned to, so
/// the window maths needs only the scroll offset (`.lst .li` in the design).
const ROW_H: f32 = 28.0;
/// Height of a group's count pill.
const PILL_H: f32 = 16.0;

fn msg(m: PrimitivePickerMsg) -> LibraryMessage {
    LibraryMessage::PrimitivePicker(m)
}

/// Title of the picker for `kind`; the native window title of a detached
/// picker reads the same.
pub(crate) fn title(kind: PrimitiveKind, i18n: &I18n) -> String {
    let id = match kind {
        PrimitiveKind::Symbol => ids::LIBRARY_PRIMITIVE_PICKER_TITLE_SYMBOL,
        PrimitiveKind::Footprint => ids::LIBRARY_PRIMITIVE_PICKER_TITLE_FOOTPRINT,
        PrimitiveKind::Sim => ids::LIBRARY_PRIMITIVE_PICKER_TITLE_SIM,
        _ => ids::LIBRARY_PRIMITIVE_PICKER_TITLE_PRIMITIVE,
    };
    i18n.message_plain(id, &[])
}

/// The internal PN of the library row the pick is for, when its library is
/// open; `None` for a pick that writes no row.
fn target_pn(state: &LibraryState, picker: &PrimitivePickerState) -> Option<String> {
    let address = picker.target.address()?;
    let row_id = address.row_id.as_uuid();
    state
        .library_at(&address.library_path)?
        .cached_components
        .iter()
        .find(|c| c.row_id == row_id)
        .map(|c| c.internal_pn.as_str().to_owned())
}

pub fn view<'a>(
    state: &'a LibraryState,
    picker: &'a PrimitivePickerState,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
    windowed: bool,
) -> Element<'a, LibraryMessage> {
    // The copy confirm takes the picker's place, as Change Component's
    // preview does in the component picker.
    if let Some(symbol) = picker.copy_confirm.as_ref() {
        return copy_confirm::view(state, picker, symbol, tokens, i18n, windowed);
    }
    let r = crate::styles::tokens::resolve(tokens);

    let note = target_pn(state, picker)
        .map(|pn| t!(i18n, ids::LIBRARY_PRIMITIVE_PICKER_FOR, "pn" => pn).into_string());
    let header = header_with_note_closing(
        &title(picker.kind, i18n),
        note,
        msg(PrimitivePickerMsg::Cancel),
        windowed,
        tokens,
    );

    let search = container(search_field(
        t!(i18n, ids::LIBRARY_PRIMITIVE_PICKER_FILTER),
        &picker.filter,
        |s| msg(PrimitivePickerMsg::SetFilter(s)),
        None,
        tokens,
    ))
    .padding(Padding {
        top: SP.m + SP.xs,
        right: SP.xl,
        bottom: SP.s + SP.xs,
        left: SP.xl,
    });

    let needle = i18n.fold_for_search(picker.filter.trim());
    let items = list_items(i18n, state, picker, &needle);
    let list: Element<'a, LibraryMessage> = if items.is_empty() {
        let id = if state.open_libraries.is_empty() {
            ids::LIBRARY_PRIMITIVE_PICKER_NO_LIBRARIES
        } else {
            ids::LIBRARY_PRIMITIVE_PICKER_NO_MATCH
        };
        empty_card(t!(i18n, id), tokens)
    } else {
        rows_view(&items, picker.scroll_y, tokens, i18n)
    };
    let rule = r.line_soft;
    let list = container(list)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_: &Theme| container::Style {
            border: Border::default().top(iced::border::Side::default().width(1.0).color(rule)),
            ..container::Style::default()
        })
        .boxed();
    let mut body = column![list]
        .spacing(0)
        .width(Length::Fill)
        .height(Length::Fill);
    if let Some(err) = picker.error.as_ref() {
        let message = text(UserText::new(err.as_str()))
            .size(TS.body)
            .width(Length::Fill)
            .boxed();
        body = body.push(
            container(notice_banner(PillTone::Err, message, Vec::new(), tokens))
                .padding([SP.s, SP.xl])
                .boxed(),
        );
    }

    let footer = footer_strip(
        vec![dialog_button(
            t!(i18n, ids::LIBRARY_PRIMITIVE_PICKER_BROWSE),
            DialogButtonKind::Ghost,
            Some(msg(PrimitivePickerMsg::Browse)),
            tokens,
        )],
        vec![cancel_btn(
            t!(i18n, ids::DIALOG_BUTTON_CANCEL),
            msg(PrimitivePickerMsg::Cancel),
            tokens,
        )],
        tokens,
    );

    container(
        column![header, search, body, footer]
            .width(super::card_len(windowed, PICKER_W))
            .height(super::card_len(windowed, PICKER_H)),
    )
    .style(crate::styles::modal_card(tokens))
    .clip(true)
    .boxed()
}

/// The virtualised list: only the entries in view (plus overscan) become
/// widgets; two spacers of the skipped height keep the scrollbar and the
/// total height unchanged.
fn rows_view<'a>(
    items: &[ListItem],
    scroll_y: f32,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let window = list_window(scroll_y, items.len());
    let mut list_col: iced::widget::Column<Element<'a, LibraryMessage>> = column![].spacing(0);
    if window.start > 0 {
        list_col = list_col.push(Space::new().height(window.start as f32 * ROW_H).boxed());
    }
    for item in &items[window.clone()] {
        list_col = list_col.push(match item {
            ListItem::Header { group, count } => group_head(group, *count, tokens, i18n),
            ListItem::Row { file, name, choice } => {
                let label = row_label(i18n, file, name.as_deref());
                let (on_press, meta) = match choice {
                    Choice::Pick(reference) => (msg(PrimitivePickerMsg::Pick(*reference)), None),
                    // A symbol of a library that is not open: choosing it
                    // offers to link it or copy it.
                    Choice::Loose(symbol) => (
                        msg(PrimitivePickerMsg::PickLoose(symbol.clone())),
                        symbol
                            .library
                            .is_some()
                            .then(|| t!(i18n, ids::LIBRARY_PRIMITIVE_PICKER_OPEN_LIBRARY)),
                    ),
                };
                primitive_row(label, meta, on_press, tokens)
            }
        });
    }
    if window.end < items.len() {
        list_col = list_col.push(
            Space::new()
                .height((items.len() - window.end) as f32 * ROW_H)
                .boxed(),
        );
    }
    scrollable(container(list_col).padding([SP.xs, SP.m]))
        .on_scroll(|scroll| {
            msg(PrimitivePickerMsg::Scrolled(
                scroll.viewport.absolute_offset().y,
            ))
        })
        .height(Length::Fill)
        .width(Length::Fill)
        .boxed()
}

/// A row's words: "file · name", or the file's path alone.
fn row_label(i18n: &I18n, file: &str, name: Option<&str>) -> Label<'static> {
    match name {
        Some(name) => t!(
            i18n,
            ids::LIBRARY_PRIMITIVE_PICKER_ROW,
            "file" => file,
            "name" => name
        )
        .into(),
        None => UserText::new(file).into(),
    }
}

/// A group's words, before the head upper-cases them.
fn group_title(i18n: &I18n, group: &Group) -> String {
    match group {
        Group::Suggested => i18n.message_plain(ids::LIBRARY_PRIMITIVE_PICKER_GROUP_SUGGESTED, &[]),
        Group::Library(name) => name.clone(),
        Group::NotOpen(library) => i18n.message_plain(
            ids::LIBRARY_PRIMITIVE_PICKER_GROUP_NOT_OPEN,
            &[("library", library.as_str().into())],
        ),
        Group::NoLibrary => i18n.message_plain(ids::LIBRARY_PRIMITIVE_PICKER_GROUP_NO_LIBRARY, &[]),
    }
}

/// A group head (`.gh`): the section ink, upper-cased for the message
/// language, then the count in a neutral pill.
fn group_head<'a>(
    group: &Group,
    count: usize,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let ink = crate::styles::tokens::resolve(tokens).ink3;
    // Catalog text (or a library's name) already rendered, upper-cased for
    // the message language.
    let words = text(UserText::new(
        i18n.upper_for_display(&group_title(i18n, group)),
    ))
    .size(TS.section)
    .color(ink)
    .wrapping(Wrapping::None)
    .ellipsis(Ellipsis::End);
    let pill = container(
        // A bare count, grouped for the formatting locale.
        text(UserText::new(i18n.count(count as i64)))
            .size(TS.caption)
            .wrapping(Wrapping::None),
    )
    .padding([0.0, SP.s + SP.xs])
    .height(PILL_H)
    .align_y(iced::alignment::Vertical::Center)
    .style(crate::styles::pill(tokens, PillTone::Neutral));
    container(
        row![words, pill]
            .spacing(SP.s + SP.xs)
            .align_y(iced::Alignment::Center),
    )
    .padding(Padding {
        top: SP.m,
        right: SP.m,
        bottom: SP.xs,
        left: SP.m,
    })
    .width(Length::Fill)
    .height(ROW_H)
    .align_y(iced::alignment::Vertical::Bottom)
    .boxed()
}

/// One primitive (`.li`): its words in mono, then an optional muted meta at
/// the right end. A click picks at once.
fn primitive_row<'a>(
    label: Label<'static>,
    meta: Option<signex_i18n::Localized>,
    on_press: LibraryMessage,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let r = crate::styles::tokens::resolve(tokens);
    let ink = theme_ext::text_primary(tokens);
    let mut line = row![
        text(label)
            .size(TS.body)
            .font(mono())
            .color(ink)
            .wrapping(Wrapping::None)
            .ellipsis(Ellipsis::End)
            .width(Length::Fill)
    ]
    .spacing(SP.m)
    .padding([0.0, SP.m])
    .height(Length::Fill)
    .align_y(iced::Alignment::Center);
    if let Some(meta) = meta {
        line = line.push(
            text(meta)
                .size(TS.caption)
                .color(r.ink4)
                .wrapping(Wrapping::None)
                .boxed(),
        );
    }
    let hover = crate::styles::tokens::hover_overlay(tokens);
    button(line)
        .padding(0)
        .width(Length::Fill)
        .height(ROW_H)
        .on_press(on_press)
        .style(move |_: &Theme, status: iced::widget::button::Status| {
            let background = matches!(status, iced::widget::button::Status::Hovered)
                .then_some(Background::Color(hover));
            iced::widget::button::Style {
                background,
                border: Border {
                    radius: Radii::GRAPHITE.small.into(),
                    ..Border::default()
                },
                text_color: ink,
                ..iced::widget::button::Style::default()
            }
        })
        .boxed()
}

/// The group heads the list shows for `picker`, each with its count, for
/// tests: "In library x (not open) (1)".
#[cfg(test)]
pub(crate) fn header_labels(state: &LibraryState, picker: &PrimitivePickerState) -> Vec<String> {
    let i18n = I18n::en_us_ref();
    list_items(i18n, state, picker, "")
        .into_iter()
        .filter_map(|item| match item {
            ListItem::Header { group, count } => {
                Some(format!("{} ({count})", group_title(i18n, &group)))
            }
            ListItem::Row { .. } => None,
        })
        .collect()
}

/// Every listed row as `(group head, row words)`, for tests:
/// `("In library x (not open)", "Held.snxsym · HELD")`.
#[cfg(test)]
pub(crate) fn row_labels(
    state: &LibraryState,
    picker: &PrimitivePickerState,
) -> Vec<(String, String)> {
    let i18n = I18n::en_us_ref();
    let mut group = String::new();
    let mut rows = Vec::new();
    for item in list_items(i18n, state, picker, "") {
        match item {
            ListItem::Header { group: g, .. } => group = group_title(i18n, &g),
            ListItem::Row { file, name, .. } => {
                let words = match name {
                    Some(name) => i18n.message_plain(
                        ids::LIBRARY_PRIMITIVE_PICKER_ROW,
                        &[("file", file.into()), ("name", name.into())],
                    ),
                    None => file,
                };
                rows.push((group.clone(), words));
            }
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::list::{contains_ci_in, list_window};
    use super::*;
    use crate::library::browser::OVERSCAN_ROWS;
    use signex_types::theme::ThemeId;

    /// [`contains_ci_in`] under en-US.
    fn contains_ci(hay: &str, needle: &str) -> bool {
        contains_ci_in(I18n::en_us_ref(), hay, needle)
    }

    #[test]
    fn contains_ci_matches_case_insensitively() {
        assert!(contains_ci("LM317_Adj", "lm317"));
        assert!(contains_ci("Résistor", "résis"));
        assert!(contains_ci("anything", ""));
        assert!(!contains_ci("LM317", "lm7"));
        assert!(!contains_ci("ab", "abc"));
    }

    #[test]
    fn the_primitive_filter_follows_the_search_fold() {
        let tr = I18n::with_message_locale_for_test("tr");
        let en = I18n::en_us();
        assert!(contains_ci_in(&en, "STRAßE", "strasse"));
        assert!(contains_ci_in(&tr, "İSTANBUL", "istanbul"));
        assert!(contains_ci_in(&tr, "ISPARTA", "ısparta"));
        assert!(!contains_ci_in(&tr, "ISPARTA", "isparta"));
        assert!(contains_ci_in(&en, "ISPARTA", "isparta"));
    }

    fn max_built() -> usize {
        (PICKER_H / ROW_H).ceil() as usize + 1 + 2 * OVERSCAN_ROWS
    }

    #[test]
    fn huge_list_builds_only_visible_plus_overscan() {
        let w = list_window(0.0, 10_000);
        assert_eq!(w.start, 0);
        assert!(w.len() <= max_built(), "built {}", w.len());
        let mid = list_window(5_000.0 * ROW_H, 10_000);
        assert!(mid.len() <= max_built() && mid.start > 0 && mid.end < 10_000);
    }

    #[test]
    fn stale_offset_after_filter_shrink_is_clamped() {
        // Scrolled deep into a 10k list, then a filter leaves 100 entries.
        let w = list_window(9_000.0 * ROW_H, 100);
        assert!(!w.is_empty() && w.end == 100);
        assert!(list_window(1.0e9, 0).is_empty());
    }

    #[test]
    fn short_list_is_built_whole() {
        assert_eq!(list_window(0.0, 5), 0..5);
    }

    /// The titles read the catalog.
    #[test]
    fn titles_come_from_the_catalog() {
        let i18n = I18n::en_us_ref();
        assert_eq!(title(PrimitiveKind::Symbol, i18n), "Pick Symbol");
        assert_eq!(title(PrimitiveKind::Footprint, i18n), "Pick Footprint");
        assert_eq!(title(PrimitiveKind::Sim, i18n), "Pick Sim Model");
    }

    /// A row reads "file · name", or the path alone; a not-open library's
    /// group says so.
    #[test]
    fn row_words_join_the_file_and_the_name() {
        let i18n = I18n::en_us_ref();
        let plain = |l: Label<'_>| l.as_str().replace(['\u{2068}', '\u{2069}'], "");
        assert_eq!(
            plain(row_label(i18n, "resistor.snxsym", Some("R"))),
            "resistor.snxsym · R"
        );
        assert_eq!(
            plain(row_label(i18n, "/work/old/res_trim.snxsym", None)),
            "/work/old/res_trim.snxsym"
        );
        assert_eq!(
            group_title(i18n, &Group::NotOpen("power-ics".into())),
            "In library power-ics (not open)"
        );
    }

    /// The group heads and the rows build from tokens on every built-in
    /// theme (Graphite Light and Dark included).
    #[test]
    fn the_list_parts_build_for_every_theme() {
        let i18n = I18n::en_us_ref();
        for id in ThemeId::BUILTINS {
            let tokens = signex_types::theme::theme_tokens(*id);
            for group in [
                Group::Suggested,
                Group::Library("passives".into()),
                Group::NotOpen("power-ics".into()),
                Group::NoLibrary,
            ] {
                let _ = group_head(&group, 3, &tokens, i18n);
            }
            for meta in [
                None,
                Some(t!(i18n, ids::LIBRARY_PRIMITIVE_PICKER_OPEN_LIBRARY)),
            ] {
                let _ = primitive_row(
                    row_label(i18n, "resistor.snxsym", Some("R")),
                    meta,
                    msg(PrimitivePickerMsg::Cancel),
                    &tokens,
                );
            }
        }
    }
}
