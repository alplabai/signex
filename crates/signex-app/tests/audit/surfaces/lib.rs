//! Mockup group `lib` (rows m118-m142).

use crate::fixture::{self, Fixture};
use crate::surface::{Check, Surface, WindowSel};
use crate::{render, settle_support};
use signex_app::app::{
    FileMsg, FootprintDocument, GridPropertiesMsg, Message, OverlayMsg, Signex, SymbolDocument,
    TabInfo, TabKind,
};
use signex_app::dock::{DockMessage, PanelPosition};
use signex_app::library::LibraryMessage;
use signex_app::library::editor::footprint::state::FootprintContextTarget;
use signex_app::library::editor::symbol::state::SymActiveBarMenu;
use signex_app::library::editor::symbol::{SymbolPropsMsg, SymbolPropsTab, SymbolPropsUi};
use signex_app::library::messages::{
    CloseLibraryChoice, FootprintEditorMsg, PickerMsg, PrimitiveEdit, PrimitivePickerMsg,
    SymbolContextTargetMsg, SymbolEditorMsg,
};
use signex_app::library::recovery::{LibraryMissingChoice, RecoveryDialog};
use signex_app::library::state::{EditorAddress, PrimitivePickerTarget};
use signex_app::library::stock_copy::StockCopyMsg;
use signex_app::library::updates_dialog::{BumpKind, LibraryUpdateEntry, LibraryUpdatesState};
use signex_app::panels::{PanelKind, PanelMsg};
use std::path::{Path, PathBuf};

