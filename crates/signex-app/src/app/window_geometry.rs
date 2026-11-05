//! Remembered main-window geometry: the saved record, the pure rules that
//! decide whether it may be restored, and its on-disk form.
//!
//! Everything is expressed in the coordinates iced reports for a window:
//! logical pixels, with the position relative to the top-left of the monitor
//! the window is on (`window::position`). That keeps the rules independent of
//! how the desktop lays its monitors out, which iced does not expose.
//!
//! The record is only read at startup and only when the
//! `remember_window_geometry` preference is on; it is written after the
//! window has settled (see `handlers::window_geometry_events`).

use std::path::{Path, PathBuf};

/// File name under the config root.
const FILE_NAME: &str = "window_geometry.json";

/// Current on-disk format. A file with another version is ignored.
const VERSION: u32 = 1;

/// A restored window never gets smaller than this, so a corrupt record cannot
/// produce an unusable window.
const MIN_SIZE: (f32, f32) = (640.0, 400.0);

/// Share of the saved rectangle that must lie on the monitor for the record
/// to be used.
const MIN_VISIBLE_FRACTION: f32 = 0.5;

/// One monitor as the geometry rules see it.
#[derive(Debug, Clone, PartialEq)]
pub struct MonitorInfo {
    /// The OS name of the monitor, when it reports one.
    pub name: Option<String>,
    /// Logical size in pixels.
    pub size: (f32, f32),
    /// Whether the OS marks it as the primary monitor.
    pub primary: bool,
}

/// The main window's geometry as it is stored.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SavedGeometry {
    /// Format version, see [`VERSION`].
    pub version: u32,
    /// Name of the monitor the window was on.
    pub monitor_name: Option<String>,
    /// Index of that monitor in the monitor list when it was saved.
    pub monitor_index: Option<usize>,
    /// Position relative to the monitor, logical pixels.
    pub x: f32,
    /// Position relative to the monitor, logical pixels.
    pub y: f32,
    /// Logical width of the normal (not maximized) window.
    pub width: f32,
    /// Logical height of the normal (not maximized) window.
    pub height: f32,
    /// Whether the window was maximized.
    pub maximized: bool,
}

/// A reading of the live window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowReading {
    /// Index of the monitor the window is on.
    pub monitor_index: Option<usize>,
    /// Position relative to that monitor.
    pub position: (f32, f32),
    /// Logical size.
    pub size: (f32, f32),
    /// Whether the window is maximized.
    pub maximized: bool,
}

/// Where and how large the window should be restored.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RestorePlan {
    /// Index of the monitor to open on.
    pub monitor_index: usize,
    /// Position relative to that monitor.
    pub position: (f32, f32),
    /// Logical size.
    pub size: (f32, f32),
    /// Whether to start maximized.
    pub maximized: bool,
}

impl SavedGeometry {
    /// The record for `reading`. While the window is maximized its rectangle
    /// is the whole monitor, which is not what to come back to once the user
    /// un-maximizes it, so the rectangle of `previous` is kept. With no
    /// `previous` a maximized reading yields `None`: nothing is stored until
    /// the window has a normal rectangle.
    #[must_use]
    pub fn from_reading(
        reading: &WindowReading,
        monitors: &[MonitorInfo],
        previous: Option<&SavedGeometry>,
    ) -> Option<Self> {
        let index = reading.monitor_index.filter(|i| *i < monitors.len());
        let name = index.and_then(|i| monitors[i].name.clone());
        if reading.maximized {
            // Without a previous record there is no normal rectangle to come
            // back to, and the monitor-sized one is wrong: store nothing.
            let prev = previous?;
            return Some(Self {
                maximized: true,
                monitor_name: name.or_else(|| prev.monitor_name.clone()),
                monitor_index: index.or(prev.monitor_index),
                ..prev.clone()
            });
        }
        let (x, y) = reading.position;
        let (width, height) = reading.size;
        if ![x, y, width, height].iter().all(|v| v.is_finite()) || width <= 0.0 || height <= 0.0 {
            return None;
        }
        Some(Self {
            version: VERSION,
            monitor_name: name,
            monitor_index: index,
            x,
            y,
            width,
            height,
            maximized: reading.maximized,
        })
    }
}

/// The monitor `saved` was on, looked up by name first and by index second.
///
/// A record that carries a name only matches a monitor of that name: when it
/// is gone the monitor is gone, even if another monitor now sits at the old
/// index. Several monitors of one model share a name, so among equal names the
/// saved index wins. A record without a name falls back to the index alone.
#[must_use]
pub fn resolve_monitor(saved: &SavedGeometry, monitors: &[MonitorInfo]) -> Option<usize> {
    match saved.monitor_name.as_deref() {
        Some(name) => {
            let same_name = |i: &usize| monitors[*i].name.as_deref() == Some(name);
            saved
                .monitor_index
                .filter(|i| *i < monitors.len() && same_name(i))
                .or_else(|| (0..monitors.len()).find(same_name))
        }
        None => saved.monitor_index.filter(|i| *i < monitors.len()),
    }
}

