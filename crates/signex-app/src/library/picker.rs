//! Component picker modal (#129).
//!
//! Opened from File ▸ Library ▸ Place Component…, Place ▸ Component… and the
//! Active Bar's Part button; Change Component and Open Datasheet reuse it.
//!
//! Shape:
//!
//! ```text
//! ┌─[Place Component   3 libraries · 359 parts ─────────────────── X]─┐
//! │ [⌕ Search internal PN / MPN / description…]  [Add range filter…]  │
//! ├────────────────────────────────────────────────────────────────────┤
//! │ INTERNAL PN      STATE        MPN               DESCRIPTION        │
//! │ R-0402-10K-1%    ● Released   RC0402FR-0710KL   10 kΩ ±1% 0402     │
//! │ …                                                                  │
//! ├────────────────────────────────────────────────────────────────────┤
//! │ Enter to place                              [ Cancel ] [ Place ]   │
//! └────────────────────────────────────────────────────────────────────┘
//! ```

use iced::widget::{Space, column, container, row, scrollable};
use iced::{Background, Border, Color, Element, Length, Theme, Widget};
use signex_i18n::{I18n, Localized, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;
use signex_widgets::{button, text, text_input};

use super::messages::{LibraryMessage, PickerMsg, PlacedPartMsg};
use super::modal_chrome::{cancel_btn, footer_strip, header_with_note_closing, primary_btn_if};
use super::placed_part::{change_preview_view, datasheet_choice_view};
use super::range_filter::range_filter_bar;
use super::state::{PickerEntry, PickerMode, PickerState};
use crate::library::editor::kit::{lifecycle_text, lifecycle_tone};
use crate::styles::{ChromeMetrics, Radii};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;
const METRICS: ChromeMetrics = ChromeMetrics::DEFAULT;

/// The later stages' card headers (`placed_part::view`) name the dialog with
/// these; they equal the catalog's English (`titles_match_the_catalog`).
pub(crate) const TITLE_CHANGE_COMPONENT: &str = "Change Component";
pub(crate) const TITLE_OPEN_DATASHEET: &str = "Open Datasheet";

/// Title of the picker modal's current stage; the native window title of
/// a detached picker reads the same.
pub(crate) fn title(mode: &PickerMode, i18n: &I18n) -> String {
    let id = match mode {
        PickerMode::Place => ids::LIBRARY_PICKER_TITLE_PLACE,
        PickerMode::ChangeComponent(_) | PickerMode::ChangePreview(_) => {
            ids::LIBRARY_PICKER_TITLE_CHANGE
        }
        PickerMode::Datasheets(_) => ids::LIBRARY_PICKER_TITLE_DATASHEET,
    };
    i18n.message_plain(id, &[])
}

const PICKER_W: f32 = 720.0;
const PICKER_H: f32 = 480.0;
/// Width of the State column: the widest state word and its dot.
const STATE_W: f32 = 104.0;
/// Height of a part row (`.tbl td` in the design).
const ROW_H: f32 = 26.0;
/// Diameter of a lifecycle state's dot.
const DOT: f32 = 6.0;
/// Width of the selected row's leading accent bar.
const SELECTED_BAR: f32 = 2.0;

/// Draws the list `LibraryState::sync_view_caches_in` last built for the
/// picker; `view` itself filters and clones nothing.
pub fn view<'a>(
    picker: &'a PickerState,
    tokens: &'a ThemeTokens,
    i18n: &'a I18n,
    windowed: bool,
) -> Element<'a, LibraryMessage> {
    // Change Component and Open Datasheet reuse this modal: their later
    // stages draw their own card.
    match &picker.mode {
        PickerMode::ChangePreview(change) => {
            return change_preview_view(change, tokens, i18n, windowed);
        }
        PickerMode::Datasheets(choice) => {
            return datasheet_choice_view(choice, tokens, i18n, windowed);
        }
        PickerMode::Place | PickerMode::ChangeComponent(_) => {}
    }
    let changing = matches!(picker.mode, PickerMode::ChangeComponent(_));
    // Place in the Place stage; Next (on to the pin preview) when changing.
    let commit = if changing {
        LibraryMessage::PlacedPart(PlacedPartMsg::ChangePartPicked)
    } else {
        LibraryMessage::Picker(PickerMsg::PlaceSelected)
    };
    let selected = picker.selected.is_some();
    let rows = &picker.rows;
    let r = crate::styles::tokens::resolve(tokens);

    let note = t!(
        i18n,
        ids::LIBRARY_PICKER_SCOPE,
        "libraries" => rows.library_count(),
        "parts" => rows.part_count()
    )
    .into_string();
    let header = header_with_note_closing(
        &title(&picker.mode, i18n),
        Some(note),
        LibraryMessage::ClosePicker,
        windowed,
        tokens,
    );

    // Enter in the search field commits the selected row (#129).
    let search = search_field(
        t!(i18n, ids::LIBRARY_PICKER_SEARCH),
        &picker.filter,
        |s| LibraryMessage::Picker(PickerMsg::FilterChanged(s)),
        selected.then(|| commit.clone()),
        tokens,
    );
    // Range filters on the measurement columns (#1572): "Add range filter…"
    // beside the search; active filters stack in the same column.
    let range_bar = range_filter_bar(
        &picker.range_filters,
        rows.filter_columns(),
        rows.range_excluded(),
        tokens,
        i18n,
        |msg| LibraryMessage::Picker(PickerMsg::RangeFilter(msg)),
    );
    let mut top = row![search].spacing(SP.m).align_y(iced::Alignment::Start);
    if let Some(bar) = range_bar {
        top = top.push(bar);
    }
    let top = container(top).padding(iced::Padding {
        top: SP.m + SP.xs,
        right: SP.xl,
        bottom: SP.s + SP.xs,
        left: SP.xl,
    });

    let list: Element<'a, LibraryMessage> = if rows.is_empty() {
        empty_card(t!(i18n, ids::LIBRARY_PICKER_EMPTY), tokens)
    } else {
        let mut list_col = column![].spacing(0);
        for entry in rows.visible() {
            let is_selected = picker.selected.as_ref().is_some_and(|(p, c)| {
                p.as_path() == &*entry.path && c.row_id == entry.summary.row_id
            });
            list_col = list_col.push(part_row(entry, is_selected, i18n, tokens));
        }
        scrollable(list_col)
            .height(Length::Fill)
            .width(Length::Fill)
            .boxed()
    };
    let rule = r.line_soft;
    let table = container(column![table_head(i18n, tokens), list].spacing(0))
        .height(Length::Fill)
        .style(move |_: &Theme| container::Style {
            border: Border::default().top(iced::border::Side::default().width(1.0).color(rule)),
            ..container::Style::default()
        });

    // Only the key that works is named: Enter in the search field.
    let hint = text(t!(
        i18n,
        if changing {
            ids::LIBRARY_PICKER_HINT_NEXT
        } else {
            ids::LIBRARY_PICKER_HINT_PLACE
        }
    ))
    .size(TS.caption)
    .color(r.ink4)
    .boxed();
    let footer = footer_strip(
        vec![hint],
        vec![
            cancel_btn(
                t!(i18n, ids::DIALOG_BUTTON_CANCEL),
                LibraryMessage::ClosePicker,
                tokens,
            ),
            primary_btn_if(
                t!(
                    i18n,
                    if changing {
                        ids::LIBRARY_PICKER_NEXT
                    } else {
                        ids::DIALOG_BUTTON_PLACE
                    }
                ),
                commit,
                selected,
                tokens,
            ),
        ],
        tokens,
    );

    let body = container(column![top, table].spacing(0))
        .width(Length::Fill)
        .height(Length::Fill);

    container(
        column![header, body, footer]
            .width(super::card_len(windowed, PICKER_W))
            .height(super::card_len(windowed, PICKER_H)),
    )
    .style(crate::styles::modal_card(tokens))
    .clip(true)
    .boxed()
}