pub fn rows() -> Vec<Surface> {
    vec![
        Surface {
            id: "m118",
            title: "Library Browser tab",
            open: |app, _, fx| {
                fixture::mount_stock(app, fx)?;
                settle_support::settle(
                    app,
                    Message::Library(LibraryMessage::OpenLibraryBrowser(fx.stock.clone())),
                );
                // Collapse the side docks, as the mock does for the browser, so
                // the grid has the width the design assumes.
                let _ = app.update(Message::Dock(DockMessage::ToggleCollapse(
                    PanelPosition::Left,
                )));
                let _ = app.update(Message::Dock(DockMessage::ToggleCollapse(
                    PanelPosition::Right,
                )));
                // Select the first row so the preview pane shows.
                let (table, row_id) = first_stock_row(app, fx)?;
                let _ = app.update(Message::Library(LibraryMessage::BrowserSelectRow {
                    library_path: fx.stock.clone(),
                    table,
                    row_id,
                }));
                Ok(())
            },
            window: WindowSel::Auto,
            checks: &[],
        },
        Surface {
            id: "m119",
            title: "Library panel",
            // Main state: Footprints, in the footprint editor. Symbols is
            // m120; Parts and Empty are not mapped.
            open: |app, r, fx| {
                open_footprint(app, r, fx)?;
                open_panel(app, r, PanelKind::Library);
                Ok(())
            },
            window: WindowSel::Auto,
            checks: LIBRARY_PANEL,
        },
        Surface {
            id: "m120",
            title: "SCH Library panel (merged into the Library panel)",
            // The SCH Library row opens the Library panel in its Symbols kind.
            open: |app, r, fx| {
                open_symbol(app, r, fx)?;
                open_panel(app, r, PanelKind::SchLibrary);
                Ok(())
            },
            window: WindowSel::Auto,
            checks: LIBRARY_PANEL,
        },
        Surface {
            id: "m121",
            title: "Footprint Library panel (merged into the Library panel)",
            // The Footprint Library row opens the Library panel in its
            // Footprints kind.
            open: |app, r, fx| {
                open_footprint(app, r, fx)?;
                open_panel(app, r, PanelKind::FootprintLibrary);
                Ok(())
            },
            window: WindowSel::Auto,
            checks: LIBRARY_PANEL,
        },
        Surface {
            id: "m122",
            title: "Symbol Pins panel (merged into Properties ▸ Pins)",
            // The dock panel is removed by design; its content is the Pins
            // tab of the symbol editor's Properties, which this shows.
            open: |app, r, fx| {
                open_symbol(app, r, fx)?;
                symbol_props(app, SymbolPropsMsg::TabPicked(SymbolPropsTab::Pins));
                let _ = render::draw_main(r, app);
                Ok(())
            },
            window: WindowSel::Auto,
            checks: PINS_TAB,
        },
        Surface {
            id: "m122b",
            title: "Pin table tab",
            // Main state: All pins. The app shows the table inside the
            // Properties Pins tab (Edit pins in table…), not as a document
            // tab as the mockup does. Bulk edit, validation and paste states
            // are not mapped.
            open: |app, r, fx| {
                open_symbol(app, r, fx)?;
                symbol_props(app, SymbolPropsMsg::TabPicked(SymbolPropsTab::Pins));
                symbol_props(app, SymbolPropsMsg::PinTableToggled);
                let _ = render::draw_main(r, app);
                Ok(())
            },
            window: WindowSel::Auto,
            checks: PIN_TABLE,
        },
        Surface {
            id: "m123",
            title: "Symbol editor Properties (symbol / pin / graphic)",
            open: open_symbol,
            window: WindowSel::Auto,
            checks: &[],
        },
        Surface {
            id: "m124",
            title: "Footprint editor Properties (board / selection / snap)",
            open: open_footprint,
            window: WindowSel::Auto,
            checks: &[],
        },
        Surface {
            id: "m125",
            title: "Footprint Properties: Pad / pad stack",
            open: |app, r, fx| {
                open_footprint(app, r, fx)?;
                let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
                    path: fx.footprint.clone(),
                    msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SelectPad(Some(0))),
                }));
                let _ = render::draw_main(r, app);
                Ok(())
            },
            window: WindowSel::Auto,
            checks: &[],
        },
        Surface {
            id: "m126",
            title: "Footprint Properties: Pour / Keepout / Cutout / Pattern / Sketch pad",
            // Mapped state: Keepout (a sketch line with a keepout role,
            // selected). Pour, Cutout, Pattern and Sketch pad are the other
            // sub-forms of the same panel and are not mapped.
            open: |app, r, fx| {
                let path = fx.footprint.with_file_name("audit-keepout.snxfpt");
                let (footprint, line) = keepout_footprint();
                open_footprint_document(app, &path, footprint);
                let _ = render::draw_main(r, app);
                let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
                    path,
                    msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchSelect {
                        id: Some(line),
                        shift: false,
                    }),
                }));
                let _ = render::draw_main(r, app);
                Ok(())
            },
            window: WindowSel::Auto,
            checks: &[("a footprint tab is active", footprint_tab_active)],
        },
        Surface {
            id: "m127",
            title: "Library row Properties (component sheet)",
            open: |app, r, fx| {
                open_browser_row(app, fx)?;
                fixture::select_panel(app, PanelKind::Properties)?;
                let _ = render::draw_main(r, app);
                Ok(())
            },
            window: WindowSel::Auto,
            checks: &[("a browser row is selected", browser_row_selected)],
        },
        Surface {
            id: "m128",
            title: "Component editor window",
            open: |app, _, fx| {
                fixture::mount_stock(app, fx)?;
                let (table, row_id) = first_stock_row(app, fx)?;
                settle_support::settle(
                    app,
                    Message::Library(LibraryMessage::OpenComponentRow {
                        library_path: fx.stock.clone(),
                        table,
                        row_id,
                    }),
                );
                Ok(())
            },
            window: WindowSel::Auto,
            checks: &[],
        },
        Surface {
            id: "m129",
            title: "Library picker",
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                fixture::mount_stock(app, fx)?;
                settle_support::settle(app, Message::Library(LibraryMessage::OpenPicker));
                Ok(())
            },
            window: WindowSel::Auto,
            checks: PICKER,
        },
        Surface {
            id: "m130",
            title: "Edit row",
            open: |app, _, fx| open_edit_row(app, fx),
            window: WindowSel::Auto,
            checks: &[("the Edit row modal is open", edit_modal_open)],
        },
        Surface {
            id: "m131",
            title: "Primitive picker",
            // Main state: Pick (Pick Symbol for the selected row). The copy
            // confirm for a loose symbol is not mapped.
            open: |app, _, fx| {
                let (table, row_id) = open_browser_row(app, fx)?;
                settle_support::settle(
                    app,
                    Message::Library(LibraryMessage::OpenPrimitivePicker {
                        kind: signex_library::PrimitiveKind::Symbol,
                        target: PrimitivePickerTarget::BrowserRow(EditorAddress::new(
                            fx.stock.clone(),
                            table,
                            row_id,
                        )),
                    }),
                );
                Ok(())
            },
            window: WindowSel::Auto,
            checks: PRIMITIVE_PICKER,
        },
        Surface {
            id: "m132",
            title: "Library Document Options",
            open: |app, _, fx| {
                open_browser_row(app, fx)?;
                settle_support::settle(
                    app,
                    Message::Library(LibraryMessage::OpenDocumentOptions {
                        library_path: fx.stock.clone(),
                    }),
                );
                Ok(())
            },
            window: WindowSel::Auto,
            checks: DOCUMENT_OPTIONS,
        },
        Surface {
            id: "m133",
            title: "New Library (Create Options)",
            // Main state: Default. "A library is already in this folder"
            // (the refusal banner) is not mapped.
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                let folder = fx
                    .sample
                    .parent()
                    .ok_or("sample project has no folder")?
                    .join("linear-regulator-lib");
                settle_support::settle(
                    app,
                    Message::Library(LibraryMessage::CreateLibraryAtPath {
                        project_path: fx.sample.clone(),
                        lib_path: folder.join("linear-regulator-lib.snxlib"),
                    }),
                );
                Ok(())
            },
            window: WindowSel::Auto,
            checks: CREATE_OPTIONS,
        },
        Surface {
            id: "m134",
            title: "Copy stock primitive to library",
            // Mapped state: No writable library (the stock library is the
            // only one mounted). "Libraries available" needs a second,
            // writable library with its own id; not mapped.
            open: |app, _, fx| {
                app.library.pending_mounts.clear();
                app.library.stock_copy_target = None;
                app.library.stock_library_root = Some(fx.stock.clone());
                app.library
                    .open_library_read_only(fx.stock.clone())
                    .map_err(|e| format!("mount stock read-only: {e:?}"))?;
                settle_support::settle(
                    app,
                    Message::File(FileMsg::Opened(Some(fx.symbol.clone()))),
                );
                if app.library.stock_copy.is_none() {
                    return Err("opening a stock symbol did not ask where to copy it".into());
                }
                Ok(())
            },
            window: WindowSel::Auto,
            checks: STOCK_COPY,
        },
        Surface {
            id: "m135",
            title: "Library Updates",
            // The drift scan needs a team-mode library whose rows moved past
            // the placed parts; the modal state is seeded with the entries
            // such a scan returns, as the regression tests seed recovery.
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                fixture::mount_stock(app, fx)?;
                let sheet = fx.sample.with_extension("snxsch");
                let entries = update_entries(app, fx)?;
                app.library.library_updates = Some(LibraryUpdatesState::new(sheet, entries));
                let _ = app.update(Message::Library(LibraryMessage::Noop));
                Ok(())
            },
            window: WindowSel::Auto,
            checks: LIBRARY_UPDATES,
        },
        Surface {
            id: "m136",
            title: "Close library with unsaved drafts",
            open: |app, r, fx| {
                open_symbol(app, r, fx)?;
                settle_support::settle(
                    app,
                    Message::File(FileMsg::Opened(Some(fx.footprint.clone()))),
                );
                app.document_state.mark_document_dirty(&fx.symbol);
                app.document_state.mark_document_dirty(&fx.footprint);
                let _ = app.update(Message::Library(LibraryMessage::CloseLibrary(
                    fx.stock.clone(),
                )));
                if app.library.close_library_confirm.is_none() {
                    return Err("closing a library with dirty drafts raised no prompt".into());
                }
                Ok(())
            },
            window: WindowSel::Auto,
            checks: CLOSE_PROMPT,
        },
        Surface {
            id: "m137",
            title: "Delete library row confirm",
            // Main state: Delete row. "Symbol still in use" (the closed-sheet
            // scan finding the symbol) is not mapped.
            open: |app, _, fx| {
                let (table, row_id) = open_browser_row(app, fx)?;
                settle_support::settle(
                    app,
                    Message::Library(LibraryMessage::BrowserDeleteRowRequest {
                        library_path: fx.stock.clone(),
                        table,
                        row_id,
                    }),
                );
                Ok(())
            },
            window: WindowSel::Auto,
            checks: DELETE_CONFIRM,
        },
        Surface {
            id: "m138",
            title: "Library recovery",
            // Main state: Library Missing, seeded as the regression tests
            // seed it. Git Repository Missing and Broken Binding are the
            // other two cases of the same dialog.
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                let missing = fx
                    .sample
                    .parent()
                    .ok_or("sample project has no folder")?
                    .join("gone-lib/gone-lib.snxlib");
                app.library.recovery = Some(RecoveryDialog::LibraryMissing { path: missing });
                let _ = app.update(Message::Library(LibraryMessage::Noop));
                Ok(())
            },
            window: WindowSel::Auto,
            checks: RECOVERY,
        },
        Surface {
            id: "m139",
            title: "Prefix / unit pickers",
            // The prefix and unit pickers are pick lists inside the Edit row
            // modal's measurement slots; their open list is widget state an
            // update cannot reach, so this shows the slots closed.
            open: |app, _, fx| open_edit_row(app, fx),
            window: WindowSel::Auto,
            checks: &[("the Edit row modal is open", edit_modal_open)],
        },
        Surface {
            id: "m140",
            title: "Footprint active bar / context menu / Move By / Align",
            // Main state: Context menu on a pad. Move Selection By and Align
            // Selection are reached by the checks.
            open: |app, r, fx| {
                open_footprint(app, r, fx)?;
                let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
                    path: fx.footprint.clone(),
                    msg: PrimitiveEdit::Footprint(FootprintEditorMsg::ShowContextMenu {
                        x: 720.0,
                        y: 450.0,
                        target: FootprintContextTarget::Pad(0),
                    }),
                }));
                let _ = render::draw_main(r, app);
                Ok(())
            },
            window: WindowSel::Auto,
            checks: FOOTPRINT_BAR,
        },
        Surface {
            id: "m141",
            title: "Symbol editor active bar / context menu",
            // Main state: Shapes dropdown. The context menu is a check.
            open: |app, r, fx| {
                open_symbol(app, r, fx)?;
                let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
                    path: fx.symbol.clone(),
                    msg: PrimitiveEdit::Symbol(SymbolEditorMsg::ToggleActiveBarMenu(
                        SymActiveBarMenu::Shapes,
                    )),
                }));
                let _ = render::draw_main(r, app);
                Ok(())
            },
            window: WindowSel::Auto,
            checks: SYMBOL_BAR,
        },
        Surface {
            id: "m142",
            title: "Grid Properties (Cartesian grid editor)",
            open: |app, r, fx| {
                open_footprint(app, r, fx)?;
                let _ = app.update(Message::GridProperties(GridPropertiesMsg::Open));
                Ok(())
            },
            window: WindowSel::Auto,
            checks: GRID_PROPERTIES,
        },
    ]
}

