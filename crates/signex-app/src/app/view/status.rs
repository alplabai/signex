//! The status bar of one window, fed from the surface that window shows,
//! and the Grid & Snap popover its Grid read-out opens.

use iced::{Element, Length, Padding, Widget as _};

use super::*;
use crate::status_bar::{GridPopoverEvent, GridSnapSettings, LayerReadout, SurfaceReadout};

impl Signex {
    /// The status bar for `window_id`. A symbol tab shows its own grid, unit,
    /// tool and segment and no schematic selection; its Grid and unit buttons
    /// drive the symbol's library.
    pub(in crate::app::view) fn view_status_bar(
        &self,
        window_id: iced::window::Id,
    ) -> Element<'_, Message> {
        let document = &self.document_state;
        let board_import_name = document.altium_import_running.as_ref().map(|run| {
            run.input
                .file_name()
                .map(|f| f.to_string_lossy().into_owned())
                .unwrap_or_else(|| run.input.display().to_string())
        });
        let (cursor_x, cursor_y) = self.status_cursor(window_id);
        let grid = self.status_grid(window_id);
        let symbol = self.status_symbol_path(window_id);
        let selected: &[signex_types::schematic::SelectedItem] = if symbol.is_some() {
            &[]
        } else {
            &self.interaction_state.canvas_for_window(window_id).selected
        };
        let activity = self.status_activity(window_id);
        let bar = crate::status_bar::view(
            cursor_x,
            cursor_y,
            grid.visible,
            self.ui_state.snap_enabled,
            self.status_zoom(window_id),
            grid.unit,
            self.status_tool(window_id),
            grid.size_mm,
            selected,
            &document.panel_ctx.tokens,
            document.inflight_git_commits.len(),
            board_import_name.as_deref(),
            activity.as_ref(),
            SurfaceReadout {
                active_layer: self.status_layer(window_id),
                view_angle: self.status_view_angle(window_id),
                opens_grid_popover: self.status_sheet_tab(window_id).is_some(),
            },
            &self.ui_state.i18n,
        );
        bar.map(move |req| Signex::status_bar_message(window_id, symbol.as_deref(), req))
            .boxed()
    }

    /// The layer pill of `window`: the active layer's caption and name, and
    /// the colour that window's board view draws the layer in.
    fn status_layer(&self, window: iced::window::Id) -> Option<LayerReadout> {
        let (caption, name) = self.status_active_layer(window)?;
        let view = self.pcb_layer_view_of_window(Some(window));
        let layer = view.shown_active()?;
        Some(LayerReadout {
            caption,
            name,
            colour: crate::pcb_canvas::layer_style::painted_layer_colour(view.paint(), layer),
        })
    }

    /// The schematic tab `window` shows, if it shows one: only a sheet's
    /// Grid read-out opens the Grid & Snap popover (the board's grids are
    /// the Grid Manager's).
    fn status_sheet_tab(&self, window: iced::window::Id) -> Option<&crate::app::TabInfo> {
        self.document_state
            .tab_shown_in_window(window, &self.ui_state)
            .filter(|tab| matches!(tab.kind, crate::app::TabKind::Schematic))
    }

    /// Whether the Grid & Snap popover paints: it is open and its window
    /// shows a sheet. The Esc ladder reads this, not the bare `Option`.
    pub(crate) fn grid_snap_popover_shown(&self) -> bool {
        self.ui_state
            .grid_snap_popover
            .as_deref()
            .is_some_and(|popover| self.status_sheet_tab(popover.window).is_some())
    }

    /// The Grid & Snap popover of `window_id`'s status bar, above its Grid
    /// read-out. It shows the Document Options grid settings and sends the
    /// messages those rows send. Pushes the dismiss layer then the card.
    pub(in crate::app::view) fn grid_snap_popover_overlay(
        &self,
        window_id: iced::window::Id,
    ) -> Vec<Element<'_, Message>> {
        let ui = &self.ui_state;
        let Some(popover) = ui.grid_snap_popover.as_deref() else {
            return Vec::new();
        };
        if popover.window != window_id {
            return Vec::new();
        }
        let Some(tab) = self.status_sheet_tab(window_id) else {
            return Vec::new();
        };
        let settings = GridSnapSettings {
            grid_visible: ui.grid_visible,
            visible_grid_mm: ui.visible_grid_mm,
            snap_enabled: ui.snap_enabled,
            snap_grid_mm: ui.grid_size_mm,
            snap_hotspots: ui.snap_hotspots,
            unit: ui.unit,
        };
        let file_name = tab
            .path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned());
        let hotspot_key = ui
            .active_keymap
            .shortcut_label_for_id(crate::status_bar::HOTSPOTS_COMMAND);
        let tokens = &self.document_state.panel_ctx.tokens;
        let card = crate::status_bar::grid_popover_view(
            &settings,
            &popover.hover,
            file_name,
            hotspot_key,
            tokens,
            &ui.i18n,
        )
        .map(grid_popover_message);
        // The card claims the pointer over itself, so a press on its padding
        // does not reach the click-outside layer.
        let card = iced::widget::mouse_area(card).interaction(iced::mouse::Interaction::Idle);
        let status_h = crate::status_bar::status_bar_height(tokens);
        let anchored = iced::widget::container(card)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(Padding {
                top: 0.0,
                right: 0.0,
                bottom: status_h + crate::status_bar::POPUP_GAP,
                left: crate::status_bar::grid_readout_x(),
            })
            .align_x(iced::alignment::Horizontal::Left)
            .align_y(iced::alignment::Vertical::Bottom)
            .boxed();
        Self::dismiss_over(
            Message::Overlay(OverlayMsg::GridSnapPopoverDismissed),
            anchored,
        )
    }
}

