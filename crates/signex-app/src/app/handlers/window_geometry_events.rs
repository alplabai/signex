//! Window geometry bookkeeping: the settled reading of every window (the
//! source of "open next to the window the user acted from") and the
//! remembered main-window geometry.
//!
//! Move and resize events arrive in bursts. They only mark a window dirty;
//! one timer per burst re-reads the window once it has been quiet for
//! [`SETTLE`]. Nothing here runs per frame.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use iced::Task;

use crate::app::Message;
use crate::app::WindowMsg;
use crate::app::handlers::window_placement::{self, Anchor, SourceWindow};
use crate::app::state::Signex;
use crate::app::window_geometry::{self, MonitorInfo, SavedGeometry, WindowReading};

/// How long a window must stay still before it is read.
const SETTLE: Duration = Duration::from_millis(600);

/// Size the main window had before geometry was remembered (the size in
/// `main_window_settings`); the fallback when a saved record cannot be used.
const DEFAULT_MAIN_SIZE: (f32, f32) = (1400.0, 900.0);

/// What one settled read of a window returned.
#[derive(Debug, Clone)]
pub struct GeometryReadout {
    /// The window read.
    pub id: iced::window::Id,
    /// Its geometry; `None` when the platform would not say where it is.
    pub reading: Option<WindowReading>,
    /// Every monitor at the time of the read.
    pub monitors: Vec<MonitorInfo>,
}

/// Debounce state and the latest readings. Lives in `UiState`.
#[derive(Debug, Default)]
pub struct GeometryTracker {
    /// Windows with a settle timer running.
    pending: HashSet<iced::window::Id>,
    /// Windows that changed again while their timer ran.
    again: HashSet<iced::window::Id>,
    /// Last settled reading of each open window.
    readings: HashMap<iced::window::Id, WindowReading>,
    /// Monitors as of the last read.
    monitors: Vec<MonitorInfo>,
    /// The record last loaded or written for the main window.
    saved: Option<SavedGeometry>,
    /// A background write of the geometry is running; no second one starts
    /// until it reports back, so writes cannot land out of order.
    writing: bool,
    /// A record the main window was opened with, still to be checked
    /// against the real monitors.
    restore_check: Option<SavedGeometry>,
}

impl GeometryTracker {
    /// A tracker for a window opened from `restored`, which is checked on
    /// the first reading of the main window.
    #[must_use]
    pub fn with_restore(restored: Option<SavedGeometry>) -> Self {
        Self {
            saved: restored.clone(),
            restore_check: restored,
            ..Self::default()
        }
    }

    /// The last settled reading of `id` as a placement source.
    #[must_use]
    pub fn source(&self, id: iced::window::Id) -> Option<SourceWindow> {
        let r = self.readings.get(&id)?;
        Some(SourceWindow {
            monitor_index: r.monitor_index,
            position: r.position,
            size: r.size,
        })
    }
}

fn monitor_infos(list: &iced::window::MonitorList) -> Vec<MonitorInfo> {
    list.iter()
        .map(|m| {
            let size = m.size();
            MonitorInfo {
                name: m.name().map(str::to_owned),
                // Logical pixels: the fork's `size()` divides the physical size
                // by the monitor's scale factor (alplabai/iced#49).
                size: (size.width, size.height),
                primary: m.is_primary(),
            }
        })
        .collect()
}

/// Read position, size, maximized state and monitors of `id`.
fn read_window(id: iced::window::Id) -> Task<Message> {
    iced::window::position(id).then(move |position| {
        iced::window::size(id).then(move |size| {
            iced::window::is_maximized(id).then(move |maximized| {
                iced::window::list_monitors().map(move |list| {
                    let reading = position.map(|p| WindowReading {
                        monitor_index: p.monitor_index.map(|m| m.0),
                        position: (p.position.x, p.position.y),
                        size: (size.width, size.height),
                        maximized,
                    });
                    Message::Window(WindowMsg::WindowGeometryRead(Box::new(GeometryReadout {
                        id,
                        reading,
                        monitors: monitor_infos(&list),
                    })))
                })
            })
        })
    })
}

fn settle_timer(id: iced::window::Id) -> Task<Message> {
    Task::future(async move {
        tokio::time::sleep(SETTLE).await;
        Message::Window(WindowMsg::WindowGeometrySettled(id))
    })
}

