//! A symbol's Pin table as a page tab (mockup row 122b, opened by the Pins
//! tab's `Edit pins in table…`). One tab per symbol, next to its symbol
//! editor tab. Like the Layer Stack tab, its path is its owner's (the
//! `.snxsym`), so it shows the symbol's dirty dot, and Save and Undo act on
//! the symbol (docs/PROJECT_TAB.md, "Page tabs").
//!
//! The tab holds no document. It sits after its symbol's tab, so every
//! lookup by path finds the symbol's; opening, dragging and closing keep
//! that order.

use std::path::{Path, PathBuf};

use iced::widget::{Space, container};
use iced::{Element, Length, Widget as _};
use signex_i18n::{Arg, ids};

use crate::app::{Message, Signex, TabInfo, TabKind};
use crate::dock::DockMessage;

/// The index of `symbol`'s Pin table tab (`is_page`) or symbol editor tab.
fn tab_at(tabs: &[TabInfo], symbol: &Path, is_page: bool) -> Option<usize> {
    tabs.iter().position(|tab| {
        tab.path == symbol
            && if is_page {
                tab.kind.is_pin_table()
            } else {
                matches!(tab.kind, TabKind::SymbolEditor(_))
            }
    })
}

/// Move any Pin table tab left of its symbol's tab to just after it, and
/// return where the tab at `active` went.
pub(crate) fn keep_pin_tables_after_symbols(tabs: &mut Vec<TabInfo>, active: usize) -> usize {
    super::layer_stack_tab::keep_pages_after_owners(tabs, active, TabKind::is_pin_table, |kind| {
        matches!(kind, TabKind::SymbolEditor(_))
    })
}

impl Signex {
    /// `<symbol> · Pin table`; a symbol with no name goes by its file name.
    pub(crate) fn pin_table_title(&self, symbol: &Path) -> String {
        let name = self
            .document_state
            .symbol_editors
            .get(symbol)
            .map(|editor| editor.primitive().name.trim().to_owned())
            .filter(|name| !name.is_empty())
            .or_else(|| {
                symbol
                    .file_stem()
                    .map(|stem| stem.to_string_lossy().into_owned())
            })
            .unwrap_or_default();
        self.ui_state.i18n.message_plain(
            ids::EDITORS_PIN_TABLE_TAB_TITLE,
            &[("symbol", Arg::from(name))],
        )
    }

    /// Focus `symbol`'s Pin table tab, or open it right after the symbol's
    /// tab. `Edit pins in table…` lands here.
    pub(crate) fn open_pin_table_tab(&mut self, symbol: PathBuf) {
        if let Some(idx) = self.ensure_pin_table_tab(symbol)
            && idx != self.document_state.active_tab
        {
            self.park_active_schematic_session();
            self.document_state.active_tab = idx;
            self.sync_active_tab();
        }
        self.refresh_panel_ctx();
    }

    /// `symbol`'s Pin table tab, added right after the symbol's when
    /// missing. `None` when the symbol has no tab.
    fn ensure_pin_table_tab(&mut self, symbol: PathBuf) -> Option<usize> {
        let tabs = &self.document_state.tabs;
        let owner = tab_at(tabs, &symbol, false)?;
        if let Some(idx) = tab_at(tabs, &symbol, true) {
            return Some(idx);
        }
        let project_id = tabs[owner].project_id;
        let title = self.pin_table_title(&symbol);
        let at = owner + 1;
        self.document_state.tabs.insert(
            at,
            TabInfo {
                title,
                path: symbol,
                cached_document: None,
                project_id,
                kind: TabKind::PinTable,
            },
        );
        // The tab went in before any tab after the symbol's.
        if self.document_state.active_tab >= at {
            self.document_state.active_tab += 1;
        }
        Some(at)
    }

    /// Drop `symbol`'s Pin table tab while its symbol tab closes. The tab
    /// sits after the symbol's, so the symbol's index stays valid.
    pub(crate) fn drop_pin_table_tab_of(&mut self, symbol: &Path) {
        let docs = &mut self.document_state;
        let Some(page) = tab_at(&docs.tabs, symbol, true) else {
            return;
        };
        docs.tabs.remove(page);
        if docs.active_tab > page {
            docs.active_tab -= 1;
        }
        docs.active_tab = docs.active_tab.min(docs.tabs.len().saturating_sub(1));
    }

    /// The symbol whose Pin table tab the main window shows.
    pub(crate) fn pin_table_tab_in_window(&self, is_main: bool) -> Option<&Path> {
        let docs = &self.document_state;
        let tab = docs.tabs.get(docs.active_tab)?;
        (is_main && tab.kind.is_pin_table()).then_some(tab.path.as_path())
    }

    /// The Pin table of `symbol`. The rows come from the Properties panel's
    /// symbol context, built for the active tab; a pane showing another
    /// symbol's table stays blank until it is focused.
    pub(crate) fn view_pin_table_tab(&self, symbol: &Path) -> Element<'_, Message> {
        let ctx = &self.document_state.panel_ctx;
        let Some(sym) = ctx.symbol_editor.as_ref().filter(|sym| sym.path == symbol) else {
            return container(Space::new())
                .width(Length::Fill)
                .height(Length::Fill)
                .style(crate::styles::panel_region(&ctx.tokens))
                .boxed();
        };
        // The paste box's editor is read from the Model, not the context's
        // copy, so its cursor is the one typing moves.
        let table = self
            .library
            .symbol_props
            .get(symbol)
            .map_or(&sym.props.table, |props| &props.table);
        crate::panels::view_pin_table(
            sym,
            table,
            &ctx.collapsed_sections,
            ctx.numeric_draft.as_ref(),
            &ctx.tokens,
            &ctx.i18n,
        )
        .map(|msg| Message::Dock(DockMessage::Panel(msg)))
        .boxed()
    }
}

#[cfg(test)]
#[path = "pin_table_tab_tests.rs"]
mod tests;
