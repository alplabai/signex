//! Per-frame timing behind `SIGNEX_FRAME_PROBE=1` (#2102). Each timed span
//! prints one stderr line, `FRAME <ms since start> <label> <µs>`, when it ends, so a slow pan
//! shows where its time goes: `update`, `view` and the canvas layers. Off by
//! default; when off a span is a `None` and costs one cached flag read.

use std::sync::OnceLock;
use std::time::Instant;

/// Whether the probe is on: `SIGNEX_FRAME_PROBE` set to anything but `0`.
/// Read once; a diagnostic switch, not app state.
pub fn enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("SIGNEX_FRAME_PROBE").is_some_and(|v| v != "0"))
}

/// A timed span that reports when dropped.
#[must_use = "the span reports when it is dropped"]
pub struct Span {
    label: &'static str,
    detail: Option<String>,
    start: Option<Instant>,
}

/// Start timing `label`.
pub fn span(label: &'static str) -> Span {
    Span {
        label,
        detail: None,
        start: enabled().then(|| {
            epoch();
            Instant::now()
        }),
    }
}

/// Start timing `label` with a detail (e.g. the message name), built only
/// when the probe is on.
pub fn span_with(label: &'static str, detail: impl FnOnce() -> String) -> Span {
    let on = enabled();
    Span {
        label,
        detail: on.then(detail),
        start: on.then(|| {
            epoch();
            Instant::now()
        }),
    }
}

/// When the first span started; each line carries its offset from here in
/// milliseconds, so the frame rate can be read off the log.
fn epoch() -> Instant {
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    *EPOCH.get_or_init(Instant::now)
}

impl Drop for Span {
    fn drop(&mut self) {
        if let Some(start) = self.start {
            let us = start.elapsed().as_micros();
            let at_ms = start.saturating_duration_since(epoch()).as_millis();
            match &self.detail {
                Some(detail) => eprintln!("FRAME {at_ms} {} {us} {detail}", self.label),
                None => eprintln!("FRAME {at_ms} {} {us}", self.label),
            }
        }
    }
}

/// The variant path of a message's `Debug` text, without its payload:
/// `Canvas(CursorAt { x: 1.0, .. })` becomes `Canvas(CursorAt`.
pub fn message_name(debug: &str) -> String {
    debug
        .split([' ', '{', ','])
        .next()
        .unwrap_or(debug)
        .chars()
        .take(60)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::message_name;

    #[test]
    fn a_message_name_drops_its_payload() {
        assert_eq!(
            message_name("Canvas(CursorAt { x: 1.0 })"),
            "Canvas(CursorAt"
        );
        assert_eq!(message_name("Noop"), "Noop");
    }
}