/// The app message of a popover event: a setting goes the way the
/// Document Options row sends it, a hover to the popover's link state.
fn grid_popover_message(event: GridPopoverEvent) -> Message {
    match event {
        GridPopoverEvent::SettingChanged(msg) => {
            Message::Dock(crate::dock::DockMessage::Panel(msg))
        }
        GridPopoverEvent::PreviewHovered(msg) => {
            Message::Overlay(OverlayMsg::GridSnapPreviewHovered(msg))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::StatusBarRequest;
    use crate::panels::PanelMsg;
    use signex_widgets::property_illustration::{FieldId, IllustrationMsg};

    fn app_with_window() -> (Signex, iced::window::Id) {
        let (mut app, _boot) = Signex::new();
        let window = iced::window::Id::unique();
        app.ui_state.main_window_id = Some(window);
        (app, window)
    }

    fn right_click(window: iced::window::Id) -> Message {
        Signex::status_bar_message(window, None, StatusBarRequest::GridReadoutRightClicked)
    }

    /// A right-click on the Grid read-out opens the popover over the window
    /// that asked and closes the panel list; a second one closes it.
    #[test]
    fn a_right_click_on_the_grid_read_out_toggles_the_popover() {
        let (mut app, window) = app_with_window();
        app.ui_state.panel_list_open = true;
        let _ = app.update(right_click(window));
        assert_eq!(
            app.ui_state.grid_snap_popover.as_deref().map(|p| p.window),
            Some(window)
        );
        assert!(
            !app.ui_state.panel_list_open,
            "one status-bar popover at a time"
        );
        let _ = app.update(right_click(window));
        assert!(app.ui_state.grid_snap_popover.is_none());
    }

    /// Opening the panel list closes the popover; so does a press outside it.
    #[test]
    fn the_panel_list_and_a_press_outside_close_the_popover() {
        let (mut app, window) = app_with_window();
        let _ = app.update(right_click(window));
        let _ = app.update(Message::Overlay(OverlayMsg::TogglePanelList));
        assert!(app.ui_state.grid_snap_popover.is_none());
        assert!(app.ui_state.panel_list_open);
        let _ = app.update(Message::Overlay(OverlayMsg::TogglePanelList));
        let _ = app.update(right_click(window));
        let _ = app.update(Message::Overlay(OverlayMsg::GridSnapPopoverDismissed));
        assert!(app.ui_state.grid_snap_popover.is_none());
    }

    /// The popover's switches and Pitch picks change the settings the
    /// Document Options ▸ General rows show, through the same messages.
    #[test]
    fn the_popover_changes_the_document_options_settings() {
        let (mut app, _window) = app_with_window();
        let message = grid_popover_message(GridPopoverEvent::SettingChanged(PanelMsg::ToggleGrid));
        assert!(matches!(
            message,
            Message::Dock(crate::dock::DockMessage::Panel(PanelMsg::ToggleGrid))
        ));
        let visible = app.ui_state.grid_visible;
        let _ = app.update(message);
        assert_eq!(app.ui_state.grid_visible, !visible);

        let hotspots = app.ui_state.snap_hotspots;
        let _ = app.update(grid_popover_message(GridPopoverEvent::SettingChanged(
            PanelMsg::ToggleSnapHotspots,
        )));
        assert_eq!(app.ui_state.snap_hotspots, !hotspots);
        assert_eq!(app.document_state.panel_ctx.snap_hotspots, !hotspots);

        let _ = app.update(grid_popover_message(GridPopoverEvent::SettingChanged(
            PanelMsg::SetVisibleGridSize(5.08),
        )));
        assert!((app.ui_state.visible_grid_mm - 5.08).abs() < f32::EPSILON);
        assert!((app.document_state.panel_ctx.visible_grid_mm - 5.08).abs() < f32::EPSILON);
    }

    /// A hover on a Pitch row lights its callout through the popover state.
    #[test]
    fn a_pitch_row_hover_reaches_the_popover() {
        let (mut app, window) = app_with_window();
        let _ = app.update(right_click(window));
        let field = FieldId::new("grid-snap.visible-pitch");
        let _ = app.update(grid_popover_message(GridPopoverEvent::PreviewHovered(
            IllustrationMsg::FieldHovered(field.clone()),
        )));
        let popover = app.ui_state.grid_snap_popover.as_deref().expect("open");
        assert!(popover.hover.is_field_highlighted(&field));
    }

    /// The popover paints only over the window that opened it, and only
    /// while that window shows a sheet (a board's grids are not in it).
    #[test]
    fn the_popover_paints_only_over_its_own_sheet_window() {
        let (mut app, window) = app_with_window();
        let _ = app.update(right_click(window));
        assert!(
            app.grid_snap_popover_overlay(iced::window::Id::unique())
                .is_empty()
        );
        if app.status_sheet_tab(window).is_none() {
            assert!(app.grid_snap_popover_overlay(window).is_empty());
        } else {
            assert!(!app.grid_snap_popover_overlay(window).is_empty());
        }
    }

    fn next_document_tab(app: &mut Signex) {
        let command = crate::keymap::AppCommandId::new("next_document_tab").expect("id");
        let message = crate::app::command::bridge::core_to_message(
            &command,
            &crate::app::command::CommandArgs::none(),
        )
        .expect("the command resolves");
        let _ = app.update(message);
    }

    /// Ctrl+Tab from the sheet to a board closes the popover: the next Esc
    /// resets the board's tool instead of closing a popover that no longer
    /// paints, and going back to the sheet does not bring it back.
    #[test]
    fn switching_to_a_board_closes_the_popover_and_esc_resets_the_tool() {
        let (mut app, window) = app_with_window();
        for (name, kind) in [
            ("a.snxsch", crate::app::TabKind::Schematic),
            ("b.snxpcb", crate::app::TabKind::Pcb),
        ] {
            app.document_state.tabs.push(crate::app::TabInfo {
                title: name.into(),
                path: name.into(),
                cached_document: None,
                project_id: None,
                kind,
            });
        }
        app.document_state.active_tab = 0;
        let _ = app.update(right_click(window));
        assert!(app.grid_snap_popover_shown(), "open over the sheet");
        assert!(
            app.escape_overlay_message().is_some(),
            "Esc closes it first"
        );

        next_document_tab(&mut app);
        assert_eq!(app.document_state.active_tab, 1);
        assert!(app.ui_state.grid_snap_popover.is_none());
        app.interaction_state.current_tool = crate::app::Tool::Wire;
        let _ = app.update(Message::Input(crate::app::InputMsg::EscapePressed {
            window: Some(window),
        }));
        assert_eq!(app.interaction_state.current_tool, crate::app::Tool::Select);

        next_document_tab(&mut app);
        assert_eq!(app.document_state.active_tab, 0);
        assert!(app.ui_state.grid_snap_popover.is_none(), "it stays closed");
    }

    /// A popover whose window no longer shows a sheet claims no Esc, however
    /// it got there.
    #[test]
    fn a_popover_that_does_not_paint_claims_no_escape() {
        let (mut app, window) = app_with_window();
        let _ = app.update(right_click(window));
        assert!(app.ui_state.grid_snap_popover.is_some());
        assert!(!app.grid_snap_popover_shown(), "no sheet in the window");
        assert!(app.escape_overlay_message().is_none());
    }
}
