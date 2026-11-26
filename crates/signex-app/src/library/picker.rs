//! Component picker modal (mockup row 129).
//!
//! Opened from File ▸ Library ▸ Place Component…, Place ▸ Component… and the
//! Active Bar's Part button; Change Component and Open Datasheet reuse it.
//!
//! Shape:
//!
//! ```text
//! ┌─[Place Component   3 libraries · 359 parts ─────────────────── X]─┐
//! │ [⌕ Search internal PN / MPN / description…]  [Add range filter…]  │
//! ├────────────────────────────────────────────────────┬───────────────┤
//! │ INTERNAL PN      STATE      MPN          DESCRIPTION │ [SYMBOL     ] │
//! │ R-0402-10K-1%    ● Released RC0402FR-07… 10 kΩ ±1%   │ [FOOTPRINT  ] │
//! │ …                                                    │               │
//! ├────────────────────────────────────────────────────┴───────────────┤
//! │ ↑↓ to move · Enter to place                 [ Cancel ] [ Place ]   │
//! └────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! The right column previews the selected row's symbol and footprint with
//! the Library Browser's drawings, read off-thread into
//! `PickerState::preview`, so the footprint is checked before placing.

use iced::widget::scrollable::RelativeOffset;
use iced::widget::text::{Ellipsis, Wrapping};
use iced::widget::{Space, column, container, row, scrollable};
use iced::{Background, Border, Color, Element, Length, Theme, Widget};
use signex_i18n::{I18n, Localized, UserText, ids, t};
use signex_library::RowId;
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;
use signex_widgets::{button, text};

use super::browser::PreviewLook;
use super::browser::tiles::{TileSource, footprint_tile, symbol_tile};
use super::messages::{LibraryMessage, PickerMsg};
use super::modal_chrome::{cancel_btn, footer_strip, header_with_note_closing, primary_btn_if};
use super::placed_part::datasheet_choice_view;
use super::range_filter::{range_filter_add_unavailable, range_filter_bar};
use super::state::{PickerEntry, PickerMode, PickerState};
use crate::library::editor::kit::{lifecycle_text, lifecycle_tone};
use crate::panels::components_panel::details::ComponentDetails;
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

const PICKER_W: f32 = 880.0;
const PICKER_H: f32 = 480.0;
/// Width of the symbol and footprint preview column (mockup row 129).
const PREVIEW_W: f32 = 250.0;
/// Width of the State column: the widest state word and its dot.
const STATE_W: f32 = 104.0;
/// Height of a part row (`.tbl td` in the design).
const ROW_H: f32 = 26.0;
/// Diameter of a lifecycle state's dot.
const DOT: f32 = 6.0;
/// Width of the selected row's leading accent bar.
const SELECTED_BAR: f32 = 2.0;
/// Height of a preview tile: three and a half part rows.
const TILE_H: f32 = ROW_H * 3.5;
/// The list's scrollable, so ↑ and ↓ keep the selected row in view.
pub(crate) const LIST_ID: &str = "library-picker-list";

/// The list's relative offset that shows row `index` of `count`: the
/// row's proportional place, so the row lands inside the viewport
/// whatever its height (first row at the top, last at the bottom).
pub(crate) fn list_offset_showing(index: usize, count: usize) -> RelativeOffset {
    let y = if count < 2 {
        0.0
    } else {
        index.min(count - 1) as f32 / (count - 1) as f32
    };
    RelativeOffset { x: 0.0, y }
}

