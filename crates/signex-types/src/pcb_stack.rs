//! The board's layer stack (#2401) and the per-layer copper sizes of a via
//! or pad. Document data: the stack is saved in the `.snxpcb` `[stackup]`
//! section as `rows`, the sizes in the optional `layer_sizes` via and pad
//! column. Changed only through the board engine's `SetLayerStack` and item
//! replace commands. See `docs/PCB_LAYER_STACK.md`.

use serde::{Deserialize, Serialize};

use crate::format::{parse_pcb_layer, pcb_layer_token};
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
        }
    }

    /// Every number of the row is finite and not negative.
    pub fn numbers_valid(&self) -> bool {
        [
            self.thickness_mm,
            self.dielectric_constant,
            self.copper_weight_oz,
        ]
        .iter()
        .all(|v| v.is_finite() && *v >= 0.0)
    }
}

/// The physical stack of a board, top to bottom.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct LayerStack {
    #[serde(default)]
    pub layers: Vec<StackLayer>,
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
        Self { layers: rows }
    }
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

#[cfg(test)]
#[path = "pcb_stack_tests.rs"]
mod tests;
