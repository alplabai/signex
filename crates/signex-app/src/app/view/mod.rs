use iced::widget::{canvas, column, container, row, shader, stack, text_input};
use iced::{Element, Length};

/// Detached-modal native window title text.
pub(crate) mod detached_title;
pub(crate) mod dialogs;
/// Overlay identity + paint order — shared with the Esc ladder in
/// `app/bootstrap/subscription.rs`, hence `pub(crate)` and not a child
/// of the private `overlays` module.
pub(crate) mod overlay_id;
pub(crate) mod translate;

mod chrome;
mod context_menu;
mod modals;
mod overlays;
mod pdf_preview;
mod print_preview;

use super::*;

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

// ── Submenu chevron — single source of truth ─────────────────────────
//
// Right-pointing angle quote (U+203A), NOT the BLACK RIGHT-POINTING
// TRIANGLE (U+25B6) which Windows renders via the colour emoji font.
// Same glyph the menu_bar dropdowns use; the matching size below keeps
// every submenu launcher visually aligned across the whole app
// (canvas right-click, project-tree right-click, File/Edit/View menu).
const SUBMENU_ARROW: &str = "›";
const SUBMENU_ARROW_SIZE: f32 = 18.0;

/// Chrome strip search bar width in pixels — the widest it ever gets;
/// `chrome_search_bar_geometry` shrinks it below this on narrow windows.
pub(crate) const CHROME_SEARCH_BAR_WIDTH: f32 = 440.0;
/// Narrowest the chrome search bar shrinks to before it starts
/// overlapping the menu row. Still wide enough for a few query words.
pub(crate) const CHROME_SEARCH_BAR_MIN_WIDTH: f32 = 160.0;
/// Minimum left padding between the menu bar's right edge and the
/// chrome search bar's left edge.
pub(crate) const CHROME_SEARCH_LEFT_GAP: f32 = 16.0;
/// Minimum right padding between the chrome search bar's right edge
/// and the trailing window controls.
pub(crate) const CHROME_SEARCH_RIGHT_GAP: f32 = 16.0;

/// `(x, width)` of the chrome strip's search bar for a window `window_w`
/// pixels wide. Single source of truth: the chrome strip lays the bar
/// out with this width, and the command-palette dropdown anchors to this
/// `x`, so the two can never drift apart.
///
/// The bar is centred on the **window**, not on the gap between the menu
/// row and the window controls — that gap is off-centre because the menu
/// row is much wider than the three control buttons. To stay centred
/// without overlapping either side, both sides reserve the same width:
/// whichever of the two is wider. `leading_w` / `trailing_w` are the widths
/// the window controls take at the strip's left / right edge (the menu row
/// starts after the leading ones). Whatever is left over is the bar,
/// clamped to `[CHROME_SEARCH_BAR_MIN_WIDTH, CHROME_SEARCH_BAR_WIDTH]`,
/// so it shrinks as the window narrows and never grows past its design
/// width on a wide monitor. `menu_w` is the in-window menu row's width, 0
/// when the system menu bar carries the menus.
pub(crate) fn chrome_search_bar_geometry(
    window_w: f32,
    leading_w: f32,
    trailing_w: f32,
    menu_w: f32,
) -> (f32, f32) {
    let menu_end = leading_w + menu_w + CHROME_SEARCH_LEFT_GAP;
    let side = menu_end.max(trailing_w + CHROME_SEARCH_RIGHT_GAP);
    let width = (window_w - 2.0 * side).clamp(CHROME_SEARCH_BAR_MIN_WIDTH, CHROME_SEARCH_BAR_WIDTH);
    // Once the window is too narrow to centre a `MIN_WIDTH` bar, centring
    // would slide it under the menu items. Below that threshold it gives
    // up on centring and sits flush right of the menu instead — still
    // fully usable, which beats a pretty bar you can't click.
    let x = ((window_w - width) / 2.0).max(menu_end);
    (x, width)
}

/// Pointer travel (px) before a tab press counts as a drag, for both the
/// grab cursor and the drag ghost.
const TAB_DRAG_THRESHOLD_PX: f32 = 6.0;

