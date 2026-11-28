//! The wall clock as a host port (#2431, the time a domain crate never
//! reads): the date and time the computed strings print (`${CurrentDate}`,
//! `${CurrentTime}`, an empty title-block Date) and the date a watermark's
//! revision line carries. The runtime layer reads it; `view` and the core
//! read the stamp it left in state. A test pins it with [`Clock::Fixed`].

use chrono::{DateTime, Local};

/// Where the app's wall-clock reads come from.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum Clock {
    /// The system's local time.
    #[default]
    System,
    /// One instant, for a run that must print the same date every day.
    Fixed(DateTime<Local>),
}

impl Clock {
    /// The time now, by this clock.
    pub fn now(&self) -> DateTime<Local> {
        match self {
            Clock::System => Local::now(),
            Clock::Fixed(at) => *at,
        }
    }
}