// ---- checks ----

const LIBRARY_PANEL: &[Check] = &[("the Library panel is docked", library_panel_docked)];

const PINS_TAB: &[Check] = &[("Properties shows the Pins tab", pins_tab_shown)];

const PIN_TABLE: &[Check] = &[
    ("Properties shows the pin table", pin_table_shown),
    ("the toggle returns to the pin list", pin_table_toggles_back),
];

const PICKER: &[Check] = &[
    ("the picker lists rows", picker_open),
    (
        "a filter with no match empties the list",
        picker_filter_narrows,
    ),
];

const PRIMITIVE_PICKER: &[Check] = &[
    (
        "the filter text reaches the picker",
        primitive_picker_filters,
    ),
    ("Cancel closes the picker", primitive_picker_cancels),
];

const DOCUMENT_OPTIONS: &[Check] = &[
    (
        "Grid Visible toggles the draft",
        document_options_grid_toggles,
    ),
    ("Cancel closes the dialog", document_options_cancels),
];

const CREATE_OPTIONS: &[Check] = &[
    ("version control toggles", create_options_git_toggles),
    (
        "LFS turns off with version control",
        create_options_lfs_follows_git,
    ),
    ("Cancel closes the dialog", create_options_cancels),
];

const STOCK_COPY: &[Check] = &[("Cancel closes the dialog", stock_copy_cancels)];

