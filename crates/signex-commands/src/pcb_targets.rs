//! What the board placement commands could act on, for enablement.

/// What the placement commands could act on in a window, for enablement.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct PcbTargets {
    /// The window shows a board.
    pub shown: bool,
    /// Something on it is selected.
    pub any: bool,
    /// The selection names footprints and none is locked.
    pub placeable: bool,
    /// The selection names something to delete and no locked footprint,
    /// track or via.
    pub deletable: bool,
    /// The selection names something Copy carries (#2361).
    pub copyable: bool,
    /// The clipboard holds board objects to paste (#2361).
    pub pasteable: bool,
    /// A library part is on the cursor, for Space and the flip (#2364).
    pub part_in_hand: bool,
    /// A track run is being drawn, for Space and Shift+Space (#2373).
    pub routing: bool,
    /// The selection holds an unlocked track or via (#2407).
    pub lockable: bool,
    /// The selection holds a locked track or via.
    pub unlockable: bool,
    /// The board was written by a newer Signex and is shown for reading
    /// only: no editing command acts on it (#2405).
    pub read_only: bool,
    /// How many footprints the selection names, for Align (#2405).
    pub footprints: usize,
    /// The selection is two pads of one unlocked footprint: what Swap Pins
    /// and Swap Gates act on (#2410).
    pub pad_pair: bool,
    /// The selection names one unlocked footprint: the anchor a reuse block
    /// is placed around (#2410).
    pub one_footprint: bool,
    /// The selection holds a track or a via: what the route tools (Gloss
    /// Selected Routes, Add Teardrops) act on in the board menu (2026-11-18).
    pub routes: bool,
    /// Something is under the pointer: the board menu offers what acts on
    /// an object, not only the empty-area rows (2026-11-18).
    pub hovered: bool,
    /// The selection, else what is under the pointer, has a net: what
    /// Select Net and Highlight Net act on (2026-11-18).
    pub net: bool,
}
