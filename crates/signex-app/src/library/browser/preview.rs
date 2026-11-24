//! Library Browser — the 300 px preview pane of the selected row (mockup
//! row 118).
//!
//! The internal PN and description; the row actions (Edit Details, Pick
//! Symbol…, Pick Footprint…, and ⋯ holding Delete Selected); the symbol and
//! footprint tiles, drawn from the off-thread read the browser keeps in
//! `LibraryBrowserState::preview`; the field list; the where-used line.

use super::columns::ColumnKind;
use super::names::PrimitiveNames;
use super::tiles::{TileSource, footprint_tile, symbol_tile};
use super::*;
use crate::library::editor::kit;
use iced::widget::column;

/// Default type / spacing scales.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// Width of the pane.
pub(super) const PREVIEW_W: f32 = 300.0;
/// Height of the symbol and footprint tiles.
const TILE_H: f32 = 96.0;
/// Width of the field list's key column (the mock's 96 px).
const KEY_W: f32 = 96.0;

/// The message the "Edit Details" button fires: opens the Edit Component
/// Details modal for the selected row. `None` while nothing is selected or
/// the library is read-only, which leaves the button inert.
pub(super) fn edit_details_message(
    library_path: &std::path::Path,
    table: &str,
    selected: Option<RowId>,
    read_only: bool,
) -> Option<LibraryMessage> {
    let row_id = selected.filter(|_| !read_only)?;
    Some(LibraryMessage::BrowserOpenEditModal {
        library_path: library_path.to_path_buf(),
        table: table.to_string(),
        row_id,
    })
}

/// The row actions of the pane, `None` where the button is disabled: a
/// read-only library edits, binds and deletes nothing.
#[derive(Debug, Clone)]
pub(super) struct PreviewActions {
    pub(super) edit: Option<LibraryMessage>,
    pub(super) pick_symbol: Option<LibraryMessage>,
    pub(super) pick_footprint: Option<LibraryMessage>,
    pub(super) more: Option<LibraryMessage>,
    pub(super) delete: Option<LibraryMessage>,
}

pub(super) fn actions(
    library_path: &std::path::Path,
    table: &str,
    row_id: RowId,
    read_only: bool,
) -> PreviewActions {
    let pick = |kind| {
        (!read_only).then(|| LibraryMessage::OpenPrimitivePicker {
            kind,
            target: crate::library::state::PrimitivePickerTarget::BrowserRow(
                crate::library::state::EditorAddress::new(
                    library_path.to_path_buf(),
                    table.to_string(),
                    row_id,
                ),
            ),
        })
    };
    PreviewActions {
        edit: edit_details_message(library_path, table, Some(row_id), read_only),
        pick_symbol: pick(signex_library::PrimitiveKind::Symbol),
        pick_footprint: pick(signex_library::PrimitiveKind::Footprint),
        more: (!read_only).then(|| LibraryMessage::BrowserMoreMenuToggled {
            library_path: library_path.to_path_buf(),
        }),
        delete: (!read_only).then(|| LibraryMessage::BrowserDeleteRowRequest {
            library_path: library_path.to_path_buf(),
            table: table.to_string(),
            row_id,
        }),
    }
}

/// What the pane needs beyond the row: where it lives, the library state
/// for file names and where-used, and the browser's read of the drawings.
pub(super) struct PreviewSource<'a> {
    pub(super) library_path: &'a std::path::Path,
    pub(super) table: &'a str,
    pub(super) library_state: &'a LibraryState,
    pub(super) browser: &'a LibraryBrowserState,
    pub(super) read_only: bool,
}