const LIBRARY_UPDATES: &[Check] = &[
    ("a row checkbox toggles", library_updates_toggles),
    ("Cancel closes the dialog", library_updates_cancels),
];

const CLOSE_PROMPT: &[Check] = &[(
    "Cancel keeps the library mounted",
    close_prompt_cancel_keeps_library,
)];

const DELETE_CONFIRM: &[Check] = &[
    ("the confirm is open", delete_confirm_open),
    ("Cancel deletes nothing", delete_confirm_cancels),
];

const RECOVERY: &[Check] = &[("Cancel closes the dialog", recovery_cancels)];

const FOOTPRINT_BAR: &[Check] = &[
    ("the context menu is open", footprint_context_menu_open),
    (
        "Move Selection By opens and cancels",
        footprint_move_by_opens,
    ),
    ("Align… opens and cancels", footprint_align_opens),
];

const SYMBOL_BAR: &[Check] = &[
    ("the Shapes dropdown is open", symbol_shapes_open),
    (
        "a right-click closes the dropdown and opens the menu",
        symbol_context_menu_opens,
    ),
];

const GRID_PROPERTIES: &[Check] = &[
    ("the Grid Properties dialog is open", grid_properties_open),
    ("Apply writes the grid step", grid_properties_apply),
];

/// `Ok` when `ok`, else `Err(why)`.
fn ensure(ok: bool, why: impl Into<String>) -> Result<(), String> {
    if ok { Ok(()) } else { Err(why.into()) }
}

fn library_panel_docked(app: &mut Signex) -> Result<(), String> {
    let docked = [
        PanelPosition::Left,
        PanelPosition::Right,
        PanelPosition::Bottom,
    ]
    .into_iter()
    .any(|pos| {
        app.document_state
            .dock
            .panel_kinds(pos)
            .contains(&PanelKind::Library)
    });
    ensure(docked, "no Library tab in any dock")
}