/// The chrome mono face of part numbers and file names.
pub(crate) fn mono() -> iced::Font {
    crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
}

/// A dialog's search / filter field: the search glyph, then the input, on
/// the field frame. Shared by the component and primitive pickers.
pub(crate) fn search_field<'a, M: Clone + 'a>(
    placeholder: Localized,
    value: &str,
    on_input: impl Fn(String) -> M + 'a,
    on_submit: Option<M>,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    let r = crate::styles::tokens::resolve(tokens);
    let (ink, placeholder_ink, glyph_ink) = (crate::styles::ti(tokens.text), r.ink4, r.ink4);
    let selection = Color {
        a: 0.4,
        ..crate::styles::ti(tokens.accent)
    };
    let input = text_input(placeholder, value)
        .on_input(on_input)
        .on_submit_maybe(on_submit)
        .padding([SP.xs, SP.s])
        .size(TS.body)
        .width(Length::Fill)
        .style(move |_: &Theme, _| iced::widget::text_input::Style {
            background: Background::Color(Color::TRANSPARENT),
            border: Border::default(),
            placeholder: placeholder_ink,
            value: ink,
            selection,
        });
    let glyph = iced::widget::svg(crate::icons::icon_chrome_search(
        signex_types::theme::ThemeId::GraphiteDark,
    ))
    .width(TS.body)
    .height(TS.body)
    .style(move |_: &Theme, _| iced::widget::svg::Style {
        color: Some(glyph_ink),
        ..Default::default()
    });
    container(
        row![glyph, input]
            .spacing(SP.s)
            .align_y(iced::Alignment::Center),
    )
    .padding([0.0, SP.m])
    .width(Length::Fill)
    .center_y(Length::Fixed(METRICS.field))
    .style(crate::styles::field(tokens, false))
    .boxed()
}

