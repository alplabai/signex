//! The Library dock panel (mockup row 119): one panel for the symbols,
//! footprints and parts of the open libraries, replacing the SCH Library
//! (mockup row 120) and Footprint Library (mockup row 121) panels.
//!
//! Top to bottom, the same in every kind: the kind chips with their
//! counts, the breadcrumb, the filter, the toolbar (Place, Add, Edit, and
//! Delete at the right end), then the kind's list. The panel opens on the
//! kind of the active editor tab; a chip pick holds while that tab stays
//! active (see `library::state::panel_kind`). Open Library… is the folder
//! button in the dock title bar.

mod empty;
mod footprints;
mod frame;
mod parts;
mod symbols;
#[cfg(test)]
mod tests;

use iced::widget::{Column, column};
use iced::{Element, Length, Widget};
use signex_i18n::{ids, t};

use crate::dock::DockMessage;
use crate::library::state::{LibraryPanelKind, LibraryPanelTab, LibraryState};
use crate::panels::PanelContext;

use frame::{Crumb, Tools};
pub use frame::{KindCounts, open_library_glyph};

/// What a kind puts under the chips: its breadcrumb, its toolbar and its
/// list.
struct KindView<'a> {
    crumb: Crumb,
    tools: Tools,
    list: Element<'a, DockMessage>,
}

/// The editor context of `ctx`: the symbol or footprint file of the
/// active editor tab, or neither.
pub fn panel_tab(ctx: &PanelContext) -> LibraryPanelTab {
    if let Some(sym) = ctx.symbol_editor.as_ref() {
        LibraryPanelTab::Symbol(sym.path.clone())
    } else if let Some(fp) = ctx.footprint_editor.as_ref() {
        LibraryPanelTab::Footprint(fp.path.clone())
    } else {
        LibraryPanelTab::Other
    }
}

/// The kind the panel shows for `ctx`.
pub fn shown_kind(library: &LibraryState, ctx: &PanelContext) -> LibraryPanelKind {
    library.library_panel.kind_in(&panel_tab(ctx))
}

/// The count on each kind chip: the symbols of the open `.snxsym`, the
/// footprints of the open `.snxfpt`, the rows of every open library.
pub fn kind_counts(library: &LibraryState, ctx: &PanelContext) -> KindCounts {
    KindCounts {
        symbols: ctx
            .symbol_editor
            .as_ref()
            .map_or(0, |s| s.symbols_in_file.len()),
        footprints: ctx
            .footprint_editor
            .as_ref()
            .map_or(0, |f| f.internal_footprints.len()),
        parts: library
            .open_libraries
            .iter()
            .map(|lib| lib.total_rows())
            .sum(),
    }
}

/// Nothing to browse: no library open and no symbol or footprint editor.
pub fn has_nothing_open(library: &LibraryState, ctx: &PanelContext) -> bool {
    library.open_libraries.is_empty()
        && ctx.symbol_editor.is_none()
        && ctx.footprint_editor.is_none()
}

/// Render the Library panel.
pub fn view<'a>(library: &'a LibraryState, ctx: &'a PanelContext) -> Element<'a, DockMessage> {
    let i18n = &*ctx.i18n;
    let tokens = &ctx.tokens;
    let nothing_open = has_nothing_open(library, ctx);
    // With nothing open the inert chips show Parts, as the panel would
    // open once a library is.
    let kind = if nothing_open {
        LibraryPanelKind::Parts
    } else {
        shown_kind(library, ctx)
    };
    let placeholder = match kind {
        LibraryPanelKind::Symbols => t!(i18n, ids::LIBRARY_PANEL_FILTER_SYMBOLS),
        LibraryPanelKind::Footprints => t!(i18n, ids::LIBRARY_PANEL_FILTER_FOOTPRINTS),
        LibraryPanelKind::Parts => t!(i18n, ids::LIBRARY_PANEL_FILTER_PARTS),
    };
    let shown = if nothing_open {
        KindView {
            crumb: Crumb::Note(t!(i18n, ids::LIBRARY_PANEL_NO_LIBRARY)),
            tools: Tools::default(),
            list: empty::card(tokens, i18n),
        }
    } else {
        match kind {
            LibraryPanelKind::Symbols => symbols::view(library, ctx),
            LibraryPanelKind::Footprints => footprints::view(library, ctx),
            LibraryPanelKind::Parts if library.open_libraries.is_empty() => KindView {
                crumb: Crumb::Note(t!(i18n, ids::LIBRARY_PANEL_NO_LIBRARY)),
                tools: Tools::default(),
                list: empty::card(tokens, i18n),
            },
            LibraryPanelKind::Parts => parts::view(library, ctx),
        }
    };
    let head: Column<Element<'a, DockMessage>> = column![
        frame::kind_chips(kind, kind_counts(library, ctx), !nothing_open, tokens, i18n),
        frame::crumb(shown.crumb, tokens, i18n),
        frame::filter_field(&library.panel_search, placeholder, !nothing_open, tokens),
        frame::toolbar(shown.tools, tokens, i18n),
    ]
    .width(Length::Fill);
    column![
        head.boxed(),
        crate::panels::panel_scrollable(shown.list)
            .height(Length::Fill)
            .boxed(),
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .boxed()
}