fn symbol_props_ui(app: &Signex) -> Result<&SymbolPropsUi, String> {
    app.document_state
        .panel_ctx
        .symbol_editor
        .as_ref()
        .map(|ctx| &ctx.props)
        .ok_or_else(|| "no symbol editor context".to_string())
}

fn pins_tab_shown(app: &mut Signex) -> Result<(), String> {
    let tab = symbol_props_ui(app)?.tab;
    ensure(
        tab == SymbolPropsTab::Pins,
        format!("Properties shows {tab:?}"),
    )
}

fn pin_table_shown(app: &mut Signex) -> Result<(), String> {
    ensure(symbol_props_ui(app)?.pin_table, "pin_table is false")
}

fn pin_table_toggles_back(app: &mut Signex) -> Result<(), String> {
    symbol_props(app, SymbolPropsMsg::PinTableToggled);
    ensure(!symbol_props_ui(app)?.pin_table, "pin_table stayed on")
}

fn picker_rows(app: &Signex) -> Result<usize, String> {
    let picker = app.library.picker.as_ref().ok_or("the picker is closed")?;
    Ok(picker.rows.visible().count())
}

fn picker_open(app: &mut Signex) -> Result<(), String> {
    ensure(picker_rows(app)? > 0, "the picker lists no row")
}

fn picker_filter_narrows(app: &mut Signex) -> Result<(), String> {
    let _ = app.update(Message::Library(LibraryMessage::Picker(
        PickerMsg::FilterChanged("zz-no-such-part-zz".into()),
    )));
    let n = picker_rows(app)?;
    ensure(n == 0, format!("{n} rows still listed"))
}

fn primitive_picker_filters(app: &mut Signex) -> Result<(), String> {
    let _ = app.update(Message::Library(LibraryMessage::PrimitivePicker(
        PrimitivePickerMsg::SetFilter("res".into()),
    )));
    let picker = app
        .library
        .primitive_picker
        .as_ref()
        .ok_or("the primitive picker is closed")?;
    ensure(
        picker.filter == "res",
        format!("filter is {:?}", picker.filter),
    )
}

fn primitive_picker_cancels(app: &mut Signex) -> Result<(), String> {
    let _ = app.update(Message::Library(LibraryMessage::PrimitivePicker(
        PrimitivePickerMsg::Cancel,
    )));
    ensure(
        app.library.primitive_picker.is_none(),
        "the primitive picker stayed open",
    )
}

fn document_options_grid(app: &Signex) -> Result<bool, String> {
    app.library
        .document_options
        .as_ref()
        .map(|d| d.draft.grid_visible)
        .ok_or_else(|| "Document Options is closed".to_string())
}

fn document_options_grid_toggles(app: &mut Signex) -> Result<(), String> {
    let before = document_options_grid(app)?;
    let _ = app.update(Message::Library(LibraryMessage::DocumentOptionsToggleGrid));
    ensure(
        document_options_grid(app)? != before,
        "grid_visible did not change",
    )
}

fn document_options_cancels(app: &mut Signex) -> Result<(), String> {
    let _ = app.update(Message::Library(LibraryMessage::DocumentOptionsCancel));
    ensure(
        app.library.document_options.is_none(),
        "Document Options stayed open",
    )
}

fn create_flags(app: &Signex) -> Result<(bool, bool), String> {
    app.library
        .create_options
        .as_ref()
        .map(|s| (s.enable_git, s.use_lfs))
        .ok_or_else(|| "the New Library dialog is closed".to_string())
}

fn create_options_git_toggles(app: &mut Signex) -> Result<(), String> {
    let (before, _) = create_flags(app)?;
    let _ = app.update(Message::Library(
        LibraryMessage::LibraryCreateOptionsToggleGit,
    ));
    ensure(create_flags(app)?.0 != before, "enable_git did not change")
}

fn create_options_lfs_follows_git(app: &mut Signex) -> Result<(), String> {
    // Version control on, LFS on, then version control off.
    if !create_flags(app)?.0 {
        let _ = app.update(Message::Library(
            LibraryMessage::LibraryCreateOptionsToggleGit,
        ));
    }
    if !create_flags(app)?.1 {
        let _ = app.update(Message::Library(
            LibraryMessage::LibraryCreateOptionsToggleLfs,
        ));
    }
    let _ = app.update(Message::Library(
        LibraryMessage::LibraryCreateOptionsToggleGit,
    ));
    let flags = create_flags(app)?;
    ensure(flags == (false, false), format!("(git, lfs) is {flags:?}"))
}

fn create_options_cancels(app: &mut Signex) -> Result<(), String> {
    let _ = app.update(Message::Library(LibraryMessage::LibraryCreateOptionsCancel));
    ensure(
        app.library.create_options.is_none(),
        "the New Library dialog stayed open",
    )
}

