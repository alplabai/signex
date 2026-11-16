//! The settings of a [`BoardPanel`] one at a time: what the Properties
//! panel's Panel section edits and what `set_board_panel field=.. value=..`
//! names.

use super::{BoardPanel, PanelError, PanelSeparation};

/// One setting of a panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PanelField {
    Columns,
    Rows,
    SpacingX,
    SpacingY,
    RailTop,
    RailBottom,
    RailLeft,
    RailRight,
    FiducialDiameter,
    ToolingHoleDiameter,
    Separation,
    BiteHoleDiameter,
    BitePitch,
    TabWidth,
    TabsPerEdge,
}

/// What a setting holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelFieldKind {
    /// A whole number.
    Count,
    /// A length in mm.
    Length,
    /// A [`PanelSeparation`].
    Separation,
}

impl PanelField {
    pub const ALL: [Self; 15] = [
        Self::Columns,
        Self::Rows,
        Self::SpacingX,
        Self::SpacingY,
        Self::RailTop,
        Self::RailBottom,
        Self::RailLeft,
        Self::RailRight,
        Self::FiducialDiameter,
        Self::ToolingHoleDiameter,
        Self::Separation,
        Self::BiteHoleDiameter,
        Self::BitePitch,
        Self::TabWidth,
        Self::TabsPerEdge,
    ];

    /// The name a command operand and an answer use.
    pub fn id(self) -> &'static str {
        match self {
            Self::Columns => "columns",
            Self::Rows => "rows",
            Self::SpacingX => "spacing_x",
            Self::SpacingY => "spacing_y",
            Self::RailTop => "rail_top",
            Self::RailBottom => "rail_bottom",
            Self::RailLeft => "rail_left",
            Self::RailRight => "rail_right",
            Self::FiducialDiameter => "fiducial_diameter",
            Self::ToolingHoleDiameter => "tooling_hole_diameter",
            Self::Separation => "separation",
            Self::BiteHoleDiameter => "bite_hole_diameter",
            Self::BitePitch => "bite_pitch",
            Self::TabWidth => "tab_width",
            Self::TabsPerEdge => "tabs_per_edge",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.id() == id)
    }

    pub fn kind(self) -> PanelFieldKind {
        match self {
            Self::Columns | Self::Rows | Self::TabsPerEdge => PanelFieldKind::Count,
            Self::Separation => PanelFieldKind::Separation,
            _ => PanelFieldKind::Length,
        }
    }

    fn count_slot(self, panel: &mut BoardPanel) -> Option<&mut u32> {
        match self {
            Self::Columns => Some(&mut panel.columns),
            Self::Rows => Some(&mut panel.rows),
            Self::TabsPerEdge => Some(&mut panel.bites.tabs_per_edge),
            _ => None,
        }
    }

    fn length_slot(self, panel: &mut BoardPanel) -> Option<&mut f64> {
        match self {
            Self::SpacingX => Some(&mut panel.spacing_x),
            Self::SpacingY => Some(&mut panel.spacing_y),
            Self::RailTop => Some(&mut panel.rails.top),
            Self::RailBottom => Some(&mut panel.rails.bottom),
            Self::RailLeft => Some(&mut panel.rails.left),
            Self::RailRight => Some(&mut panel.rails.right),
            Self::FiducialDiameter => Some(&mut panel.fiducial_diameter),
            Self::ToolingHoleDiameter => Some(&mut panel.tooling_hole_diameter),
            Self::BiteHoleDiameter => Some(&mut panel.bites.hole_diameter),
            Self::BitePitch => Some(&mut panel.bites.pitch),
            Self::TabWidth => Some(&mut panel.bites.tab_width),
            _ => None,
        }
    }

    /// The whole number this setting holds; `None` for another kind.
    pub fn count(self, panel: &BoardPanel) -> Option<u32> {
        let mut copy = *panel;
        self.count_slot(&mut copy).map(|n| *n)
    }

    /// The length this setting holds, mm; `None` for another kind.
    pub fn length_mm(self, panel: &BoardPanel) -> Option<f64> {
        let mut copy = *panel;
        self.length_slot(&mut copy).map(|mm| *mm)
    }

    /// Set a whole-number setting; whether it changed.
    pub fn set_count(self, panel: &mut BoardPanel, value: u32) -> bool {
        self.count_slot(panel)
            .is_some_and(|slot| std::mem::replace(slot, value) != value)
    }

    /// Set a length setting, mm; whether it changed.
    pub fn set_length(self, panel: &mut BoardPanel, mm: f64) -> bool {
        self.length_slot(panel)
            .is_some_and(|slot| std::mem::replace(slot, mm) != mm)
    }

    /// Set this setting from text: a whole number, a length in mm, or a
    /// separation token (`mouse_bites`, `v_score`). Whether it changed; the
    /// panel's own rules are checked by [`BoardPanel::check`], not here.
    pub fn set_text(self, panel: &mut BoardPanel, text: &str) -> Result<bool, PanelError> {
        let text = text.trim();
        let bad = || PanelError::BadValue {
            field: self.id(),
            value: text.to_string(),
        };
        match self.kind() {
            PanelFieldKind::Count => {
                let value: u32 = text.parse().map_err(|_| bad())?;
                Ok(self.set_count(panel, value))
            }
            PanelFieldKind::Length => {
                let mm: f64 = text.parse().map_err(|_| bad())?;
                if !mm.is_finite() {
                    return Err(bad());
                }
                Ok(self.set_length(panel, mm))
            }
            PanelFieldKind::Separation => {
                let separation = PanelSeparation::from_token(text).ok_or_else(bad)?;
                Ok(std::mem::replace(&mut panel.separation, separation) != separation)
            }
        }
    }
}
