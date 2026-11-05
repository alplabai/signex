//! Where a new OS window opens: on the monitor of the window the user acted
//! from, not on the monitor of whichever window was created last.
//!
//! iced reports a window's position relative to the monitor it is on, with
//! that monitor's index (`window::position`). The monitor list carries sizes
//! but no origins, so the rules here work on that relative form only: pick
//! the source window's monitor, centre the new window there, and clamp it so
//! it is fully visible. Reading the source window is asynchronous, so the app
//! keeps the last settled reading of every window (see
//! `window_geometry_events`) and the open sites use it synchronously.

use crate::app::window_geometry::MonitorInfo;

/// The window the user acted from, as last read.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SourceWindow {
    /// Index of the monitor it is on.
    pub monitor_index: Option<usize>,
    /// Position relative to that monitor, logical pixels.
    pub position: (f32, f32),
    /// Logical size.
    pub size: (f32, f32),
}

/// What the new window is centred on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    /// The middle of the source window's monitor.
    MonitorCenter,
    /// The middle of the source window, kept fully on its monitor.
    SourceCenter,
}

/// The chosen monitor and the window's position on it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    /// Index of the monitor to open on.
    pub monitor_index: usize,
    /// Position relative to that monitor, logical pixels.
    pub position: (f32, f32),
}

/// The monitor to use: the source window's, else the primary one, else the
/// first. `None` when there are no monitors.
fn pick_monitor(source: Option<usize>, monitors: &[MonitorInfo]) -> Option<usize> {
    if monitors.is_empty() {
        return None;
    }
    source
        .filter(|i| *i < monitors.len())
        .or_else(|| monitors.iter().position(|m| m.primary))
        .or(Some(0))
}

/// Pick the monitor and a position for a window of `window_size`.
///
/// The position is clamped to the monitor so the whole window is visible;
/// a window larger than the monitor sits at the monitor's top-left corner.
#[must_use]
pub fn place(
    source: &SourceWindow,
    window_size: (f32, f32),
    monitors: &[MonitorInfo],
    anchor: Anchor,
) -> Option<Placement> {
    let index = pick_monitor(source.monitor_index, monitors)?;
    let monitor = monitors[index].size;
    let (w, h) = window_size;
    if !(w.is_finite() && h.is_finite() && monitor.0.is_finite() && monitor.1.is_finite()) {
        return None;
    }
    let source_ok = [
        source.position.0,
        source.position.1,
        source.size.0,
        source.size.1,
    ]
    .iter()
    .all(|v| v.is_finite());
    let (cx, cy) = match anchor {
        Anchor::SourceCenter if source_ok && source.monitor_index == Some(index) => (
            source.position.0 + source.size.0 / 2.0,
            source.position.1 + source.size.1 / 2.0,
        ),
        _ => (monitor.0 / 2.0, monitor.1 / 2.0),
    };
    let x = (cx - w / 2.0).clamp(0.0, (monitor.0 - w).max(0.0));
    let y = (cy - h / 2.0).clamp(0.0, (monitor.1 - h).max(0.0));
    Some(Placement {
        monitor_index: index,
        position: (x, y),
    })
}