impl Signex {
    pub fn view(&self, window_id: iced::window::Id) -> Element<'_, Message> {
        // Secondary windows (detached modals, future undocked tabs) render
        // just their own content — no menu / dock / canvas. The main
        // window's view_main drops any overlay whose modal is currently
        // detached so we don't double-render.
        if let Some(kind) = self.ui_state.windows.get(&window_id) {
            return match kind {
                super::state::WindowKind::DetachedModal(modal) => {
                    // The body fills the whole window on the dialog surface.
                    iced::widget::container(self.view_detached_modal(window_id, *modal))
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .style(crate::styles::modal_card(
                            &self.document_state.panel_ctx.tokens,
                        ))
                        .into()
                }
                // Undocked tab = full duplicate of the main app view.
                // Shared Signex state means edits sync automatically; the
                // only difference between main and undocked is the OS
                // window id they render into.
                super::state::WindowKind::UndockedTab { .. } => self.view_main_for(window_id),
                super::state::WindowKind::DetachedPanel(kind) => {
                    let ctx = &self.document_state.panel_ctx;
                    let panel = if *kind == crate::panels::PanelKind::Components {
                        crate::panels::components_panel::view(&self.library, ctx, &ctx.tokens)
                            .map(crate::dock::DockMessage::Library)
                    } else {
                        crate::panels::view_panel(*kind, ctx).map(crate::dock::DockMessage::Panel)
                    }
                    .map(Message::Dock);
                    iced::widget::container(iced::widget::scrollable(panel))
                        .padding(SP.m)
                        .into()
                }
                // Detached Component Preview window — render the same
                // editor surface as the inline tab. The editor state
                // is keyed by `EditorAddress(library_path, table,
                // row_id)` so the inline + detached cases share a
                // single state owner.
                super::state::WindowKind::ComponentEditor {
                    library_path,
                    table,
                    row_id,
                } => {
                    let tokens = &self.document_state.panel_ctx.tokens;
                    let address = crate::library::state::EditorAddress::new(
                        library_path.clone(),
                        table.clone(),
                        *row_id,
                    );
                    if let Some(editor) = self.library.editors.get(&address) {
                        crate::library::editor::view(editor, &self.library, tokens, address)
                            .map(Message::Library)
                    } else {
                        // Window mapping exists but the editor state
                        // has been dropped (rare race during teardown
                        // — the tab close path can run ahead of the
                        // OS window close). Render an empty container
                        // so the daemon doesn't panic.
                        iced::widget::container(iced::widget::Space::new()).into()
                    }
                }
            };
        }
        self.view_main_for(window_id)
    }

    /// Cursor-following translucent preview of a tab being dragged.
    /// Shape matches the real tab bar entry — rounded container with
    /// the title text, the ↗ undock indicator, and the × close icon —
    /// so it reads as "the tab itself is moving". The ghost is
    /// non-interactive; it just shows what the user is carrying.
    fn view_tab_drag_ghost(&self, title: &str) -> Element<'_, Message> {
        use iced::widget::{container, row, text};
        use signex_widgets::tab_pill::{AccentPosition, TabPill, TabPillStyle};
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let active_bg = crate::styles::ti(tokens.hover);
        let accent = crate::styles::ti(tokens.accent);
        // Match the live tab look: same TabPill widget, accent
        // stripe at the bottom, top-rounded corners. The previous
        // ghost showed an old style with inline ↗ undock + × close
        // glyphs that were removed when the tab right-click menu
        // landed.
        let pill_style = TabPillStyle {
            fill: iced::Color {
                a: 0.88,
                ..active_bg
            },
            border: crate::styles::ti(tokens.border),
            accent,
            is_active: true,
            is_last: true,
            accent_position: AccentPosition::Bottom,
        };
        let inner = container(row![text(title.to_string()).size(TS.body).color(text_c)])
            .padding([SP.s, 10.0]);
        let pill = TabPill::new(inner, pill_style);
        // Anchor near the cursor (right + below) so the pointer
        // remains visible while the ghost trails it.
        let (cx, cy) = self.interaction_state.last_mouse_pos;
        super::view::translate::Translate::new(pill, (cx + 10.0, cy + 6.0)).into()
    }

    fn view_main_for(&self, window_id: iced::window::Id) -> Element<'_, Message> {
        let ui = &self.ui_state;
        let document = &self.document_state;
        let interaction = &self.interaction_state;
        // Context-aware menu: rows keyed on a command id grey through
        // `command::is_enabled`, from this window's `CommandContext` —
        // the same predicate the keyboard path dispatches by (#368).
        let menu_ctx = crate::menu_bar::MenuContext {
            commands: self.command_context(Some(window_id)),
            has_project: document.active_project.is_some(),
            // Secondary windows (detached modal, undocked tab) borrow
            // the main window's scale. Good enough until per-window
            // scale tracking lands — it's only wrong if the user drags
            // a secondary window onto a monitor with a different DPI.
            scale_factor: crate::ui_scale::render_scale(ui.main_window_scale, ui.ui_scale),
            active_keymap: Some(std::sync::Arc::clone(&ui.active_keymap)),
            recent_projects: ui.recent_projects.entries(),
            open_panels: self.open_panels(),
        };
        let undocked = ui.main_window_id != Some(window_id);
        let menu_row = || {
            menu_bar::view(&document.panel_ctx.tokens, menu_ctx).map(move |msg| match msg {
                MenuMessage::Undo | MenuMessage::Redo if undocked => {
                    Message::Ui(UiMsg::MenuInWindow {
                        window: window_id,
                        msg,
                    })
                }
                msg => Message::Menu(msg),
            })
        };

        let left_has_panels = document.dock.has_panels(PanelPosition::Left);
        let right_has_panels = document.dock.has_panels(PanelPosition::Right);
        let bottom_has_panels = document.dock.has_panels(PanelPosition::Bottom);
        let left_collapsed = document.dock.is_collapsed(PanelPosition::Left);
        let right_collapsed = document.dock.is_collapsed(PanelPosition::Right);
        let bottom_collapsed = document.dock.is_collapsed(PanelPosition::Bottom);

        let left = self.view_dock_panel(
            PanelPosition::Left,
            left_has_panels,
            left_collapsed,
            ui.left_width,
        );
        let left_handle = self.view_resize_handle(
            DragTarget::LeftPanel,
            left_has_panels && !left_collapsed,
            true,
        );
        let center = self.view_center(window_id);
        let right_handle = self.view_resize_handle(
            DragTarget::RightPanel,
            right_has_panels && !right_collapsed,
            true,
        );
        let right = self.view_dock_panel(
            PanelPosition::Right,
            right_has_panels,
            right_collapsed,
            ui.right_width,
        );

        let center_row = row![left, left_handle, center, right_handle, right];
        let bottom_handle = self.view_resize_handle(
            DragTarget::BottomPanel,
            bottom_has_panels && !bottom_collapsed,
            false,
        );
        let bottom = self.view_dock_panel_h(
            PanelPosition::Bottom,
            bottom_has_panels,
            bottom_collapsed,
            ui.bottom_height,
        );

        let altium_import_name = document.altium_import_running.as_ref().map(|run| {
            run.input
                .file_name()
                .map(|f| f.to_string_lossy().into_owned())
                .unwrap_or_else(|| run.input.display().to_string())
        });
        let status = status_bar::view(
            ui.cursor_x,
            ui.cursor_y,
            ui.grid_visible,
            ui.snap_enabled,
            ui.zoom,
            ui.unit,
            &interaction.current_tool,
            ui.grid_size_mm,
            &interaction.canvas_for_window(window_id).selected,
            &document.panel_ctx.tokens,
            document.inflight_git_commits.len(),
            altium_import_name.as_deref(),
            self.status_activity(window_id).as_deref(),
        )
        .map(|req| Message::Ui(UiMsg::StatusBar(req)));

        // Partition tabs across windows: main owns every tab that isn't
        // currently rendered by an undocked-tab window; each undocked
        // window owns exactly its one tab. Closing a tab in one window
        // can no longer reach tabs that belong to the other.
        let all_undocked_paths: std::collections::HashSet<std::path::PathBuf> = ui
            .windows
            .values()
            .filter_map(|kind| match kind {
                super::state::WindowKind::UndockedTab { path, .. } => Some(path.clone()),
                _ => None,
            })
            .collect();
        let is_main_window = ui.main_window_id == Some(window_id);

        // Main window is borderless: wordmark + menus + drag + search +
        // min/max/close in a single 36 px row. Undocked tab windows keep
        // their OS chrome and use the plain styled strip.
        let top_chrome: Element<'_, Message> = if is_main_window {
            // The system menu bar replaces the in-window row on macOS.
            let row = (!self.native_menu.hides_in_window_menu()).then(menu_row);
            self.view_main_window_chrome(row, &document.panel_ctx.tokens)
        } else {
            menu_bar::wrap_plain(menu_row(), &document.panel_ctx.tokens)
        };
        // Native macOS decorations resize the window themselves.
        let has_resize_edges = is_main_window && !self.window_controls_layout().is_native();
        let mut main = column![top_chrome];
        let visible_paths: std::collections::HashSet<std::path::PathBuf> = if is_main_window {
            document
                .tabs
                .iter()
                .map(|t| t.path.clone())
                .filter(|p| !all_undocked_paths.contains(p))
                .collect()
        } else {
            match ui.windows.get(&window_id) {
                Some(super::state::WindowKind::UndockedTab { path, .. }) => {
                    std::iter::once(path.clone()).collect()
                }
                _ => std::collections::HashSet::new(),
            }
        };
        // Reserve the tab strip's vertical footprint regardless of
        // whether any document is open — opening the first document
        // would otherwise shift the entire chrome down by ~24 px,
        // which feels jarring. The 1 px chrome separator stays
        // visible too so the menu row always reads as a distinct
        // band above the tab strip.
        main = main.push(
            container(iced::widget::Space::new())
                .width(Length::Fill)
                .height(1)
                .style(crate::styles::chrome_separator(&document.panel_ctx.tokens)),
        );
        if !document.tabs.is_empty() && !visible_paths.is_empty() {
            // Resolve "really dragging" — Some only after the
            // cursor has travelled past a 6 px threshold from
            // the press origin. Without this, every click-to-
            // switch armed the drag state and flipped the
            // cursor to Grabbing instantaneously, plus flashed
            // the drag ghost.
            let dragging = ui.tab_dragging.and_then(|(idx, ox, oy)| {
                let (mx, my) = interaction.last_mouse_pos;
                let dx = mx - ox;
                let dy = my - oy;
                if dx * dx + dy * dy > TAB_DRAG_THRESHOLD_PX * TAB_DRAG_THRESHOLD_PX {
                    Some(idx)
                } else {
                    None
                }
            });
            let dirty_paths =
                tab_bar::dirty_tab_paths(&document.tabs, &document.dirty_paths, &self.library);
            let read_only_paths = tab_bar::read_only_tab_paths(&document.tabs, &document.engines);
            main = main.push(
                tab_bar::view(
                    &document.tabs,
                    document.active_tab,
                    dragging,
                    &visible_paths,
                    &dirty_paths,
                    &read_only_paths,
                    &document.panel_ctx.tokens,
                )
                .map(move |msg| Message::Tab { window_id, msg }),
            );
        } else {
            // Same height as the tab strip (`tab_bar::TAB_STRIP_HEIGHT`), so
            // the chrome does not jump when the first tab opens.
            main = main.push(tab_bar::placeholder(&document.panel_ctx.tokens));
        }
        let main = main
            .push(center_row)
            .push(bottom_handle)
            .push(bottom)
            .push(status);

        // Borderless window needs its own edge-resize hit zones — the OS
        // frame would normally handle this, but `decorations: false`
        // removes WS_THICKFRAME on Windows. Tab windows keep OS
        // decorations so they skip the overlay entirely. The overlay is
        // applied later as a Stack layer over `main` so the content
        // keeps its natural origin and overlay y-coordinates stay
        // correct.
        let main: Element<'_, Message> = main.into();

        // No open-predicate here on purpose. Every overlay builder owns
        // its own "am I open?" guard and returns no layers when closed
        // (see `overlay_id::extend_overlay`), so the layer list is the
        // single answer to "is anything painted". A hand-written copy of
        // those guards drifted — ErrorNotice, NetlistIncompletePrompt and
        // PrintPreview were missing, so they only painted on an editor tab
        // (#114). A new overlay needs a builder guard and an `OverlayId`,
        // nothing else.
        // A confirm dialog in its own window locks this one: a layer eats the
        // pointer and a click raises that dialog. It sits above the non-modal
        // chrome (menus, Active Bar, text edit, panels) and below the dialogs,
        // so nothing can be edited through it and an in-window dialog that
        // opens meanwhile (quit confirm, a notice) can still be answered.
        let lock = self
            .locking_confirm_window()
            .is_some()
            .then(|| self.locked_window_layer());
        let mut overlays = self.collect_overlays(lock);
        // Tab drag ghost: only renders once the cursor has travelled past
        // the same 6 px threshold the cursor gating uses
        // (`tab_bar::view`). Mirrors that gate here so press-without-move
        // keeps the ghost off.
        if let Some((tab_idx, ox, oy)) = ui.tab_dragging
            && let Some(tab) = document.tabs.get(tab_idx)
        {
            let (mx, my) = interaction.last_mouse_pos;
            let dx = mx - ox;
            let dy = my - oy;
            if dx * dx + dy * dy > TAB_DRAG_THRESHOLD_PX * TAB_DRAG_THRESHOLD_PX {
                overlays.push(self.view_tab_drag_ghost(&tab.title));
            }
        }
        if !overlays.is_empty() {
            let mut stack = iced::widget::Stack::new().push(main);
            // Resize edges sit above the content but below functional
            // overlays (Active Bar, menus, modals) so the 6 px border
            // strip doesn't eat clicks on those.
            if has_resize_edges {
                stack = stack.push(Self::resize_edges_overlay());
            }
            for overlay in overlays {
                stack = stack.push(overlay);
            }
            stack.into()
        } else if has_resize_edges {
            iced::widget::Stack::new()
                .push(main)
                .push(Self::resize_edges_overlay())
                .into()
        } else {
            main
        }
    }

    /// Full-window layer that takes every pointer event while a detached
    /// confirm dialog locks the window; pressing it raises that dialog.
    fn locked_window_layer(&self) -> Element<'_, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let hint = iced::widget::container(
            iced::widget::text("A dialog is open: click to show it, Esc to cancel")
                .size(12)
                .color(crate::styles::ti(tokens.text_secondary)),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        // A light dim, so a locked window reads as locked at a glance.
        .style(|_: &iced::Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                0.0, 0.0, 0.0, 0.22,
            ))),
            ..Default::default()
        });
        iced::widget::opaque(
            iced::widget::mouse_area(hint)
                .on_press(Message::Window(WindowMsg::LockedWindowClicked)),
        )
    }

    fn view_center(&self, window_id: iced::window::Id) -> Element<'_, Message> {
        let is_main = self.ui_state.main_window_id == Some(window_id);
        if let Some(project_id) = self.project_tab_in_window(is_main) {
            return self.view_project_tab(project_id);
        }

        // When the active tab is a Component Preview, render the
        // editor inside the main window's content pane. The same
        // surface lights up via the `WindowKind::ComponentEditor`
        // branch in `view()` when the user undocks the tab into its
        // own OS window.
        if is_main
            && let Some(active_tab) = self.document_state.tabs.get(self.document_state.active_tab)
            && let Some(editor_id) = active_tab.kind.as_component_editor()
        {
            let tokens = &self.document_state.panel_ctx.tokens;
            let address = crate::library::state::EditorAddress::new(
                editor_id.library_path.clone(),
                editor_id.table.clone(),
                editor_id.row_id,
            );
            return if let Some(editor) = self.library.editors.get(&address) {
                crate::library::editor::view(editor, &self.library, tokens, address)
                    .map(Message::Library)
            } else {
                container(
                    column![
                        iced::widget::text("Component Editor — state not yet loaded")
                            .size(TS.title)
                            .color(crate::styles::ti(tokens.text_secondary)),
                    ]
                    .spacing(SP.s)
                    .align_x(iced::Alignment::Center),
                )
                .center(Length::Fill)
                .style(crate::styles::panel_region(tokens))
                .into()
            };
        }

        // Standalone primitive editor tabs. `.snxsym` / `.snxfpt`
        // open as main-window document tabs alongside `.snxsch` /
        // `.snxpcb`. Lookup is path-keyed via
        // `DocumentState.symbol_editors` / `footprint_editors`.
        if is_main
            && let Some(active_tab) = self.document_state.tabs.get(self.document_state.active_tab)
        {
            if let Some(path) = active_tab.kind.as_symbol_editor()
                && let Some(editor) = self.document_state.symbol_editors.get(path)
            {
                let panel_ctx = &self.document_state.panel_ctx;
                // Per-library display settings — Altium-style
                // Document Options. Resolve the `.snxlib/` ancestor
                // of the symbol's path so every primitive editor
                // opened from the same library shares the same
                // grid / unit / background. Lone-file edits (no
                // mounted library) get safe defaults.
                let display = self
                    .library
                    .containing_library(path)
                    .map(|lib| lib.display)
                    .unwrap_or_default();
                return crate::library::editor::standalone::view_symbol(
                    editor, panel_ctx, display, path,
                )
                .map(Message::Library);
            }
            if let Some(path) = active_tab.kind.as_footprint_editor()
                && let Some(editor) = self.document_state.footprint_editors.get(path)
            {
                let tokens = &self.document_state.panel_ctx.tokens;
                let theme_id = self.ui_state.theme_id;
                return crate::library::editor::standalone::view_footprint(
                    editor, tokens, theme_id,
                )
                .map(Message::Library);
            }
            // Library Browser tab — `.snxlib` opened as a main-window
            // tab. Per-tab state lives in
            // `LibraryState.library_browsers` keyed by the same path
            // that lives on `TabInfo.path`.
            if let Some(path) = active_tab.kind.as_library_browser() {
                let tokens = &self.document_state.panel_ctx.tokens;
                if let Some(browser) = self.library.library_browsers.get(path) {
                    return crate::library::browser::view(path, &self.library, browser, tokens)
                        .map(Message::Library);
                } else {
                    // Fallback when somehow the browser-state map is
                    // out of sync with the tabs vector. Keeps the tab
                    // renderable rather than crashing.
                    return container(
                        iced::widget::text("Library Browser — state not yet loaded")
                            .size(TS.title)
                            .color(crate::styles::ti(tokens.text_secondary)),
                    )
                    .center(Length::Fill)
                    .style(crate::styles::panel_region(tokens))
                    .into();
                }
            }
        }

        let has_schematic = if is_main {
            self.has_active_schematic()
        } else {
            // An undocked tab window renders if its path still has a
            // live engine in the HashMap. Falls back to the main
            // predicate when the window has already been dropped from
            // the windows map (mid-close frame).
            self.document_state
                .engine_for_window(window_id, &self.ui_state)
                .is_some()
        };
        if has_schematic {
            // Canvas events from non-main windows need to carry the
            // window_id through to the dispatch layer so the right
            // per-window canvas receives the mutation. Keyboard
            // shortcuts that synthesize `Message::CanvasEvent` keep
            // targeting the main canvas unchanged.
            // #631 — the canvas Program is built here, per frame, from a
            // borrow of the window's `CanvasSlot` plus settings read
            // straight off `UiState`. It no longer owns copies of them.
            let slot = self.interaction_state.canvas_for_window(window_id);
            let prefs = crate::canvas::CanvasViewPrefs {
                placement: self.placement_ghost_for_window(window_id),
                ..self.canvas_view_prefs()
            };
            let base: Element<'_, Message> = if crate::feature_flags::SCHEMATIC_GPU_RENDER_ENABLED {
                crate::canvas::gpu_mount::view(slot, prefs)
            } else {
                canvas(crate::canvas::SchematicCanvas::new(slot, prefs))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into()
            };
            let base = if is_main {
                base
            } else {
                base.map(move |msg| match msg {
                    Message::CanvasEvent(event) => {
                        Message::CanvasEventInWindow { window_id, event }
                    }
                    other => other,
                })
            };
            // A sheet from a newer Signex is read-only: say so on the canvas
            // (#1402), in the main and in undocked windows alike.
            let read_only = self
                .document_state
                .engine_for_window(window_id, &self.ui_state)
                .is_some_and(|engine| engine.document().is_read_only());
            // Always a two-layer stack, so the widget tree keeps its shape
            // when a tab switch changes the read-only state.
            let badge: Element<'_, Message> = if read_only {
                overlays::read_only_badge::read_only_badge(&self.document_state.panel_ctx.tokens)
            } else {
                iced::widget::Space::new().into()
            };
            stack![base, badge].into()
        } else if self.pcb_for_window(window_id).is_some() {
            // Each window reads its own PCB canvas, and an undocked
            // window's events carry its id, as for the schematic (#494).
            let pcb_canvas = self.interaction_state.pcb_canvas_for_window(window_id);
            let base: Element<'_, Message> = canvas(pcb_canvas)
                .width(Length::Fill)
                .height(Length::Fill)
                .into();
            let base = if is_main {
                base
            } else {
                base.map(move |msg| match msg {
                    Message::CanvasEvent(event) => {
                        Message::CanvasEventInWindow { window_id, event }
                    }
                    other => other,
                })
            };
            if pcb_canvas.gpu_render {
                // Stack the GPU shader ABOVE the CPU canvas: the canvas paints
                // the opaque background + grid and owns pan/zoom/cursor/fit;
                // the shader draws the board content on top, compositing over
                // the grid. The shader's `update` is a no-op that never
                // captures, so pointer events fall through to the canvas below.
                let scene = pcb_canvas.gpu_scene().unwrap_or_default();
                // Read the generation after `gpu_scene()` so it matches the
                // geometry just returned; it lets the shader skip re-uploading
                // unchanged geometry on pan/zoom.
                let generation = pcb_canvas.scene_generation();
                let (offset_x, offset_y, scale) = pcb_canvas.live_camera();
                // `PcbSurface` selects this surface's pipeline slot — iced keys
                // a stored pipeline by the primitive's TypeId, process-wide, so
                // every surface needs its own marker (see `scene_shader`).
                let content_shader = shader(crate::scene_shader::SceneShaderProgram::<
                    crate::scene_shader::PcbSurface,
                >::new(
                    scene, Some(generation), [offset_x, offset_y], scale
                ))
                .width(Length::Fill)
                .height(Length::Fill);
                stack![base, content_shader].into()
            } else {
                base
            }
        } else {
            // "Nothing loaded" gets the actionable start screen; "project
            // loaded, no document picked" keeps its own quieter hint.
            use crate::start_screen::{self, Surface};
            let tokens = &self.document_state.panel_ctx.tokens;
            match Surface::select(self.document_state.active_project.is_some()) {
                Surface::ProjectHint => start_screen::hint_view(tokens),
                Surface::Start => {
                    let open_shortcut = self.keymap_shortcut_label("open_document");
                    start_screen::view(
                        tokens,
                        &open_shortcut,
                        self.ui_state.recent_projects.entries(),
                    )
                }
            }
        }
    }

    /// Assemble the floating overlay stack painted over the main view.
    ///
    /// Pure assembler: each overlay's guard + widget tree lives in a
    /// dedicated `*_overlay` builder (see the `overlays` module and its
    /// `bars` / `modals` submodules), and the order they stack in lives
    /// in [`overlay_id::PAINT_ORDER`] — walked FORWARD here, so later
    /// entries paint on top of earlier ones.
    ///
    /// That same array is walked BACKWARD by the Esc ladder
    /// (`app/bootstrap/subscription.rs`), which is the whole point of
    /// #535: "Esc closes the topmost overlay" stops being a rule two
    /// files must obey by hand — in opposite directions — and becomes
    /// one list read from both ends.
    ///
    /// [`overlay_id::visible`] applies the blocking-modal cutoff, which
    /// is why this loop has no early return of its own.
    fn collect_overlays<'a>(
        &'a self,
        mut lock: Option<Element<'a, Message>>,
    ) -> Vec<Element<'a, Message>> {
        let mut layers = Vec::new();
        for id in overlay_id::visible(self.has_blocking_modal()) {
            // The lock layer goes in where the dialogs start.
            if *id == overlay_id::OverlayId::Preferences
                && let Some(layer) = lock.take()
            {
                layers.push(layer);
            }
            let first_new = layers.len();
            self.extend_overlay(&mut layers, *id);
            // #562 — stop the wheel here rather than let it reach the
            // canvas. Applied per overlay id, not per builder: most
            // builders wrap their card in neither `wrap_modal` nor
            // `dismiss_layer` (Preferences renders its body directly), so
            // a guard placed in those two helpers alone missed them.
            // Every overlay passes through this loop exactly once.
            //
            // `mouse_area` captures the wheel only where the cursor is
            // over the wrapped element, and only wheel events — clicks
            // still fall through exactly as before, which matters for the
            // overlays that deliberately leave the canvas its right-press
            // pan (see `dismiss_layer`).
            if overlay_id::swallows_scroll(*id) {
                for layer in &mut layers[first_new..] {
                    let inner = std::mem::replace(layer, iced::widget::Space::new().into());
                    *layer = iced::widget::mouse_area(inner)
                        .on_scroll(|_| Message::noop())
                        .into();
                }
            }
        }
        // A blocking modal cut the walk short before the dialogs: the lock
        // goes under what is left.
        if let Some(layer) = lock {
            layers.insert(0, layer);
        }
        layers
    }
}

