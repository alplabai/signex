//! Further states of the `lib` and `new-fpsketch` mockup rows, reached with
//! real messages on a fixture library whose resistor row carries a
//! measurement, a distributor listing and a cached price. `open` fails when
//! the state was not reached.
//!
//! Not here: Broken Binding of m138 (nothing in the app raises that dialog)
//! and the open role dropdown of m190 (no pointer step can aim a pick list).

use super::helpers_d::reached;
use super::helpers_f as h;
use super::pointer::PointerStep;
use super::{lib, states_final, states_sketch};
use crate::fixture::{self, Fixture};
use crate::render;
use crate::surface::{OpenFn, Surface, WindowSel};
use signex_app::app::{Message, Signex, UiMsg};
use signex_app::dock::{DockMessage, PanelPosition};
use signex_app::library::LibraryMessage;
use signex_app::library::editor::footprint::state::FootprintContextTarget;
use signex_app::library::editor::footprint::{EditorMode, SketchTool};
use signex_app::library::messages::{
    BrowserEditMsg, EditorMsg, FootprintEditorMsg, ParamKindMsg, PickerMsg, PrimitiveEdit,
    PrimitivePickerMsg, RangeBound, RangeFilterMsg, RoleTag, SymbolContextTargetMsg,
    SymbolEditorMsg,
};
use signex_app::library::part_sync::Choice;
use signex_app::library::recovery::RecoveryDialog;
use signex_app::library::state::{EditorAddress, PrimitiveChoice, PrimitivePickerTarget};
use signex_app::library::updates_dialog::{
    BumpKind, ConflictChoice, LibraryUpdateEntry, LibraryUpdatesState,
};
use signex_app::panels::{PanelKind, PanelMsg};
use signex_library::distributor::PriceBreak;
use signex_library::{
    CachedPricing, DistributorListing, LibraryAdapter as _, LocalGitAdapter, ParamValue,
    PricingUpdate, PrimitiveKind, PrimitiveRef, RowId,
};
use signex_sketch::SketchData;
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_types::project::{LibraryEntry, LibraryEntryKind};
use signex_types::quantity::{SiPrefix, Unit};
use signex_types::schematic::StampField;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

/// The stock resistor row the fixture fills in.
const RESISTOR_PN: &str = "STOCK-R";
const RESISTOR_SKU: &str = "311-10.0KLRCT-ND";
/// Where pad 1 of the stock SOIC-8 sits in the sketch workspace (m188).
const PAD_1_HOVER: &[PointerStep] = &[PointerStep::HoverAt(431, 555)];

fn row(id: &'static str, title: &'static str, open: OpenFn) -> Surface {
    Surface {
        id,
        title,
        open,
        window: WindowSel::Auto,
        checks: &[],
    }
}

pub fn rows() -> Vec<Surface> {
    let mut rows = library_rows();
    rows.extend(dialog_rows());
    rows.extend(editor_rows());
    rows
}

fn library_rows() -> Vec<Surface> {
    vec![
        row("m118-range", "Browser: range filter", open_browser_range),
        row("m118-price", "Browser: Price column", open_browser_price),
        row("m128-edits", "Component editor: edit pills", open_row_edits),
        row("m129-searched", "Picker: searched", open_picker_searched),
        row("m130-conflict", "Edit row: PN conflict", open_edit_conflict),
        row("m130-stale", "Edit row: row changed", open_edit_stale),
        row("m131-highlight", "Primitive picker: row", open_highlight),
        row("m131-groups", "Primitive picker: groups", open_loose_groups),
        row("m131-copy", "Primitive picker: copy", open_copy_confirm),
    ]
}

fn dialog_rows() -> Vec<Surface> {
    vec![
        row("m133-exists", "New Library: refused", open_create_refused),
        row("m135-conflict", "Library Updates: conflict", open_conflict),
        row("m137-in-use", "Delete Symbol: in use", open_symbol_in_use),
        row("m138-git", "Recovery: Git missing", open_git_missing),
    ]
}