/// The centred empty-result card (`.empty`): muted text in a hairline frame.
/// iced draws no dashed border, so the frame is solid in the strong line ink.
pub(crate) fn empty_card<'a, M: 'a>(message: Localized, tokens: &ThemeTokens) -> Element<'a, M> {
    let r = crate::styles::tokens::resolve(tokens);
    let (ink, line) = (r.ink3, r.line_strong);
    container(
        container(text(message).size(TS.body).color(ink))
            .padding([SP.xl, SP.l])
            .center_x(Length::Fill)
            .style(move |_: &Theme| container::Style {
                border: Border {
                    width: 1.0,
                    color: line,
                    radius: Radii::GRAPHITE.card.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            }),
    )
    .padding(SP.m)
    .width(Length::Fill)
    .boxed()
}

/// One column's width; the head and the rows share them so they line up.
fn cell_width(column: usize) -> Length {
    match column {
        0 => Length::FillPortion(2),
        1 => Length::Fixed(STATE_W),
        2 => Length::FillPortion(3),
        _ => Length::FillPortion(4),
    }
}

/// The column heads: caption size, uppercase in the message language, the
/// muted section ink on the sunken fill over a hairline.
fn table_head<'a>(i18n: &I18n, tokens: &ThemeTokens) -> Element<'a, LibraryMessage> {
    let r = crate::styles::tokens::resolve(tokens);
    let mut line = row![].spacing(0).align_y(iced::Alignment::Center);
    for (column, id) in [
        ids::LIBRARY_PICKER_COLUMN_INTERNAL_PN,
        ids::LIBRARY_PICKER_COLUMN_STATE,
        ids::LIBRARY_PICKER_COLUMN_MPN,
        ids::LIBRARY_PICKER_COLUMN_DESCRIPTION,
    ]
    .into_iter()
    .enumerate()
    {
        let words = t!(i18n, id).into_string();
        // Catalog text already rendered, upper-cased for the message language.
        let head = text(UserText::new(i18n.upper_for_display(&words)))
            .size(TS.caption)
            .color(r.ink3);
        line = line.push(
            container(head)
                .padding([0.0, SP.m])
                .width(cell_width(column))
                .boxed(),
        );
    }
    let (fill, rule) = (r.sunken, r.line_soft);
    container(line)
        .width(Length::Fill)
        .center_y(Length::Fixed(METRICS.row))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border::default().bottom(iced::border::Side::default().width(1.0).color(rule)),
            ..container::Style::default()
        })
        .boxed()
}

/// A lifecycle state as the design's `.lc`: a dot and the word, both in the
/// state's tone.
fn lifecycle_cell<'a>(
    state: signex_library::LifecycleState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let (_, ink) = crate::styles::pill_colors(tokens, lifecycle_tone(state));
    let dot = container(Space::new())
        .width(DOT)
        .height(DOT)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(ink)),
            border: Border {
                radius: (DOT / 2.0).into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    let word = text(lifecycle_text(i18n, state)).size(TS.body).color(ink);
    row![dot, word]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center)
        .boxed()
}

