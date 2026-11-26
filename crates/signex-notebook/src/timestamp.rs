//! When a reading was taken: `2026-11-24T15:12`, `2026-11-24 15:12` or a
//! bare date, `2026-11-24`. Local bench time as written; no time zone is
//! read or added, so a notebook prints the same everywhere.

use std::fmt;

use crate::date::IsoDate;

/// A date and an optional time of day to the minute. Orders oldest first;
/// a bare date sorts before any time on that day.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp {
    date: IsoDate,
    /// Minutes after midnight.
    minute: Option<u16>,
}

impl Timestamp {
    /// Read `YYYY-MM-DD`, then optionally `T` or a space and `HH:MM`
    /// (seconds, `:SS`, are read and dropped).
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        let date = text.get(..10)?;
        let time = &text[10..];
        let date = IsoDate::parse(date)?;
        if time.is_empty() {
            return Some(Self { date, minute: None });
        }
        let time = time.strip_prefix(['T', ' '])?;
        let mut parts = time.split(':');
        let hour: u16 = two_digits(parts.next()?)?;
        let minute: u16 = two_digits(parts.next()?)?;
        match parts.next() {
            Some(seconds) if two_digits(seconds).is_some_and(|s| s < 60) => {}
            Some(_) => return None,
            None => {}
        }
        if parts.next().is_some() || hour > 23 || minute > 59 {
            return None;
        }
        Some(Self {
            date,
            minute: Some(hour * 60 + minute),
        })
    }

    pub fn date(self) -> IsoDate {
        self.date
    }

    /// `(hour, minute)`, when a time was written.
    pub fn time(self) -> Option<(u16, u16)> {
        self.minute.map(|m| (m / 60, m % 60))
    }

    /// Minutes since 1970-01-01 00:00, a bare date counting as midnight:
    /// the x position of a timeline point.
    pub fn minutes_since_epoch(self) -> i64 {
        self.date.days_since_epoch() * 1440 + i64::from(self.minute.unwrap_or(0))
    }
}

/// `2026-11-24 15:12`, or the bare date: how the table and the timeline
/// print it.
impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.time() {
            Some((hour, minute)) => write!(f, "{} {hour:02}:{minute:02}", self.date),
            None => write!(f, "{}", self.date),
        }
    }
}

/// Written as its text in a JSON result.
impl serde::Serialize for Timestamp {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

fn two_digits(text: &str) -> Option<u16> {
    (text.len() == 2 && text.bytes().all(|b| b.is_ascii_digit()))
        .then(|| text.parse().ok())
        .flatten()
}