/// Draws the list `LibraryState::sync_view_caches_in` last built for the
/// picker; `view` itself filters and clones nothing.
pub(crate) fn view<'a>(
    picker: &'a PickerState,
    tokens: &'a ThemeTokens,
    i18n: &'a I18n,
    windowed: bool,
    look: PreviewLook,
) -> Element<'a, LibraryMessage> {
    // Change Component (two steps, mockup row 129) and Open Datasheet reuse
    // this modal with their own card.
    match &picker.mode {
        PickerMode::ChangeComponent(_) | PickerMode::ChangePreview(_) => {
            if let Some(card) = crate::app::view::dialogs::change_component::view(
                picker, tokens, i18n, windowed, look,
            ) {
                return card;
            }
        }
        PickerMode::Datasheets(choice) => {
            return datasheet_choice_view(choice, tokens, i18n, windowed);
        }
        PickerMode::Place => {}
    }
    let commit = LibraryMessage::Picker(PickerMsg::PlaceSelected);
    let selected = picker.shown_selection().is_some();
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

    // Enter in the search field commits the selected row while the list
    // shows it (mockup row 129).
    let search = crate::styles::dialog_search_field(
        t!(i18n, ids::LIBRARY_PICKER_SEARCH),
        &picker.filter,
        |s| LibraryMessage::Picker(PickerMsg::FilterChanged(s)),
        selected.then(|| commit.clone()),
        tokens,
    );
    // Range filters on the measurement columns (#1572): "Add range filter…"
    // beside the search; active filters stack in the same column. With no
    // measurement column the control stays, disabled (mockup row 129).
    let range_bar = range_filter_bar(
        &picker.range_filters,
        rows.filter_columns(),
        rows.range_excluded(),
        tokens,
        i18n,
        |msg| LibraryMessage::Picker(PickerMsg::RangeFilter(msg)),
    )
    .unwrap_or_else(|| range_filter_add_unavailable(tokens, i18n));
    let top = row![search, range_bar]
        .spacing(SP.m)
        .align_y(iced::Alignment::Start);
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
            .id(LIST_ID)
            .height(Length::Fill)
            .width(Length::Fill)
            .boxed()
    };
    let table = column![table_head(i18n, tokens), list]
        .spacing(0)
        .width(Length::Fill)
        .height(Length::Fill);
    let split = list_and_preview(
        table.boxed(),
        preview_column(picker, look, tokens, i18n),
        tokens,
    );

    // ↑ and ↓ move the selection; Enter in the search field commits it.
    let hint = text(t!(i18n, ids::LIBRARY_PICKER_HINT_PLACE))
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
            primary_btn_if(t!(i18n, ids::DIALOG_BUTTON_PLACE), commit, selected, tokens),
        ],
        tokens,
    );

    let body = container(column![top, split].spacing(0))
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

/// The list and the preview column side by side under a hairline, a
/// hairline between them.
fn list_and_preview<'a>(
    list: Element<'a, LibraryMessage>,
    preview: Element<'a, LibraryMessage>,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let rule = crate::styles::tokens::resolve(tokens).line_soft;
    let divider = container(Space::new())
        .width(crate::styles::HAIRLINE)
        .height(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(rule)),
            ..container::Style::default()
        });
    container(row![list, divider.boxed(), preview].spacing(0))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_: &Theme| container::Style {
            border: Border::default().top(iced::border::Side::default().width(1.0).color(rule)),
            ..container::Style::default()
        })
        .boxed()
}

/// The picker's read of the selected row, once it is that row's.
fn selected_details(picker: &PickerState) -> Option<&ComponentDetails> {
    let (path, summary) = picker.selected.as_ref()?;
    let key = picker.preview.key.as_ref()?;
    let same_row = key.library_path == *path && key.row_id == RowId::from_uuid(summary.row_id);
    same_row
        .then_some(picker.preview.loaded.as_deref())
        .flatten()
}

/// The selected row's read is running, or has not started yet.
fn preview_loading(picker: &PickerState) -> bool {
    let Some((path, summary)) = picker.selected.as_ref() else {
        return false;
    };
    let current = picker.preview.key.as_ref().is_some_and(|key| {
        key.library_path == *path && key.row_id == RowId::from_uuid(summary.row_id)
    });
    !current || picker.preview.in_flight()
}

/// What the shared tiles draw for the selected row; `None` with nothing
/// selected. Until the row's read lands, neither side counts as unbound.
fn tile_source(picker: &PickerState) -> Option<TileSource<'_>> {
    picker.selected.as_ref()?;
    let details = selected_details(picker);
    Some(TileSource {
        details,
        loading: preview_loading(picker),
        symbol_unbound: details.is_some_and(|d| d.symbol_unbound),
        footprint_unbound: details.is_some_and(|d| d.footprint_unbound),
    })
}

/// The preview column: the symbol tile over the footprint tile, drawn as
/// the Library Browser's preview pane draws them. Change Component's pick
/// step shows the same column.
pub(crate) fn preview_column<'a>(
    picker: &'a PickerState,
    look: PreviewLook,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let source = tile_source(picker);
    column![
        symbol_tile(source.as_ref(), look, TILE_H, tokens, i18n),
        footprint_tile(source.as_ref(), TILE_H, tokens, i18n),
    ]
    .spacing(SP.s)
    .padding(SP.m)
    .width(Length::Fixed(PREVIEW_W))
    .height(Length::Fill)
    .boxed()
}