fn editor_rows() -> Vec<Surface> {
    vec![
        row("m140-move-by", "Move Selection By", open_move_by),
        row("m140-align", "Align Selection", open_align),
        row("m141-context", "Symbol context menu", open_symbol_menu),
        Surface {
            window: WindowSel::MainPointer(PAD_1_HOVER),
            ..row("m188-snap", "Snap badge on pad 1", open_line_armed)
        },
        row("m190-outcome", "Bake: outcome", open_bake_outcome),
        row("m190-profiles", "Bake: Roles list", open_role_list),
    ]
}

fn library_msg(app: &mut Signex, msg: LibraryMessage) -> Result<(), String> {
    h::settle(app, Message::Library(msg))
}

fn browser_edit(app: &mut Signex, fx: &Fixture, msg: BrowserEditMsg) -> Result<(), String> {
    library_msg(
        app,
        LibraryMessage::BrowserEdit {
            library_path: fx.stock.clone(),
            msg,
        },
    )
}

/// Write the resistor's value, listing and cached price into the library
/// copy on disk, before the app mounts it.
fn fill_resistor(fx: &Fixture) -> Result<(), String> {
    let adapter = LocalGitAdapter::open(&fx.stock).map_err(|e| format!("open stock: {e:?}"))?;
    let table = "components";
    let mut part = adapter
        .read_table(table)
        .map_err(|e| format!("read stock: {e:?}"))?
        .into_iter()
        .find(|r| r.internal_pn.as_str() == RESISTOR_PN)
        .ok_or("stock has no resistor row")?;
    part.description = "10k 0402 1% thick film resistor".into();
    part.parameters.insert(
        "value".into(),
        ParamValue::Measurement {
            value: 10_000.0,
            unit: "ohm".into(),
        },
    );
    part.supply
        .push(DistributorListing::new("DigiKey", RESISTOR_SKU));
    part.content_hash =
        signex_library::hash_row_content(&part).map_err(|e| format!("hash row: {e:?}"))?;
    let row_id = part.row_id;
    adapter
        .update_row(table, part, "audit fixture")
        .map_err(|e| format!("write row: {e:?}"))?;
    let update = PricingUpdate {
        table: table.into(),
        row_id,
        distributor: "DigiKey".into(),
        pricing: CachedPricing {
            sku: RESISTOR_SKU.into(),
            currency: "USD".into(),
            stock: Some(124_000),
            breaks: vec![
                PriceBreak {
                    qty: 1,
                    unit_price: 0.10,
                },
                PriceBreak {
                    qty: 100,
                    unit_price: 0.021,
                },
            ],
            fetched_at: chrono::Utc::now(),
        },
    };
    let stored = adapter
        .store_pricing(vec![update], &AtomicBool::new(false), "audit fixture")
        .map_err(|e| format!("store price: {e:?}"))?;
    reached(stored.stored == 1, "the fixture price was not stored")
}

/// The table and id of the stock resistor row.
fn resistor(app: &Signex, fx: &Fixture) -> Result<(String, RowId), String> {
    let (_, part) = app
        .library
        .all_components()
        .into_iter()
        .find(|(_, c)| c.internal_pn.as_str() == RESISTOR_PN)
        .ok_or("the mounted library lists no resistor")?;
    let table = app
        .library
        .table_of_row(&fx.stock, part.row_id)
        .ok_or("no table for the resistor")?;
    Ok((table, RowId(part.row_id)))
}

/// The filled-in library's Browser tab with the resistor selected and the
/// side docks collapsed, as m118 shows it.
fn open_filled_browser(app: &mut Signex, fx: &Fixture) -> Result<(), String> {
    fill_resistor(fx)?;
    fixture::mount_stock(app, fx)?;
    library_msg(app, LibraryMessage::OpenLibraryBrowser(fx.stock.clone()))?;
    for side in [PanelPosition::Left, PanelPosition::Right] {
        let _ = app.update(Message::Dock(DockMessage::ToggleCollapse(side)));
    }
    let (table, row_id) = resistor(app, fx)?;
    library_msg(
        app,
        LibraryMessage::BrowserSelectRow {
            library_path: fx.stock.clone(),
            table,
            row_id,
        },
    )
}

