//! The Library panel's Footprints kind (#119, was the Footprint Library
//! panel, #121): the footprints inside the `.snxfpt` open in the footprint
//! editor. A click highlights a row; Edit (or a double click) makes it the
//! one the editor shows.

use iced::widget::{Column, container};
use iced::{Element, Length, Widget};
use signex_i18n::{ids, t};

use super::KindView;
use super::frame::{self, Crumb, RowMark, RowText, SP, Tools, panel};
use crate::dock::DockMessage;
use crate::library::state::LibraryState;
use crate::panels::{FootprintEditorPanelContext, PanelContext, PanelMsg};

pub(super) fn view<'a>(library: &'a LibraryState, ctx: &'a PanelContext) -> KindView<'a> {
    let i18n = &*ctx.i18n;
    let tokens = &ctx.tokens;
    let Some(fp) = ctx.footprint_editor.as_ref() else {
        return KindView {
            crumb: Crumb::Note(t!(i18n, ids::LIBRARY_PANEL_NO_FOOTPRINT_FILE)),
            tools: Tools::default(),
            list: frame::list_note(t!(i18n, ids::PANELS_FOOTPRINT_LIBRARY_EMPTY), 0.0, tokens),
        };
    };
    // The footprint's display name: the file stem is a uuid.
    let file = if fp.footprint_name.is_empty() {
        frame::file_name_or(
            &fp.path,
            t!(i18n, ids::PANELS_FOOTPRINT_LIBRARY_UNKNOWN_FILE),
        )
    } else {
        fp.footprint_name.clone()
    };
    let crumb = Crumb::File {
        library: frame::library_name_of(&fp.path, library),
        file,
        count: t!(
            i18n,
            ids::LIBRARY_PANEL_CRUMB_FOOTPRINTS,
            "count" => fp.internal_footprints.len()
        ),
    };
    let selected = fp.internal_selected_idx;
    // Place stays off: placing a footprint from here is not wired yet.
    let tools = Tools {
        place: None,
        add: Some(panel(PanelMsg::FpLibraryAddInternal)),
        edit: selected.map(|i| panel(PanelMsg::FpLibraryEditInternal(i))),
        delete: selected.map(|i| panel(PanelMsg::FpLibraryDeleteInternal(i))),
    };
    KindView {
        crumb,
        tools,
        list: list(fp, &library.panel_search, ctx),
    }
}

/// The footprint rows that match the filter. The one the editor shows has
/// the selection fill; one only highlighted has a lighter one.
fn list<'a>(
    fp: &'a FootprintEditorPanelContext,
    filter: &str,
    ctx: &'a PanelContext,
) -> Element<'a, DockMessage> {
    let i18n = &*ctx.i18n;
    let tokens = &ctx.tokens;
    let needle = i18n.fold_for_search(filter.trim());
    let mut col: Column<Element<'a, DockMessage>> = Column::new().spacing(0).width(Length::Fill);
    let mut shown = 0;
    for (idx, row) in fp.internal_footprints.iter().enumerate() {
        if !needle.is_empty() && !i18n.fold_for_search(&row.name).contains(&needle) {
            continue;
        }
        shown += 1;
        let mark = if row.is_active {
            RowMark::Selected
        } else if fp.internal_selected_idx == Some(idx) {
            RowMark::Highlighted
        } else {
            RowMark::None
        };
        col = col.push(frame::list_row(
            RowText {
                name: row.name.clone(),
                caption: Some(t!(i18n, ids::LIBRARY_PANEL_PADS, "count" => row.pad_count).into()),
            },
            mark,
            0.0,
            panel(PanelMsg::FpLibrarySelectInternal(idx)),
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
    container(col)
        .padding([0.0, SP.s])
        .width(Length::Fill)
        .boxed()
}