/// The `window::Position` for a new window: on the source window's monitor
/// when it is known, otherwise `fallback` (what the call site used before).
#[must_use]
pub fn position_for(
    source: Option<&SourceWindow>,
    window_size: (f32, f32),
    monitors: &[MonitorInfo],
    anchor: Anchor,
    fallback: iced::window::Position,
) -> iced::window::Position {
    let placed = source.and_then(|s| place(s, window_size, monitors, anchor));
    match placed {
        Some(p) => iced::window::Position::Specific(iced::window::PositionOnMonitor {
            monitor_index: Some(iced::window::MonitorIndex(p.monitor_index)),
            position: iced::Point::new(p.position.0, p.position.1),
        }),
        None => fallback,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mon(w: f32, h: f32, primary: bool) -> MonitorInfo {
        MonitorInfo {
            name: None,
            size: (w, h),
            primary,
        }
    }

    fn src(monitor: Option<usize>, x: f32, y: f32) -> SourceWindow {
        SourceWindow {
            monitor_index: monitor,
            position: (x, y),
            size: (1000.0, 700.0),
        }
    }

    #[test]
    fn single_monitor_centres_on_the_monitor() {
        let monitors = [mon(1920.0, 1080.0, true)];
        let p = place(
            &src(Some(0), 100.0, 100.0),
            (400.0, 200.0),
            &monitors,
            Anchor::MonitorCenter,
        )
        .unwrap();
        assert_eq!(p.monitor_index, 0);
        assert_eq!(p.position, (760.0, 440.0));
    }

    #[test]
    fn source_on_the_second_monitor_opens_there() {
        let monitors = [mon(1920.0, 1080.0, true), mon(2560.0, 1440.0, false)];
        let p = place(
            &src(Some(1), 0.0, 0.0),
            (400.0, 200.0),
            &monitors,
            Anchor::MonitorCenter,
        )
        .unwrap();
        assert_eq!(p.monitor_index, 1);
        assert_eq!(p.position, (1080.0, 620.0));
    }

    #[test]
    fn source_center_anchor_follows_the_source_window() {
        let monitors = [mon(1920.0, 1080.0, true), mon(2560.0, 1440.0, false)];
        // Source centre is (600, 450); a 400x200 window is centred there.
        let p = place(
            &src(Some(1), 100.0, 100.0),
            (400.0, 200.0),
            &monitors,
            Anchor::SourceCenter,
        )
        .unwrap();
        assert_eq!(p.monitor_index, 1);
        assert_eq!(p.position, (400.0, 350.0));
    }

    #[test]
    fn straddling_source_is_clamped_onto_its_monitor() {
        let monitors = [mon(1920.0, 1080.0, true), mon(1920.0, 1080.0, false)];
        // Mostly on monitor 1, hanging 600 px off its left edge: the centre
        // is at x = -100, the 800 px wide window is pulled to the edge.
        let source = SourceWindow {
            monitor_index: Some(1),
            position: (-600.0, 50.0),
            size: (1000.0, 700.0),
        };
        let p = place(&source, (800.0, 400.0), &monitors, Anchor::SourceCenter).unwrap();
        assert_eq!(p.monitor_index, 1);
        assert_eq!(p.position.0, 0.0);
        // Hanging off the right edge instead.
        let source = SourceWindow {
            position: (1500.0, 50.0),
            ..source
        };
        let p = place(&source, (800.0, 400.0), &monitors, Anchor::SourceCenter).unwrap();
        assert_eq!(p.position.0, 1120.0);
    }

    #[test]
    fn negative_coordinates_never_leave_the_monitor() {
        let monitors = [mon(1920.0, 1080.0, true)];
        let source = SourceWindow {
            monitor_index: Some(0),
            position: (-5000.0, -5000.0),
            size: (100.0, 100.0),
        };
        let p = place(&source, (400.0, 300.0), &monitors, Anchor::SourceCenter).unwrap();
        assert_eq!(p.position, (0.0, 0.0));
    }

    #[test]
    fn window_larger_than_the_monitor_sits_at_the_corner() {
        let monitors = [mon(1280.0, 720.0, true)];
        let p = place(
            &src(Some(0), 0.0, 0.0),
            (1400.0, 900.0),
            &monitors,
            Anchor::MonitorCenter,
        )
        .unwrap();
        assert_eq!(p.position, (0.0, 0.0));
    }

    #[test]
    fn unknown_source_monitor_uses_the_primary_then_the_first() {
        let monitors = [mon(1000.0, 800.0, false), mon(2000.0, 1000.0, true)];
        let p = place(
            &src(None, 0.0, 0.0),
            (200.0, 200.0),
            &monitors,
            Anchor::SourceCenter,
        )
        .unwrap();
        assert_eq!(p.monitor_index, 1);
        // The source's own geometry is not used for another monitor.
        assert_eq!(p.position, (900.0, 400.0));
        let monitors = [mon(1000.0, 800.0, false), mon(2000.0, 1000.0, false)];
        let p = place(
            &src(Some(9), 0.0, 0.0),
            (200.0, 200.0),
            &monitors,
            Anchor::MonitorCenter,
        )
        .unwrap();
        assert_eq!(p.monitor_index, 0);
    }

    #[test]
    fn no_monitors_or_garbage_gives_no_placement() {
        assert_eq!(
            place(
                &src(Some(0), 0.0, 0.0),
                (10.0, 10.0),
                &[],
                Anchor::MonitorCenter
            ),
            None
        );
        let monitors = [mon(1920.0, 1080.0, true)];
        assert_eq!(
            place(
                &src(Some(0), 0.0, 0.0),
                (f32::NAN, 10.0),
                &monitors,
                Anchor::MonitorCenter
            ),
            None
        );
        // A non-finite source falls back to the monitor centre.
        let source = SourceWindow {
            monitor_index: Some(0),
            position: (f32::NAN, 0.0),
            size: (1.0, 1.0),
        };
        let p = place(&source, (400.0, 200.0), &monitors, Anchor::SourceCenter).unwrap();
        assert_eq!(p.position, (760.0, 440.0));
    }

    #[test]
    fn position_for_falls_back_to_centered_without_a_source() {
        let monitors = [mon(1920.0, 1080.0, true)];
        let pos = position_for(
            None,
            (400.0, 200.0),
            &monitors,
            Anchor::MonitorCenter,
            iced::window::Position::Centered,
        );
        assert!(matches!(pos, iced::window::Position::Centered));
        let source = src(Some(0), 0.0, 0.0);
        let pos = position_for(
            Some(&source),
            (400.0, 200.0),
            &monitors,
            Anchor::MonitorCenter,
            iced::window::Position::Default,
        );
        match pos {
            iced::window::Position::Specific(p) => {
                assert_eq!(p.monitor_index, Some(iced::window::MonitorIndex(0)));
                assert_eq!(p.position, iced::Point::new(760.0, 440.0));
            }
            other => panic!("unexpected {other:?}"),
        }
        let pos = position_for(
            Some(&source),
            (400.0, 200.0),
            &[],
            Anchor::MonitorCenter,
            iced::window::Position::Default,
        );
        assert!(matches!(pos, iced::window::Position::Default));
    }
}