fn open_browser_range(
    app: &mut Signex,
    _: &mut iced::Renderer,
    fx: &Fixture,
) -> Result<(), String> {
    open_filled_browser(app, fx)?;
    let key = || "value".to_string();
    for msg in [
        RangeFilterMsg::ColumnPicked {
            key: key(),
            unit: Unit::Ohm,
        },
        RangeFilterMsg::NumberEdited {
            key: key(),
            bound: RangeBound::Low,
            text: "1".into(),
        },
        RangeFilterMsg::PrefixPicked {
            key: key(),
            bound: RangeBound::Low,
            prefix: SiPrefix::Kilo,
        },
    ] {
        library_msg(
            app,
            LibraryMessage::BrowserRangeFilter {
                library_path: fx.stock.clone(),
                msg,
            },
        )?;
    }
    // A range edit skips the end-of-update sync the grid is drawn from.
    library_msg(app, LibraryMessage::Noop)?;
    let filtered = app
        .library
        .library_browsers
        .get(&fx.stock)
        .is_some_and(|b| !b.range_filters.is_empty());
    reached(filtered, "the range filter did not take")
}

fn open_browser_price(
    app: &mut Signex,
    _: &mut iced::Renderer,
    fx: &Fixture,
) -> Result<(), String> {
    open_filled_browser(app, fx)?;
    let priced = app
        .library
        .library_at(&fx.stock)
        .is_some_and(|l| !l.pricing.is_empty());
    reached(priced, "the mounted library read no cached price")
}

/// A custom measurement holding text that does not parse: the header shows
/// both the unsaved and the parse pill.
fn open_row_edits(app: &mut Signex, _: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    fixture::mount_stock(app, fx)?;
    let (table, row_id) = resistor(app, fx)?;
    library_msg(
        app,
        LibraryMessage::OpenComponentRow {
            library_path: fx.stock.clone(),
            table: table.clone(),
            row_id,
        },
    )?;
    let name = "tolerance";
    for msg in [
        EditorMsg::ParamNewNameChanged(name.into()),
        EditorMsg::ParamAddCustom {
            kind: ParamKindMsg::Measurement("%".into()),
        },
        EditorMsg::ParamSetMeasurementBuf {
            name: name.into(),
            buf: "1x".into(),
            slot: None,
        },
    ] {
        library_msg(
            app,
            LibraryMessage::EditorEvent {
                library_path: fx.stock.clone(),
                table: table.clone(),
                row_id,
                msg,
            },
        )?;
    }
    library_msg(app, LibraryMessage::Noop)
}

fn open_picker_searched(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
) -> Result<(), String> {
    fill_resistor(fx)?;
    fixture::open_sample(app, r, fx);
    fixture::mount_stock(app, fx)?;
    library_msg(app, LibraryMessage::OpenPicker)?;
    library_msg(
        app,
        LibraryMessage::Picker(PickerMsg::FilterChanged("10k 0402".into())),
    )?;
    let (_, row_id) = resistor(app, fx)?;
    library_msg(
        app,
        LibraryMessage::Picker(PickerMsg::SelectComponent(row_id.0)),
    )?;
    let selected = app
        .library
        .picker
        .as_ref()
        .is_some_and(|p| p.shown_selection().is_some());
    reached(selected, "the search did not list the resistor")
}