/// The preview pane for `row`.
pub(super) fn view_preview<'a>(
    row: &'a ComponentRow,
    names: &PrimitiveNames<'_>,
    source: PreviewSource<'a>,
    look: PreviewLook,
    tokens: &'a ThemeTokens,
    i18n: &'a I18n,
) -> Element<'a, LibraryMessage> {
    let r = crate::styles::tokens::resolve(tokens);
    let row_id = RowId::from_uuid(row.row_id);
    let acts = actions(source.library_path, source.table, row_id, source.read_only);
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);

    let mut head: Column<Element<'a, LibraryMessage>> = column![
        text(UserText::new(row.internal_pn.as_str().to_string()))
            .size(TS.label)
            .font(iced::Font {
                weight: iced::font::Weight::Semibold,
                ..mono
            })
            .color(theme_ext::text_primary(tokens))
            .boxed()
    ];
    if !row.description.is_empty() {
        head = head.push(
            text(UserText::new(row.description.clone()))
                .size(TS.caption)
                .color(r.ink4)
                .boxed(),
        );
    }
    head = head.push(Space::new().height(SP.m).boxed());
    head = head.push(action_row(&acts, tokens, i18n));
    if source.browser.more_menu_open {
        head = head.push(more_menu(acts.delete.clone(), tokens, i18n));
    }
    if source.read_only {
        head = head.push(
            text(t!(i18n, ids::LIBRARY_BROWSER_READ_ONLY_NOTE))
                .size(TS.caption)
                .color(r.ink4)
                .boxed(),
        );
    }

    let tile_read = tile_source(row, source.browser);
    let tiles = row![
        container(symbol_tile(Some(&tile_read), look, TILE_H, tokens, i18n))
            .width(Length::FillPortion(1))
            .boxed(),
        container(footprint_tile(Some(&tile_read), TILE_H, tokens, i18n))
            .width(Length::FillPortion(1))
            .boxed(),
    ]
    .spacing(SP.m);

    let mut body: Column<Element<'a, LibraryMessage>> = column![
        container(head.spacing(SP.xs)).padding(iced::Padding {
            top: SP.l,
            right: SP.l,
            bottom: SP.m,
            left: SP.l,
        }),
        container(tiles).padding([0.0, SP.l]),
        container(field_list(row, names, source.library_state, tokens, i18n)).padding(SP.l),
    ];
    let uses = source.library_state.where_used_for(row_id);
    let projects = uses
        .iter()
        .map(|u| &u.project_path)
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    body = body.push(
        container(
            text(t!(
                i18n,
                ids::LIBRARY_PREVIEW_WHERE_USED,
                "sheets" => uses.len(),
                "projects" => projects
            ))
            .size(TS.caption)
            .color(r.ink4),
        )
        .padding(iced::Padding {
            top: 0.0,
            right: SP.l,
            bottom: SP.l,
            left: SP.l,
        })
        .boxed(),
    );

    let panel = crate::styles::ti(tokens.panel_bg);
    let line = theme_ext::border_color(tokens);
    container(
        scrollable(body)
            .direction(crate::styles::thin_vertical())
            .style(crate::styles::scrollbar_style(tokens))
            .height(Length::Fill),
    )
    .width(Length::Fixed(PREVIEW_W))
    .height(Length::Fill)
    .style(move |_: &Theme| iced::widget::container::Style {
        background: Some(iced::Background::Color(panel)),
        border: Border {
            color: line,
            left: iced::border::Side::default().width(1.0),
            ..Border::default()
        },
        ..iced::widget::container::Style::default()
    })
    .boxed()
}

/// Edit Details, Pick Symbol…, Pick Footprint… and ⋯, wrapping inside the
/// pane.
fn action_row<'a>(
    acts: &PreviewActions,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let ink = crate::styles::tokens::resolve(tokens).ink3;
    let size = crate::styles::ChromeMetrics::DEFAULT.row;
    let more = button(
        container(super::header::tinted_icon(
            crate::icons::icon_chrome_more(signex_types::theme::ThemeId::GraphiteDark),
            TS.body,
            ink,
        ))
        .center_x(Length::Fixed(size))
        .center_y(Length::Fixed(size)),
    )
    .padding(0)
    .on_press_maybe(acts.more.clone())
    .style(crate::styles::ghost_button(tokens));
    let more = super::grid::with_tooltip(
        more.boxed(),
        t!(i18n, ids::LIBRARY_BROWSER_MORE_ACTIONS),
        tokens,
    );
    row![
        kit::small_action(
            t!(i18n, ids::LIBRARY_BROWSER_EDIT_DETAILS),
            acts.edit.clone(),
            tokens
        ),
        kit::small_action(
            t!(i18n, ids::LIBRARY_BROWSER_PICK_SYMBOL),
            acts.pick_symbol.clone(),
            tokens
        ),
        kit::small_action(
            t!(i18n, ids::LIBRARY_BROWSER_PICK_FOOTPRINT),
            acts.pick_footprint.clone(),
            tokens
        ),
        more,
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center)
    .wrap()
    .boxed()
}

