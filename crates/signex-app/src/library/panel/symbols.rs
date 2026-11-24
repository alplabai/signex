//! The Library panel's Symbols kind (mockup row 119, was the SCH Library
//! panel, mockup row 120): the symbols of the `.snxsym` open in the symbol
//! editor, the parts of the active symbol, and the warning of a Delete that
//! hits a placed symbol.

use iced::widget::{Column, column, container, row};
use iced::{Alignment, Element, Length, Padding, Widget};
use signex_i18n::{I18n, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, raw, text};

use super::KindView;
use super::frame::{self, Crumb, RowMark, RowText, SP, TS, Tools, panel};
use crate::dock::DockMessage;
use crate::library::state::LibraryState;
use crate::panels::{PanelContext, PanelMsg, SymbolDeleteWarning, SymbolEditorPanelContext};
use crate::styles::ti;
use crate::styles::tokens::resolve;

/// Collapse key of the "Parts of <symbol>" section.
pub(super) const PARTS_SECTION: &str = "library-panel-symbol-parts";

pub(super) fn view<'a>(library: &'a LibraryState, ctx: &'a PanelContext) -> KindView<'a> {
    let i18n = &*ctx.i18n;
    let tokens = &ctx.tokens;
    let Some(sym) = ctx.symbol_editor.as_ref() else {
        return KindView {
            crumb: Crumb::Note(t!(i18n, ids::LIBRARY_PANEL_NO_SYMBOL_FILE)),
            tools: Tools {
                place_blocked: Some(ids::LIBRARY_PANEL_PLACE_SYMBOLS),
                ..Tools::default()
            },
            list: frame::list_note(t!(i18n, ids::PANELS_SCH_LIBRARY_EMPTY), 0.0, tokens),
        };
    };
    let crumb = Crumb::File {
        library: frame::library_name_of(&sym.path, library),
        file: frame::file_name_or(&sym.path, t!(i18n, ids::PANELS_SCH_LIBRARY_UNTITLED_FILE)),
        count: t!(
            i18n,
            ids::LIBRARY_PANEL_CRUMB_SYMBOLS,
            "count" => sym.symbols_in_file.len()
        ),
    };
    // Place stays off with its reason until symbols have this flow (mockup
    // row 119). Edit stays off: the symbol is already open in its tab. Delete
    // acts on the active symbol only while the filter shows its row.
    let needle = i18n.fold_for_search(library.panel_search.trim());
    let active_shown = sym
        .symbols_in_file
        .iter()
        .any(|entry| entry.idx == sym.active_idx && shows(entry, &needle, i18n));
    let tools = Tools {
        place: None,
        place_blocked: Some(ids::LIBRARY_PANEL_PLACE_SYMBOLS),
        add: Some(panel(PanelMsg::SchLibraryAddSymbol)),
        edit: None,
        delete: (sym.symbols_in_file.len() > 1 && active_shown)
            .then(|| panel(PanelMsg::SchLibraryDeleteSymbol(sym.active_idx))),
        delete_blocked: None,
    };
    KindView {
        crumb,
        tools,
        list: list(sym, &library.panel_search, ctx),
    }
}

/// Whether a symbol's row shows under the folded filter `needle`: its name
/// or description holds it.
fn shows(entry: &crate::panels::SymbolFileEntry, needle: &str, i18n: &I18n) -> bool {
    needle.is_empty()
        || i18n.fold_for_search(&entry.name).contains(needle)
        || i18n.fold_for_search(&entry.description).contains(needle)
}

/// The symbol rows that match the filter, the parts section and the
/// in-use warning.
fn list<'a>(
    sym: &'a SymbolEditorPanelContext,
    filter: &str,
    ctx: &'a PanelContext,
) -> Element<'a, DockMessage> {
    let i18n = &*ctx.i18n;
    let tokens = &ctx.tokens;
    let needle = i18n.fold_for_search(filter.trim());
    let mut col: Column<Element<'a, DockMessage>> = Column::new().spacing(0).width(Length::Fill);
    let mut shown = 0;
    for entry in &sym.symbols_in_file {
        if !shows(entry, &needle, i18n) {
            continue;
        }
        shown += 1;
        let mark = if entry.idx == sym.active_idx {
            RowMark::Selected
        } else {
            RowMark::None
        };
        col = col.push(frame::list_row(
            RowText {
                name: entry.name.clone(),
                caption: Some(UserText::new(entry.description.clone()).into()),
            },
            mark,
            0.0,
            panel(PanelMsg::SchLibrarySelectSymbol(entry.idx)),
            tokens,
        ));
    }
    if shown == 0 && !needle.is_empty() {
        col = col.push(frame::list_note(
            t!(i18n, ids::LIBRARY_PANEL_NO_MATCHES),
            0.0,
            tokens,
        ));
    }
    let mut body: Column<Element<'a, DockMessage>> = column![
        container(col)
            .padding([0.0, SP.s])
            .width(Length::Fill)
            .boxed()
    ]
    .width(Length::Fill);
    if let Some(parts) = parts_section(sym, ctx) {
        body = body.push(parts);
    }
    if let Some(warning) = sym.delete_warning.as_ref() {
        body = body.push(delete_warning(warning, tokens, i18n));
    }
    body.boxed()
}