/// Template and custom parameter rows, with the Internal PN of another row.
fn open_edit_conflict(
    app: &mut Signex,
    _: &mut iced::Renderer,
    fx: &Fixture,
) -> Result<(), String> {
    states_final::open_measurement_pair(app, fx)?;
    states_final::add_measurement(app, fx, "50", "V", SiPrefix::One)?;
    browser_edit(app, fx, BrowserEditMsg::SetInternalPn("STOCK-C".into()))?;
    let taken = app
        .library
        .library_browsers
        .get(&fx.stock)
        .and_then(|b| b.edit_modal.as_ref())
        .is_some_and(|m| m.draft.internal_pn.as_str() == "STOCK-C");
    reached(taken, "the draft did not take the other row's PN")
}

/// The row changed on disk while the modal was open, so Save is refused and
/// the footer offers Reload saved version.
fn open_edit_stale(app: &mut Signex, _: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    lib::open_edit_row(app, fx)?;
    let address = app
        .library
        .library_browsers
        .get(&fx.stock)
        .and_then(|b| b.edit_modal.as_ref())
        .map(|m| m.address.clone())
        .ok_or("the Edit row modal did not open")?;
    let other = LocalGitAdapter::open(&fx.stock).map_err(|e| format!("open stock: {e:?}"))?;
    let mut stored = other
        .read_table(&address.table)
        .map_err(|e| format!("read stock: {e:?}"))?
        .into_iter()
        .find(|r| r.row_id == address.row_id.0)
        .ok_or("the row is not on disk")?;
    stored.description = "Changed by another session".into();
    stored.content_hash =
        signex_library::hash_row_content(&stored).map_err(|e| format!("hash row: {e:?}"))?;
    other
        .update_row(&address.table, stored, "audit fixture")
        .map_err(|e| format!("write row: {e:?}"))?;
    browser_edit(app, fx, BrowserEditMsg::SetDescription("My edit".into()))?;
    h::unlogged(|| browser_edit(app, fx, BrowserEditMsg::Save))?;
    let stale = app
        .library
        .library_browsers
        .get(&fx.stock)
        .and_then(|b| b.edit_modal.as_ref())
        .is_some_and(|m| m.stale_base);
    reached(stale, "the save was not refused as stale")
}

/// Pick Symbol for the resistor row, in its Browser tab.
fn open_symbol_picker(app: &mut Signex, fx: &Fixture) -> Result<(), String> {
    fixture::mount_stock(app, fx)?;
    library_msg(app, LibraryMessage::OpenLibraryBrowser(fx.stock.clone()))?;
    let (table, row_id) = resistor(app, fx)?;
    library_msg(
        app,
        LibraryMessage::OpenPrimitivePicker {
            kind: PrimitiveKind::Symbol,
            target: PrimitivePickerTarget::BrowserRow(EditorAddress::new(
                fx.stock.clone(),
                table,
                row_id,
            )),
        },
    )
}

fn open_highlight(app: &mut Signex, _: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    open_symbol_picker(app, fx)?;
    let library = app
        .library
        .library_at(&fx.stock)
        .ok_or("stock is not mounted")?;
    let symbol = library
        .cached_symbols
        .iter()
        .find(|s| s.name == "R")
        .ok_or("stock lists no R symbol")?;
    let choice = PrimitiveChoice::Pick(PrimitiveRef::new(library.library_id, symbol.uuid));
    library_msg(
        app,
        LibraryMessage::PrimitivePicker(PrimitivePickerMsg::RowHighlighted(choice)),
    )?;
    let lit = app
        .library
        .primitive_picker
        .as_ref()
        .is_some_and(|p| p.selection.highlighted.is_some());
    reached(lit, "the row was not highlighted")
}

