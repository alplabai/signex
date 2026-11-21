//! The board counters of the status bar and their popovers (#2460): a
//! counter's click opens or closes its popover, and each popover action runs
//! against the board of the window it opened over, through `run_in_window`
//! as that window's own canvas events do.

use iced::Task;

use super::super::*;
use crate::status_bar::{BoardCounter, BoardPopoverEvent, BoardStatusPopover};

/// How often the DRC counter's "N min ago" is read against the clock.
pub(crate) const BOARD_CLOCK_TICK: std::time::Duration = std::time::Duration::from_secs(30);

impl Signex {
    /// A click on `counter` in `window`'s bar: its popover opens there, or
    /// closes when it is the one open. One status-bar popup at a time.
    pub(super) fn board_counter_clicked(
        &mut self,
        window: iced::window::Id,
        counter: BoardCounter,
    ) -> Task<Message> {
        let kind = counter.popover();
        let status = &mut self.ui_state.board_status;
        let open_here = status
            .popover
            .as_ref()
            .is_some_and(|p| p.window == window && p.kind == kind);
        status.popover = (!open_here).then(|| BoardStatusPopover::new(window, kind));
        if status.popover.is_some() {
            self.ui_state.grid_snap_popover = None;
            self.ui_state.panel_list_open = false;
        }
        Task::none()
    }

    fn close_board_popover(&mut self) {
        self.ui_state.board_status.popover = None;
    }

    /// An event of the open board popover, acted on in its window.
    pub(super) fn board_popover_event(&mut self, event: BoardPopoverEvent) -> Task<Message> {
        use BoardPopoverEvent as E;
        let Some(window) = self
            .ui_state
            .board_status
            .popover
            .as_ref()
            .map(|p| p.window)
        else {
            return Task::none();
        };
        match event {
            E::Dismissed => {
                self.close_board_popover();
                Task::none()
            }
            E::LayerPicked(layer) => {
                let task = self.dispatch_pcb_message(PcbMsg::ViewInWindow {
                    window,
                    event: PcbViewMsg::ActiveLayerPicked(layer),
                });
                // The dropdown closes on a pick; the Active layer popover stays.
                if !self.shows_pcb_layer_strip() {
                    self.close_board_popover();
                }
                task
            }
            E::LayersPanelOpened => {
                self.close_board_popover();
                self.update(Message::Overlay(OverlayMsg::OpenPanel(
                    crate::panels::PanelKind::Layers,
                )))
            }
            E::GridPresetPicked(mm) => self.set_snap_grid(mm),
            E::CustomGridEdited(entry) => {
                if let Some(popover) = self.ui_state.board_status.popover.as_mut() {
                    popover.custom_grid = entry;
                    popover.custom_refused = false;
                }
                Task::none()
            }
            E::CustomGridSubmitted => self.custom_grid_submitted(),
            E::SnapToggled => {
                self.update(Message::Ui(UiMsg::StatusBar(StatusBarRequest::ToggleSnap)))
            }
            E::GridSettingsOpened => {
                self.close_board_popover();
                self.open_board_grid_settings()
            }
            E::NetZoomed(net) => self.run_in_window(window, true, |app| {
                app.frame_unrouted(Some(net));
                Task::none()
            }),
            E::RouteNextRequested => {
                self.close_board_popover();
                self.run_in_window(window, true, |app| {
                    app.frame_unrouted(None);
                    app.dispatch_pcb_message(PcbMsg::PlaceToolPicked(
                        crate::app::pcb_tools::BoardPlaceTool::InteractiveRouting,
                    ))
                })
            }
            E::ViolationZoomed(key) => self.run_in_window(window, true, |app| {
                app.zoom_to_violation(&key);
                Task::none()
            }),
            E::DrcRunRequested => {
                self.run_in_window(window, true, |app| app.handle_pcb_drc_requested())
            }
            E::MessagesOpened => {
                self.close_board_popover();
                self.update(Message::Overlay(OverlayMsg::OpenPanel(
                    crate::panels::PanelKind::Messages,
                )))
            }
            E::SelectedItemZoomed(item) => self.run_in_window(window, true, |app| {
                app.frame_board_item(item);
                Task::none()
            }),
            E::PropertiesOpened => {
                self.close_board_popover();
                self.handle_menu_message(MenuMessage::OpenPropertiesPanel)
            }
            E::SchematicSelectRequested => {
                self.close_board_popover();
                self.run_in_window(window, true, |app| {
                    app.update(Message::CrossProbe(CrossProbeMsg::ProbeRequested))
                })
            }
            E::SelectionCleared => {
                self.close_board_popover();
                self.run_in_window(window, true, |app| {
                    if app.interaction_state.pcb_selection.clear() {
                        app.refresh_pcb_highlight();
                    }
                    Task::none()
                })
            }
        }
    }

    /// Enter in the Grid popover's Custom… field: a pitch above 0 mm becomes
    /// the grid; anything else is refused and changes nothing.
    fn custom_grid_submitted(&mut self) -> Task<Message> {
        let Some(popover) = self.ui_state.board_status.popover.as_mut() else {
            return Task::none();
        };
        match crate::status_bar::custom_pitch(&popover.custom_grid) {
            Some(mm) => {
                popover.custom_refused = false;
                self.set_snap_grid(mm)
            }
            None => {
                popover.custom_refused = true;
                Task::none()
            }
        }
    }