/// The ⋯ menu: Delete Selected, in the danger ink, which asks first.
fn more_menu<'a>(
    delete: Option<LibraryMessage>,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let danger = theme_ext::error_color(tokens);
    let item = button(
        text(t!(i18n, ids::LIBRARY_BROWSER_DELETE_SELECTED))
            .size(TS.body)
            .color(danger),
    )
    .padding([SP.s, SP.m])
    .width(Length::Fill)
    .on_press_maybe(delete)
    .style(crate::styles::menu_item(tokens));
    row![
        Space::new().width(Length::Fill).boxed(),
        container(item)
            .width(Length::Fixed(PREVIEW_W / 2.0))
            .padding(SP.xs)
            .style(crate::styles::context_menu(tokens))
            .boxed(),
    ]
    .boxed()
}

/// The browser's read for `row`, when it is the row the read was for.
fn loaded_for<'a>(
    row: &ComponentRow,
    browser: &'a LibraryBrowserState,
) -> Option<&'a crate::panels::components_panel::details::ComponentDetails> {
    let key = browser.preview.key.as_ref()?;
    (key.row_id == RowId::from_uuid(row.row_id))
        .then_some(browser.preview.loaded.as_deref())
        .flatten()
}

/// The read for `row` is still running (or not started).
fn loading(row: &ComponentRow, browser: &LibraryBrowserState) -> bool {
    let current = browser
        .preview
        .key
        .as_ref()
        .is_some_and(|k| k.row_id == RowId::from_uuid(row.row_id));
    !current || browser.preview.in_flight()
}

/// What the shared tiles draw for `row` from the browser's read.
fn tile_source<'a>(row: &ComponentRow, browser: &'a LibraryBrowserState) -> TileSource<'a> {
    TileSource {
        details: loaded_for(row, browser),
        loading: loading(row, browser),
        symbol_unbound: row.is_symbol_unbound(),
        footprint_unbound: row.is_footprint_unbound(),
    }
}

/// The file a primitive lives in (its listed name for a file named by a
/// uuid), else its listed name, else a dash.
fn primitive_label(
    reference: Option<&signex_library::PrimitiveRef>,
    dir: &str,
    ext: &str,
    listed: Option<&str>,
    library_state: &LibraryState,
) -> String {
    let Some(reference) = reference else {
        return String::new();
    };
    library_state
        .primitive_file(reference, dir, ext)
        .and_then(|p| crate::library::primitive_label::shown_file_name(&p, listed, ext))
        .or_else(|| listed.map(str::to_string))
        .unwrap_or_default()
}

/// Class, Manufacturer, MPN, Revision, Symbol, Footprint, Updated, then
/// the row's parameters.
fn field_list<'a>(
    row: &'a ComponentRow,
    names: &PrimitiveNames<'_>,
    library_state: &LibraryState,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let rev = if row.released {
        t!(i18n, ids::LIBRARY_BROWSER_REVISION_RELEASED, "version" => row.version.as_str())
            .into_string()
            .replace(['\u{2068}', '\u{2069}'], "")
    } else {
        row.version.clone()
    };
    let symbol = if row.is_symbol_unbound() {
        Default::default()
    } else {
        primitive_label(
            Some(&row.symbol_ref),
            "symbols",
            "snxsym",
            names.symbol(&row.symbol_ref),
            library_state,
        )
    };
    let footprint = row
        .footprint_ref
        .as_ref()
        .filter(|_| !row.is_footprint_unbound())
        .map(|fp| {
            primitive_label(
                Some(fp),
                "footprints",
                "snxfpt",
                names.footprint(fp),
                library_state,
            )
        })
        .unwrap_or_default();
    let cell = |kind: ColumnKind| kind.cell_value_named(row, names);
    let fields = [
        (
            ids::LIBRARY_PREVIEW_CLASS,
            row.class.as_str().to_string(),
            false,
        ),
        (
            ids::LIBRARY_PREVIEW_MANUFACTURER,
            cell(ColumnKind::Manufacturer),
            false,
        ),
        (ids::LIBRARY_PREVIEW_MPN, cell(ColumnKind::Mpn), true),
        (ids::LIBRARY_PREVIEW_REVISION, rev, false),
        (ids::LIBRARY_PREVIEW_SYMBOL, symbol, true),
        (ids::LIBRARY_PREVIEW_FOOTPRINT, footprint, true),
        (
            ids::LIBRARY_PREVIEW_UPDATED,
            cell(ColumnKind::Updated),
            false,
        ),
    ];
    let mut list: Column<Element<'a, LibraryMessage>> = column![].spacing(SP.s);
    for (key, value, is_mono) in fields {
        list = list.push(field_row(
            t!(i18n, key).into(),
            value,
            is_mono,
            tokens,
            i18n,
        ));
    }
    // A parameter key is user content: shown in capitals like the fixed
    // keys, through the casemap of the UI language.
    for (key, value) in &row.parameters {
        let caption = UserText::new(i18n.upper_for_display(key));
        list = list.push(field_row(
            caption.into(),
            value.display(),
            true,
            tokens,
            i18n,
        ));
    }
    list.boxed()
}