fn stock_copy_cancels(app: &mut Signex) -> Result<(), String> {
    let _ = app.update(Message::Library(LibraryMessage::StockCopy(
        StockCopyMsg::Cancelled,
    )));
    ensure(
        app.library.stock_copy.is_none(),
        "the Copy to Library dialog stayed open",
    )
}

fn first_update(app: &Signex) -> Result<(uuid::Uuid, bool), String> {
    app.library
        .library_updates
        .as_ref()
        .and_then(|s| s.entries.first())
        .map(|e| (e.symbol_uuid, e.selected))
        .ok_or_else(|| "Library Updates is closed or empty".to_string())
}

fn library_updates_toggles(app: &mut Signex) -> Result<(), String> {
    let (uuid, before) = first_update(app)?;
    let _ = app.update(Message::Library(
        LibraryMessage::LibraryUpdatesToggleSelection(uuid),
    ));
    ensure(
        first_update(app)?.1 != before,
        "the checkbox did not change",
    )
}

fn library_updates_cancels(app: &mut Signex) -> Result<(), String> {
    let _ = app.update(Message::Library(LibraryMessage::LibraryUpdatesCancel));
    ensure(
        app.library.library_updates.is_none(),
        "Library Updates stayed open",
    )
}

fn close_prompt_cancel_keeps_library(app: &mut Signex) -> Result<(), String> {
    let stock = stock_root(app)?;
    let _ = app.update(Message::Library(LibraryMessage::CloseLibraryConfirm(
        CloseLibraryChoice::Cancel,
    )));
    ensure(
        app.library.close_library_confirm.is_none(),
        "the close prompt stayed open",
    )?;
    ensure(
        app.library.library_at(&stock).is_some(),
        "Cancel closed the library",
    )
}

fn delete_confirm_open(app: &mut Signex) -> Result<(), String> {
    let stock = stock_root(app)?;
    let open = app
        .library
        .library_browsers
        .get(&stock)
        .is_some_and(|b| b.delete_confirm.is_some());
    ensure(open, "no delete confirm")
}

fn stock_row_count(app: &Signex, stock: &Path) -> Option<usize> {
    app.library
        .library_at(stock)
        .map(|l| l.cached_components.len())
}

fn delete_confirm_cancels(app: &mut Signex) -> Result<(), String> {
    let stock = stock_root(app)?;
    let before = stock_row_count(app, &stock);
    let _ = app.update(Message::Library(LibraryMessage::BrowserDeleteRowCancel {
        library_path: stock.clone(),
    }));
    let open = app
        .library
        .library_browsers
        .get(&stock)
        .is_some_and(|b| b.delete_confirm.is_some());
    ensure(!open, "the confirm stayed open")?;
    ensure(stock_row_count(app, &stock) == before, "a row was deleted")
}

fn recovery_cancels(app: &mut Signex) -> Result<(), String> {
    let _ = app.update(Message::Library(LibraryMessage::RecoveryLibraryMissing(
        LibraryMissingChoice::Cancel,
    )));
    ensure(
        app.library.recovery.is_none(),
        "the recovery dialog stayed open",
    )
}

fn footprint_event(app: &mut Signex, msg: FootprintEditorMsg) -> Result<(), String> {
    let path = active_path(app)?;
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path,
        msg: PrimitiveEdit::Footprint(msg),
    }));
    Ok(())
}

fn active_footprint(app: &Signex) -> Result<&FootprintDocument, String> {
    let path = active_path(app)?;
    app.document_state
        .footprint_editors
        .get(&path)
        .ok_or_else(|| format!("{} is not a footprint editor", path.display()))
}

fn footprint_tab_active(app: &mut Signex) -> Result<(), String> {
    active_footprint(app).map(|_| ())
}

fn footprint_context_menu_open(app: &mut Signex) -> Result<(), String> {
    let open = active_footprint(app)?.state.context_menu.is_some();
    ensure(open, "no context menu")
}

fn footprint_move_by_opens(app: &mut Signex) -> Result<(), String> {
    footprint_event(app, FootprintEditorMsg::CloseContextMenu)?;
    footprint_event(app, FootprintEditorMsg::MoveByOpen)?;
    let open = active_footprint(app)?.state.move_by_modal.is_some();
    ensure(open, "Move Selection By did not open")?;
    footprint_event(app, FootprintEditorMsg::MoveByCancel)?;
    let open = active_footprint(app)?.state.move_by_modal.is_some();
    ensure(!open, "Move Selection By stayed open")
}

