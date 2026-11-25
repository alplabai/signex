//! The board's layer stack (#2401) and the per-layer copper sizes of a via
//! or pad. Document data: the stack is saved in the `.snxpcb` `[stackup]`
//! section as `rows`, the sizes in the optional `layer_sizes` via and pad
//! column. Changed only through the board engine's `SetLayerStack` and item
//! replace commands. See `docs/PCB_LAYER_STACK.md`.

use serde::{Deserialize, Serialize};

use crate::format::{mm_as_stored_nm, parse_pcb_layer, pcb_layer_token};
use crate::pcb::{LayerDef, PCB_DEFAULT_THICKNESS_MM};
use crate::pcb_layer::{CopperLayer, InnerCopper, PcbLayer};

/// Thickness of a copper row a derived stack starts with: 1 oz copper.
pub const DEFAULT_COPPER_MM: f64 = 0.035;
/// Thickness of a solder mask row a derived stack starts with.
pub const DEFAULT_MASK_MM: f64 = 0.01;
/// Relative permittivity of FR-4, the dielectric of a derived stack.
pub const FR4_DIELECTRIC_CONSTANT: f64 = 4.2;
/// Relative permittivity of a solder mask of a derived stack.
pub const MASK_DIELECTRIC_CONSTANT: f64 = 3.5;
/// Copper weight of a derived copper row, oz/ft².
pub const DEFAULT_COPPER_WEIGHT_OZ: f64 = 1.0;
/// The most copper layers a board can name (`CopperLayer::COUNT`).
pub const MAX_COPPER_LAYERS: usize = CopperLayer::COUNT;

/// What one row of the stack is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StackLayerKind {
    Overlay,
    Paste,
    SolderMask,
    /// Copper that carries tracks.
    Signal,
    /// Copper poured as a plane.
    Plane,
    Dielectric,
}

impl StackLayerKind {
    pub const ALL: [StackLayerKind; 6] = [
        Self::Signal,
        Self::Plane,
        Self::Dielectric,
        Self::SolderMask,
        Self::Paste,
        Self::Overlay,
    ];

    pub fn is_copper(self) -> bool {
        matches!(self, Self::Signal | Self::Plane)
    }

    /// The `layer_type` a copper row gives its board layer.
    pub fn layer_type(self) -> &'static str {
        match self {
            Self::Plane => "plane",
            _ => "signal",
        }
    }
}

/// One row of the stack, top to bottom.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StackLayer {
    pub kind: StackLayerKind,
    /// The name the user gave the row; empty shows a name made from its
    /// kind and place.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// The board layer a copper row is (`TopCopper`, `Inner<n>Copper`,
    /// `BottomCopper`). Empty on every other row, and on a copper row added
    /// in the manager before the edit is applied.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub layer: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub material: String,
    #[serde(default)]
    pub thickness_mm: f64,
    /// Relative permittivity (Er); 0 where it does not apply.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub dielectric_constant: f64,
    /// Copper weight in oz/ft²; 0 for a row that is not copper.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub copper_weight_oz: f64,
    /// Loss tangent (Df), copied from the library material picked for the
    /// row (mockup row 177); 0 while none is known.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub loss_tangent: f64,
}

fn is_zero(value: &f64) -> bool {
    *value == 0.0
}

impl StackLayer {
    /// A new row of `kind` with the values a derived stack gives it.
    pub fn new(kind: StackLayerKind) -> Self {
        let (material, thickness_mm, dielectric_constant, copper_weight_oz) = match kind {
            StackLayerKind::Signal | StackLayerKind::Plane => {
                ("Copper", DEFAULT_COPPER_MM, 0.0, DEFAULT_COPPER_WEIGHT_OZ)
            }
            StackLayerKind::Dielectric => ("FR-4", 0.2, FR4_DIELECTRIC_CONSTANT, 0.0),
            StackLayerKind::SolderMask => (
                "Solder Resist",
                DEFAULT_MASK_MM,
                MASK_DIELECTRIC_CONSTANT,
                0.0,
            ),
            StackLayerKind::Paste | StackLayerKind::Overlay => ("", 0.0, 0.0, 0.0),
        };
        Self {
            kind,
            name: String::new(),
            layer: String::new(),
            material: material.to_string(),
            thickness_mm,
            dielectric_constant,
            copper_weight_oz,
            loss_tangent: 0.0,
        }
    }

