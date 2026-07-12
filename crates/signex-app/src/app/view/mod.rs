use iced::widget::{canvas, column, container, row, text_input};
use iced::{Element, Length};

pub(crate) mod dialogs;
pub(crate) mod translate;

mod context_menu;
mod pdf_preview;
mod print_preview;
mod modals;
mod chrome;
mod overlays;

use super::*;

// ── Submenu chevron — single source of truth ─────────────────────────
//
// Right-pointing angle quote (U+203A), NOT the BLACK RIGHT-POINTING
// TRIANGLE (U+25B6) which Windows renders via the colour emoji font.
// Same glyph the menu_bar dropdowns use; the matching size below keeps
// every submenu launcher visually aligned across the whole app
// (canvas right-click, project-tree right-click, File/Edit/View menu).
const SUBMENU_ARROW: &str = "›";
const SUBMENU_ARROW_SIZE: f32 = 18.0;

/// Chrome strip search bar width in pixels.
pub(crate) const CHROME_SEARCH_BAR_WIDTH: f32 = 440.0;
/// Fixed gap between the chrome search bar's right edge and the
/// chrome controls (min/max/close).
pub(crate) const CHROME_SEARCH_BAR_RIGHT_GAP: f32 = 12.0;
/// One chrome control button (min / max / close) width — see
/// `chrome_btn` in `view_main_window_chrome`.
pub(crate) const CHROME_CONTROL_BTN_W: f32 = 46.0;
/// Total controls strip width — three buttons.
pub(crate) const CHROME_CONTROLS_W: f32 = CHROME_CONTROL_BTN_W * 3.0;
/// Minimum left padding between the menu bar's right edge and the
/// chrome search bar's left edge.
pub(crate) const CHROME_SEARCH_LEFT_GAP: f32 = 16.0;
/// Minimum right padding between the chrome search bar's right edge
/// and the window-controls strip.
pub(crate) const CHROME_SEARCH_RIGHT_GAP: f32 = 16.0;

