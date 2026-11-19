//! The Library panel's Parts kind (#119): every open library as a row that
//! expands to its tables and their component rows (#1895). The filter
//! matches the rows (part number, MPN, manufacturer) and keeps a library
//! while any of its rows match; a match on the library name keeps all of
//! its rows. A click highlights a row, a double click opens it, and the
//! toolbar acts on the highlighted row.

use std::collections::HashMap;

use iced::widget::{Column, container, row};
use iced::{Alignment, Element, Length, Padding, Widget};
use signex_i18n::{I18n, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, text};

use super::KindView;
use super::frame::{self, Crumb, INDENT_1, INDENT_2, RowMark, RowText, SP, TS, Tools};
use crate::dock::DockMessage;
use crate::library::messages::{LibraryMessage, PickerMsg};
use crate::library::state::{ComponentsPanelRowKey, LibraryState, OpenLibrary};
use crate::panels::PanelContext;
use crate::styles::ti;
use crate::styles::tokens::resolve;

pub(super) fn view<'a>(library: &'a LibraryState, ctx: &'a PanelContext) -> KindView<'a> {
    let i18n = &*ctx.i18n;
    let selected = highlighted(library);
    let shown = selected
        .as_ref()
        .and_then(|key| library.library_at(&key.library_path))
        .or_else(|| library.open_libraries.first());
    let crumb = match shown {
        Some(lib) => Crumb::File {
            library: Some(lib.display_name.clone()),
            file: frame::file_name_or(
                &lib.root,
                t!(i18n, ids::PANELS_FOOTPRINT_LIBRARY_UNKNOWN_FILE),
            ),
            count: t!(i18n, ids::LIBRARY_PANEL_CRUMB_PARTS, "count" => lib.total_rows()),
        },
        None => Crumb::Note(t!(i18n, ids::LIBRARY_PANEL_NO_LIBRARY)),
    };
    KindView {
        crumb,
        tools: tools(library, ctx, selected),
        list: list(library, ctx),
    }
}

/// The highlighted row, while its library is open and still holds it.
fn highlighted(library: &LibraryState) -> Option<ComponentsPanelRowKey> {
    let key = library.panel_selected.as_ref()?;
    let present = library
        .library_at(&key.library_path)?
        .tables
        .get(&key.table)?
        .iter()
        .any(|r| signex_library::RowId::from_uuid(r.row_id) == key.row_id);
    present.then(|| key.clone())
}

/// The toolbar acts on the highlighted row through the messages the
/// Library Browser and the Place Component picker already send. A tool
/// with nothing to act on, or no way to act, is disabled.
fn tools(library: &LibraryState, ctx: &PanelContext, key: Option<ComponentsPanelRowKey>) -> Tools {
    let Some(key) = key else {
        return Tools::default();
    };
    let writable = !library.is_read_only(&key.library_path);
    // The delete confirmation lives on the library's browser state.
    let has_browser = library.library_browsers.contains_key(&key.library_path);
    let can_place = ctx.has_schematic || ctx.has_pcb;
    Tools {
        place: can_place.then(|| {
            frame::lib(LibraryMessage::PlaceLibraryComponent {
                library_path: key.library_path.clone(),
                table: key.table.clone(),
                row_id: key.row_id,
            })
        }),
        add: writable.then(|| {
            frame::lib(LibraryMessage::BrowserAddComponent {
                library_path: key.library_path.clone(),
                table: Some(key.table.clone()),
            })
        }),
        edit: Some(frame::lib(LibraryMessage::OpenComponentRow {
            library_path: key.library_path.clone(),
            table: key.table.clone(),
            row_id: key.row_id,
        })),
        delete: (writable && has_browser).then(|| {
            frame::lib(LibraryMessage::BrowserDeleteRowRequest {
                library_path: key.library_path.clone(),
                table: key.table.clone(),
                row_id: key.row_id,
            })
        }),
    }
}

/// Every open library with a row matching the filter, each expanded to its
/// tables and rows while the user opened it or a filter is set.
fn list<'a>(library: &'a LibraryState, ctx: &'a PanelContext) -> Element<'a, DockMessage> {
    let i18n = &*ctx.i18n;
    let tokens = &ctx.tokens;
    let filtering = !library.panel_search.trim().is_empty();
    let rows = &library.library_panel.rows;
    let mut col: Column<Element<'a, DockMessage>> = Column::new().spacing(0).width(Length::Fill);
    let mut shown = 0;
    for lib in &library.open_libraries {
        if filtering && rows.visible_count(&lib.root) == 0 {
            continue;
        }
        shown += 1;
        let expanded = filtering || library.panel_expanded.contains(&lib.root);
        col = col.push(library_row(lib, expanded, tokens, i18n));
        if expanded {
            col = col.push(tables(lib, library, filtering, tokens, i18n));
        }
    }
    if shown == 0 && filtering {
        col = col.push(frame::list_note(
            t!(i18n, ids::LIBRARY_PANEL_NO_MATCHES),
            0.0,
            tokens,
        ));
    }
    container(col)
        .padding([0.0, SP.s])
        .width(Length::Fill)
        .boxed()
}

