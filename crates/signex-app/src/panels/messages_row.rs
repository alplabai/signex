//! What a Messages row shows in its Source and Time cells: a short source
//! name for the log target a diagnostic came from, and the wall-clock time
//! it was logged.

use chrono::{DateTime, TimeZone, Utc};
use signex_i18n::{MessageId, ids};

/// The short Source name of a Messages row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MessageSource {
    App,
    Tasks,
    Erc,
    Drc,
    Library,
    Preferences,
    Sketch,
    Project,
    Graphics,
    Interface,
    System,
}

impl MessageSource {
    /// The source of a diagnostic, from its code
    /// (`crate::diagnostics::diagnostic_code_from_target`, e.g.
    /// `SIGNEX-LIBRARY` or `SIGNEX_APP-DIAGNOSTICS`).
    pub(crate) fn of_code(code: &str) -> Self {
        if code.starts_with("GPU-") {
            return Self::Graphics;
        }
        let mut segments = code.split('-');
        let first = segments.next().unwrap_or_default();
        let crate_word = match first {
            "SIGNEX" | "APP" => None,
            _ => match first.strip_prefix("SIGNEX_") {
                Some(word) => Some(word),
                None => return Self::of_foreign_crate(first),
            },
        };
        crate_word
            .into_iter()
            .chain(segments)
            .find_map(Self::of_word)
            .unwrap_or(Self::App)
    }

    /// A target outside the Signex crates (a dependency's own log line).
    fn of_foreign_crate(first: &str) -> Self {
        // Graphics is read first: `ICED_WGPU` is the renderer, not `ICED`.
        const GRAPHICS: [&str; 5] = ["ICED_WGPU", "WGPU", "NAGA", "COSMIC_TEXT", "GLYPHON"];
        const INTERFACE: [&str; 2] = ["ICED", "WINIT"];
        if GRAPHICS.iter().any(|p| first.starts_with(p)) {
            Self::Graphics
        } else if INTERFACE.iter().any(|p| first.starts_with(p)) {
            Self::Interface
        } else {
            Self::System
        }
    }

    /// One crate or module word of a Signex target; `None` reads on.
    fn of_word(word: &str) -> Option<Self> {
        Some(match word {
            "TASK_DRIVER" => Self::Tasks,
            "ERC" | "ERC_DSL" => Self::Erc,
            "DRC" => Self::Drc,
            "LIBRARY" | "LIBRARY_SERVER" | "DISTRIBUTOR" | "SEARCH" | "DATABASE" => Self::Library,
            "PREFS" | "PREFERENCES" | "KEYMAP" | "FONTS" => Self::Preferences,
            "FOOTPRINT_PAD" | "BAKE" => Self::Sketch,
            w if w.starts_with("SKETCH") => Self::Sketch,
            "PROJECT" | "FORMAT" | "FS" | "HISTORY" | "NET" | "ENGINE" | "OUTPUT" => Self::Project,
            "GFX" | "RENDERER" => Self::Graphics,
            "WINDOW" | "UI" | "TABS" | "WIDGETS" => Self::Interface,
            _ => return None,
        })
    }

    /// The catalog message of the name.
    pub(crate) fn id(self) -> MessageId {
        match self {
            Self::App => ids::PANELS_MESSAGES_SOURCE_APP,
            Self::Tasks => ids::PANELS_MESSAGES_SOURCE_TASKS,
            Self::Erc => ids::PANELS_MESSAGES_SOURCE_ERC,
            Self::Drc => ids::PANELS_MESSAGES_SOURCE_DRC,
            Self::Library => ids::PANELS_MESSAGES_SOURCE_LIBRARY,
            Self::Preferences => ids::PANELS_MESSAGES_SOURCE_PREFERENCES,
            Self::Sketch => ids::PANELS_MESSAGES_SOURCE_SKETCH,
            Self::Project => ids::PANELS_MESSAGES_SOURCE_PROJECT,
            Self::Graphics => ids::PANELS_MESSAGES_SOURCE_GRAPHICS,
            Self::Interface => ids::PANELS_MESSAGES_SOURCE_INTERFACE,
            Self::System => ids::PANELS_MESSAGES_SOURCE_SYSTEM,
        }
    }
}

/// `at` on the operating system's local clock as `HH:MM:SS`. Display only.
pub(crate) fn clock_time(at: DateTime<Utc>) -> String {
    clock_time_in(at, &chrono::Local)
}

fn clock_time_in<Z: TimeZone>(at: DateTime<Utc>, zone: &Z) -> String
where
    Z::Offset: std::fmt::Display,
{
    at.with_timezone(zone).format("%H:%M:%S").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::diagnostic_code_from_target;

    fn source(target: &str) -> MessageSource {
        MessageSource::of_code(&diagnostic_code_from_target(target))
    }

    #[test]
    fn known_targets_get_their_short_source_name() {
        assert_eq!(source("signex::task_driver"), MessageSource::Tasks);
        assert_eq!(source("signex_app::diagnostics"), MessageSource::App);
        assert_eq!(source("signex::library"), MessageSource::Library);
        assert_eq!(source("signex_library::database"), MessageSource::Library);
        assert_eq!(source("signex::distributor"), MessageSource::Library);
        assert_eq!(source("signex_erc::rules"), MessageSource::Erc);
        assert_eq!(
            source("signex_app::app::handlers::erc::erc_run"),
            MessageSource::Erc
        );
        assert_eq!(source("signex::keymap"), MessageSource::Preferences);
        assert_eq!(source("signex::sketch_constraints"), MessageSource::Sketch);
        assert_eq!(source("signex::project"), MessageSource::Project);
        assert_eq!(source("signex_net::netlist"), MessageSource::Project);
        assert_eq!(source("signex_engine::history"), MessageSource::Project);
        assert_eq!(source("signex_output::pdf"), MessageSource::Project);
        assert_eq!(source("signex::window"), MessageSource::Interface);
        assert_eq!(source("signex"), MessageSource::App);
    }

    #[test]
    fn graphics_records_and_foreign_crates_are_named_too() {
        assert_eq!(
            MessageSource::of_code("GPU-ADAPTER-SELECTED"),
            MessageSource::Graphics
        );
        assert_eq!(source("wgpu_core::device"), MessageSource::Graphics);
        assert_eq!(source("iced_winit::program"), MessageSource::Interface);
        assert_eq!(source("iced_wgpu::window"), MessageSource::Graphics);
        assert_eq!(source("reqwest::connect"), MessageSource::System);
        assert_eq!(MessageSource::of_code("APP-EVENT"), MessageSource::App);
    }

    #[test]
    fn the_time_cell_is_the_wall_clock_to_the_second() {
        let at = DateTime::parse_from_rfc3339("2026-11-11T14:32:08.750Z")
            .expect("valid instant")
            .with_timezone(&Utc);
        assert_eq!(clock_time_in(at, &Utc), "14:32:08");
        let plus3 = chrono::FixedOffset::east_opt(3 * 3600).expect("valid offset");
        assert_eq!(clock_time_in(at, &plus3), "17:32:08");
    }
}