    /// Every number of the row is finite and not negative.
    pub fn numbers_valid(&self) -> bool {
        [
            self.thickness_mm,
            self.dielectric_constant,
            self.copper_weight_oz,
            self.loss_tangent,
        ]
        .iter()
        .all(|v| v.is_finite() && *v >= 0.0)
    }
}

/// The physical stack of a board, top to bottom.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct LayerStack {
    /// The stackup's own name (mockup row 105); `None` shows the board's
    /// file name. Set only by the board engine's `SetStackupName`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default)]
    pub layers: Vec<StackLayer>,
    /// The board's solder mask, silkscreen and surface finish (mockup rows
    /// 105 and 176, Finish sub-tab); `None` until the Finish sub-tab is
    /// first edited. Saved as `[stackup.finish]`, under `snxpcb/1.10`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finish: Option<BoardFinish>,
}

/// A typed stackup name as stored: trimmed, `None` when nothing is left.
pub fn clean_stack_name(text: &str) -> Option<String> {
    let name = text.trim();
    (!name.is_empty()).then(|| name.to_string())
}

impl LayerStack {
    /// The sum of every row's thickness, mm.
    pub fn total_thickness_mm(&self) -> f64 {
        self.layers.iter().map(|l| l.thickness_mm).sum()
    }

    /// How many rows are copper.
    pub fn copper_count(&self) -> usize {
        self.layers.iter().filter(|l| l.kind.is_copper()).count()
    }

    /// The copper rows, top to bottom.
    pub fn copper(&self) -> impl Iterator<Item = &StackLayer> {
        self.layers.iter().filter(|l| l.kind.is_copper())
    }

    /// Whether the stack mirrors about its centre line (mockup row 105):
    /// dielectric material and thickness, and copper weight. Names and the
    /// Signal / Plane type are not compared.
    pub fn is_symmetric(&self) -> bool {
        let rows = &self.layers;
        rows.iter()
            .zip(rows.iter().rev())
            .take(rows.len() / 2)
            .all(|(a, b)| rows_mirror(a, b))
    }

    /// The stack a board without a stored one shows: overlay, paste and mask
    /// on each face, its copper layers (planes as the layer list marks
    /// them) with FR-4 between them, sized so the total is the board's
    /// thickness. Two copper layers when the board lists none.
    pub fn derived(layers: &[LayerDef], board_thickness_mm: f64) -> Self {
        let copper = board_copper(layers);
        let total = if board_thickness_mm.is_finite() && board_thickness_mm > 0.0 {
            board_thickness_mm
        } else {
            PCB_DEFAULT_THICKNESS_MM
        };
        let fixed = copper.len() as f64 * DEFAULT_COPPER_MM + 2.0 * DEFAULT_MASK_MM;
        let gaps = copper.len().saturating_sub(1).max(1) as f64;
        let dielectric_mm = ((total - fixed) / gaps).max(0.0);
        let mut rows = vec![
            StackLayer::new(StackLayerKind::Overlay),
            StackLayer::new(StackLayerKind::Paste),
            StackLayer::new(StackLayerKind::SolderMask),
        ];
        for (i, (layer, plane)) in copper.iter().enumerate() {
            if i > 0 {
                rows.push(StackLayer {
                    thickness_mm: dielectric_mm,
                    ..StackLayer::new(StackLayerKind::Dielectric)
                });
            }
            let kind = if *plane {
                StackLayerKind::Plane
            } else {
                StackLayerKind::Signal
            };
            rows.push(StackLayer {
                layer: pcb_layer_token(PcbLayer::Copper(*layer)),
                ..StackLayer::new(kind)
            });
        }
        rows.extend([
            StackLayer::new(StackLayerKind::SolderMask),
            StackLayer::new(StackLayerKind::Paste),
            StackLayer::new(StackLayerKind::Overlay),
        ]);
        Self {
            name: None,
            layers: rows,
            finish: None,
        }
    }

