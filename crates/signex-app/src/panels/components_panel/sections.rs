//! One library section of the Components panel: its header (chevron, name,
//! source badge pill, part count or "shown of total" while filtering), then,
//! while expanded, its part rows.

use iced::widget::{Column, column, container};
use iced::{Element, Widget};
use signex_i18n::{I18n, Localized, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::text;

use super::rows::{RowColors, RowContext, RowFlags, RowLook, view_row_button};
use super::thumbs::ThumbLook;
use super::{HINT_TEXT_SIZE, SP, look};
use crate::library::messages::LibraryMessage;
use crate::library::state::{
    ComponentsMountSource, ComponentsPanelRowKey, ComponentsPanelState, OpenLibrary,
};

/// Most rows built per library per frame; the rest are summarised by a
/// "N more — refine search" row so a huge library cannot stall the view.
pub(super) const MAX_ROWS_PER_LIBRARY: usize = 200;

/// The header count: the part count, or "shown of total" while a filter is
/// active.
pub(super) fn section_count(shown: usize, total: usize, filtering: bool, i18n: &I18n) -> Localized {
    if filtering {
        t!(i18n, ids::PANELS_COMPONENTS_SHOWN_OF_TOTAL, "shown" => shown, "total" => total)
    } else {
        t!(i18n, ids::PANELS_COMPONENTS_COUNT, "count" => total)
    }
}

/// Render one mounted library: its header, then, while expanded, every
/// component matching the panel-wide filter.
pub(super) fn view_library_block<'a>(
    lib: &'a OpenLibrary,
    source: ComponentsMountSource,
    panel: &'a ComponentsPanelState,
    needle: &str,
    thumb_look: ThumbLook,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let r = crate::styles::tokens::resolve(tokens);
    let colors = RowColors {
        text: signex_widgets::theme_ext::text_primary(tokens),
        muted: r.ink4,
        // Graphite selects with the neutral selected token, not the accent wash.
        tint: crate::styles::ti(tokens.selection),
        hover: crate::styles::tokens::overlay(tokens, crate::styles::tokens::Overlay::Hover),
        menu: r.accent_soft,
    };
    let cx = RowContext {
        style: panel.row_style,
        colors,
        look: RowLook::graphite(),
        tokens,
        i18n,
    };

    let rows = &panel.rows;
    let matched = rows.visible_count(&lib.root);
    let collapsed = panel.collapsed_libraries.contains(&lib.root);
    let mut col: Column<Element<'a, LibraryMessage>> = column![].spacing(0);
    col = col.push(look::library_header(
        lib.root.clone(),
        lib.display_name.clone(),
        source,
        section_count(matched, lib.total_rows(), !needle.is_empty(), i18n),
        collapsed,
        tokens,
        i18n,
    ));
    if collapsed {
        return col.boxed();
    }

    // Rows come from the cache `update` keeps current (lowercase keys and
    // the filtered list), so a frame does no per-row string work and
    // builds widgets for at most `MAX_ROWS_PER_LIBRARY` rows (#1353).
    let previews = panel.row_style.shows_previews();
    let menu_key = panel.row_menu.as_ref().map(|m| &m.key);
    for (table, index) in rows.visible_rows(&lib.root, MAX_ROWS_PER_LIBRARY) {
        let Some(r) = lib.tables.get(table).and_then(|t| t.get(index)) else {
            continue;
        };
        let key = ComponentsPanelRowKey {
            library_path: lib.root.clone(),
            table: table.to_owned(),
            row_id: signex_library::RowId::from_uuid(r.row_id),
        };
        let flags = RowFlags {
            selected: panel.selected.as_ref() == Some(&key),
            hovered: panel.hovered.as_ref() == Some(&key),
            menu_open: menu_key == Some(&key),
        };
        let thumb = previews
            .then(|| panel.thumbs.get(&r.symbol_ref).cloned())
            .flatten()
            .map(|symbol| thumb_look.thumb(symbol));
        col = col.push(view_row_button(key, r, flags, thumb, &cx));
    }

    let muted = colors.muted;
    if matched > MAX_ROWS_PER_LIBRARY {
        col = col.push(
            container(
                text(t!(
                    i18n,
                    ids::PANELS_COMPONENTS_MORE_ROWS,
                    "count" => (matched - MAX_ROWS_PER_LIBRARY) as i64
                ))
                .size(HINT_TEXT_SIZE)
                .color(muted),
            )
            .padding([SP.xs, cx.look.inset])
            .boxed(),
        );
    }

    if matched == 0 && !needle.is_empty() {
        col = col.push(
            container(
                text(t!(i18n, ids::PANELS_COMPONENTS_NO_MATCHES_IN_LIBRARY))
                    .size(HINT_TEXT_SIZE)
                    .color(muted),
            )
            .padding([SP.xs, cx.look.inset])
            .boxed(),
        );
    }

    col.boxed()
}