/// A project listing one symbol file of a library that is not open and one
/// file in no library, then the picker that groups them.
fn open_loose_picker(app: &mut Signex, fx: &Fixture) -> Result<(), String> {
    let dir = h::new_dir(fx, "picker-project")?;
    let sheet = dir.join("linear-regulator.snxsch");
    h::copy_sample_sheet(fx, &sheet)?;
    let loose = dir.join("regulator-sense.snxsym");
    copy(&fx.symbol, &loose)?;
    let closed = dir.join("sensors-lib");
    std::fs::create_dir_all(closed.join("symbols")).map_err(|e| format!("library: {e}"))?;
    copy(&fx.stock, &closed.join("sensors-lib.snxlib"))?;
    let owned = closed.join("symbols/ina219.snxsym");
    copy(&fx.symbol, &owned)?;
    let project = h::write_project(&dir, "picker", &["linear-regulator.snxsch"], None)?;
    h::open(app, &project)?;
    let loaded = app
        .document_state
        .projects
        .last_mut()
        .ok_or("the project did not open")?;
    for path in [loose, owned] {
        loaded.data.libraries.push(symbol_entry(path));
    }
    open_symbol_picker(app, fx)?;
    let groups = app
        .library
        .primitive_picker
        .as_ref()
        .is_some_and(|p| p.loose.iter().any(|s| s.library.is_some()) && p.loose.len() >= 2);
    reached(groups, "the picker did not list both loose symbols")
}

fn symbol_entry(path: PathBuf) -> LibraryEntry {
    LibraryEntry {
        path,
        kind: LibraryEntryKind::Shared,
        library_id: None,
        unknown: Default::default(),
    }
}

fn copy(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::copy(from, to)
        .map(|_| ())
        .map_err(|e| format!("copy {}: {e}", to.display()))
}

fn open_loose_groups(app: &mut Signex, _: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    open_loose_picker(app, fx)
}

fn open_copy_confirm(app: &mut Signex, _: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    open_loose_picker(app, fx)?;
    let symbol = app
        .library
        .primitive_picker
        .as_ref()
        .and_then(|p| p.loose.iter().find(|s| s.library.is_none()).cloned())
        .ok_or("no symbol in no library")?;
    library_msg(
        app,
        LibraryMessage::PrimitivePicker(PrimitivePickerMsg::PickLoose(symbol)),
    )?;
    let asked = app
        .library
        .primitive_picker
        .as_ref()
        .is_some_and(|p| p.copy_confirm.is_some());
    reached(asked, "picking a loose symbol asked nothing")
}

/// Create Library on a path that already holds a library.
fn open_create_refused(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
) -> Result<(), String> {
    fixture::open_sample(app, r, fx);
    let folder = fx
        .sample
        .parent()
        .ok_or("sample project has no folder")?
        .join("linear-regulator-lib");
    std::fs::create_dir_all(&folder).map_err(|e| format!("library folder: {e}"))?;
    let lib_path = folder.join("linear-regulator-lib.snxlib");
    copy(&fx.stock, &lib_path)?;
    library_msg(
        app,
        LibraryMessage::CreateLibraryAtPath {
            project_path: fx.sample.clone(),
            lib_path,
        },
    )?;
    library_msg(app, LibraryMessage::LibraryCreateOptionsConfirm)?;
    let refused = app
        .library
        .create_options
        .as_ref()
        .is_some_and(|s| s.existing_library);
    reached(refused, "the existing library was not refused")
}

/// Library Updates with a part whose MPN moved on both sides. The state is
/// seeded with what a drift scan returns, as row m135 seeds it.
fn open_conflict(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    fixture::open_sample(app, r, fx);
    fixture::mount_stock(app, fx)?;
    let sheet = fx.sample.with_extension("snxsch");
    let library = app
        .library
        .library_at(&fx.stock)
        .ok_or("stock is not mounted")?;
    let (library_id, library_name) = (library.library_id, library.display_name.clone());
    let (_, part) = app
        .library
        .all_components()
        .into_iter()
        .find(|(_, c)| c.internal_pn.as_str() == RESISTOR_PN)
        .ok_or("the mounted library lists no resistor")?;
    let entry = LibraryUpdateEntry {
        symbol_uuid: uuid::Uuid::now_v7(),
        ref_des: "R1".into(),
        internal_pn: part.internal_pn.as_str().to_string(),
        lib_id: format!("{library_id}:{}", part.row_id),
        library_id,
        library_name,
        row_id: RowId(part.row_id),
        library_path: fx.stock.clone(),
        current_version: "1.2.0".into(),
        latest_version: "1.2.1".into(),
        bump_kind: BumpKind::Patch,
        selected: false,
        conflicts: vec![ConflictChoice {
            field: StampField::Mpn,
            mine: "RC0402FR-0710KL".into(),
            library: "RC0402FR-1310KL".into(),
            choice: Choice::KeepMine,
        }],
    };
    app.library.library_updates = Some(LibraryUpdatesState::new(sheet, vec![entry]));
    library_msg(app, LibraryMessage::Noop)
}