/// One library row: the chevron toggle, `name.snxlib`, the row-count pill
/// and Open, which opens its Library Browser tab.
fn library_row<'a>(
    lib: &'a OpenLibrary,
    expanded: bool,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, DockMessage> {
    let r = resolve(tokens);
    let ink = ti(tokens.text);
    let toggle = button(
        row![
            signex_widgets::chevron::chevron(expanded, TS.body, r.ink3).boxed(),
            text(UserText::new(format!("{}.snxlib", lib.display_name)))
                .size(TS.body)
                .font(iced::Font {
                    weight: iced::font::Weight::Medium,
                    ..iced::Font::DEFAULT
                })
                .color(ink)
                .wrapping(iced::widget::text::Wrapping::None)
                .ellipsis(iced::widget::text::Ellipsis::End)
                .width(Length::Fill)
                .boxed(),
            crate::library::editor::kit::toned_pill(
                t!(i18n, ids::LIBRARY_PANEL_ROWS, "count" => lib.total_rows()).into_string(),
                crate::styles::PillTone::Neutral,
                tokens,
            ),
        ]
        .spacing(SP.s)
        .align_y(Alignment::Center),
    )
    .padding([0.0, SP.s])
    .height(frame::ROW_H)
    .width(Length::Fill)
    .on_press(lib_msg(PickerMsg::PanelLibraryToggled(lib.root.clone())))
    .style(crate::styles::subtle_button(tokens));
    let open = button(text(t!(i18n, ids::LIBRARY_PANEL_OPEN)).size(TS.caption))
        .padding([SP.xs, SP.m])
        .on_press(frame::lib(LibraryMessage::OpenLibraryBrowser(
            lib.root.clone(),
        )))
        .style(crate::styles::outline_button(tokens));
    row![toggle.boxed(), open.boxed()]
        .spacing(SP.s)
        .align_y(Alignment::Center)
        .boxed()
}

fn lib_msg(message: PickerMsg) -> DockMessage {
    frame::lib(LibraryMessage::Picker(message))
}

/// The tables of an expanded library by name, each with its count and the
/// rows that match the filter under it. Without a filter every table shows,
/// empty ones too.
fn tables<'a>(
    lib: &'a OpenLibrary,
    library: &'a LibraryState,
    filtering: bool,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, DockMessage> {
    let r = resolve(tokens);
    let mut matched: HashMap<&str, Vec<usize>> = HashMap::new();
    for (table, index) in library
        .library_panel
        .rows
        .visible_rows(&lib.root, usize::MAX)
    {
        matched.entry(table).or_default().push(index);
    }
    let mut col: Column<Element<'a, DockMessage>> = Column::new().spacing(0).width(Length::Fill);
    let tables = lib.panel_tables();
    if tables.is_empty() {
        col = col.push(frame::list_note(
            t!(i18n, ids::LIBRARY_PANEL_NO_TABLES),
            INDENT_1,
            tokens,
        ));
    }
    for (table, rows) in tables {
        let hits: Vec<usize> = matched.remove(table).unwrap_or_default();
        if filtering && hits.is_empty() {
            continue;
        }
        let count = if filtering { hits.len() } else { rows.len() };
        col = col.push(
            container(
                row![
                    text(UserText::new(table.to_owned()))
                        .size(TS.body)
                        .color(r.ink3)
                        .width(Length::Fill)
                        .boxed(),
                    text(t!(i18n, ids::LIBRARY_PANEL_COUNT, "count" => count))
                        .size(TS.caption)
                        .font(frame::mono())
                        .color(r.ink4)
                        .boxed(),
                ]
                .align_y(Alignment::Center),
            )
            .padding(Padding {
                top: 0.0,
                right: SP.m,
                bottom: 0.0,
                left: SP.m + INDENT_1,
            })
            .height(frame::ROW_H)
            .center_y(frame::ROW_H)
            .width(Length::Fill)
            .boxed(),
        );
        for index in hits {
            let Some(part) = rows.get(index) else {
                continue;
            };
            let key = ComponentsPanelRowKey {
                library_path: lib.root.clone(),
                table: table.to_owned(),
                row_id: signex_library::RowId::from_uuid(part.row_id),
            };
            let mark = if library.panel_selected.as_ref() == Some(&key) {
                RowMark::Selected
            } else {
                RowMark::None
            };
            col = col.push(frame::list_row(
                RowText {
                    name: part.internal_pn.as_str().to_owned(),
                    caption: Some(UserText::new(part.description.clone()).into()),
                },
                mark,
                INDENT_2,
                lib_msg(PickerMsg::PanelComponentClicked(key)),
                tokens,
            ));
        }
    }
    col.boxed()
}
