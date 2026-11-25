//! What a board view draws differently from the board palette (#2363): the
//! colour the user picked for a layer, and how transparent each kind of
//! object is drawn. State of one view, never written to the `.snxpcb` (the
//! app keeps it per board in the user's preferences, #2480); the colours
//! are overrides on top of the one board palette.

use signex_types::pcb_layer::{PcbLayer, TechnicalLayer};

/// A kind of object on a board, as the View Options tab lists them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BoardObject {
    Pads,
    Vias,
    Arcs,
    Tracks,
    Polygons,
    Regions,
    Fills,
    Texts,
    Rooms,
    Dimensions,
    Keepouts,
    Coordinates,
    Body3d,
}

impl BoardObject {
    pub const ALL: [BoardObject; 13] = [
        Self::Pads,
        Self::Vias,
        Self::Arcs,
        Self::Tracks,
        Self::Polygons,
        Self::Regions,
        Self::Fills,
        Self::Texts,
        Self::Rooms,
        Self::Dimensions,
        Self::Keepouts,
        Self::Coordinates,
        Self::Body3d,
    ];

    /// The token a command operand names the kind by.
    pub fn token(self) -> &'static str {
        match self {
            Self::Pads => "pads",
            Self::Vias => "vias",
            Self::Arcs => "arcs",
            Self::Tracks => "tracks",
            Self::Polygons => "polygons",
            Self::Regions => "regions",
            Self::Fills => "fills",
            Self::Texts => "texts",
            Self::Rooms => "rooms",
            Self::Dimensions => "dimensions",
            Self::Keepouts => "keepouts",
            Self::Coordinates => "coordinates",
            Self::Body3d => "body_3d",
        }
    }

    /// The kind `token` names, ignoring case.
    pub fn from_token(token: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|kind| token.trim().eq_ignore_ascii_case(kind.token()))
    }

    /// Whether the board view can hide this kind: the board draws it.
    pub fn can_hide(self) -> bool {
        matches!(
            self,
            Self::Pads
                | Self::Vias
                | Self::Arcs
                | Self::Tracks
                | Self::Polygons
                | Self::Regions
                | Self::Fills
                | Self::Texts
                | Self::Dimensions
                | Self::Keepouts
        )
    }

    /// Whether the board view can draw this kind transparent: the copper
    /// it draws in its scene (copper arcs too; drawn arcs on other layers
    /// stay opaque).
    pub fn can_fade(self) -> bool {
        matches!(
            self,
            Self::Pads
                | Self::Vias
                | Self::Arcs
                | Self::Tracks
                | Self::Polygons
                | Self::Regions
                | Self::Fills
        )
    }

    fn index(self) -> usize {
        self as usize
    }
}

/// The colours and transparencies one board view draws with.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BoardPaint {
    /// A layer the user gave its own colour, opaque RGB.
    colours: Vec<(PcbLayer, [u8; 3])>,
    /// Per [`BoardObject`], in percent: 0 opaque, 100 invisible.
    transparency: [u8; BoardObject::ALL.len()],
    /// Show only unrouted is on (mockup row 164): the board is drawn faint
    /// under its ratsnest. Set on a snapshot's paint only.
    board_dimmed: bool,
    /// A highlighted net (#2514): copper on every other net is drawn
    /// dimmed by the Preferences ▸ PCB ▸ Display amount. Snapshot only.
    net_focus: Option<NetFocus>,
}

/// The net drawn at full brightness, and how much the rest of the copper
/// dims, percent: 0 not at all, 100 invisible.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NetFocus {
    pub net: u32,
    pub dim_percent: u8,
}

impl BoardPaint {
    /// This paint with copper off `net` dimmed by `dim_percent` (clamped
    /// to 100 %), or (`None`) with no net in focus.
    pub fn with_net_focus(mut self, net: Option<u32>, dim_percent: u8) -> Self {
        self.net_focus = net.map(|net| NetFocus {
            net,
            dim_percent: dim_percent.min(100),
        });
        self
    }

    /// The net in focus, if any.
    pub fn net_focus(&self) -> Option<NetFocus> {
        self.net_focus
    }

    /// The alpha factor copper on `net` is drawn with: 1 with no net in
    /// focus or on the net in focus, else what the dim amount leaves.
    pub fn net_opacity(&self, net: u32) -> f32 {
        match self.net_focus {
            Some(focus) if focus.net != net => 1.0 - f32::from(focus.dim_percent) / 100.0,
            _ => 1.0,
        }
    }