impl Signex {
    pub fn view(&self, window_id: iced::window::Id) -> Element<'_, Message> {
        // Secondary windows (detached modals, future undocked tabs) render
        // just their own content — no menu / dock / canvas. The main
        // window's view_main drops any overlay whose modal is currently
        // detached so we don't double-render.
        if let Some(kind) = self.ui_state.windows.get(&window_id) {
            return match kind {
                super::state::WindowKind::DetachedModal(modal) => self.view_detached_modal(*modal),
                // Undocked tab = full duplicate of the main app view.
                // Shared Signex state means edits sync automatically; the
                // only difference between main and undocked is the OS
                // window id they render into.
                super::state::WindowKind::UndockedTab { .. } => self.view_main_for(window_id),
                super::state::WindowKind::DetachedPanel(kind) => {
                    let panel = crate::panels::view_panel(*kind, &self.document_state.panel_ctx)
                        .map(crate::dock::DockMessage::Panel)
                        .map(Message::Dock);
                    iced::widget::container(iced::widget::scrollable(panel))
                        .padding(8)
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
        let inner =
            container(row![text(title.to_string()).size(11).color(text_c)]).padding([4, 10]);
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
        // Context-aware menu: each leaf gates on whether its action
        // makes sense in the current app state. `has_schematic` /
        // `has_selection` drive most entries; undo / redo consult
        // the engine's history so they grey out when empty.
        // v0.14.2: surface the active tab's primitive-editor kind to
        // the menu so File ▸ Save / Save As enable themselves for
        // `.snxsym` and `.snxfpt` standalone editor tabs.
        let active_tab_kind = document.tabs.get(document.active_tab).map(|t| &t.kind);
        let has_symbol_editor =
            matches!(active_tab_kind, Some(crate::app::TabKind::SymbolEditor(_)));
        let has_footprint_editor = matches!(
            active_tab_kind,
            Some(crate::app::TabKind::FootprintEditor(_))
        );

        let menu_ctx = crate::menu_bar::MenuContext {
            has_schematic: self.has_active_schematic(),
            has_pcb: self.has_active_pcb(),
            has_project: document.active_project.is_some(),
            has_selection: !interaction.canvas_for_window(window_id).selected.is_empty(),
            can_undo: document
                .engine_for_window(window_id, ui)
                .map(|e| e.can_undo())
                .unwrap_or(false),
            can_redo: document
                .engine_for_window(window_id, ui)
                .map(|e| e.can_redo())
                .unwrap_or(false),
            has_symbol_editor,
            has_footprint_editor,
            // Secondary windows (detached modal, undocked tab) borrow
            // the main window's scale. Good enough until per-window
            // scale tracking lands — it's only wrong if the user drags
            // a secondary window onto a monitor with a different DPI.
            scale_factor: ui.main_window_scale,
            active_keymap: Some(ui.active_keymap.clone()),
        };
        let menu_row = menu_bar::view(&document.panel_ctx.tokens, menu_ctx).map(Message::Menu);

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
            self.view_main_window_chrome(menu_row, &document.panel_ctx.tokens)
        } else {
            menu_bar::wrap_plain(menu_row, &document.panel_ctx.tokens)
        };
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
            const DRAG_THRESHOLD_PX: f32 = 6.0;
            let dragging = ui.tab_dragging.and_then(|(idx, ox, oy)| {
                let (mx, my) = interaction.last_mouse_pos;
                let dx = mx - ox;
                let dy = my - oy;
                if dx * dx + dy * dy > DRAG_THRESHOLD_PX * DRAG_THRESHOLD_PX {
                    Some(idx)
                } else {
                    None
                }
            });
            main = main.push(
                tab_bar::view(
                    &document.tabs,
                    document.active_tab,
                    dragging,
                    &visible_paths,
                    &document.panel_ctx.tokens,
                )
                .map(move |msg| Message::Tab { window_id, msg }),
            );
        } else {
            // Empty placeholder strip with the same metrics as
            // tab_bar::view: 2 px outer padding + 22 px tall inner
            // pill = 26 px total. Without this the chrome jumps
            // when the first tab opens.
            let placeholder = container(iced::widget::Space::new())
                .width(Length::Fill)
                .height(26)
                .style(crate::styles::toolbar_strip(&document.panel_ctx.tokens));
            main = main.push(placeholder);
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

        // v0.13 — `has_active_bar` is now true for ANY editor tab
        // that mounts an active bar (schematic / footprint /
        // symbol library) so the layers Stack mounts and the bar
        // layer fires from `view_main_for` regardless of editor
        // kind.
        let active_tab_kind_any = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .map(|t| &t.kind);
        let has_footprint_bar = matches!(
            active_tab_kind_any,
            Some(crate::app::TabKind::FootprintEditor(_))
        );
        let has_symbol_bar = matches!(
            active_tab_kind_any,
            Some(crate::app::TabKind::SymbolEditor(_))
        );
        let has_active_bar = self.has_active_schematic() || has_footprint_bar || has_symbol_bar;
        let dragging_tab = ui.tab_dragging.is_some();
        let needs_overlay = has_active_bar
            || interaction.editing_text.is_some()
            || interaction.context_menu.is_some()
            || interaction.project_tree_context_menu.is_some()
            || interaction.tab_context_menu.is_some()
            || interaction.active_bar_menu.is_some()
            || interaction.canvas.placement_paused
            || self
                .document_state
                .tabs
                .get(self.document_state.active_tab)
                .and_then(|tab| tab.kind.as_footprint_editor())
                .and_then(|path| self.document_state.footprint_editors.get(path))
                .map(|ed| {
                    ed.state.placement_paused
                        || ed.state.active_bar_menu.is_some()
                        || ed.state.move_by_modal.is_some()
                })
                .unwrap_or(false)
            || ui.panel_list_open
            || ui.find_replace.open
            || ui.preferences_open
            || ui.keyboard_shortcuts_open
            || ui.first_run_tour_open
            || ui.rename_dialog.is_some()
            || ui.remove_dialog.is_some()
            || ui.project_close_confirm.is_some()
            || ui.app_quit_confirm.is_some()
            || ui.project_options.is_some()
            || ui.enable_version_control.is_some()
            || ui.grid_properties.is_some()
            || ui.selection_filter_custom.is_some()
            || interaction.grid_picker.is_some()
            || document.bom_preview.is_some()
            || ui.annotate_dialog_open
            || ui.annotate_reset_confirm
            || ui.erc_dialog_open
            || !document.dock.floating.is_empty()
            || dragging_tab
            || ui.net_color_custom.show
            // Library-side modals (New Component, Place Component picker,
            // Pick Symbol/Footprint primitive picker) can be triggered
            // from non-canvas contexts — e.g. the Library Browser tab's
            // Add Component button. Without these flags the overlay
            // Stack would never be built and the modal layer in
            // collect_overlays would silently no-op.
            || self.library.new_component.is_some()
            || self.library.picker.is_some()
            || self.library.primitive_picker.is_some()
            || self.library.close_library_confirm.is_some()
            || self.library.document_options.is_some()
            || self.library.recovery.is_some()
            || self.library.create_options.is_some()
            || self.library.library_updates.is_some()
            || self
                .library
                .library_browsers
                .values()
                .any(|s| s.edit_modal.is_some() || s.delete_confirm.is_some())
            || ui.command_palette.open
            // Hover tooltip — needs the overlay stack to render even
            // when no other modal is open; otherwise `view_hover_tooltip`
            // produces an Element that's silently dropped on every
            // frame the user hovers a symbol over a bare canvas.
            // Mirrors the needs_overlay-predicate-gates-modal pattern.
            || (interaction.hover_symbol_uuid.is_some()
                && interaction
                    .hover_started_at
                    .is_some_and(|t| t.elapsed() >= std::time::Duration::from_millis(700)));

        if needs_overlay {
            let mut overlays = self.collect_overlays();
            // Tab drag ghost: only renders once the cursor has
            // travelled past the same 6 px threshold the cursor
            // gating uses (`tab_bar::view`). Mirrors that gate
            // here so press-without-move keeps the ghost off.
            if let Some((tab_idx, ox, oy)) = ui.tab_dragging
                && let Some(tab) = document.tabs.get(tab_idx)
            {
                const DRAG_GHOST_THRESHOLD_PX: f32 = 6.0;
                let (mx, my) = interaction.last_mouse_pos;
                let dx = mx - ox;
                let dy = my - oy;
                if dx * dx + dy * dy > DRAG_GHOST_THRESHOLD_PX * DRAG_GHOST_THRESHOLD_PX {
                    overlays.push(self.view_tab_drag_ghost(&tab.title));
                }
            }
            let mut stack = iced::widget::Stack::new().push(main);
            // Resize edges sit above the content but below functional
            // overlays (Active Bar, menus, modals) so the 6 px border
            // strip doesn't eat clicks on those.
            if is_main_window {
                stack = stack.push(Self::resize_edges_overlay());
            }
            for overlay in overlays {
                stack = stack.push(overlay);
            }
            stack.into()
        } else if is_main_window {
            iced::widget::Stack::new()
                .push(main)
                .push(Self::resize_edges_overlay())
                .into()
        } else {
            main.into()
        }
    }

    fn view_center(&self, window_id: iced::window::Id) -> Element<'_, Message> {
        let is_main = self.ui_state.main_window_id == Some(window_id);

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
                            .size(13)
                            .color(crate::styles::ti(tokens.text_secondary)),
                    ]
                    .spacing(4)
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
                let theme_id = self.ui_state.theme_id;
                return crate::library::editor::standalone::view_symbol(
                    editor, panel_ctx, display, theme_id, path,
                )
                .map(Message::Library);
            }
            if let Some(path) = active_tab.kind.as_footprint_editor()
                && let Some(editor) = self.document_state.footprint_editors.get(path)
            {
                let tokens = &self.document_state.panel_ctx.tokens;
                let theme_id = self.ui_state.theme_id;
                let custom_presets = &self.interaction_state.custom_filter_presets;
                return crate::library::editor::standalone::view_footprint(
                    editor,
                    tokens,
                    theme_id,
                    custom_presets,
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
                            .size(13)
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
            let base: Element<'_, Message> =
                canvas(self.interaction_state.canvas_for_window(window_id))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into();
            if is_main {
                base
            } else {
                base.map(move |msg| match msg {
                    Message::CanvasEvent(event) => {
                        Message::CanvasEventInWindow { window_id, event }
                    }
                    other => other,
                })
            }
        } else if self.has_active_pcb() {
            canvas(&self.interaction_state.pcb_canvas)
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        } else {
            // Distinguish "nothing loaded at all" from "project loaded,
            // but no document picked yet" — the second case is what
            // the user sees right after opening a .standard_pro before
            // clicking any node in the project tree.
            let (title, hint) = if self.document_state.active_project.is_some() {
                (
                    "No document selected",
                    "Choose a schematic or PCB from the project tree".to_string(),
                )
            } else {
                let open_shortcut = self.keymap_shortcut_label("open_document", "Ctrl+O");
                (
                    "No document open",
                    format!("Open a project with File > Open or {open_shortcut}"),
                )
            };
            container(
                column![
                    iced::widget::text(title).size(14).color(crate::styles::ti(
                        self.document_state.panel_ctx.tokens.text_secondary
                    )),
                    iced::widget::text(hint).size(11).color(crate::styles::ti(
                        self.document_state.panel_ctx.tokens.text_secondary
                    )),
                ]
                .spacing(8)
                .align_x(iced::Alignment::Center),
            )
            .center(Length::Fill)
            .style(crate::styles::panel_region(
                &self.document_state.panel_ctx.tokens,
            ))
            .into()
        }
    }

    /// Assemble the floating overlay stack painted over the main view.
    ///
    /// Pure assembler: each overlay's guard + widget tree lives in a
    /// dedicated `*_overlay` builder (see the `overlays` module and its
    /// `bars` / `modals` submodules). Call ORDER here is
    /// load-bearing — overlays stack visually in push order, so the
    /// sequence below reproduces the original inline push order exactly.
    /// `has_blocking_modal` short-circuits the tool/menu overlays just as
    /// the inline code did once a blocking modal owns the stack.
    fn collect_overlays(&self) -> Vec<Element<'_, Message>> {
        let mut layers = Vec::new();

        // Pre-blocking overlays: export-error, print/BOM preview, and
        // the custom net-colour picker.
        layers.extend(self.export_error_overlay());
        layers.extend(self.print_preview_overlay());
        layers.extend(self.bom_preview_overlay());
        layers.extend(self.net_color_custom_overlay());

        // Blocking modals must own the overlay stack — stop here so no
        // tool/menu overlay paints above them.
        if self.has_blocking_modal() {
            return layers;
        }

        // Editor-surface chrome: pause card, active bars (schematic /
        // footprint / symbol), and the in-canvas text-edit input.
        layers.extend(self.placement_paused_overlay());
        layers.extend(self.schematic_active_bar_overlay());
        layers.extend(self.footprint_active_bar_overlay());
        layers.extend(self.footprint_context_menu_overlay());
        layers.extend(self.footprint_move_by_overlay());
        layers.extend(self.symbol_editor_active_bar_overlay());
        layers.extend(self.text_edit_overlay());

        // Right-click menus, grid picker, panel list, dock drag zones,
        // and floating panels.
        layers.extend(self.active_bar_menu_overlay());
        layers.extend(self.context_menu_overlay());
        layers.extend(self.tab_context_menu_overlay());
        layers.extend(self.project_tree_context_menu_overlay());
        layers.extend(self.grid_picker_overlay());
        layers.extend(self.panel_list_overlay());
        layers.extend(self.dock_drag_zone_overlay());
        layers.extend(self.floating_panels_overlay());

        // Dialogs + library modals.
        layers.extend(self.preferences_overlay());
        layers.extend(self.find_replace_overlay());
        layers.extend(self.keyboard_shortcuts_overlay());
        layers.extend(self.first_run_tour_overlay());
        layers.extend(self.simple_dialogs_overlay());
        layers.extend(self.detachable_dialogs_overlay());
        layers.extend(self.library_picker_overlay());
        layers.extend(self.new_component_overlay());
        layers.extend(self.edit_row_modal_overlay());
        layers.extend(self.delete_confirm_overlay());
        layers.extend(self.primitive_picker_overlay());
        layers.extend(self.document_options_overlay());
        layers.extend(self.create_options_overlay());
        layers.extend(self.close_library_confirm_overlay());
        layers.extend(self.library_recovery_overlay());

        // Command palette dropdown, hover tooltip, and library-updates
        // modal — painted last so they sit above every other layer.
        layers.extend(self.command_palette_overlay());
        layers.extend(self.view_hover_tooltip());
        layers.extend(self.library_updates_overlay());

        layers
    }
}
