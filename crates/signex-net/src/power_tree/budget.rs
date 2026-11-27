//! The power budget per block (#1313): what the parts of one sheet draw from
//! each rail and supply to each rail. A generated block diagram has one block
//! per sheet, linked by the sheet's document id, so a block's budget is its
//! sheet's. Rules: `docs/POWER_TREE.md`.

use std::collections::BTreeMap;

use signex_types::block_diagram::BlockDiagram;
use uuid::Uuid;

use super::topology::PowerTopology;
use super::tree::PowerTree;

/// Under this share of a limit a budget is fine (mockup row 171).
pub const FINE_BELOW: f64 = 0.5;
/// Above this share of a limit a budget is over (mockup row 171).
pub const OVER_ABOVE: f64 = 0.9;

/// How much of a limit a budget uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetLevel {
    /// Under half.
    Fine,
    /// From half to 90 %.
    Close,
    /// Over 90 %.
    Over,
}

impl BudgetLevel {
    /// The level of `used` out of `limit`; `None` without a limit above 0.
    pub fn of(used: f64, limit: f64) -> Option<Self> {
        if !limit.is_finite() || limit <= 0.0 || !used.is_finite() {
            return None;
        }
        let share = used / limit;
        Some(if share > OVER_ABOVE {
            Self::Over
        } else if share >= FINE_BELOW {
            Self::Close
        } else {
            Self::Fine
        })
    }
}

/// What one sheet's parts draw from one rail.
#[derive(Debug, Clone, PartialEq)]
pub struct RailDraw {
    pub net: String,
    /// What the parts that tell draw together, in A.
    pub current_a: f64,
    /// Whether every part of the sheet on the rail tells.
    pub is_complete: bool,
    /// Those parts, by their place in [`PowerTopology::parts`].
    pub parts: Vec<usize>,
}

/// What one sheet's parts supply to one rail.
#[derive(Debug, Clone, PartialEq)]
pub struct RailFeed {
    pub net: String,
    /// The supplying parts' limits together, in A; `None` when one is not
    /// told.
    pub limit_a: Option<f64>,
    pub parts: Vec<usize>,
}

/// The budget of one sheet, and so of the block that stands for it.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetBudget {
    /// The sheet's document id.
    pub sheet: Uuid,
    /// By rail name.
    pub draws: Vec<RailDraw>,
    /// By rail name.
    pub feeds: Vec<RailFeed>,
}

impl SheetBudget {
    /// What the sheet draws in W: each draw times its rail's voltage. `None`
    /// when a draw is not complete or its rail has no voltage.
    pub fn power_w(&self, tree: &PowerTree) -> Option<f64> {
        let mut watts = 0.0;
        for draw in &self.draws {
            let volts = tree.rail(&draw.net)?.voltage?.volts.abs();
            if !draw.is_complete {
                return None;
            }
            watts += draw.current_a * volts;
        }
        Some(watts)
    }
}

/// The budget of every sheet that has a part on a rail, by sheet id. Two
/// occurrences of one sheet add up: a sheet placed twice draws twice.
pub fn sheet_budgets(topology: &PowerTopology, tree: &PowerTree) -> Vec<SheetBudget> {
    let mut draws: BTreeMap<(Uuid, String), RailDraw> = BTreeMap::new();
    let mut feeds: BTreeMap<(Uuid, String), RailFeed> = BTreeMap::new();
    for rail in &tree.rails {
        for load in &rail.loads {
            let sheet = topology.parts[load.part].sheet_id;
            let draw = draws
                .entry((sheet, rail.net.clone()))
                .or_insert_with(|| RailDraw {
                    net: rail.net.clone(),
                    current_a: 0.0,
                    is_complete: true,
                    parts: Vec::new(),
                });
            match load.current_a {
                Some(current) => draw.current_a += current,
                None => draw.is_complete = false,
            }
            draw.parts.push(load.part);
        }
        for source in &rail.sources {
            let sheet = topology.parts[source.part].sheet_id;
            let feed = feeds
                .entry((sheet, rail.net.clone()))
                .or_insert_with(|| RailFeed {
                    net: rail.net.clone(),
                    limit_a: Some(0.0),
                    parts: Vec::new(),
                });
            feed.limit_a = feed.limit_a.zip(source.limit_a).map(|(a, b)| a + b);
            feed.parts.push(source.part);
        }
    }
    let mut sheets: BTreeMap<Uuid, SheetBudget> = BTreeMap::new();
    for ((sheet, _), draw) in draws {
        let budget = sheets
            .entry(sheet)
            .or_insert_with(|| SheetBudget::empty(sheet));
        budget.draws.push(draw);
    }
    for ((sheet, _), feed) in feeds {
        let budget = sheets
            .entry(sheet)
            .or_insert_with(|| SheetBudget::empty(sheet));
        budget.feeds.push(feed);
    }
    sheets.into_values().collect()
}

impl SheetBudget {
    fn empty(sheet: Uuid) -> Self {
        Self {
            sheet,
            draws: Vec::new(),
            feeds: Vec::new(),
        }
    }
}

/// One block of a diagram and its budget.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockBudget<'a> {
    pub block: Uuid,
    /// The budget of the sheet the block is linked to; `None` for a block no
    /// sheet backs, or whose sheet has no part on a rail.
    pub budget: Option<&'a SheetBudget>,
}

/// Every block of `diagram`, in its drawing order, with the budget of the
/// sheet it is linked to.
pub fn block_budgets<'a>(
    diagram: &BlockDiagram,
    sheets: &'a [SheetBudget],
) -> Vec<BlockBudget<'a>> {
    diagram
        .blocks
        .iter()
        .map(|block| {
            let sheet = block.sheet.as_ref().map(|link| link.sheet.as_uuid());
            BlockBudget {
                block: block.id,
                budget: sheet.and_then(|id| sheets.iter().find(|budget| budget.sheet == id)),
            }
        })
        .collect()
}
