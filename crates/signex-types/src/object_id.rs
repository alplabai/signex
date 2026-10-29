//! Object ids. Every new id is a UUID v7 (`Uuid::now_v7`); the rule and its
//! reasons are in `CONTRIBUTING.md` §"Object ids".
//!
//! A v7 id starts with a 48-bit millisecond timestamp, so its leading hex
//! digits are shared by everything created in the same minute. A short form
//! for display therefore takes the random tail, never the prefix.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use uuid::Uuid;

/// Hex digits in [`short_id`].
pub const SHORT_ID_LEN: usize = 8;

/// An 8-hex-digit label for a human to tell ids apart: the last digits,
/// which are random in both v7 and the older v4 ids.
pub fn short_id(id: &Uuid) -> String {
    let simple = id.simple().to_string();
    simple[simple.len() - SHORT_ID_LEN..].to_string()
}

/// When a v7 id was created, read from its timestamp. `None` for any other
/// version, including the v4 ids older files carry.
pub fn created_at(id: &Uuid) -> Option<SystemTime> {
    if id.get_version_num() != 7 {
        return None;
    }
    let (secs, nanos) = id.get_timestamp()?.to_unix();
    UNIX_EPOCH.checked_add(Duration::new(secs, nanos))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_id_is_the_random_tail() {
        let id = Uuid::parse_str("12345678-9abc-7ef0-9234-56789abcdef0").unwrap();
        assert_eq!(short_id(&id), "9abcdef0");
    }

    #[test]
    fn ids_made_together_get_different_short_ids() {
        // Same-millisecond v7 ids share all 12 leading hex digits.
        let (a, b) = (Uuid::now_v7(), Uuid::now_v7());
        assert_ne!(short_id(&a), short_id(&b));
    }

    #[test]
    fn created_at_reads_a_v7_timestamp() {
        let before = SystemTime::now() - Duration::from_millis(1);
        let made = created_at(&Uuid::now_v7()).expect("a v7 id has a time");
        let after = SystemTime::now() + Duration::from_millis(1);
        assert!(before <= made && made <= after);
    }

    #[test]
    fn created_at_is_none_for_other_versions() {
        let v4 = Uuid::parse_str("12345678-9abc-4ef0-9234-56789abcdef0").unwrap();
        assert_eq!(created_at(&v4), None);
        assert_eq!(created_at(&Uuid::nil()), None);
    }
}