/// Delete on a symbol a component row binds: its file holds a second symbol,
/// so only the in-use warning stands in the way.
fn open_symbol_in_use(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
) -> Result<(), String> {
    fixture::mount_stock(app, fx)?;
    let file = fx
        .stock
        .parent()
        .ok_or("stock has no folder")?
        .join("symbols/R.snxsym");
    h::open(app, &file)?;
    let _ = render::draw_main(r, app);
    h::open_panel(app, PanelKind::SchLibrary);
    for msg in [
        PanelMsg::SchLibraryAddSymbol,
        PanelMsg::SchLibraryDeleteSymbol(0),
    ] {
        h::settle(app, Message::Dock(DockMessage::Panel(msg)))?;
    }
    let warned = app
        .library
        .pending_symbol_delete
        .as_ref()
        .is_some_and(|p| p.rows > 0);
    reached(warned, "Delete raised no in-use warning")
}

/// A library whose `.git` no longer opens, opened the way File > Library does.
fn open_git_missing(app: &mut Signex, _: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    let git = fx.stock.parent().ok_or("stock has no folder")?.join(".git");
    std::fs::create_dir_all(git).map_err(|e| format!("broken .git: {e}"))?;
    app.library.pending_mounts.clear();
    h::unlogged(|| library_msg(app, LibraryMessage::OpenLibraryAt(Some(fx.stock.clone()))))?;
    reached(
        matches!(
            app.library.recovery,
            Some(RecoveryDialog::GitMissing { .. })
        ),
        "the broken repository raised no recovery dialog",
    )
}

/// The first stock footprint with pad 1 under a right-click menu that was
/// then dismissed, so the pad stays the selection.
fn open_footprint_pad(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
) -> Result<(), String> {
    fixture::mount_stock(app, fx)?;
    h::open(app, &fx.footprint)?;
    let _ = render::draw_main(r, app);
    h::fp_edit(app, &fx.footprint, FootprintEditorMsg::SelectPad(Some(0)));
    h::fp_edit(
        app,
        &fx.footprint,
        FootprintEditorMsg::ShowContextMenu {
            x: 720.0,
            y: 450.0,
            target: FootprintContextTarget::Pad(0),
        },
    );
    h::fp_edit(app, &fx.footprint, FootprintEditorMsg::CloseContextMenu);
    Ok(())
}

fn open_move_by(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    open_footprint_pad(app, r, fx)?;
    h::fp_edit(app, &fx.footprint, FootprintEditorMsg::MoveByOpen);
    let open = app
        .document_state
        .footprint_editors
        .get(&fx.footprint)
        .is_some_and(|e| e.state.move_by_modal.is_some());
    reached(open, "Move Selection By did not open")
}

fn open_align(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    open_footprint_pad(app, r, fx)?;
    h::fp_edit(app, &fx.footprint, FootprintEditorMsg::AlignOpen);
    let open = app
        .document_state
        .footprint_editors
        .get(&fx.footprint)
        .is_some_and(|e| e.state.align_modal.is_some());
    reached(open, "Align did not open")
}