    /// This paint with the board drawn faint under its ratsnest, or not.
    pub fn with_board_dimmed(mut self, dimmed: bool) -> Self {
        self.board_dimmed = dimmed;
        self
    }

    /// Whether the board is drawn faint under its ratsnest.
    pub fn is_board_dimmed(&self) -> bool {
        self.board_dimmed
    }

    /// Whether the board view draws everything on `layer` in the colour
    /// given to it. Not yet for the keep-out layers: the board's keepout
    /// regions keep the palette colour.
    pub fn can_recolour(layer: PcbLayer) -> bool {
        !matches!(
            layer,
            PcbLayer::Technical(TechnicalLayer::KeepoutTop | TechnicalLayer::KeepoutBottom)
        )
    }

    /// The colour the user picked for `layer`, if any.
    pub fn colour(&self, layer: PcbLayer) -> Option<[u8; 3]> {
        self.colours
            .iter()
            .find(|(l, _)| *l == layer)
            .map(|(_, c)| *c)
    }

    /// Give `layer` its own colour, or (`None`) the palette's again.
    pub fn set_colour(&mut self, layer: PcbLayer, colour: Option<[u8; 3]>) {
        self.colours.retain(|(l, _)| *l != layer);
        if let Some(colour) = colour {
            self.colours.push((layer, colour));
        }
    }

    /// The transparency of `kind`, percent.
    pub fn transparency(&self, kind: BoardObject) -> u8 {
        self.transparency[kind.index()]
    }

    /// Set the transparency of `kind`, clamped to 100 %.
    pub fn set_transparency(&mut self, kind: BoardObject, percent: u8) {
        self.transparency[kind.index()] = percent.min(100);
    }

    /// The alpha factor `kind` is drawn with: 1 opaque, 0 invisible.
    pub fn opacity(&self, kind: BoardObject) -> f32 {
        1.0 - f32::from(self.transparency(kind)) / 100.0
    }

    /// Whether this paint draws exactly as the palette does.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::pcb_layer::CopperLayer;

    #[test]
    fn every_kind_round_trips_its_token() {
        for kind in BoardObject::ALL {
            assert_eq!(BoardObject::from_token(kind.token()), Some(kind));
        }
        assert_eq!(
            BoardObject::from_token(" Tracks "),
            Some(BoardObject::Tracks)
        );
        assert_eq!(BoardObject::from_token("wires"), None);
    }

    #[test]
    fn a_colour_is_set_replaced_and_cleared() {
        let top = PcbLayer::Copper(CopperLayer::Top);
        let mut paint = BoardPaint::default();
        paint.set_colour(top, Some([1, 2, 3]));
        paint.set_colour(top, Some([4, 5, 6]));
        assert_eq!(paint.colour(top), Some([4, 5, 6]));
        paint.set_colour(top, None);
        assert!(paint.is_default());
    }

    #[test]
    fn transparency_is_clamped_and_becomes_an_alpha() {
        let mut paint = BoardPaint::default();
        assert_eq!(paint.opacity(BoardObject::Tracks), 1.0);
        paint.set_transparency(BoardObject::Tracks, 250);
        assert_eq!(paint.transparency(BoardObject::Tracks), 100);
        paint.set_transparency(BoardObject::Tracks, 25);
        assert!((paint.opacity(BoardObject::Tracks) - 0.75).abs() < 1e-6);
    }

    /// #2514: the net in focus stays opaque; every other net, the unnamed
    /// net 0 too, keeps what the dim amount leaves.
    #[test]
    fn a_net_in_focus_dims_the_other_nets_only() {
        assert_eq!(BoardPaint::default().net_opacity(7), 1.0);
        for (percent, other) in [(0, 1.0), (50, 0.5), (100, 0.0)] {
            let paint = BoardPaint::default().with_net_focus(Some(3), percent);
            assert_eq!(paint.net_opacity(3), 1.0, "{percent} %");
            assert!((paint.net_opacity(4) - other).abs() < 1e-6, "{percent} %");
            assert!((paint.net_opacity(0) - other).abs() < 1e-6, "{percent} %");
        }
        let clamped = BoardPaint::default().with_net_focus(Some(3), 250);
        assert_eq!(clamped.net_focus().map(|f| f.dim_percent), Some(100));
        assert!(BoardPaint::default().with_net_focus(None, 50).is_default());
    }
}
