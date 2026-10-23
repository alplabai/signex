//! Byte-budget truncation that never splits a UTF-8 character.
//!
//! `&s[..n]` panics when byte `n` falls inside a multi-byte character, and a
//! panic in the UI thread loses every unsaved document. Any text a user can
//! influence (file names, library keys, log lines) must be cut through here.

/// The longest prefix of `s` that is at most `max_bytes` bytes and ends on a
/// character boundary.
pub(crate) fn prefix_within_bytes(s: &str, max_bytes: usize) -> &str {
    &s[..s.floor_char_boundary(max_bytes)]
}

#[cfg(test)]
mod tests {
    use super::prefix_within_bytes;

    #[test]
    fn ascii_is_cut_exactly_at_the_budget() {
        assert_eq!(prefix_within_bytes("abcdef", 3), "abc");
    }

    #[test]
    fn short_input_is_returned_whole() {
        assert_eq!(prefix_within_bytes("abc", 10), "abc");
    }

    #[test]
    fn cut_inside_a_two_byte_char_backs_off_to_its_start() {
        assert_eq!(prefix_within_bytes("aç", 2), "a");
    }

    #[test]
    fn cut_inside_a_four_byte_emoji_backs_off_to_its_start() {
        assert_eq!(prefix_within_bytes("ab😀", 4), "ab");
    }
}