fn footprint_align_opens(app: &mut Signex) -> Result<(), String> {
    footprint_event(app, FootprintEditorMsg::AlignOpen)?;
    let open = active_footprint(app)?.state.align_modal.is_some();
    ensure(open, "Align did not open")?;
    footprint_event(app, FootprintEditorMsg::AlignCancel)?;
    let open = active_footprint(app)?.state.align_modal.is_some();
    ensure(!open, "Align stayed open")
}

fn active_symbol(app: &Signex) -> Result<&SymbolDocument, String> {
    let path = active_path(app)?;
    app.document_state
        .symbol_editors
        .get(&path)
        .ok_or_else(|| format!("{} is not a symbol editor", path.display()))
}

fn symbol_shapes_open(app: &mut Signex) -> Result<(), String> {
    let menu = active_symbol(app)?.active_bar_menu;
    ensure(
        menu == Some(SymActiveBarMenu::Shapes),
        format!("open dropdown is {menu:?}"),
    )
}

fn symbol_context_menu_opens(app: &mut Signex) -> Result<(), String> {
    let path = active_path(app)?;
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path,
        msg: PrimitiveEdit::Symbol(SymbolEditorMsg::ShowContextMenu {
            x: 720.0,
            y: 450.0,
            target: SymbolContextTargetMsg::Empty,
        }),
    }));
    let symbol = active_symbol(app)?;
    ensure(symbol.active_bar_menu.is_none(), "the dropdown stayed open")?;
    ensure(symbol.context_menu.is_some(), "no context menu")
}

fn grid_properties_open(app: &mut Signex) -> Result<(), String> {
    ensure(
        app.ui_state.grid_properties.is_some(),
        "grid_properties is None",
    )
}

fn grid_properties_apply(app: &mut Signex) -> Result<(), String> {
    let _ = app.update(Message::GridProperties(GridPropertiesMsg::SetStep(
        "0.5".into(),
    )));
    let _ = app.update(Message::GridProperties(GridPropertiesMsg::Apply));
    let step = active_footprint(app)?.state.snap_options.grid_step_mm;
    ensure((step - 0.5).abs() < 1e-9, format!("grid step is {step} mm"))
}

fn browser_row_selected(app: &mut Signex) -> Result<(), String> {
    let stock = stock_root(app)?;
    let selected = app
        .library
        .library_browsers
        .get(&stock)
        .is_some_and(|b| b.selected_row.is_some());
    ensure(selected, "no browser row selected")
}

fn edit_modal_open(app: &mut Signex) -> Result<(), String> {
    let stock = stock_root(app)?;
    let open = app
        .library
        .library_browsers
        .get(&stock)
        .is_some_and(|b| b.edit_modal.is_some());
    ensure(open, "no Edit row modal")
}

// ---- open helpers ----

/// The path of the active document tab.
fn active_path(app: &Signex) -> Result<PathBuf, String> {
    let d = &app.document_state;
    d.tabs
        .get(d.active_tab)
        .map(|t| t.path.clone())
        .ok_or_else(|| "no active tab".to_string())
}

/// The stock library the fixture mounted.
fn stock_root(app: &Signex) -> Result<PathBuf, String> {
    app.library
        .stock_library_root
        .clone()
        .ok_or_else(|| "no stock library mounted".to_string())
}

/// The table and id of the first component row the stock library lists.
fn first_stock_row(app: &Signex, fx: &Fixture) -> Result<(String, signex_library::RowId), String> {
    let (_, row) = app
        .library
        .all_components()
        .into_iter()
        .next()
        .ok_or("stock lists no component")?;
    let table = app
        .library
        .table_of_row(&fx.stock, row.row_id)
        .ok_or("no table for the row")?;
    Ok((table, signex_library::RowId(row.row_id)))
}

/// Open the first stock footprint in its editor.
fn open_footprint(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    fixture::mount_stock(app, fx)?;
    settle_support::settle(
        app,
        Message::File(FileMsg::Opened(Some(fx.footprint.clone()))),
    );
    let _ = render::draw_main(r, app);
    Ok(())
}

/// Open the first stock symbol in its editor.
fn open_symbol(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    fixture::mount_stock(app, fx)?;
    settle_support::settle(app, Message::File(FileMsg::Opened(Some(fx.symbol.clone()))));
    let _ = render::draw_main(r, app);
    Ok(())
}

/// Open `kind` the way the View ▸ Panels menu does. The layout persist task
/// it returns is dropped, so no prefs file is written.
fn open_panel(app: &mut Signex, r: &mut iced::Renderer, kind: PanelKind) {
    let _ = app.update(Message::Overlay(OverlayMsg::OpenPanel(kind)));
    let _ = render::draw_main(r, app);
}

fn symbol_props(app: &mut Signex, msg: SymbolPropsMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::SymbolProps(
        msg,
    ))));
}