/// The fraction of the rectangle at `(x, y)` of `size` that lies on a
/// monitor of `monitor` size at the origin. Zero for an empty rectangle.
#[must_use]
pub fn visible_fraction(position: (f32, f32), size: (f32, f32), monitor: (f32, f32)) -> f32 {
    let (w, h) = size;
    if !(w > 0.0 && h > 0.0) || !position.0.is_finite() || !position.1.is_finite() {
        return 0.0;
    }
    let overlap_w = (position.0 + w).min(monitor.0) - position.0.max(0.0);
    let overlap_h = (position.1 + h).min(monitor.1) - position.1.max(0.0);
    if overlap_w <= 0.0 || overlap_h <= 0.0 {
        return 0.0;
    }
    (overlap_w * overlap_h) / (w * h)
}

/// Decide whether `saved` can be restored on the monitors that exist now, and
/// where. `None` means "use the default placement": the record is unusable,
/// its monitor is gone, or less than half of its rectangle would be visible.
///
/// A rectangle larger than the monitor (the resolution shrank) is cut down to
/// it, and the position is moved so the whole window is on the monitor.
#[must_use]
pub fn plan_restore(saved: &SavedGeometry, monitors: &[MonitorInfo]) -> Option<RestorePlan> {
    if saved.version != VERSION {
        return None;
    }
    let all_finite = [saved.x, saved.y, saved.width, saved.height]
        .iter()
        .all(|v| v.is_finite());
    if !all_finite || saved.width <= 0.0 || saved.height <= 0.0 {
        return None;
    }
    let index = resolve_monitor(saved, monitors)?;
    let monitor = monitors[index].size;
    if !(monitor.0 > 0.0 && monitor.1 > 0.0) {
        return None;
    }
    if visible_fraction((saved.x, saved.y), (saved.width, saved.height), monitor)
        < MIN_VISIBLE_FRACTION
    {
        return None;
    }
    let width = saved.width.max(MIN_SIZE.0).min(monitor.0);
    let height = saved.height.max(MIN_SIZE.1).min(monitor.1);
    let x = saved.x.clamp(0.0, (monitor.0 - width).max(0.0));
    let y = saved.y.clamp(0.0, (monitor.1 - height).max(0.0));
    Some(RestorePlan {
        monitor_index: index,
        position: (x, y),
        size: (width, height),
        maximized: saved.maximized,
    })
}

/// The path of the geometry file, `None` when there is no config directory.
#[must_use]
pub fn file_path() -> Option<PathBuf> {
    crate::config_root::config_root().map(|root| root.join(FILE_NAME))
}