/// Where a window with a stale record must move to be usable: `None` when the
/// pre-feature window (default size, where the OS put it) is mostly on a
/// monitor, else the centre of its monitor. The OS default position cannot be
/// asked for again once the window exists, so moving is only done to rescue a
/// window that would otherwise be off screen.
fn stale_record_move(
    reading: Option<WindowReading>,
    monitors: &[MonitorInfo],
) -> Option<window_placement::Placement> {
    let reading = reading?;
    let visible = reading
        .monitor_index
        .and_then(|i| monitors.get(i))
        .map(|m| window_geometry::visible_fraction(reading.position, DEFAULT_MAIN_SIZE, m.size));
    if visible.is_some_and(|v| v >= 0.5) {
        return None;
    }
    let source = SourceWindow {
        monitor_index: reading.monitor_index,
        position: reading.position,
        size: reading.size,
    };
    window_placement::place(&source, DEFAULT_MAIN_SIZE, monitors, Anchor::MonitorCenter)
}

/// The stale-record fallback: not maximized, the pre-feature size, and a move
/// only when [`stale_record_move`] asks for one.
fn default_main_placement(
    id: iced::window::Id,
    reading: Option<WindowReading>,
    monitors: &[MonitorInfo],
) -> Task<Message> {
    let mut tasks = vec![
        iced::window::maximize(id, false),
        iced::window::resize(
            id,
            iced::Size::new(DEFAULT_MAIN_SIZE.0, DEFAULT_MAIN_SIZE.1),
        ),
    ];
    if let Some(placement) = stale_record_move(reading, monitors) {
        tasks.push(iced::window::move_to(
            id,
            iced::window::PositionOnMonitor {
                monitor_index: Some(iced::window::MonitorIndex(placement.monitor_index)),
                position: iced::Point::new(placement.position.0, placement.position.1),
            },
        ));
    }
    Task::batch(tasks)
}

impl Signex {
    /// A window moved or resized: start its settle timer unless one runs.
    pub(crate) fn note_window_geometry_changed(&mut self, id: iced::window::Id) -> Task<Message> {
        let tracker = &mut self.ui_state.window_geometry;
        if tracker.pending.insert(id) {
            settle_timer(id)
        } else {
            tracker.again.insert(id);
            Task::none()
        }
    }

    /// The settle timer of `id` fired: re-arm it if the window changed
    /// meanwhile, otherwise read the window.
    pub(crate) fn window_geometry_settled(&mut self, id: iced::window::Id) -> Task<Message> {
        let tracker = &mut self.ui_state.window_geometry;
        if tracker.again.remove(&id) {
            return settle_timer(id);
        }
        tracker.pending.remove(&id);
        read_window(id)
    }

    /// Store a reading; for the main window also correct a restored
    /// placement and persist the geometry.
    pub(crate) fn window_geometry_read(&mut self, readout: GeometryReadout) -> Task<Message> {
        let GeometryReadout {
            id,
            reading,
            monitors,
        } = readout;
        let is_main = self.ui_state.main_window_id == Some(id);
        // A window that closed while its read was in flight must not come back.
        if !is_main && !self.ui_state.windows.contains_key(&id) {
            return Task::none();
        }
        let tracker = &mut self.ui_state.window_geometry;
        tracker.monitors = monitors;
        // Validate the restored record even when the platform reports no
        // position: the check needs only the monitors.
        let restore = if is_main {
            tracker.restore_check.take()
        } else {
            None
        };
        if let Some(reading) = reading {
            tracker.readings.insert(id, reading);
        }
        if let Some(saved) = restore {
            return self.reconcile_restored_main(id, &saved, reading);
        }
        match reading {
            Some(reading) if is_main => self.persist_main_geometry(reading),
            _ => Task::none(),
        }
    }