    /// Whether the stack holds data a build before `snxpcb/1.10` would drop
    /// on save: a finish, or a row's loss tangent.
    pub fn has_materials_data(&self) -> bool {
        self.finish.is_some() || self.layers.iter().any(|l| l.loss_tangent != 0.0)
    }

    /// The finish the Finish sub-tab shows: the stored one, else the
    /// default.
    pub fn shown_finish(&self) -> BoardFinish {
        self.finish.clone().unwrap_or_default()
    }
}

/// Whether two rows mirrored about the centre line match.
fn rows_mirror(a: &StackLayer, b: &StackLayer) -> bool {
    const SAME_MM: f64 = 1e-6;
    if a.kind.is_copper() && b.kind.is_copper() {
        return (a.copper_weight_oz - b.copper_weight_oz).abs() < SAME_MM;
    }
    a.kind == b.kind
        && a.material == b.material
        && (a.thickness_mm - b.thickness_mm).abs() < SAME_MM
}

/// The copper layers `layers` lists, top to bottom, and whether each is a
/// plane; Top and Bottom when it lists none.
fn board_copper(layers: &[LayerDef]) -> Vec<(CopperLayer, bool)> {
    let mut copper: Vec<(CopperLayer, bool)> = layers
        .iter()
        .filter_map(|def| {
            let layer = parse_pcb_layer(&def.name)?.copper()?;
            let plane = ["plane", "power"]
                .iter()
                .any(|k| def.layer_type.eq_ignore_ascii_case(k));
            Some((layer, plane))
        })
        .collect();
    copper.sort_by_key(|(layer, _)| *layer);
    copper.dedup_by_key(|(layer, _)| *layer);
    if copper.is_empty() {
        copper = vec![(CopperLayer::Top, false), (CopperLayer::Bottom, false)];
    }
    copper
}

/// The copper layers a board's layer list names, top to bottom; Top and
/// Bottom when it names none. The one rule the layer tabs, the stack and the
/// via span check read.
pub fn board_copper_layers(layers: &[LayerDef]) -> Vec<CopperLayer> {
    board_copper(layers)
        .into_iter()
        .map(|(layer, _)| layer)
        .collect()
}

/// The copper layer at `index` of `count` copper rows, top to bottom: the
/// first is Top, the last Bottom, the rest Inner 1, 2, ... in order. `None`
/// for fewer than two rows, more than the board can name, or an index out
/// of range.
pub fn copper_at(index: usize, count: usize) -> Option<CopperLayer> {
    if !(2..=MAX_COPPER_LAYERS).contains(&count) || index >= count {
        return None;
    }
    if index == 0 {
        Some(CopperLayer::Top)
    } else if index == count - 1 {
        Some(CopperLayer::Bottom)
    } else {
        InnerCopper::new(u8::try_from(index).ok()?).map(CopperLayer::Inner)
    }
}

// ---------------------------------------------------------------------------
// Board finish (mockup rows 105 and 176, Finish sub-tab)
// ---------------------------------------------------------------------------

/// Solder mask thickness over bare laminate a new finish starts with, mm.
pub const DEFAULT_MASK_OVER_LAMINATE_MM: f64 = 0.025;

/// The plating on exposed copper.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceFinish {
    /// Electroless nickel, immersion gold; the gold the 3D view draws.
    #[default]
    Enig,
    Enepig,
    LeadFreeHasl,
    LeadedHasl,
    ImmersionSilver,
    ImmersionTin,
    /// Organic solderability preservative.
    Osp,
    HardGold,
}