fn open_symbol_menu(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    fixture::mount_stock(app, fx)?;
    h::open(app, &fx.symbol)?;
    let _ = render::draw_main(r, app);
    library_msg(
        app,
        LibraryMessage::PrimitiveEditorEvent {
            path: fx.symbol.clone(),
            msg: PrimitiveEdit::Symbol(SymbolEditorMsg::ShowContextMenu {
                x: 720.0,
                y: 450.0,
                target: SymbolContextTargetMsg::Empty,
            }),
        },
    )?;
    let open = app
        .document_state
        .symbol_editors
        .get(&fx.symbol)
        .is_some_and(|e| e.context_menu.is_some());
    reached(open, "the symbol context menu did not open")
}

/// The stock footprint in sketch mode.
fn open_stock_sketch(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    fixture::mount_stock(app, fx)?;
    h::open(app, &fx.footprint)?;
    let _ = render::draw_main(r, app);
    h::fp_edit(
        app,
        &fx.footprint,
        FootprintEditorMsg::SetMode(EditorMode::Sketch),
    );
    let _ = render::draw_main(r, app);
    Ok(())
}

/// The Line tool armed; the capture's hover over pad 1 raises the badge. The
/// idle cursor message fills the canvas words the badge is built from.
fn open_line_armed(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    open_stock_sketch(app, r, fx)?;
    h::fp_edit(
        app,
        &fx.footprint,
        FootprintEditorMsg::SketchSetTool(SketchTool::Line),
    );
    h::settle(app, Message::Ui(UiMsg::DragMove(431.0, 555.0)))?;
    let armed = app
        .document_state
        .footprint_editors
        .get(&fx.footprint)
        .is_some_and(|e| matches!(e.state.active_tool, SketchTool::Line));
    reached(armed, "the Line tool did not arm")
}

/// A Bake with nothing selected afterwards, so Properties keeps the outcome.
fn open_bake_outcome(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    open_stock_sketch(app, r, fx)?;
    library_msg(
        app,
        LibraryMessage::PrimitiveEditorEvent {
            path: fx.footprint.clone(),
            msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchBakeRequested),
        },
    )?;
    library_msg(app, LibraryMessage::Noop)?;
    let baked = app
        .document_state
        .footprint_editors
        .get(&fx.footprint)
        .is_some_and(|e| e.state.mode == EditorMode::Normal);
    reached(baked, "Bake did not leave sketch mode")
}

/// Two closed profiles, one a courtyard and one unassigned, nothing selected:
/// Properties lists a role row for each.
fn open_role_list(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    let plane = PlaneId::new();
    let mut sketch = SketchData::default();
    sketch.planes.push(Plane {
        id: plane,
        kind: PlaneKind::BoardTop,
    });
    let outer = add_rectangle(&mut sketch, plane, (-4.0, -3.0), (4.0, 3.0));
    add_rectangle(&mut sketch, plane, (-2.0, -1.0), (2.0, 1.0));
    let path = states_sketch::open_sketch(app, r, fx, sketch)?;
    for id in outer {
        let role = RoleTag::Courtyard;
        h::fp_edit(app, &path, FootprintEditorMsg::SketchSetRole { id, role });
    }
    library_msg(app, LibraryMessage::Noop)
}

/// A closed rectangle on `plane`; returns its four edges.
fn add_rectangle(
    sketch: &mut SketchData,
    plane: PlaneId,
    (x0, y0): (f64, f64),
    (x1, y1): (f64, f64),
) -> Vec<SketchEntityId> {
    let corners: Vec<SketchEntityId> = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
        .into_iter()
        .map(|(x, y)| {
            let id = SketchEntityId::new();
            sketch
                .entities
                .push(Entity::new(id, plane, EntityKind::Point { x, y }));
            id
        })
        .collect();
    (0..4)
        .map(|i| {
            let id = SketchEntityId::new();
            let (start, end) = (corners[i], corners[(i + 1) % 4]);
            sketch
                .entities
                .push(Entity::new(id, plane, EntityKind::Line { start, end }));
            id
        })
        .collect()
}