    /// The main window opened from a saved record before the monitor list was
    /// known. Check the record against the monitors now: put the window where
    /// the plan says, or back to the default placement when the record cannot
    /// be used (monitor gone, mostly off screen).
    fn reconcile_restored_main(
        &mut self,
        id: iced::window::Id,
        saved: &SavedGeometry,
        reading: Option<WindowReading>,
    ) -> Task<Message> {
        let monitors = self.ui_state.window_geometry.monitors.clone();
        if monitors.is_empty() {
            // The monitor list failed: nothing can be validated, so the
            // record is left as it is and the window as it opened.
            return Task::none();
        }
        match window_geometry::plan_restore(saved, &monitors) {
            Some(plan) => {
                if plan.maximized {
                    return Task::none();
                }
                let already = reading.is_some_and(|reading| {
                    reading.monitor_index == Some(plan.monitor_index)
                        && reading.position == plan.position
                        && reading.size == plan.size
                });
                if already {
                    return Task::none();
                }
                Task::batch([
                    iced::window::resize(id, iced::Size::new(plan.size.0, plan.size.1)),
                    iced::window::move_to(
                        id,
                        iced::window::PositionOnMonitor {
                            monitor_index: Some(iced::window::MonitorIndex(plan.monitor_index)),
                            position: iced::Point::new(plan.position.0, plan.position.1),
                        },
                    ),
                ])
            }
            None => {
                // The record is stale: forget it and show the window as it
                // was before geometry was remembered.
                self.ui_state.window_geometry.saved = None;
                default_main_placement(id, reading, &monitors)
            }
        }
    }

    /// The record to write for `reading`, when remembering is on and it
    /// differs from what is stored.
    fn geometry_record_to_write(&self, reading: &WindowReading) -> Option<SavedGeometry> {
        if !self.ui_state.remember_window_geometry {
            return None;
        }
        let tracker = &self.ui_state.window_geometry;
        let record =
            SavedGeometry::from_reading(reading, &tracker.monitors, tracker.saved.as_ref())?;
        (tracker.saved.as_ref() != Some(&record)).then_some(record)
    }