/// Read the record at `path`. A missing, unreadable or malformed file is
/// "nothing saved".
#[must_use]
pub fn load_from(path: &Path) -> Option<SavedGeometry> {
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Read the record from the config directory.
#[must_use]
pub fn load() -> Option<SavedGeometry> {
    load_from(&file_path()?)
}

/// Write `saved` to `path` atomically.
pub fn save_to(path: &Path, saved: &SavedGeometry) -> std::io::Result<()> {
    let bytes = serde_json::to_vec_pretty(saved).map_err(std::io::Error::other)?;
    signex_types::atomic_io::atomic_write(path, &bytes)
}

/// Write `saved` to the config directory.
pub fn save(saved: &SavedGeometry) -> std::io::Result<()> {
    match file_path() {
        Some(path) => save_to(&path, saved),
        None => Ok(()),
    }
}

/// Make `settings` open the window as `saved` describes. The monitor is the
/// one saved at index; the app checks the result against the real monitor
/// list once the window exists (see `handlers::window_geometry_events`),
/// because the list is not available before the event loop runs.
pub fn apply_to_settings(settings: &mut iced::window::Settings, saved: &SavedGeometry) {
    if !saved.width.is_finite() || !saved.height.is_finite() || saved.width <= 0.0 {
        return;
    }
    if saved.height <= 0.0 || !saved.x.is_finite() || !saved.y.is_finite() {
        return;
    }
    settings.size = iced::Size::new(saved.width.max(MIN_SIZE.0), saved.height.max(MIN_SIZE.1));
    settings.maximized = saved.maximized;
    settings.position = iced::window::Position::Specific(iced::window::PositionOnMonitor {
        monitor_index: saved.monitor_index.map(iced::window::MonitorIndex),
        position: iced::Point::new(saved.x, saved.y),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mon(name: &str, w: f32, h: f32) -> MonitorInfo {
        MonitorInfo {
            name: Some(name.to_owned()),
            size: (w, h),
            primary: false,
        }
    }

    fn saved(name: Option<&str>, index: Option<usize>, x: f32, y: f32) -> SavedGeometry {
        SavedGeometry {
            version: VERSION,
            monitor_name: name.map(str::to_owned),
            monitor_index: index,
            x,
            y,
            width: 1400.0,
            height: 900.0,
            maximized: false,
        }
    }

    #[test]
    fn restores_on_the_same_monitor() {
        let monitors = [mon("A", 2560.0, 1440.0)];
        let plan = plan_restore(&saved(Some("A"), Some(0), 100.0, 50.0), &monitors).unwrap();
        assert_eq!(plan.monitor_index, 0);
        assert_eq!(plan.position, (100.0, 50.0));
        assert_eq!(plan.size, (1400.0, 900.0));
    }

    #[test]
    fn monitor_found_by_name_when_the_order_changed() {
        let monitors = [mon("B", 1920.0, 1080.0), mon("A", 2560.0, 1440.0)];
        let plan = plan_restore(&saved(Some("A"), Some(0), 10.0, 10.0), &monitors).unwrap();
        assert_eq!(plan.monitor_index, 1);
    }

    #[test]
    fn equal_names_prefer_the_saved_index() {
        let monitors = [mon("Dell", 1920.0, 1080.0), mon("Dell", 1920.0, 1080.0)];
        assert_eq!(
            resolve_monitor(&saved(Some("Dell"), Some(1), 0.0, 0.0), &monitors),
            Some(1)
        );
        assert_eq!(
            resolve_monitor(&saved(Some("Dell"), Some(7), 0.0, 0.0), &monitors),
            Some(0)
        );
    }

    #[test]
    fn disconnected_monitor_gives_no_plan() {
        let monitors = [mon("B", 1920.0, 1080.0)];
        assert_eq!(
            plan_restore(&saved(Some("A"), Some(0), 10.0, 10.0), &monitors),
            None
        );
    }

    #[test]
    fn unnamed_record_falls_back_to_the_index() {
        let monitors = [mon("B", 1920.0, 1080.0), mon("C", 1920.0, 1080.0)];
        assert_eq!(
            resolve_monitor(&saved(None, Some(1), 0.0, 0.0), &monitors),
            Some(1)
        );
        assert_eq!(
            resolve_monitor(&saved(None, Some(2), 0.0, 0.0), &monitors),
            None
        );
        assert_eq!(
            resolve_monitor(&saved(None, None, 0.0, 0.0), &monitors),
            None
        );
    }

    #[test]
    fn shrunk_resolution_cuts_the_window_to_the_monitor() {
        let monitors = [mon("A", 1280.0, 720.0)];
        let plan = plan_restore(&saved(Some("A"), Some(0), 100.0, 50.0), &monitors).unwrap();
        assert_eq!(plan.size, (1280.0, 720.0));
        assert_eq!(plan.position, (0.0, 0.0));
    }

    #[test]
    fn rectangle_mostly_off_the_monitor_gives_no_plan() {
        let monitors = [mon("A", 1920.0, 1080.0)];
        // Only 520 of 1400 columns on the monitor: 37 percent.
        assert_eq!(
            plan_restore(&saved(Some("A"), Some(0), 1400.0, 0.0), &monitors),
            None
        );
        // Entirely below the monitor.
        assert_eq!(
            plan_restore(&saved(Some("A"), Some(0), 0.0, 5000.0), &monitors),
            None
        );
    }

    #[test]
    fn exactly_half_visible_is_enough_and_is_pulled_back() {
        let monitors = [mon("A", 1920.0, 1080.0)];
        let plan = plan_restore(&saved(Some("A"), Some(0), 1220.0, 0.0), &monitors).unwrap();
        assert_eq!(plan.position, (520.0, 0.0));
    }

    #[test]
    fn negative_origin_is_clamped_onto_the_monitor() {
        let monitors = [mon("A", 1920.0, 1080.0)];
        let plan = plan_restore(&saved(Some("A"), Some(0), -200.0, -30.0), &monitors).unwrap();
        assert_eq!(plan.position, (0.0, 0.0));
    }

    #[test]
    fn zero_size_and_non_finite_records_give_no_plan() {
        let monitors = [mon("A", 1920.0, 1080.0)];
        let mut s = saved(Some("A"), Some(0), 0.0, 0.0);
        s.width = 0.0;
        assert_eq!(plan_restore(&s, &monitors), None);
        let mut s = saved(Some("A"), Some(0), 0.0, 0.0);
        s.height = -5.0;
        assert_eq!(plan_restore(&s, &monitors), None);
        let s = saved(Some("A"), Some(0), f32::NAN, 0.0);
        assert_eq!(plan_restore(&s, &monitors), None);
        let zero_monitor = [mon("A", 0.0, 0.0)];
        assert_eq!(
            plan_restore(&saved(Some("A"), Some(0), 0.0, 0.0), &zero_monitor),
            None
        );
        assert_eq!(
            plan_restore(&saved(Some("A"), Some(0), 0.0, 0.0), &[]),
            None
        );
    }

    #[test]
    fn unknown_version_gives_no_plan() {
        let monitors = [mon("A", 1920.0, 1080.0)];
        let mut s = saved(Some("A"), Some(0), 0.0, 0.0);
        s.version = 99;
        assert_eq!(plan_restore(&s, &monitors), None);
    }

    #[test]
    fn tiny_saved_size_is_raised_to_the_minimum() {
        let monitors = [mon("A", 1920.0, 1080.0)];
        let mut s = saved(Some("A"), Some(0), 0.0, 0.0);
        s.width = 100.0;
        s.height = 50.0;
        let plan = plan_restore(&s, &monitors).unwrap();
        assert_eq!(plan.size, MIN_SIZE);
    }

    #[test]
    fn maximized_reading_keeps_the_previous_rectangle() {
        let monitors = [mon("A", 1920.0, 1080.0)];
        let prev = saved(Some("A"), Some(0), 100.0, 80.0);
        let reading = WindowReading {
            monitor_index: Some(0),
            position: (0.0, 0.0),
            size: (1920.0, 1080.0),
            maximized: true,
        };
        let record = SavedGeometry::from_reading(&reading, &monitors, Some(&prev)).unwrap();
        assert!(record.maximized);
        assert_eq!((record.x, record.y), (100.0, 80.0));
        assert_eq!((record.width, record.height), (1400.0, 900.0));
    }

    #[test]
    fn maximized_reading_without_a_previous_record_stores_nothing() {
        let monitors = [mon("A", 1920.0, 1080.0)];
        let reading = WindowReading {
            monitor_index: Some(0),
            position: (0.0, 0.0),
            size: (1920.0, 1080.0),
            maximized: true,
        };
        assert_eq!(SavedGeometry::from_reading(&reading, &monitors, None), None);
    }

    #[test]
    fn reading_records_the_monitor_name_and_rejects_garbage() {
        let monitors = [mon("A", 1920.0, 1080.0)];
        let reading = WindowReading {
            monitor_index: Some(0),
            position: (12.0, 34.0),
            size: (1000.0, 700.0),
            maximized: false,
        };
        let record = SavedGeometry::from_reading(&reading, &monitors, None).unwrap();
        assert_eq!(record.monitor_name.as_deref(), Some("A"));
        assert_eq!(record.monitor_index, Some(0));
        let bad = WindowReading {
            size: (0.0, 700.0),
            ..reading
        };
        assert_eq!(SavedGeometry::from_reading(&bad, &monitors, None), None);
        let out_of_range = WindowReading {
            monitor_index: Some(5),
            ..reading
        };
        let record = SavedGeometry::from_reading(&out_of_range, &monitors, None).unwrap();
        assert_eq!(record.monitor_index, None);
    }

    #[test]
    fn file_round_trip_and_garbage_file() {
        let dir = std::env::temp_dir().join(format!("signex-wg-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(FILE_NAME);
        let record = saved(Some("A"), Some(0), 5.0, 6.0);
        save_to(&path, &record).unwrap();
        assert_eq!(load_from(&path), Some(record));
        std::fs::write(&path, b"{ not json").unwrap();
        assert_eq!(load_from(&path), None);
        assert_eq!(load_from(&dir.join("missing.json")), None);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn settings_get_the_saved_rectangle() {
        let mut settings = iced::window::Settings::default();
        let mut s = saved(Some("A"), Some(1), 20.0, 30.0);
        s.maximized = true;
        apply_to_settings(&mut settings, &s);
        assert_eq!(settings.size, iced::Size::new(1400.0, 900.0));
        assert!(settings.maximized);
        match settings.position {
            iced::window::Position::Specific(p) => {
                assert_eq!(p.monitor_index, Some(iced::window::MonitorIndex(1)));
                assert_eq!(p.position, iced::Point::new(20.0, 30.0));
            }
            other => panic!("unexpected position {other:?}"),
        }
        let before = settings.size;
        s.width = f32::NAN;
        apply_to_settings(&mut settings, &s);
        assert_eq!(settings.size, before);
    }
}
