//! The directions of a command-line Apply (#2462): the rows a caller names
//! by id take their direction, and every other row is skipped. A name that
//! no row has, a row named twice or a direction the row does not allow
//! refuses the whole choice, with nothing changed.

use std::collections::HashSet;
use std::fmt;

use super::answer::unavailable_text;
use super::{CompareReport, Direction, Unavailable, rows};

/// Why [`CompareReport::choose`] changed nothing.
#[derive(Debug, Clone, PartialEq)]
pub enum ChooseRefusal {
    /// No row has this id.
    UnknownRow(String),
    /// The row is named more than once.
    Twice(String),
    /// The row does not allow the direction.
    NotAllowed {
        row: String,
        direction: Direction,
        why: Unavailable,
    },
}

impl fmt::Display for ChooseRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownRow(row) => write!(f, "no difference has the id {row}"),
            Self::Twice(row) => write!(f, "the difference {row} is named twice"),
            Self::NotAllowed {
                row,
                direction,
                why,
            } => write!(
                f,
                "the difference {row} cannot take {}: {}",
                direction.token(),
                unavailable_text(why)
            ),
        }
    }
}

impl CompareReport {
    /// Skip every row, then give each row `picks` names its direction.
    /// Returns whether a footprint row's tick changed, after which the net
    /// rows must be recomputed ([`Self::refresh_nets`]) and the choice made
    /// again, as [`Self::set_direction`] says.
    pub fn choose(&mut self, picks: &[(String, Direction)]) -> Result<bool, ChooseRefusal> {
        let mut named = HashSet::new();
        let mut chosen = Vec::with_capacity(picks.len());
        for (token, direction) in picks {
            let token = token.trim();
            if !named.insert(token) {
                return Err(ChooseRefusal::Twice(token.to_string()));
            }
            let Some(index) = self.rows.iter().position(|row| row.token() == token) else {
                return Err(ChooseRefusal::UnknownRow(token.to_string()));
            };
            if let Some(why) = self.rows[index].unavailable(*direction) {
                return Err(ChooseRefusal::NotAllowed {
                    row: token.to_string(),
                    direction: *direction,
                    why,
                });
            }
            chosen.push((index, *direction));
        }
        let mut footprint_changed = false;
        for index in 0..self.rows.len() {
            footprint_changed |= self.set_direction_unfolded(index, Direction::Skip);
        }
        for (index, direction) in chosen {
            footprint_changed |= self.set_direction_unfolded(index, direction);
        }
        rows::tick_folded(&mut self.eco);
        Ok(footprint_changed)
    }
}