/// One part: PN and MPN in mono, the state, the description muted. The
/// selected row takes the selection tint and a leading accent bar.
fn part_row<'a>(
    entry: &'a PickerEntry,
    is_selected: bool,
    i18n: &I18n,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let summary = &entry.summary;
    let r = crate::styles::tokens::resolve(tokens);
    let (ink, muted) = (theme_ext::text_primary(tokens), r.ink4);
    let cell = |content: Element<'a, LibraryMessage>, column: usize| {
        container(content)
            .padding([0.0, SP.m])
            .width(cell_width(column))
            .clip(true)
    };
    let pn = text(UserText::new(summary.internal_pn.as_str()))
        .size(TS.body)
        .font(mono())
        .color(ink)
        .boxed();
    let mpn = text(UserText::new(summary.mpn.as_str()))
        .size(TS.body)
        .font(mono())
        .color(ink)
        .boxed();
    let description = text(UserText::new(summary.description.as_str()))
        .size(TS.body)
        .color(muted)
        .boxed();
    let line = row![
        cell(pn, 0),
        cell(lifecycle_cell(summary.state, i18n, tokens), 1),
        cell(mpn, 2),
        cell(description, 3),
    ]
    .spacing(0)
    .height(Length::Fill)
    .align_y(iced::Alignment::Center);
    let (tint, hover, rule, bar) = (
        crate::styles::tokens::selection_tint(tokens),
        crate::styles::tokens::hover_overlay(tokens),
        r.line_soft,
        crate::styles::ti(tokens.accent),
    );
    button(line)
        .padding(0)
        .width(Length::Fill)
        .height(Length::Fixed(ROW_H))
        .on_press(LibraryMessage::Picker(PickerMsg::SelectComponent(
            summary.row_id,
        )))
        .style(move |_: &Theme, status: iced::widget::button::Status| {
            let background = match (is_selected, status) {
                (true, _) => Some(Background::Color(tint)),
                (false, iced::widget::button::Status::Hovered) => Some(Background::Color(hover)),
                _ => None,
            };
            let mut border =
                Border::default().bottom(iced::border::Side::default().width(1.0).color(rule));
            if is_selected {
                border = border.left(iced::border::Side::default().width(SELECTED_BAR).color(bar));
            }
            iced::widget::button::Style {
                background,
                border,
                text_color: ink,
                ..iced::widget::button::Style::default()
            }
        })
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_i18n::Arg;
    use signex_types::theme::ThemeId;

    /// The later stages' headers read the constants; the window title and
    /// the list stage read the catalog. They must say the same.
    #[test]
    fn titles_match_the_catalog() {
        let i18n = I18n::en_us_ref();
        assert_eq!(
            i18n.message_plain(ids::LIBRARY_PICKER_TITLE_CHANGE, &[]),
            TITLE_CHANGE_COMPONENT
        );
        assert_eq!(
            i18n.message_plain(ids::LIBRARY_PICKER_TITLE_DATASHEET, &[]),
            TITLE_OPEN_DATASHEET
        );
        assert_eq!(title(&PickerMode::Place, i18n), "Place Component");
    }

    /// The header note counts libraries and parts with plural forms.
    #[test]
    fn the_scope_note_uses_plural_forms() {
        let i18n = I18n::en_us_ref();
        let note = |libraries: usize, parts: usize| {
            i18n.message_plain(
                ids::LIBRARY_PICKER_SCOPE,
                &[
                    ("libraries", Arg::from(libraries)),
                    ("parts", Arg::from(parts)),
                ],
            )
        };
        assert_eq!(note(3, 359), "3 libraries · 359 parts");
        assert_eq!(note(1, 1), "1 library · 1 part");
    }

    /// The table head, the empty card, the state cells and the search field
    /// build from tokens on every built-in theme (Graphite Light and Dark
    /// included).
    #[test]
    fn the_table_builds_for_every_theme() {
        let i18n = I18n::en_us_ref();
        for id in ThemeId::BUILTINS {
            let tokens = signex_types::theme::theme_tokens(*id);
            let _ = table_head(i18n, &tokens);
            let _ = empty_card::<LibraryMessage>(t!(i18n, ids::LIBRARY_PICKER_EMPTY), &tokens);
            for state in [
                signex_library::LifecycleState::Draft,
                signex_library::LifecycleState::InReview,
                signex_library::LifecycleState::Released,
                signex_library::LifecycleState::Deprecated,
                signex_library::LifecycleState::Obsolete,
            ] {
                let _ = lifecycle_cell(state, i18n, &tokens);
            }
            let _ = search_field(
                t!(i18n, ids::LIBRARY_PICKER_SEARCH),
                "",
                |s| LibraryMessage::Picker(PickerMsg::FilterChanged(s)),
                None,
                &tokens,
            );
        }
    }
}