impl SurfaceFinish {
    pub const ALL: [Self; 8] = [
        Self::Enig,
        Self::Enepig,
        Self::LeadFreeHasl,
        Self::LeadedHasl,
        Self::ImmersionSilver,
        Self::ImmersionTin,
        Self::Osp,
        Self::HardGold,
    ];

    /// Hot-air levelling leaves an uneven surface: not for fine pitch.
    pub fn is_hasl(self) -> bool {
        matches!(self, Self::LeadFreeHasl | Self::LeadedHasl)
    }
}

/// How the solder mask is applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaskType {
    /// Liquid photoimageable.
    #[default]
    Lpi,
    DryFilm,
}

impl MaskType {
    pub const ALL: [Self; 2] = [Self::Lpi, Self::DryFilm];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaskGloss {
    #[default]
    Glossy,
    Matte,
}

impl MaskGloss {
    pub const ALL: [Self; 2] = [Self::Glossy, Self::Matte];
}

/// The board faces a mask or silkscreen is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishSides {
    #[default]
    Both,
    Top,
    Bottom,
}

impl FinishSides {
    pub const ALL: [Self; 3] = [Self::Both, Self::Top, Self::Bottom];
}

/// A mask or silkscreen colour the fab offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishColor {
    Green,
    Red,
    Blue,
    Black,
    White,
    Yellow,
    Purple,
}

impl FinishColor {
    pub const MASK: [Self; 7] = [
        Self::Green,
        Self::Red,
        Self::Blue,
        Self::Black,
        Self::White,
        Self::Yellow,
        Self::Purple,
    ];
    pub const SILKSCREEN: [Self; 3] = [Self::White, Self::Black, Self::Yellow];
}

/// The solder mask. Its thickness over copper and its Dk are the Solder
/// Mask rows' of the stack, so they have one home. A field the file leaves
/// out takes its [`Default`] value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MaskFinish {
    pub kind: MaskType,
    pub color: FinishColor,
    pub gloss: MaskGloss,
    pub sides: FinishSides,
    /// Thickness over bare laminate, mm.
    pub over_laminate_mm: f64,
}

impl Default for MaskFinish {
    fn default() -> Self {
        Self {
            kind: MaskType::default(),
            color: FinishColor::Green,
            gloss: MaskGloss::default(),
            sides: FinishSides::default(),
            over_laminate_mm: DEFAULT_MASK_OVER_LAMINATE_MM,
        }
    }
}

/// A field the file leaves out takes its [`Default`] value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SilkscreenFinish {
    pub color: FinishColor,
    pub sides: FinishSides,
}

impl Default for SilkscreenFinish {
    fn default() -> Self {
        Self {
            color: FinishColor::White,
            sides: FinishSides::default(),
        }
    }
}

/// The board's mask, silkscreen and surface finish, as the board file
/// stores them.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct BoardFinish {
    #[serde(default)]
    pub surface: SurfaceFinish,
    #[serde(default)]
    pub mask: MaskFinish,
    #[serde(default)]
    pub silkscreen: SilkscreenFinish,
}

impl BoardFinish {
    /// Every number is finite and not negative.
    pub fn numbers_valid(&self) -> bool {
        let mm = self.mask.over_laminate_mm;
        mm.is_finite() && mm >= 0.0
    }
}

// ---------------------------------------------------------------------------
// Per-layer copper sizes
// ---------------------------------------------------------------------------

/// The size of a via or pad on one copper layer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayerSize<T> {
    /// The copper layer's token (`TopCopper`, `Inner<n>Copper`,
    /// `BottomCopper`).
    pub layer: String,
    pub size: T,
}