/// One key / value line: the key in small muted capitals, 96 px wide; the
/// value on one line, a dash when empty.
fn field_row<'a>(
    key: signex_widgets::Label<'a>,
    value: String,
    mono: bool,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let r = crate::styles::tokens::resolve(tokens);
    let mut value = if value.is_empty() {
        text(t!(i18n, ids::LIBRARY_BROWSER_NO_VALUE))
    } else {
        text(UserText::new(value))
    }
    .size(TS.body)
    .color(theme_ext::text_primary(tokens))
    .wrapping(iced::widget::text::Wrapping::None)
    .width(Length::Fill);
    if mono {
        value = value.font(crate::fonts::iced_font_for_family(
            crate::fonts::CHROME_MONO_FONT,
        ));
    }
    row![
        text(key)
            .size(TS.caption)
            .color(r.ink4)
            .width(Length::Fixed(KEY_W))
            .boxed(),
        container(value).clip(true).width(Length::Fill).boxed(),
    ]
    .spacing(SP.m)
    .align_y(iced::Alignment::Center)
    .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_details_targets_the_selected_row_and_is_inert_otherwise() {
        let path = std::path::Path::new("parts.snxlib");
        let row_id = RowId::from_uuid(uuid::Uuid::now_v7());
        match edit_details_message(path, "R", Some(row_id), false) {
            Some(LibraryMessage::BrowserOpenEditModal {
                library_path,
                table,
                row_id: got,
            }) => {
                assert_eq!(library_path, path);
                assert_eq!(table, "R");
                assert_eq!(got, row_id);
            }
            other => panic!("expected BrowserOpenEditModal, got {other:?}"),
        }
        assert!(edit_details_message(path, "R", None, false).is_none());
        assert!(edit_details_message(path, "R", Some(row_id), true).is_none());
    }

    /// Mockup row 118: Delete Selected moved from the action row and the per-row
    /// gutter into the ⋯ menu; it still asks first (`DeleteRowRequest`).
    /// Pick Symbol… and Pick Footprint… open the primitive picker for the
    /// row. A read-only library leaves every action off.
    #[test]
    fn the_preview_actions_pick_and_delete_through_the_existing_flows() {
        let path = std::path::Path::new("parts.snxlib");
        let row_id = RowId::from_uuid(uuid::Uuid::now_v7());
        let acts = actions(path, "R", row_id, false);
        assert!(matches!(
            acts.delete,
            Some(LibraryMessage::BrowserDeleteRowRequest { row_id: got, .. }) if got == row_id
        ));
        assert!(matches!(
            acts.pick_symbol,
            Some(LibraryMessage::OpenPrimitivePicker {
                kind: signex_library::PrimitiveKind::Symbol,
                ..
            })
        ));
        assert!(matches!(
            acts.pick_footprint,
            Some(LibraryMessage::OpenPrimitivePicker {
                kind: signex_library::PrimitiveKind::Footprint,
                ..
            })
        ));
        assert!(matches!(
            acts.more,
            Some(LibraryMessage::BrowserMoreMenuToggled { .. })
        ));
        let ro = actions(path, "R", row_id, true);
        assert!(ro.edit.is_none() && ro.pick_symbol.is_none() && ro.pick_footprint.is_none());
        assert!(ro.more.is_none() && ro.delete.is_none());
    }
}
