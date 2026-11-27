//! Annotation numbering schemes (#1292): one counter across the project, or a
//! block of numbers per page, so page 2 hands out R201, R202 and so on.
//!
//! A numbering unit is what Annotate numbers in one go: a sheet file, or one
//! placement of a file the project places more than once. Under a page scheme
//! each unit owns the block of its page number; blocks never overlap, so a
//! unit only needs to know the numbers already held inside its own block.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::split_reference;

/// How Annotate numbers a new designator. Stored as `"continuous"`,
/// `"page_hundreds"` or `"page_thousands"` in the `.snxprj`
/// `annotation.numbering` key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NumberingScheme {
    /// The next number after the highest one the project holds.
    #[default]
    Continuous,
    /// Page N numbers from N×100+1 to N×100+99.
    PageHundreds,
    /// Page N numbers from N×1000+1 to N×1000+999.
    PageThousands,
}

impl NumberingScheme {
    pub const ALL: &'static [NumberingScheme] = &[
        NumberingScheme::Continuous,
        NumberingScheme::PageHundreds,
        NumberingScheme::PageThousands,
    ];

    /// The stored token.
    pub fn token(self) -> &'static str {
        match self {
            NumberingScheme::Continuous => "continuous",
            NumberingScheme::PageHundreds => "page_hundreds",
            NumberingScheme::PageThousands => "page_thousands",
        }
    }

    /// Parse a token strictly: `None` for one that is not a scheme.
    pub fn parse(token: &str) -> Option<Self> {
        let token = token.trim();
        Self::ALL
            .iter()
            .copied()
            .find(|scheme| scheme.token().eq_ignore_ascii_case(token))
    }

    /// The size of one page's block; `None` for the continuous scheme.
    pub fn block_size(self) -> Option<u32> {
        match self {
            NumberingScheme::Continuous => None,
            NumberingScheme::PageHundreds => Some(100),
            NumberingScheme::PageThousands => Some(1000),
        }
    }
}

impl std::fmt::Display for NumberingScheme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NumberingScheme::Continuous => write!(f, "Continuous"),
            NumberingScheme::PageHundreds => write!(f, "Page number × 100"),
            NumberingScheme::PageThousands => write!(f, "Page number × 1000"),
        }
    }
}

/// The numbers one unit may hand out, `first..=last`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NumberBlock {
    pub first: u32,
    pub last: u32,
}

impl NumberBlock {
    fn holds(self, number: u32) -> bool {
        (self.first..=self.last).contains(&number)
    }
}

/// Why a page scheme cannot number a project.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BlockProblem {
    /// Two units print the same page number and would share one block.
    #[error("page {page} is printed on more than one sheet; number the sheets first")]
    SharedPage { page: usize, units: Vec<usize> },
    /// The page's block would pass the largest designator number.
    #[error("page {page} is too high a page number for this numbering scheme")]
    PageTooHigh { page: usize },
}

/// Why one unit cannot hand out another number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum BlockIssue {
    /// The unit has no page, so no block.
    #[error("this sheet has no page number for the numbering scheme")]
    NoBlock,
    /// Every number of the block is taken.
    #[error("every number from {} to {} is taken", .0.first, .0.last)]
    Full(NumberBlock),
}

/// The blocks of a page scheme, one per unit, and the highest number each
/// prefix already holds inside each block.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NumberBlocks {
    blocks: Vec<Option<NumberBlock>>,
    held: Vec<HashMap<String, u32>>,
}

impl NumberBlocks {
    /// The blocks of `scheme` for units numbered `pages` (`None`: a unit that
    /// hands out nothing in this run, so it gets no block). `kept` is every
    /// designator that keeps its number through the run, on any unit: a kept
    /// number inside a block is never handed out again. `Ok(None)` for the
    /// continuous scheme.
    pub fn new<'a>(
        scheme: NumberingScheme,
        pages: &[Option<usize>],
        kept: impl IntoIterator<Item = &'a str>,
    ) -> Result<Option<Self>, BlockProblem> {
        let Some(size) = scheme.block_size() else {
            return Ok(None);
        };
        let mut by_page: HashMap<usize, Vec<usize>> = HashMap::new();
        for (unit, page) in pages.iter().enumerate() {
            if let Some(page) = page {
                by_page.entry(*page).or_default().push(unit);
            }
        }
        let mut shared: Vec<(usize, Vec<usize>)> = by_page
            .into_iter()
            .filter(|(_, units)| units.len() > 1)
            .collect();
        shared.sort();
        if let Some((page, units)) = shared.into_iter().next() {
            return Err(BlockProblem::SharedPage { page, units });
        }
        let blocks = pages
            .iter()
            .map(|page| page.map(|page| block_of(page, size)).transpose())
            .collect::<Result<Vec<_>, _>>()?;
        let mut held = vec![HashMap::new(); blocks.len()];
        for reference in kept {
            let (prefix, number) = split_reference(reference);
            let Some(number) = number.filter(|_| !prefix.is_empty()) else {
                continue;
            };
            let unit = blocks
                .iter()
                .position(|block| block.is_some_and(|block| block.holds(number)));
            if let Some(unit) = unit {
                let highest: &mut u32 = held[unit].entry(prefix.to_owned()).or_insert(0);
                *highest = (*highest).max(number);
            }
        }
        Ok(Some(Self { blocks, held }))
    }

    /// The first number `unit` hands out; `None` for a unit with no block.
    pub fn start(&self, unit: usize) -> Option<u32> {
        self.blocks.get(unit).copied().flatten().map(|b| b.first)
    }

    /// The next free `prefix` number of `unit`, claimed in `issued` (the
    /// unit's own numbers so far, one map per unit and run): one past the
    /// highest number held or issued in the block, as the continuous scheme
    /// is one past the highest in the project.
    pub fn issue(
        &self,
        unit: usize,
        prefix: &str,
        issued: &mut HashMap<String, u32>,
    ) -> Result<u32, BlockIssue> {
        let block = self
            .blocks
            .get(unit)
            .copied()
            .flatten()
            .ok_or(BlockIssue::NoBlock)?;
        let held = self.held[unit].get(prefix).copied();
        let last = issued
            .entry(prefix.to_owned())
            .or_insert_with(|| held.unwrap_or(block.first - 1));
        let next = last
            .checked_add(1)
            .filter(|next| *next <= block.last)
            .ok_or(BlockIssue::Full(block))?;
        *last = next;
        Ok(next)
    }
}

/// Page `page`'s block: `page × size + 1` to `page × size + size - 1`.
fn block_of(page: usize, size: u32) -> Result<NumberBlock, BlockProblem> {
    let too_high = || BlockProblem::PageTooHigh { page };
    let base = u32::try_from(page)
        .ok()
        .and_then(|page| page.checked_mul(size))
        .ok_or_else(too_high)?;
    let last = base.checked_add(size - 1).ok_or_else(too_high)?;
    Ok(NumberBlock {
        first: base + 1,
        last,
    })
}

#[cfg(test)]
#[path = "numbering_tests.rs"]
mod tests;
