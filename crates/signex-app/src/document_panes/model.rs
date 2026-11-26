//! The main window's panes after a tab was dropped on an edge of the
//! document area (mockup row 207, #1307): plain data and its rules, generic
//! over the key that names a tab so the rules test without an app. Design:
//! docs/MENU_BAR.md, "View ▸ Window ▸ Split".

use crate::app::TileLayout;

/// The most panes one window splits into (mockup row 207, Tile Grid's four).
pub const MAX_PANES: usize = 4;

/// The share of the document area's width (height) from a side that drops
/// on that side's edge; nearer the middle a drop keeps the tab where it is.
pub const EDGE_SHARE: f32 = 0.25;

/// The edge of the document area a tab is dropped on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropEdge {
    Left,
    Right,
    Top,
    Bottom,
}

impl DropEdge {
    /// Left and right split side by side; top and bottom stack.
    pub fn direction(self) -> TileLayout {
        match self {
            Self::Left | Self::Right => TileLayout::SideBySide,
            Self::Top | Self::Bottom => TileLayout::Stacked,
        }
    }

    /// Whether the new pane goes before the others (left or top).
    pub fn is_leading(self) -> bool {
        matches!(self, Self::Left | Self::Top)
    }

    /// The token the agent reply and the stored panes use.
    pub fn key(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
            Self::Top => "top",
            Self::Bottom => "bottom",
        }
    }
}

/// What a drop does where the cursor is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropTarget {
    /// A new pane on this edge.
    Edge(DropEdge),
    /// The middle: the tab stays a tab where it is.
    Centre,
}

/// The drop target at `cursor` over `area` (x, y, width, height): the
/// nearest edge within [`EDGE_SHARE`] of it, else the centre. `None` off the
/// area, on an empty area or for a cursor that is not a number.
pub fn drop_target(area: (f32, f32, f32, f32), cursor: (f32, f32)) -> Option<DropTarget> {
    let (x, y, width, height) = area;
    let (cx, cy) = cursor;
    let is_inside = cx.is_finite()
        && cy.is_finite()
        && width > 0.0
        && height > 0.0
        && cx >= x
        && cy >= y
        && cx <= x + width
        && cy <= y + height;
    if !is_inside {
        return None;
    }
    let (fx, fy) = ((cx - x) / width, (cy - y) / height);
    let sides = [
        (fx, DropEdge::Left),
        (1.0 - fx, DropEdge::Right),
        (fy, DropEdge::Top),
        (1.0 - fy, DropEdge::Bottom),
    ];
    let (share, edge) = sides
        .into_iter()
        .fold((f32::INFINITY, DropEdge::Left), |nearest, side| {
            if side.0 < nearest.0 { side } else { nearest }
        });
    Some(if share < EDGE_SHARE {
        DropTarget::Edge(edge)
    } else {
        DropTarget::Centre
    })
}

/// Why a split was not made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitRefusal {
    /// The tab is not in the main window.
    NotDocked,
    /// Moving the only tab would leave the other pane empty.
    LastTab,
    /// The panes run the other way; a split across them is not offered.
    Crosswise,
    /// The window has [`MAX_PANES`] panes already.
    TooManyPanes,
}

/// One pane: the tabs dropped into it, and the tab it shows while another
/// pane has the focus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pane<K> {
    pub tabs: Vec<K>,
    pub shown: Option<K>,
}

impl<K> Pane<K> {
    fn of(tab: K, shown: Option<K>) -> Self {
        Self {
            tabs: vec![tab],
            shown,
        }
    }
}

/// The panes of the main window. A docked tab no pane lists sits in the
/// `rest` pane, so a document opened while split joins that pane. A tab
/// listed in two panes is a second view of its document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneSplit<K> {
    pub direction: TileLayout,
    pub panes: Vec<Pane<K>>,
    pub rest: usize,
    /// The pane with the focus, which shows the active tab.
    pub focused: usize,
}

/// A pane as the window shows it: its index in [`PaneSplit::panes`] and its
/// tabs as positions in the docked list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShownPane {
    pub pane: usize,
    pub tabs: Vec<usize>,
}

impl<K: Clone + PartialEq> PaneSplit<K> {
    /// The first split: `tab`, one of `docked`, dropped on `edge` while
    /// `active` is the active tab. Moved, it leaves every other tab in the
    /// first pane; as a second view (`second_view`) it stays there too. The
    /// new pane has the focus.
    pub fn first(
        docked: &[K],
        tab: &K,
        active: &K,
        edge: DropEdge,
        second_view: bool,
    ) -> Result<Self, SplitRefusal> {
        if !docked.contains(tab) {
            return Err(SplitRefusal::NotDocked);
        }
        if !second_view && docked.len() < 2 {
            return Err(SplitRefusal::LastTab);
        }
        let dropped = Pane::of(tab.clone(), Some(tab.clone()));
        let stays = (second_view || active != tab).then(|| active.clone());
        let rest = Pane {
            tabs: if second_view {
                vec![tab.clone()]
            } else {
                Vec::new()
            },
            shown: stays,
        };
        let (panes, rest, focused) = if edge.is_leading() {
            (vec![dropped, rest], 1, 0)
        } else {
            (vec![rest, dropped], 0, 1)
        };
        Ok(Self {
            direction: edge.direction(),
            panes,
            rest,
            focused,
        })
    }