impl<T> LayerSize<T> {
    pub fn new(layer: CopperLayer, size: T) -> Self {
        Self {
            layer: pcb_layer_token(PcbLayer::Copper(layer)),
            size,
        }
    }

    /// The copper layer named, or `None` for a token that is not copper.
    pub fn copper(&self) -> Option<CopperLayer> {
        parse_pcb_layer(&self.layer)?.copper()
    }
}

/// Copper sizes of a via (a diameter) or pad (an x/y size) that differ by
/// layer (#2401). An item without them has one size on every layer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum LayerSizes<T> {
    /// One size on the top layer, one on every inner layer, one on the
    /// bottom layer.
    TopMiddleBottom { top: T, middle: T, bottom: T },
    /// A size per listed copper layer; a layer not listed keeps the item's
    /// own size.
    Full { layers: Vec<LayerSize<T>> },
}

impl<T: Copy> LayerSizes<T> {
    /// The size on `layer`; `None` where the item keeps its own size.
    pub fn on(&self, layer: CopperLayer) -> Option<T> {
        match self {
            Self::TopMiddleBottom {
                top,
                middle,
                bottom,
            } => Some(match layer {
                CopperLayer::Top => *top,
                CopperLayer::Inner(_) => *middle,
                CopperLayer::Bottom => *bottom,
            }),
            Self::Full { layers } => layers
                .iter()
                .find(|l| l.copper() == Some(layer))
                .map(|l| l.size),
        }
    }

    /// The size on the layer named `token`; `None` for a name that is not
    /// copper, or where the item keeps its own size.
    pub fn on_token(&self, token: &str) -> Option<T> {
        self.on(parse_pcb_layer(token)?.copper()?)
    }

    /// Every size the item lists.
    pub fn sizes(&self) -> Vec<T> {
        match self {
            Self::TopMiddleBottom {
                top,
                middle,
                bottom,
            } => vec![*top, *middle, *bottom],
            Self::Full { layers } => layers.iter().map(|l| l.size).collect(),
        }
    }
}

/// Why per-layer sizes cannot be saved: the list rule the board engine and
/// the `.snxpcb` writer share.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayerSizesFault {
    /// A full list that names no layer.
    NoLayers,
    /// An entry whose name is not a copper layer.
    NotCopper(String),
    /// A copper layer named by two entries.
    NamedTwice(String),
}

impl<T> LayerSizes<T> {
    /// What keeps the list from being saved; `None` for one that saves.
    /// The sizes themselves are checked with [`size_storable`].
    pub fn list_fault(&self) -> Option<LayerSizesFault> {
        let Self::Full { layers } = self else {
            return None;
        };
        if layers.is_empty() {
            return Some(LayerSizesFault::NoLayers);
        }
        for (i, entry) in layers.iter().enumerate() {
            if parse_pcb_layer(&entry.layer)
                .and_then(PcbLayer::copper)
                .is_none()
            {
                return Some(LayerSizesFault::NotCopper(entry.layer.clone()));
            }
            if layers[..i].iter().any(|l| l.layer == entry.layer) {
                return Some(LayerSizesFault::NamedTwice(entry.layer.clone()));
            }
        }
        None
    }
}

/// Whether `mm` is a per-layer copper size the file can hold: finite and at
/// least 1 nm once rounded to the whole nm it is saved as.
pub fn size_storable(mm: f64) -> bool {
    mm_as_stored_nm(mm).is_some_and(|nm| nm > 0)
}

/// How a via or pad sizes its copper by layer: the three choices of its
/// Properties stack row (#2401).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StackMode {
    /// One size on every layer.
    Simple,
    TopMiddleBottom,
    /// A size per copper layer.
    Full,
}

impl<T: Copy> LayerSizes<T> {
    pub fn mode(&self) -> StackMode {
        match self {
            Self::TopMiddleBottom { .. } => StackMode::TopMiddleBottom,
            Self::Full { .. } => StackMode::Full,
        }
    }

