//! The Library panel's Footprints kind (mockup row 119, was the Footprint
//! Library panel, mockup row 121): the footprints inside the `.snxfpt` open in the footprint
//! editor. A click highlights a row; Edit (or a double click) makes it the
//! one the editor shows.

use iced::widget::{Column, container};
use iced::{Element, Length, Widget};
use signex_i18n::{ids, t};

use super::KindView;
use super::frame::{self, Crumb, RowLook, RowMark, RowText, SP, Tools, panel};
use crate::dock::DockMessage;
use crate::library::state::{LibraryPanelKind, LibraryState};
use crate::panels::{FootprintEditorPanelContext, FootprintLibInternalRow, PanelContext, PanelMsg};

pub(super) fn view<'a>(library: &'a LibraryState, ctx: &'a PanelContext) -> KindView<'a> {
    let i18n = &*ctx.i18n;
    let tokens = &ctx.tokens;
    let Some(fp) = ctx.footprint_editor.as_ref() else {
        return KindView {
            crumb: Crumb::Note(t!(i18n, ids::LIBRARY_PANEL_NO_FOOTPRINT_FILE)),
            tools: Tools {
                place_blocked: Some(ids::LIBRARY_PANEL_NO_FOOTPRINT_FILE),
                ..Tools::default()
            },
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
    // Edit and Delete act on a footprint only while the filter shows its
    // row. Edit also takes the row the editor shows, which draws selected
    // (mockup row 119); Delete needs an explicit highlight.
    let needle = i18n.fold_for_search(library.panel_search.trim());
    let in_view = |idx: &usize| {
        fp.internal_footprints
            .get(*idx)
            .is_some_and(|row| shows(row, &needle, i18n))
    };
    let selected = fp.internal_selected_idx.filter(in_view);
    let edited = fp
        .internal_selected_idx
        .or_else(|| fp.internal_footprints.iter().position(|row| row.is_active))
        .filter(in_view);
    let (place, place_blocked) = place_tool(fp, edited);
    let tools = Tools {
        place,
        place_blocked,
        add: Some(panel(PanelMsg::FpLibraryAddInternal)),
        edit: edited.map(|i| panel(PanelMsg::FpLibraryEditInternal(i))),
        delete: selected.map(|i| panel(PanelMsg::FpLibraryDeleteInternal(i))),
        delete_blocked: None,
    };
    KindView {
        crumb,
        tools,
        list: list(
            fp,
            &library.panel_search,
            RowLook::of(
                library.library_panel.row_style,
                LibraryPanelKind::Footprints,
            ),
            ctx,
        ),
    }
}

/// What Place does for the row Edit would act on: its message, or why it is
/// off (mockup row 119).
fn place_tool(
    fp: &FootprintEditorPanelContext,
    row: Option<usize>,
) -> (Option<DockMessage>, Option<signex_i18n::MessageId>) {
    let Some(idx) = row else {
        return (None, Some(ids::LIBRARY_PANEL_PLACE_NO_ROW));
    };
    match fp.internal_footprints.get(idx).and_then(|r| r.place_block) {
        Some(block) => (None, Some(block.reason())),
        None => (Some(panel(PanelMsg::FpLibraryPlaceInternal(idx))), None),
    }
}

/// Whether a footprint's row shows under the folded filter `needle`.
fn shows(row: &FootprintLibInternalRow, needle: &str, i18n: &signex_i18n::I18n) -> bool {
    needle.is_empty() || i18n.fold_for_search(&row.name).contains(needle)
}

/// The footprint rows that match the filter. The one the editor shows has
/// the selection fill; one only highlighted has a lighter one.
fn list<'a>(
    fp: &'a FootprintEditorPanelContext,
    filter: &str,
    look: RowLook,
    ctx: &'a PanelContext,
) -> Element<'a, DockMessage> {
    let i18n = &*ctx.i18n;
    let tokens = &ctx.tokens;
    let needle = i18n.fold_for_search(filter.trim());
    let mut col: Column<Element<'a, DockMessage>> = Column::new().spacing(0).width(Length::Fill);
    let mut shown = 0;
    for (idx, row) in fp.internal_footprints.iter().enumerate() {
        if !shows(row, &needle, i18n) {
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
            look,
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
