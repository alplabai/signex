//! Locale formatting outside messages: numbers, lists, dates and times.
//!
//! Every function follows the *formatting locale* (see [`I18n::format_locale`]),
//! the language the user asked for, which stays the requested one when the
//! messages fall back to en-US (`tr-TR` has no catalog yet, but its number,
//! date and list rules exist in ICU).
//!
//! Engineering numbers (lengths, angles, component values) never come here:
//! they keep `.` in every locale.

use std::sync::PoisonError;

use chrono::{DateTime, Datelike, Local, NaiveDateTime, TimeZone, Timelike, Utc};
use icu_datetime::DateTimeFormatter;
use icu_datetime::fieldsets::YMD;
use icu_datetime::input::{Date, DateTime as IcuDateTime, Time};
use icu_decimal::input::Decimal;
use icu_list::ListFormatter;
use icu_list::options::{ListFormatterOptions, ListLength};

use crate::i18n::I18n;
use crate::numbers::{format_decimal, format_exact};
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

impl I18n {
    /// `value` with the locale's separators (`1.234,5` in `de`). For counts,
    /// sizes and ratios only: engineering values keep `.` and are never
    /// wrapped in [`LocalNumber`]. A NaN or infinity shows as Rust prints it.
    ///
    /// A bare float does not compile; wrap it in [`LocalNumber`]:
    ///
    /// ```compile_fail,E0308
    /// let i18n = signex_i18n::I18n::en_us();
    /// let _ = i18n.number(1.27_f64);
    /// ```
    ///
    /// ```
    /// let i18n = signex_i18n::I18n::en_us();
    /// assert_eq!(i18n.number(signex_i18n::LocalNumber(1.27)), "1.27");
    /// ```
    pub fn number(&self, value: LocalNumber) -> String {
        format_decimal(self.format_locale(), value.0).unwrap_or_else(|| value.0.to_string())
    }

    /// An integer count with the locale's grouping (`1,234`; `1.234` in `de`),
    /// exact for every `i64`. No percent helper: `icu_decimal` has no percent
    /// pattern, and the sign's place differs by language, so a percentage is
    /// a message with a [`LocalNumber`] argument.
    pub fn count(&self, count: i64) -> String {
        format_exact(self.format_locale(), &Decimal::from(count))
            .unwrap_or_else(|| count.to_string())
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
            Err(error) => {
                self.warn_format_once("list", || {
                    log::warn!("No list pattern for {}: {error}", self.format_locale());
                });
                items
                    .iter()
                    .map(AsRef::as_ref)
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        }
    }

    /// `utc` as a date and time on the operating system's local clock.
    pub fn date_time(&self, utc: DateTime<Utc>, length: DateLength) -> String {
        self.show(wall_clock(utc, &Local), length, true)
    }

    /// `utc` as a date on the operating system's local clock.
    pub fn date(&self, utc: DateTime<Utc>, length: DateLength) -> String {
        self.show(wall_clock(utc, &Local), length, false)
    }

    /// Run `log` the first time `what` fails: a view formats every frame.
    pub(crate) fn warn_format_once(&self, what: &'static str, log: impl FnOnce()) {
        let mut warned = self
            .format_warned
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if warned.insert(what) {
            log();
        }
    }

    /// `local` (a wall clock) in the formatting locale. Falls back to
    /// ISO 8601 when ICU rejects the value or has no pattern.
    pub(crate) fn show(&self, local: NaiveDateTime, length: DateLength, with_time: bool) -> String {
        let iso = || local.format("%Y-%m-%dT%H:%M:%S").to_string();
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
            self.warn_format_once("date-input", || {
                log::warn!("A date outside ICU's range; showing ISO 8601");
            });
            return iso();
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
        formatted.unwrap_or_else(|error| {
            self.warn_format_once("date", || {
                log::warn!(
                    "No date pattern for {}: {error}; showing ISO 8601",
                    self.format_locale()
                );
            });
            iso()
        })
    }
}

/// `utc` on `zone`'s wall clock. Production passes [`Local`], which reads the
/// OS zone (`TZ`, `/etc/localtime`, the Windows registry) on every call,
/// unnamed offset zones included.
pub(crate) fn wall_clock<Z: TimeZone>(utc: DateTime<Utc>, zone: &Z) -> NaiveDateTime {
    utc.with_timezone(zone).naive_local()
}