#[cfg(test)]
mod overlay_paint_tests;

#[cfg(test)]
mod chrome_geometry_tests {
    use super::*;

    /// The bar is centred on the window, shrinks monotonically as the
    /// window narrows, and never runs off the right edge or under the
    /// menu items.
    #[test]
    fn search_bar_stays_centred_and_clamped() {
        // Windows-style: nothing leading, 138 px of controls trailing.
        let in_window = crate::menu_bar::approx_menu_bar_width();
        search_bar_invariants(0.0, 138.0, in_window);
        // Native macOS lights: room leading, nothing trailing.
        search_bar_invariants(78.0, 0.0, in_window);
        // Round buttons on both sides.
        search_bar_invariants(56.0, 40.0, in_window);
        // The system menu bar carries the menus: nothing to clear.
        search_bar_invariants(78.0, 0.0, 0.0);
    }

    fn search_bar_invariants(leading_w: f32, trailing_w: f32, menu_w: f32) {
        let menu_end = leading_w + menu_w + CHROME_SEARCH_LEFT_GAP;
        for window_w in [1000.0_f32, 1280.0, 1440.0, 1920.0, 3840.0] {
            let (x, w) = chrome_search_bar_geometry(window_w, leading_w, trailing_w, menu_w);
            assert!(
                (CHROME_SEARCH_BAR_MIN_WIDTH..=CHROME_SEARCH_BAR_WIDTH).contains(&w),
                "bar width {w} out of range at {window_w}"
            );
            assert!(
                x >= menu_end,
                "bar slid under the menu at {window_w}: x={x}"
            );
            assert!(x + w <= window_w, "bar overflows the window at {window_w}");
            // Centred whenever the window is wide enough for it.
            if window_w - 2.0 * menu_end >= CHROME_SEARCH_BAR_MIN_WIDTH {
                assert!(
                    (x + w / 2.0 - window_w / 2.0).abs() < 0.01,
                    "bar off window centre at {window_w}: x={x} w={w}"
                );
            }
        }
        // Narrowing the window never widens the bar.
        let (_, wide) = chrome_search_bar_geometry(1920.0, leading_w, trailing_w, menu_w);
        let (_, narrow) = chrome_search_bar_geometry(1100.0, leading_w, trailing_w, menu_w);
        assert!(narrow <= wide, "bar grew as the window shrank");
    }
}
