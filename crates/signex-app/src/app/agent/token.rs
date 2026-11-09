//! The per-boot session token and the boot id (#1924). The token is 256 random
//! bits from the operating system, compared in constant time.

use std::fmt;
use std::hash::{Hash, Hasher};

/// A session token. It never prints: `Debug` shows only that it is one.
#[derive(Clone, PartialEq, Eq)]
pub(super) struct Token(String);

impl Token {
    /// A fresh token, or `None` when the system has no randomness: the
    /// listener then does not start, rather than run without one.
    pub(super) fn generate() -> Option<Self> {
        random_hex(32).map(Self)
    }

    pub(super) fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether `candidate` is this token. The time taken depends on the
    /// token's length, which is public, never on how much of `candidate`
    /// matches.
    pub(super) fn matches(&self, candidate: &str) -> bool {
        let ours = self.0.as_bytes();
        let theirs = candidate.as_bytes();
        let mut difference = ours.len() ^ theirs.len();
        for (index, byte) in ours.iter().enumerate() {
            difference |= usize::from(byte ^ theirs.get(index).copied().unwrap_or(0));
        }
        std::hint::black_box(difference) == 0
    }
}

impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Token(..)")
    }
}

impl Hash for Token {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

/// A random id for this boot, as 16 hex characters. Not a secret: it names
/// the files and registry entries of one app run.
pub(super) fn boot_id() -> Option<String> {
    random_hex(8)
}

fn random_hex(bytes: usize) -> Option<String> {
    let mut raw = vec![0_u8; bytes];
    getrandom::fill(&mut raw).ok()?;
    Some(raw.iter().map(|byte| format!("{byte:02x}")).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_token_matches_only_itself() {
        let token = Token::generate().expect("randomness");
        assert_eq!(token.as_str().len(), 64);
        assert!(token.matches(token.as_str()));
        assert!(!token.matches(""));
        assert!(!token.matches(&token.as_str()[..63]));
        assert!(!token.matches(&format!("{}0", token.as_str())));
        let mut flipped = token.as_str().to_string();
        flipped.replace_range(63.., if flipped.ends_with('0') { "1" } else { "0" });
        assert!(!token.matches(&flipped));
    }

    #[test]
    fn two_boots_do_not_share_a_token_or_a_boot_id() {
        let (a, b) = (Token::generate().unwrap(), Token::generate().unwrap());
        assert_ne!(a, b);
        assert_ne!(boot_id().unwrap(), boot_id().unwrap());
        assert_eq!(boot_id().unwrap().len(), 16);
    }

    #[test]
    fn a_token_never_prints() {
        let token = Token::generate().unwrap();
        assert!(!format!("{token:?}").contains(token.as_str()));
    }
}