    /// A further split of an existing one: a new pane on `edge` takes `tab`
    /// (moved, or as a second view) and the focus. Refused across the
    /// panes' direction and past [`MAX_PANES`].
    pub fn add(
        &mut self,
        docked: &[K],
        tab: &K,
        edge: DropEdge,
        second_view: bool,
    ) -> Result<(), SplitRefusal> {
        if !docked.contains(tab) {
            return Err(SplitRefusal::NotDocked);
        }
        if edge.direction() != self.direction {
            return Err(SplitRefusal::Crosswise);
        }
        self.prune(docked);
        if self.panes.len() >= MAX_PANES {
            return Err(SplitRefusal::TooManyPanes);
        }
        if second_view {
            // It stays where it shows now: listed, so the new pane's list
            // does not take it out of the rest pane.
            if !self.lists(tab) {
                self.panes[self.rest].tabs.push(tab.clone());
            }
        } else {
            for pane in &mut self.panes {
                pane.tabs.retain(|key| key != tab);
            }
        }
        let at = if edge.is_leading() {
            0
        } else {
            self.panes.len()
        };
        self.panes
            .insert(at, Pane::of(tab.clone(), Some(tab.clone())));
        if at == 0 {
            self.rest += 1;
        }
        self.focused = at;
        self.prune(docked);
        Ok(())
    }

    fn lists(&self, key: &K) -> bool {
        self.panes.iter().any(|pane| pane.tabs.contains(key))
    }

    /// The panes over `docked` (the main window's tabs in strip order), the
    /// rest pane taking every tab no pane lists. A pane with no docked tab
    /// is left out.
    pub fn resolve(&self, docked: &[K]) -> Vec<ShownPane> {
        self.panes
            .iter()
            .enumerate()
            .filter_map(|(index, pane)| {
                let tabs: Vec<usize> = docked
                    .iter()
                    .enumerate()
                    .filter(|(_, key)| {
                        pane.tabs.contains(key) || (index == self.rest && !self.lists(key))
                    })
                    .map(|(at, _)| at)
                    .collect();
                (!tabs.is_empty()).then_some(ShownPane { pane: index, tabs })
            })
            .collect()
    }

    /// Which of `shown` has the focus while the docked tab at `active` is
    /// active: the focused pane when it holds that tab, else the first that
    /// does.
    pub fn focused_of(&self, shown: &[ShownPane], active: usize) -> Option<usize> {
        shown
            .iter()
            .position(|p| p.pane == self.focused && p.tabs.contains(&active))
            .or_else(|| shown.iter().position(|p| p.tabs.contains(&active)))
    }

    /// The docked position each of `shown` draws: the focused one the active
    /// tab, every other the tab it last showed while it still holds it, else
    /// its first.
    pub fn drawn_tabs(
        &self,
        docked: &[K],
        shown: &[ShownPane],
        focused: usize,
        active: usize,
    ) -> Vec<usize> {
        shown
            .iter()
            .enumerate()
            .map(|(n, pane)| {
                if n == focused {
                    return active;
                }
                self.panes[pane.pane]
                    .shown
                    .as_ref()
                    .and_then(|key| docked.iter().position(|k| k == key))
                    .filter(|at| pane.tabs.contains(at))
                    .unwrap_or(pane.tabs[0])
            })
            .collect()
    }

    /// The active tab moved to `active` (a tab strip click, a tab opened):
    /// the pane holding it takes the focus and remembers it, so the pane it
    /// left keeps showing what it showed.
    pub fn follow_active(&mut self, docked: &[K], active: &K) {
        let Some(at) = docked.iter().position(|key| key == active) else {
            return;
        };
        let shown = self.resolve(docked);
        if let Some(n) = self.focused_of(&shown, at) {
            let pane = shown[n].pane;
            self.focused = pane;
            self.panes[pane].shown = Some(active.clone());
        }
    }

    /// A press on shown pane `n`: it takes the focus, and the docked
    /// position of the tab it draws is returned to make active. `None` when
    /// there is no such pane.
    pub fn focus(&mut self, docked: &[K], n: usize, active: usize) -> Option<usize> {
        let shown = self.resolve(docked);
        let focused = self.focused_of(&shown, active).unwrap_or(0);
        let target = *self.drawn_tabs(docked, &shown, focused, active).get(n)?;
        self.focused = shown[n].pane;
        Some(target)
    }

    /// Forget the tabs no longer docked and drop the panes left with
    /// nothing to show. `false` when fewer than two panes are left: the
    /// split is over.
    pub fn prune(&mut self, docked: &[K]) -> bool {
        for pane in &mut self.panes {
            pane.tabs.retain(|key| docked.contains(key));
            if pane.shown.as_ref().is_some_and(|key| !docked.contains(key)) {
                pane.shown = None;
            }
        }
        let keep: Vec<usize> = self.resolve(docked).iter().map(|p| p.pane).collect();
        let (focused, rest) = (self.focused, self.rest);
        let (mut new_focused, mut new_rest) = (0, None);
        let mut panes = Vec::with_capacity(keep.len());
        for (index, pane) in std::mem::take(&mut self.panes).into_iter().enumerate() {
            if !keep.contains(&index) {
                continue;
            }
            if index == focused {
                new_focused = panes.len();
            }
            if index == rest {
                new_rest = Some(panes.len());
            }
            panes.push(pane);
        }
        self.panes = panes;
        self.focused = new_focused;
        // A dropped rest pane held no unlisted tab; the first pane takes
        // the ones opened from now on.
        self.rest = new_rest.unwrap_or(0);
        self.panes.len() >= 2
    }
}