    /// Grids…: the board's grid settings, the Grid Manager on the General
    /// tab of the board's Properties.
    fn open_board_grid_settings(&mut self) -> Task<Message> {
        let open = self.update(Message::Overlay(OverlayMsg::OpenPanel(
            crate::panels::PanelKind::Properties,
        )));
        let general = self.update(Message::Dock(crate::dock::DockMessage::Panel(
            crate::panels::PanelMsg::PcbBoard(crate::panels::PcbBoardMsg::TabPicked(
                crate::panels::BoardTab::General,
            )),
        )));
        Task::batch([open, general])
    }

    /// Frame unrouted connections of the board in view: those of `net`, or
    /// with `None` the first one the Unrouted popover lists.
    fn frame_unrouted(&mut self, net: Option<u32>) {
        let Some(path) = self.active_tab_path() else {
            return;
        };
        let revision = self.board_revision(&path);
        let Some(counts) = self
            .interaction_state
            .pcb_ratsnest
            .counts_for(&path, revision)
        else {
            return;
        };
        let lines: Vec<pcb_geom::RatLine> = match net {
            Some(net) => counts
                .per_net
                .get(&net)
                .map(|n| n.lines.clone())
                .unwrap_or_default(),
            None => counts
                .nets_by_unrouted()
                .first()
                .and_then(|net| counts.per_net.get(net))
                .and_then(|n| n.lines.first().copied())
                .into_iter()
                .collect(),
        };
        let points = lines.iter().flat_map(|l| [l.from, l.to]);
        self.frame_board_points(points);
    }

    /// Frame one object of the board in view.
    fn frame_board_item(&mut self, item: pcb_geom::ItemRef) {
        let Some(board) = self.active_pcb() else {
            return;
        };
        let index = pcb_geom::BoardIndex::build_selectable(board);
        let corners: Vec<pcb_geom::PointNm> = index
            .items()
            .filter(|indexed| indexed.item == item)
            .flat_map(|indexed| {
                let r = indexed.shape.aabb();
                [r.min, r.max]
            })
            .collect();
        self.frame_board_points(corners.into_iter());
    }

    /// Fit the board view of the acting window to the box around `points`.
    fn frame_board_points(&mut self, points: impl Iterator<Item = pcb_geom::PointNm>) {
        let mut bounds: Option<[f64; 4]> = None;
        for p in points {
            let (x, y) = (pcb_geom::nm_to_mm(p.x), pcb_geom::nm_to_mm(p.y));
            bounds = Some(match bounds {
                None => [x, y, x, y],
                Some([x0, y0, x1, y1]) => [x0.min(x), y0.min(y), x1.max(x), y1.max(y)],
            });
        }
        let Some(bounds) = bounds else {
            return;
        };
        let canvas = &mut self.interaction_state.pcb_canvas;
        canvas
            .pending_fit
            .set(Some(crate::app::handlers::canvas::fit_view::fit_rect(
                bounds,
            )));
        canvas.clear_bg_cache();
        canvas.clear_content_cache();
    }

    /// A violation's Zoom: select its objects and centre the view on it, as
    /// a row of the DRC panel does. A refusal is said in Messages.
    fn zoom_to_violation(&mut self, key: &str) {
        let Some(path) = self.active_tab_path() else {
            return;
        };
        if let Err(why) = self.show_drc_violation(&path, key) {
            let line = signex_i18n::t!(
                self.ui_state.i18n,
                signex_i18n::ids::DRC_PANEL_SHOW_REFUSED,
                "why" => why
            );
            crate::diagnostics::log_warning(line.into_string());
        }
    }

    /// The board at `path`'s revision now; 0 before it has one.
    pub(crate) fn board_revision(&self, path: &std::path::Path) -> u64 {
        self.document_state
            .pcb_revisions
            .get(path)
            .copied()
            .unwrap_or(0)
    }

    /// The 30 s tick behind the DRC counter's age: only while a board whose
    /// last run has landed is shown in a window, so it costs nothing
    /// otherwise.
    pub(crate) fn board_clock_subscription(&self) -> iced::Subscription<Message> {
        let Some(done) = self.interaction_state.pcb_drc.last() else {
            return iced::Subscription::none();
        };
        let in_main = self.active_pcb().is_some()
            && self.active_tab_path().as_deref() == Some(done.path.as_path());
        let undocked = self.ui_state.windows.values().any(|kind| {
            matches!(kind, crate::app::state::WindowKind::UndockedTab { path, .. } if *path == done.path)
        });
        if !(in_main || undocked) {
            return iced::Subscription::none();
        }
        iced::time::every(BOARD_CLOCK_TICK)
            .map(|_| Message::Ui(UiMsg::StatusBar(StatusBarRequest::BoardClockTicked)))
    }
}

#[cfg(test)]
#[path = "board_status_tests.rs"]
mod tests;