    /// Set the size on `layer`: in top/middle/bottom form, the size of the
    /// part `layer` is in; in full form, `layer`'s own entry.
    pub fn set(&mut self, layer: CopperLayer, size: T) {
        match self {
            Self::TopMiddleBottom {
                top,
                middle,
                bottom,
            } => match layer {
                CopperLayer::Top => *top = size,
                CopperLayer::Inner(_) => *middle = size,
                CopperLayer::Bottom => *bottom = size,
            },
            Self::Full { layers } => match layers.iter_mut().find(|l| l.copper() == Some(layer)) {
                Some(entry) => entry.size = size,
                None => layers.push(LayerSize::new(layer, size)),
            },
        }
    }
}

/// `sizes` in `mode`: each layer keeps the size it had, `own` where it had
/// none. `copper` is the board's copper, top to bottom, the layers a full
/// stack lists. `None` is [`StackMode::Simple`].
pub fn sizes_in_mode<T: Copy>(
    sizes: Option<&LayerSizes<T>>,
    own: T,
    mode: StackMode,
    copper: &[CopperLayer],
) -> Option<LayerSizes<T>> {
    let on = |layer: CopperLayer| sizes.and_then(|s| s.on(layer)).unwrap_or(own);
    let middle = copper
        .iter()
        .copied()
        .find(|l| matches!(l, CopperLayer::Inner(_)))
        .map_or(own, on);
    match mode {
        StackMode::Simple => None,
        StackMode::TopMiddleBottom => Some(LayerSizes::TopMiddleBottom {
            top: on(CopperLayer::Top),
            middle,
            bottom: on(CopperLayer::Bottom),
        }),
        StackMode::Full => Some(LayerSizes::Full {
            layers: copper.iter().map(|&l| LayerSize::new(l, on(l))).collect(),
        }),
    }
}

/// Per-layer sizes written as text (an agent's operand): empty or `simple`
/// for one size, `top=..;middle=..;bottom=..`, or `<copper layer>=..;..`
/// with each size read by `value`.
pub fn parse_layer_sizes<T>(
    text: &str,
    value: impl Fn(&str) -> Option<T>,
) -> Result<Option<LayerSizes<T>>, String> {
    let text = text.trim();
    if text.is_empty() || text.eq_ignore_ascii_case("simple") {
        return Ok(None);
    }
    let mut entries = Vec::new();
    for part in text.split(';').map(str::trim).filter(|p| !p.is_empty()) {
        let (key, raw) = part
            .split_once('=')
            .ok_or_else(|| format!("`{part}` is not <layer>=<size>"))?;
        let size = value(raw.trim()).ok_or_else(|| format!("`{raw}` is not a size"))?;
        entries.push((key.trim().to_string(), size));
    }
    let keys: Vec<&str> = entries.iter().map(|(k, _)| k.as_str()).collect();
    if keys == ["top", "middle", "bottom"] {
        let mut sizes = entries.into_iter().map(|(_, s)| s);
        let (Some(top), Some(middle), Some(bottom)) = (sizes.next(), sizes.next(), sizes.next())
        else {
            return Err("top, middle and bottom each need a size".to_string());
        };
        return Ok(Some(LayerSizes::TopMiddleBottom {
            top,
            middle,
            bottom,
        }));
    }
    let layers = entries
        .into_iter()
        .map(|(layer, size)| LayerSize { layer, size })
        .collect();
    let sizes = LayerSizes::Full { layers };
    match sizes.list_fault() {
        None => Ok(Some(sizes)),
        Some(LayerSizesFault::NoLayers) => Err("no copper layer is named".to_string()),
        Some(LayerSizesFault::NotCopper(key)) => Err(format!("{key} is not a copper layer")),
        Some(LayerSizesFault::NamedTwice(key)) => Err(format!("{key} is named twice")),
    }
}

#[cfg(test)]
#[path = "pcb_stack_tests.rs"]
mod tests;