/// The chrome mono face of part numbers and file names.
pub(crate) fn mono() -> iced::Font {
    crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
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

/// A table cell's value on one line, ended with an ellipsis: the row height
/// is fixed, so a wrapped second line would be clipped (mockup row 129).
fn one_line_cell<'a>(
    value: &'a str,
    font: Option<iced::Font>,
    ink: Color,
) -> Element<'a, LibraryMessage> {
    let cell = text(UserText::new(value))
        .size(TS.body)
        .color(ink)
        .width(Length::Fill)
        .wrapping(Wrapping::None)
        .ellipsis(Ellipsis::End);
    match font {
        Some(font) => cell.font(font).boxed(),
        None => cell.boxed(),
    }
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
    let pn = one_line_cell(summary.internal_pn.as_str(), Some(mono()), ink);
    let mpn = one_line_cell(summary.mpn.as_str(), Some(mono()), ink);
    let description = one_line_cell(summary.description.as_str(), None, muted);
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
mod search_row_tests;

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

    /// Mockup row 129: a value wider than its cell stays on one line.
    #[test]
    fn a_long_cell_value_stays_on_one_line() {
        use crate::layout_probe::lay_out_texts;
        let height = |value: &str| {
            lay_out_texts(
                one_line_cell(value, None, Color::BLACK),
                iced::Size::new(60.0, 200.0),
            )[0]
            .bounds
            .height
        };
        let long = "Polarized capacitor with a long description";
        assert!((height(long) - height("C")).abs() < 0.5);
    }

    fn summary(row_id: uuid::Uuid) -> signex_library::ComponentSummary {
        signex_library::ComponentSummary {
            row_id,
            internal_pn: signex_library::InternalPn::new("R-0402-10K-1%"),
            mpn: "RC0402FR-0710KL".into(),
            state: signex_library::LifecycleState::Released,
            description: "10 kΩ ±1% 0402".into(),
        }
    }

    /// A picker with one row selected and its preview read started.
    fn selected_picker() -> (PickerState, crate::library::state::ComponentsPanelRowKey) {
        let row_id = uuid::Uuid::now_v7();
        let path = std::path::PathBuf::from("/lib/passives");
        let key = crate::library::state::ComponentsPanelRowKey {
            library_path: path.clone(),
            table: "resistors".into(),
            row_id: RowId::from_uuid(row_id),
        };
        let mut picker = PickerState {
            selected: Some((path, summary(row_id))),
            ..PickerState::default()
        };
        picker.preview.begin(key.clone(), None);
        (picker, key)
    }

    fn details(symbol_unbound: bool, footprint_unbound: bool) -> Box<ComponentDetails> {
        Box::new(ComponentDetails {
            internal_pn: "R-0402-10K-1%".into(),
            class: String::new(),
            lifecycle: String::new(),
            state: signex_library::LifecycleState::Released,
            manufacturer: String::new(),
            mpn: String::new(),
            description: String::new(),
            has_footprint_ref: !footprint_unbound,
            symbol_unbound,
            footprint_unbound,
            symbol: None,
            footprint: None,
            default_footprint: None,
            alternate_footprints: Vec::new(),
            parameters: Vec::new(),
            notes: Vec::new(),
        })
    }

    fn laid_out(picker: &PickerState) -> Vec<crate::layout_probe::LaidOutText> {
        let tokens = signex_types::theme::theme_tokens(ThemeId::GraphiteLight);
        let i18n = I18n::en_us_ref();
        let look = PreviewLook::for_tests();
        crate::layout_probe::lay_out_texts(
            view(picker, &tokens, i18n, false, look),
            iced::Size::new(1200.0, 800.0),
        )
    }

    /// Mockup row 129: the card is 880 px wide, and its symbol and
    /// footprint tiles sit in the right-hand 250 px column.
    #[test]
    fn the_card_grows_to_hold_a_250_px_preview_column() {
        let tokens = signex_types::theme::theme_tokens(ThemeId::GraphiteLight);
        let size = crate::layout_probe::root_size(
            view(
                &PickerState::default(),
                &tokens,
                I18n::en_us_ref(),
                false,
                PreviewLook::for_tests(),
            ),
            iced::Size::new(1200.0, 800.0),
        );
        assert!((size.width - 880.0).abs() < 0.5, "{size:?}");
        let texts = laid_out(&PickerState::default());
        // The column starts 250 px from the card's right edge; a tag sits
        // inside the column's padding, the tile's hairline and its own
        // padding.
        let tag_x = 880.0 - 250.0 + SP.m + crate::styles::HAIRLINE + SP.m;
        for tag in ["SYMBOL", "FOOTPRINT"] {
            let bounds = crate::layout_probe::find(&texts, tag).bounds;
            assert!((bounds.x - tag_x).abs() < 0.5, "{tag} at {bounds:?}");
            assert!(bounds.x + bounds.width <= 880.0, "{tag} at {bounds:?}");
        }
        // The symbol tile is above the footprint tile.
        let symbol = crate::layout_probe::find(&texts, "SYMBOL").bounds;
        let footprint = crate::layout_probe::find(&texts, "FOOTPRINT").bounds;
        assert!(symbol.y < footprint.y);
    }

    /// While the selected row's read runs both tiles say so; once it lands
    /// an unbound footprint says it has none and a symbol that could not be
    /// read says that.
    #[test]
    fn the_preview_tiles_follow_the_selected_rows_read() {
        let (mut picker, key) = selected_picker();
        let texts = laid_out(&picker);
        let loading = texts.iter().filter(|t| t.text == "Loading…").count();
        assert_eq!(loading, 2);

        let generation = picker.preview.generation;
        assert!(
            picker
                .preview
                .finish(&key, generation, Ok(details(false, true)))
        );
        let texts = laid_out(&picker);
        assert!(texts.iter().any(|t| t.text == "No footprint bound"));
        assert!(texts.iter().any(|t| t.text == "Could not be read"));
        assert!(!texts.iter().any(|t| t.text == "Loading…"));
    }

    /// The picker says what the Library Browser says: an unbound symbol
    /// (a nil reference included, decided by the row) reads "No symbol
    /// bound", and a bound footprint that could not be read says so.
    #[test]
    fn an_unbound_symbol_reads_as_in_the_library_browser() {
        let (mut picker, key) = selected_picker();
        let generation = picker.preview.generation;
        assert!(
            picker
                .preview
                .finish(&key, generation, Ok(details(true, false)))
        );
        let texts = laid_out(&picker);
        assert!(texts.iter().any(|t| t.text == "No symbol bound"));
        assert!(texts.iter().any(|t| t.text == "Could not be read"));
        assert!(!texts.iter().any(|t| t.text == "No footprint bound"));
    }

    /// Nothing selected: the tiles show only their tags.
    #[test]
    fn with_nothing_selected_the_tiles_are_empty() {
        let texts = laid_out(&PickerState::default());
        for word in ["Loading…", "Could not be read", "No footprint bound"] {
            assert!(!texts.iter().any(|t| t.text == word), "{word}");
        }
    }

    /// Mockup row 129: in the laid-out dialog, a part whose PN and
    /// description are far wider than their columns keeps both on one line
    /// inside the row height (the ellipsis ends them).
    #[test]
    fn a_long_part_row_stays_one_line_in_the_dialog() {
        use crate::layout_probe::lay_out_texts;
        use crate::library::state::{LibraryState, OpenLibrary};
        let pn = "STOCK-C_Polarized_Radial_D10.0mm_P5.00mm_Long";
        let description = "Polarized capacitor, radial, 10 mm diameter, 5 mm pitch, long leads";
        let mut state = LibraryState::default();
        state.open_libraries.push(OpenLibrary {
            root: std::path::PathBuf::from("/libs/stock.snxlib"),
            display_name: "stock".into(),
            library_id: uuid::Uuid::from_u128(1),
            tables: std::collections::HashMap::new(),
            cached_components: vec![signex_library::ComponentSummary {
                row_id: uuid::Uuid::from_u128(2),
                internal_pn: signex_library::InternalPn::new(pn),
                mpn: "UVR1H101MPD".into(),
                state: signex_library::LifecycleState::Released,
                description: description.into(),
            }],
            cached_symbols: Vec::new(),
            cached_footprints: Vec::new(),
            cached_sims: Vec::new(),
            cached_classes: Vec::new(),
            observed_stamp: None,
            dangling: Default::default(),
            own_edit_stamp: None,
            display: Default::default(),
            pricing: Default::default(),
        });
        state.touch_data();
        state.picker = Some(PickerState::default());
        state.sync_view_caches();
        let tokens = signex_types::theme::theme_tokens(ThemeId::GraphiteLight);
        let card = view(
            state.picker.as_ref().unwrap(),
            &tokens,
            I18n::en_us_ref(),
            false,
            PreviewLook::for_tests(),
        );
        let texts = lay_out_texts(card, iced::Size::new(PICKER_W, PICKER_H));
        for value in [pn, description] {
            let cell = texts
                .iter()
                .find(|t| t.text == value)
                .unwrap_or_else(|| panic!("{value:?} is drawn"));
            assert!(
                cell.bounds.height < ROW_H,
                "{value:?} wraps: {}",
                cell.bounds.height
            );
        }
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
            let _ = crate::styles::dialog_search_field(
                t!(i18n, ids::LIBRARY_PICKER_SEARCH),
                "",
                |s| LibraryMessage::Picker(PickerMsg::FilterChanged(s)),
                None,
                &tokens,
            );
        }
    }
}