/// Open the stock library's Browser tab with its first row selected.
fn open_browser_row(
    app: &mut Signex,
    fx: &Fixture,
) -> Result<(String, signex_library::RowId), String> {
    fixture::mount_stock(app, fx)?;
    settle_support::settle(
        app,
        Message::Library(LibraryMessage::OpenLibraryBrowser(fx.stock.clone())),
    );
    let (table, row_id) = first_stock_row(app, fx)?;
    let _ = app.update(Message::Library(LibraryMessage::BrowserSelectRow {
        library_path: fx.stock.clone(),
        table: table.clone(),
        row_id,
    }));
    Ok((table, row_id))
}

/// Browser ▸ Edit Details on the first stock row.
pub(super) fn open_edit_row(app: &mut Signex, fx: &Fixture) -> Result<(), String> {
    let (table, row_id) = open_browser_row(app, fx)?;
    settle_support::settle(
        app,
        Message::Library(LibraryMessage::BrowserOpenEditModal {
            library_path: fx.stock.clone(),
            table,
            row_id,
        }),
    );
    Ok(())
}

/// Two drifted parts against stock rows: one patch bump (checked by
/// default) and one major bump (unchecked).
fn update_entries(app: &Signex, fx: &Fixture) -> Result<Vec<LibraryUpdateEntry>, String> {
    let library = app
        .library
        .library_at(&fx.stock)
        .ok_or("stock is not mounted")?;
    let rows: Vec<_> = app.library.all_components().into_iter().take(2).collect();
    if rows.len() < 2 {
        return Err("stock lists fewer than two components".into());
    }
    let bumps = [
        ("C1", "1.2.0", "1.2.1", BumpKind::Patch),
        ("U1", "1.2.0", "2.0.0", BumpKind::Major),
    ];
    let mut entries = Vec::new();
    for ((_, row), (ref_des, current, latest, bump_kind)) in rows.into_iter().zip(bumps) {
        entries.push(LibraryUpdateEntry {
            symbol_uuid: uuid::Uuid::now_v7(),
            ref_des: ref_des.into(),
            internal_pn: row.internal_pn.as_str().to_string(),
            lib_id: format!("{}:{}", library.library_id, row.row_id),
            library_id: library.library_id,
            library_name: library.display_name.clone(),
            row_id: signex_library::RowId(row.row_id),
            library_path: fx.stock.clone(),
            current_version: current.into(),
            latest_version: latest.into(),
            bump_kind,
            selected: false,
            conflicts: Vec::new(),
        });
    }
    Ok(entries)
}

/// Put `footprint` in a footprint editor tab at `path`, made active.
fn open_footprint_document(app: &mut Signex, path: &Path, footprint: signex_library::Footprint) {
    let file = signex_library::FootprintFile::from_footprint(footprint);
    app.document_state.footprint_editors.insert(
        path.to_path_buf(),
        FootprintDocument::new(path.to_path_buf(), file),
    );
    app.document_state.tabs.push(TabInfo {
        title: "audit-keepout".into(),
        path: path.to_path_buf(),
        cached_document: None,
        project_id: None,
        kind: TabKind::FootprintEditor(path.to_path_buf()),
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
}

/// A footprint whose sketch holds a 2 mm line with a no-routing keepout on
/// the top copper; returns the line's id.
fn keepout_footprint() -> (signex_library::Footprint, signex_sketch::id::SketchEntityId) {
    use signex_sketch::attr::{KeepoutAttr, KeepoutKinds};
    use signex_sketch::entity::{Entity, EntityKind};
    use signex_sketch::id::SketchEntityId;
    use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
    use signex_sketch::sketch::SketchData;
    use signex_types::pcb_layer::{CopperLayer, PcbLayer};

    let plane = PlaneId::new();
    let mut sketch = SketchData::default();
    sketch.planes.push(Plane {
        id: plane,
        kind: PlaneKind::BoardTop,
    });
    let (a, b, line) = (
        SketchEntityId::new(),
        SketchEntityId::new(),
        SketchEntityId::new(),
    );
    sketch
        .entities
        .push(Entity::new(a, plane, EntityKind::Point { x: 0.0, y: 0.0 }));
    sketch
        .entities
        .push(Entity::new(b, plane, EntityKind::Point { x: 2.0, y: 0.0 }));
    let mut edge = Entity::new(line, plane, EntityKind::Line { start: a, end: b });
    edge.keepout = Some(KeepoutAttr {
        layer: PcbLayer::Copper(CopperLayer::Top),
        kinds: KeepoutKinds::NO_ROUTING,
    });
    sketch.entities.push(edge);
    let mut footprint = signex_library::Footprint::empty("keepout");
    footprint.sketch = Some(sketch);
    (footprint, line)
}
