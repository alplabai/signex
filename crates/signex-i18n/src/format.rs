//! Locale formatting outside messages: numbers, lists, dates and times.
//!
//! Every function follows the *formatting locale* (see [`I18n::format_locale`]),
//! the language the user asked for, which stays the requested one when the
//! messages fall back to en-US (`tr-TR` has no catalog yet, but its number,
//! date and list rules exist in ICU).
//!
//! Engineering numbers (lengths, angles, component values) never come here:
//! they keep `.` in every locale.

use std::sync::{LazyLock, PoisonError};

use chrono::{DateTime, Datelike, Timelike, Utc};
use chrono_tz::Tz;
use icu_datetime::DateTimeFormatter;
use icu_datetime::fieldsets::YMD;
use icu_datetime::input::{Date, DateTime as IcuDateTime, Time};
use icu_list::ListFormatter;
use icu_list::options::{ListFormatterOptions, ListLength};

use crate::i18n::I18n;
use crate::numbers::format_decimal;
use crate::types::LocalNumber;

/// How much of a date or time to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateLength {
    /// `1/15/25`
    Short,
    /// `Jan 15, 2025`
    Medium,
    /// `January 15, 2025`
    Long,
}

/// The conjunction of a list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListKind {
    /// `a, b, and c`
    And,
    /// `a, b, or c`
    Or,
}

/// The operating system's zone, read once per process: a zone change while
/// the app runs shows after a restart.
static OS_ZONE: LazyLock<Tz> = LazyLock::new(|| {
    iana_time_zone::get_timezone()
        .ok()
        .and_then(|id| id.parse::<Tz>().ok())
        .unwrap_or_else(|| {
            log::warn!("The operating system time zone is unknown; showing UTC");
            Tz::UTC
        })
});

impl I18n {
    /// `value` with the locale's separators (`1.234,5` in `de`). For counts,
    /// sizes and ratios only: engineering values keep `.` and are never
    /// wrapped in [`LocalNumber`]. A NaN or infinity shows as Rust prints it.
    pub fn number(&self, value: LocalNumber) -> String {
        format_decimal(self.format_locale(), value.0).unwrap_or_else(|| value.0.to_string())
    }

    /// An integer count with the locale's grouping (`1,234`; `1.234` in `de`).
    /// No percent helper: `icu_decimal` has no percent pattern, and the sign's
    /// place differs by language, so a percentage is a message with a
    /// [`LocalNumber`] argument.
    pub fn count(&self, count: i64) -> String {
        // An i64 beyond 2^53 loses its low digits through f64.
        self.number(LocalNumber(count as f64))
    }

    /// `items` joined by the locale's list pattern: `a, b, and c` in
    /// `en-US`, `a, b ve c` in `tr`. Empty gives an empty string.
    pub fn list(&self, items: &[impl AsRef<str>], kind: ListKind) -> String {
        let options = ListFormatterOptions::default().with_length(ListLength::Wide);
        let prefs = self.format_locale().into();
        let formatter = match kind {
            ListKind::And => ListFormatter::try_new_and(prefs, options),
            ListKind::Or => ListFormatter::try_new_or(prefs, options),
        };
        match formatter {
            Ok(f) => f.format(items.iter().map(AsRef::as_ref)).to_string(),
            Err(_) => items
                .iter()
                .map(AsRef::as_ref)
                .collect::<Vec<_>>()
                .join(", "),
        }
    }

    /// `utc` as a date and time in the operating system's zone.
    pub fn date_time(&self, utc: DateTime<Utc>, length: DateLength) -> String {
        self.show(utc, &OS_ZONE, length, true)
    }

    /// `utc` as a date in the operating system's zone.
    pub fn date(&self, utc: DateTime<Utc>, length: DateLength) -> String {
        self.show(utc, &OS_ZONE, length, false)
    }

    /// [`Self::date_time`] in the IANA zone `zone_id` (`Europe/Istanbul`). An
    /// unknown id shows UTC and is logged once.
    pub fn date_time_in(&self, utc: DateTime<Utc>, zone_id: &str, length: DateLength) -> String {
        self.show(utc, &self.zone_or_utc(zone_id), length, true)
    }

    /// [`Self::date`] in the IANA zone `zone_id`; see [`Self::date_time_in`].
    pub fn date_in(&self, utc: DateTime<Utc>, zone_id: &str, length: DateLength) -> String {
        self.show(utc, &self.zone_or_utc(zone_id), length, false)
    }

    fn zone_or_utc(&self, zone_id: &str) -> Tz {
        zone_id.parse::<Tz>().unwrap_or_else(|_| {
            let mut seen = self
                .bad_zones
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if seen.insert(zone_id.to_owned()) {
                log::warn!("Unknown time zone {zone_id}; showing UTC");
            }
            Tz::UTC
        })
    }

    fn show(&self, utc: DateTime<Utc>, zone: &Tz, length: DateLength, with_time: bool) -> String {
        let local = utc.with_timezone(zone).naive_local();
        let input = Date::try_new_iso(local.year(), local.month() as u8, local.day() as u8)
            .ok()
            .zip(
                Time::try_new(
                    local.hour() as u8,
                    local.minute() as u8,
                    local.second() as u8,
                    local.nanosecond(),
                )
                .ok(),
            )
            .map(|(date, time)| IcuDateTime { date, time });
        let Some(input) = input else {
            return utc.to_rfc3339();
        };
        let fields = match length {
            DateLength::Short => YMD::short(),
            DateLength::Medium => YMD::medium(),
            DateLength::Long => YMD::long(),
        };
        let prefs = self.format_locale().into();
        let formatted = if with_time {
            DateTimeFormatter::try_new(prefs, fields.with_time_hm())
                .map(|f| f.format(&input).to_string())
        } else {
            DateTimeFormatter::try_new(prefs, fields).map(|f| f.format(&input).to_string())
        };
        formatted.unwrap_or_else(|_| utc.to_rfc3339())
    }
}