    /// Write the main window's geometry in the background when it differs
    /// from what is stored. A small file, written once per settled change,
    /// never per event; `saved` is updated when the write reports back.
    fn persist_main_geometry(&mut self, reading: WindowReading) -> Task<Message> {
        if self.ui_state.window_geometry.writing {
            // Picked up again when the running write reports back.
            return Task::none();
        }
        let Some(record) = self.geometry_record_to_write(&reading) else {
            return Task::none();
        };
        self.ui_state.window_geometry.writing = true;
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    window_geometry::save(&record)
                        .map(|()| record)
                        .map_err(|err| err.to_string())
                })
                .await
                .unwrap_or_else(|err| Err(err.to_string()))
            },
            |result| Message::Window(WindowMsg::WindowGeometrySaved(result)),
        )
    }

    /// A background geometry write finished. Only a successful write becomes
    /// the stored record; the latest reading is then checked again, because
    /// the window may have changed while the write ran.
    pub(crate) fn window_geometry_saved(
        &mut self,
        result: Result<SavedGeometry, String>,
    ) -> Task<Message> {
        let tracker = &mut self.ui_state.window_geometry;
        tracker.writing = false;
        match result {
            Ok(record) => tracker.saved = Some(record),
            Err(err) => {
                tracing::warn!(target: "signex::window", "window geometry not saved: {err}");
                return Task::none();
            }
        }
        let latest = self
            .ui_state
            .main_window_id
            .and_then(|id| self.ui_state.window_geometry.readings.get(&id).copied());
        match latest {
            Some(reading) => self.persist_main_geometry(reading),
            None => Task::none(),
        }
    }

    /// The main window is asked to close: store the last settled geometry
    /// now, because the window is gone before another read or a background
    /// write could finish. This one write is synchronous on purpose.
    ///
    /// It can overlap a background write that is still in flight (the
    /// `writing` flag of the tracker). Both writes use unique temporary
    /// names, so the file cannot be corrupted, but the last rename wins: if
    /// the window closes within milliseconds of a settle, the older
    /// background record can land after this one. The only effect is a
    /// slightly stale restored position.
    pub(crate) fn persist_main_geometry_on_close(&mut self) {
        let Some(id) = self.ui_state.main_window_id else {
            return;
        };
        let Some(reading) = self.ui_state.window_geometry.readings.get(&id).copied() else {
            return;
        };
        let Some(record) = self.geometry_record_to_write(&reading) else {
            return;
        };
        match window_geometry::save(&record) {
            Ok(()) => self.ui_state.window_geometry.saved = Some(record),
            Err(err) => {
                tracing::warn!(target: "signex::window", "window geometry not saved: {err}");
            }
        }
    }

    /// Forget a closed window.
    pub(crate) fn forget_window_geometry(&mut self, id: iced::window::Id) {
        let tracker = &mut self.ui_state.window_geometry;
        tracker.pending.remove(&id);
        tracker.again.remove(&id);
        tracker.readings.remove(&id);
    }

    /// Where a new window of `size` opens: on the monitor of the window the
    /// user acted from (the focused one, else the main window), centred
    /// there. Uses `fallback` while that window has not been read yet.
    pub(crate) fn new_window_position(
        &self,
        size: iced::Size,
        anchor: Anchor,
        fallback: iced::window::Position,
    ) -> iced::window::Position {
        let tracker = &self.ui_state.window_geometry;
        let source = self
            .ui_state
            .focused_window
            .and_then(|id| tracker.source(id))
            .or_else(|| {
                self.ui_state
                    .main_window_id
                    .and_then(|id| tracker.source(id))
            });
        window_placement::position_for(
            source.as_ref(),
            (size.width, size.height),
            &tracker.monitors,
            anchor,
            fallback,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pins the fork fix for alplabai/iced#49: `MonitorData::size()` used to
    /// multiply by the scale factor, which made a 4K monitor at 200% look
    /// 7680x4320 wide. This fails against fork revisions before that fix.
    #[test]
    fn monitor_infos_report_logical_sizes() {
        use iced::Size;
        use iced::window::{MonitorData, MonitorList};

        let mut list = MonitorList::empty();
        list.add_monitor(
            true,
            MonitorData::new(Size::new(1920, 1080), 1.0, Some("one".into()), None),
        );
        list.add_monitor(
            false,
            MonitorData::new(Size::new(3840, 2160), 2.0, Some("two".into()), None),
        );
        list.add_monitor(
            false,
            MonitorData::new(Size::new(2560, 1440), 1.25, None, None),
        );

        let sizes: Vec<_> = monitor_infos(&list).into_iter().map(|m| m.size).collect();

        assert_eq!(
            sizes,
            [(1920.0, 1080.0), (1920.0, 1080.0), (2048.0, 1152.0)]
        );
    }

    #[test]
    fn tracker_exposes_a_reading_as_a_source() {
        let id = iced::window::Id::unique();
        let mut tracker = GeometryTracker::default();
        assert_eq!(tracker.source(id), None);
        tracker.readings.insert(
            id,
            WindowReading {
                monitor_index: Some(1),
                position: (5.0, 6.0),
                size: (700.0, 500.0),
                maximized: false,
            },
        );
        let source = tracker.source(id).unwrap();
        assert_eq!(source.monitor_index, Some(1));
        assert_eq!(source.position, (5.0, 6.0));
        assert_eq!(source.size, (700.0, 500.0));
    }

    fn mon(w: f32, h: f32) -> MonitorInfo {
        MonitorInfo {
            name: None,
            size: (w, h),
            primary: true,
        }
    }

    fn reading_at(x: f32, y: f32) -> WindowReading {
        WindowReading {
            monitor_index: Some(0),
            position: (x, y),
            size: (800.0, 600.0),
            maximized: false,
        }
    }

    #[test]
    fn stale_record_leaves_a_visible_window_where_the_os_put_it() {
        let monitors = [mon(1920.0, 1080.0)];
        assert!(stale_record_move(Some(reading_at(100.0, 80.0)), &monitors).is_none());
        // No position known: nothing to move.
        assert!(stale_record_move(None, &monitors).is_none());
    }

    #[test]
    fn stale_record_rescues_a_window_that_is_off_screen() {
        let monitors = [mon(1920.0, 1080.0)];
        let placement = stale_record_move(Some(reading_at(1800.0, 900.0)), &monitors).unwrap();
        assert_eq!(placement.monitor_index, 0);
        assert_eq!(placement.position, (260.0, 90.0));
    }

    #[test]
    fn restore_check_starts_set_only_with_a_record() {
        assert!(GeometryTracker::with_restore(None).restore_check.is_none());
        let record = SavedGeometry {
            version: 1,
            monitor_name: None,
            monitor_index: Some(0),
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 600.0,
            maximized: false,
        };
        let tracker = GeometryTracker::with_restore(Some(record.clone()));
        assert_eq!(tracker.restore_check, Some(record.clone()));
        assert_eq!(tracker.saved, Some(record));
    }
}