/// "Parts of <symbol>": part 0 (shared) and each part of a multi-part
/// symbol; a click makes the part active. Collapsible.
fn parts_section<'a>(
    sym: &'a SymbolEditorPanelContext,
    ctx: &'a PanelContext,
) -> Option<Element<'a, DockMessage>> {
    if sym.active_max_part <= 1 && !sym.active_has_part_zero {
        return None;
    }
    let i18n = &*ctx.i18n;
    let tokens = &ctx.tokens;
    let r = resolve(tokens);
    let name = sym
        .symbols_in_file
        .iter()
        .find(|e| e.idx == sym.active_idx)
        .map(|e| e.name.clone())
        .unwrap_or_default();
    let count = u32::from(sym.active_max_part) + u32::from(sym.active_has_part_zero);
    let collapsed = ctx.collapsed_sections.contains(PARTS_SECTION);
    let header = button(
        row![
            signex_widgets::chevron::chevron(!collapsed, TS.body, r.ink4).boxed(),
            text(t!(i18n, ids::LIBRARY_PANEL_PARTS_OF, "symbol" => name))
                .size(TS.section)
                .color(r.ink3)
                .width(Length::Fill)
                .boxed(),
            crate::library::editor::kit::toned_pill(
                i18n.count(i64::from(count)),
                crate::styles::PillTone::Neutral,
                tokens,
            ),
        ]
        .spacing(SP.s)
        .align_y(Alignment::Center),
    )
    .padding(Padding {
        top: 0.0,
        right: SP.m,
        bottom: 0.0,
        left: SP.s,
    })
    .height(frame::ROW_H)
    .width(Length::Fill)
    .on_press(panel(PanelMsg::ToggleSection(PARTS_SECTION.to_owned())))
    .style(crate::styles::subtle_button(tokens));
    let mut col: Column<Element<'a, DockMessage>> =
        column![header.boxed()].spacing(0).width(Length::Fill);
    if collapsed {
        return Some(section_frame(col));
    }
    let primary = ti(tokens.text);
    let muted = ti(tokens.text_secondary);
    let fill = ti(tokens.selection);
    if sym.active_has_part_zero {
        col = col.push(
            crate::panels::part_tree_row(
                t!(i18n, ids::PANELS_SCH_LIBRARY_PART_SHARED),
                0,
                sym.active_part == 0,
                primary,
                muted,
                fill,
            )
            .map(DockMessage::Panel)
            .boxed(),
        );
    }
    for part in 1..=sym.active_max_part {
        col = col.push(
            crate::panels::part_tree_row(
                t!(i18n, ids::PANELS_SCH_LIBRARY_PART, "part" => u32::from(part)),
                part,
                sym.active_part == part,
                primary,
                muted,
                fill,
            )
            .map(DockMessage::Panel)
            .boxed(),
        );
    }
    Some(section_frame(col))
}

fn section_frame<'a>(col: Column<Element<'a, DockMessage>>) -> Element<'a, DockMessage> {
    container(col)
        .padding(Padding {
            top: SP.m,
            right: SP.s,
            bottom: 0.0,
            left: SP.s,
        })
        .width(Length::Fill)
        .boxed()
}

/// The in-use warning shown instead of deleting at once (#1845): names the
/// rows and sheets, and lets the user delete anyway.
fn delete_warning<'a>(
    warning: &'a SymbolDeleteWarning,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, DockMessage> {
    let r = resolve(tokens);
    let ink = ti(tokens.text);
    let mut block: Column<Element<'a, DockMessage>> = Column::new()
        .spacing(SP.xs)
        .push(text(warning.heading(i18n)).size(TS.body).color(ink).boxed());
    for line in &warning.lines {
        block = block.push(
            // raw: the in-use lines are worded in English by the delete check
            // in `library::symbol_delete` (#2060).
            text(raw(line.as_str()))
                .size(TS.caption)
                .color(r.ink3)
                .boxed(),
        );
    }
    block = block.push(
        text(t!(i18n, ids::PANELS_SCH_LIBRARY_DELETE_WARNING_NOTE))
            .size(TS.caption)
            .color(r.ink3)
            .boxed(),
    );
    let confirm = button(text(t!(i18n, ids::PANELS_SCH_LIBRARY_DELETE_ANYWAY)).size(TS.body))
        .padding([SP.xs, SP.m])
        .on_press(panel(PanelMsg::SchLibraryDeleteConfirmed(warning.idx)))
        .style(crate::styles::danger_button(tokens));
    let cancel = button(text(t!(i18n, ids::PANELS_SCH_LIBRARY_DELETE_CANCEL)).size(TS.body))
        .padding([SP.xs, SP.m])
        .on_press(panel(PanelMsg::SchLibraryDeleteCancelled))
        .style(crate::styles::outline_button(tokens));
    block = block.push(row![confirm.boxed(), cancel.boxed()].spacing(SP.m).boxed());
    container(block)
        .padding([SP.m, SP.m])
        .width(Length::Fill)
        .boxed()
}
