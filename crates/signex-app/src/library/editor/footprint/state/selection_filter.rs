//! v0.18.13 — Altium-style Selection Filter (12 togglable kinds).

/// Each flag gates whether the corresponding kind is selectable in
/// the canvas. `Pads` is the only one functionally wired today; the
/// others are stored for forward compatibility so the pill row
/// reflects user intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionFilter {
    pub pads: bool,
    pub tracks: bool,
    pub arcs: bool,
    pub pours: bool,
    pub bodies_3d: bool,
    pub keepouts: bool,
    pub cutouts: bool,
    pub texts: bool,
    pub vias: bool,
    pub regions: bool,
    pub fills: bool,
    pub other: bool,
}

impl Default for SelectionFilter {
    fn default() -> Self {
        Self {
            pads: true,
            tracks: true,
            arcs: true,
            pours: true,
            bodies_3d: true,
            keepouts: true,
            cutouts: true,
            texts: true,
            vias: true,
            regions: true,
            fills: true,
            other: true,
        }
    }
}

/// Selection-filter pill identifier — drives the panel pill row +
/// the dispatcher's mutation. Order matches Altium's PCB Library
/// editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionFilterKind {
    Bodies3d,
    Keepouts,
    Tracks,
    Arcs,
    Pads,
    Vias,
    Regions,
    Fills,
    Texts,
    Other,
    Pours,
    Cutouts,
}

impl SelectionFilterKind {
    /// Altium's 10 user-visible pill kinds in display order.
    pub const ALTIUM_PILLS: &'static [SelectionFilterKind] = &[
        Self::Bodies3d,
        Self::Keepouts,
        Self::Tracks,
        Self::Arcs,
        Self::Pads,
        Self::Vias,
        Self::Regions,
        Self::Fills,
        Self::Texts,
        Self::Other,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Bodies3d => "3D Bodies",
            Self::Keepouts => "Keepouts",
            Self::Tracks => "Tracks",
            Self::Arcs => "Arcs",
            Self::Pads => "Pads",
            Self::Vias => "Vias",
            Self::Regions => "Regions",
            Self::Fills => "Fills",
            Self::Texts => "Texts",
            Self::Other => "Other",
            Self::Pours => "Pours",
            Self::Cutouts => "Cutouts",
        }
    }
}

impl SelectionFilter {
    pub fn get(&self, kind: SelectionFilterKind) -> bool {
        match kind {
            SelectionFilterKind::Pads => self.pads,
            SelectionFilterKind::Tracks => self.tracks,
            SelectionFilterKind::Arcs => self.arcs,
            SelectionFilterKind::Pours => self.pours,
            SelectionFilterKind::Bodies3d => self.bodies_3d,
            SelectionFilterKind::Keepouts => self.keepouts,
            SelectionFilterKind::Cutouts => self.cutouts,
            SelectionFilterKind::Texts => self.texts,
            SelectionFilterKind::Vias => self.vias,
            SelectionFilterKind::Regions => self.regions,
            SelectionFilterKind::Fills => self.fills,
            SelectionFilterKind::Other => self.other,
        }
    }

    pub fn toggle(&mut self, kind: SelectionFilterKind) {
        match kind {
            SelectionFilterKind::Pads => self.pads = !self.pads,
            SelectionFilterKind::Tracks => self.tracks = !self.tracks,
            SelectionFilterKind::Arcs => self.arcs = !self.arcs,
            SelectionFilterKind::Pours => self.pours = !self.pours,
            SelectionFilterKind::Bodies3d => self.bodies_3d = !self.bodies_3d,
            SelectionFilterKind::Keepouts => self.keepouts = !self.keepouts,
            SelectionFilterKind::Cutouts => self.cutouts = !self.cutouts,
            SelectionFilterKind::Texts => self.texts = !self.texts,
            SelectionFilterKind::Vias => self.vias = !self.vias,
            SelectionFilterKind::Regions => self.regions = !self.regions,
            SelectionFilterKind::Fills => self.fills = !self.fills,
            SelectionFilterKind::Other => self.other = !self.other,
        }
    }
}
