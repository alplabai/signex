//! The Layer Stack manager of a board (#2401, mockup row 105), drawn as a
//! document tab (`app/layer_stack_tab`). A pure view over facts the app copies
//! from the board (`app/handlers/pcb_layer_stack`); every edit is a
//! [`LayerStackMsg`] the app turns into one `SetLayerStack` board step.

mod drills;
mod page;
mod section;
mod table;
mod toolbar;

pub use page::view;

use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;

use signex_i18n::{I18n, Localized, MessageId, ids, t};
use signex_types::pcb_stack::{LayerStack, StackLayerKind};

use super::{PanelMsg, form_look};
use crate::styles::{Spacing, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Micrometres in a millimetre.
const UM_PER_MM: f64 = 1000.0;
/// Millimetres in a mil.
const MM_PER_MIL: f64 = 0.0254;

/// A typed cell of the table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StackField {
    Name,
    Material,
    Thickness,
    DielectricConstant,
    CopperWeight,
}

impl StackField {
    /// A cell typed as text, not as a number.
    pub fn is_text(self) -> bool {
        matches!(self, Self::Name | Self::Material)
    }
}

/// The unit the tab shows and reads lengths in. The board stays in mm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StackUnit {
    #[default]
    Mm,
    Um,
    Mil,
}

impl StackUnit {
    pub const ALL: [Self; 3] = [Self::Mm, Self::Um, Self::Mil];

    /// `mm` in this unit.
    pub fn from_mm(self, mm: f64) -> f64 {
        match self {
            Self::Mm => mm,
            Self::Um => mm * UM_PER_MM,
            Self::Mil => mm / MM_PER_MIL,
        }
    }

    /// `value` in this unit, in mm.
    pub fn to_mm(self, value: f64) -> f64 {
        match self {
            Self::Mm => value,
            Self::Um => value / UM_PER_MM,
            Self::Mil => value * MM_PER_MIL,
        }
    }

    fn message(self) -> MessageId {
        match self {
            Self::Mm => ids::PCB_STACK_UNIT_MM,
            Self::Um => ids::PCB_STACK_UNIT_UM,
            Self::Mil => ids::PCB_STACK_UNIT_MIL,
        }
    }

    /// The unit's catalog name.
    pub fn label(self, i18n: &I18n) -> Localized {
        t!(i18n, self.message())
    }

    /// `mm` as a cell shows it in this unit.
    pub fn show(self, mm: f64) -> String {
        super::pcb_primitive_properties::format_number(self.from_mm(mm))
    }
}

/// The sub-tab in view. The other sub-tabs of mockup row 105 are not drawn:
/// Finish, Materials and Templates are #2798; Impedance and Back Drill wait
/// until after the release (DECISIONS, 2026-11-22).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StackSubTab {
    #[default]
    Stack,
    Drills,
}

impl StackSubTab {
    pub const ALL: [Self; 2] = [Self::Stack, Self::Drills];

    fn message(self) -> MessageId {
        match self {
            Self::Stack => ids::PCB_STACK_SUB_STACK,
            Self::Drills => ids::PCB_STACK_SUB_DRILLS,
        }
    }
}

/// An event of the Layer Stack manager. Each names the board it was shown
/// for, so a tab change in between cannot send the edit to another board.
#[derive(Debug, Clone, PartialEq)]
pub enum LayerStackMsg {
    RowSelected {
        board: PathBuf,
        row: usize,
    },
    FieldTyped {
        board: PathBuf,
        row: usize,
        field: StackField,
        text: String,
    },
    /// Enter in a cell: its draft is applied.
    FieldSubmitted {
        board: PathBuf,
        row: usize,
        field: StackField,
    },
    KindPicked {
        board: PathBuf,
        row: usize,
        kind: StackLayerKind,
    },
    /// A layer of `kind` added below the selected row.
    LayerAdded {
        board: PathBuf,
        kind: StackLayerKind,
    },
    /// The selected row removed.
    RowRemoved {
        board: PathBuf,
    },
    /// The selected row moved one place up or down.
    RowMoved {
        board: PathBuf,
        up: bool,
    },
    /// Text typed into the toolbar's stackup name.
    StackupNameTyped {
        board: PathBuf,
        text: String,
    },
    /// Enter in the stackup name: its draft is applied.
    StackupNameSubmitted {
        board: PathBuf,
    },
    /// The unit lengths are shown and typed in.
    UnitPicked {
        board: PathBuf,
        unit: StackUnit,
    },
    SubTabPicked {
        board: PathBuf,
        tab: StackSubTab,
    },
}

/// One row of the Drill pairs table.
#[derive(Debug, Clone, PartialEq)]
pub struct DrillPairRow {
    /// "Through 1–4".
    pub name: String,
    pub from: String,
    pub to: String,
    pub kind: String,
    /// The 1-based copper places the pair runs between.
    pub span: (usize, usize),
    pub depth_mm: Option<f64>,
    pub min_drill_mm: Option<f64>,
    pub aspect_ratio: Option<f64>,
    pub vias: usize,
}

/// One row of the Via types table.
#[derive(Debug, Clone, PartialEq)]
pub struct ViaTypeRow {
    pub name: String,
    pub pair: String,
    pub drill_mm: f64,
    pub finished_mm: f64,
    pub pad_mm: f64,
    pub protection: String,
    pub used: usize,
}

/// What sits on the board layer of one copper row.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CopperUse {
    /// Items on the layer, as Remove counts them.
    pub items: usize,
    /// The nets of the pours on the layer, sorted, each once.
    pub nets: Vec<String>,
}

/// What the manager shows of the board in view.
#[derive(Debug, Clone, PartialEq)]
pub struct LayerStackFacts {
    pub board: PathBuf,
    /// The stored stack, or the one derived from the board's layers.
    pub stack: LayerStack,
    /// The board stores its stack; `false` shows the derived one.
    pub stored: bool,
    pub read_only: bool,
    /// The name each row shows: its own, else its layer's or kind's.
    pub names: Vec<String>,
    pub selected: Option<usize>,
    /// Text typed into a cell and not applied yet.
    pub drafts: BTreeMap<(usize, StackField), String>,
    /// Text typed into the stackup name and not applied yet.
    pub name_draft: Option<String>,
    pub unit: StackUnit,
    pub sub_tab: StackSubTab,
    pub drill_pairs: Vec<DrillPairRow>,
    pub via_types: Vec<ViaTypeRow>,
    /// The board belongs to a project, whose Rules hold the fab profile.
    pub has_project: bool,
    /// Per row, what sits on a copper row's layer; `None` on other rows.
    pub copper_use: Vec<Option<CopperUse>>,
    /// The project's fab profile, as named; `None` when it names none.
    pub fab_profile: Option<String>,
}

/// A copper kind in a row's Type dropdown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KindChoice {
    pub kind: StackLayerKind,
    pub label: String,
}

impl fmt::Display for KindChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

/// The catalog name of a row kind.
pub fn kind_label(i18n: &I18n, kind: StackLayerKind) -> Localized {
    let id = match kind {
        StackLayerKind::Signal => ids::PCB_STACK_KIND_SIGNAL,
        StackLayerKind::Plane => ids::PCB_STACK_KIND_PLANE,
        StackLayerKind::Dielectric => ids::PCB_STACK_KIND_DIELECTRIC,
        StackLayerKind::SolderMask => ids::PCB_STACK_KIND_SOLDER_MASK,
        StackLayerKind::Paste => ids::PCB_STACK_KIND_PASTE,
        StackLayerKind::Overlay => ids::PCB_STACK_KIND_OVERLAY,
    };
    t!(i18n, id)
}

fn msg(m: LayerStackMsg) -> PanelMsg {
    PanelMsg::LayerStack(m)
}

/// The 1-based copper place of each row, top to bottom (the # column and a
/// drill pair's span count it); `None` on a row that is not copper.
fn copper_places(stack: &LayerStack) -> Vec<Option<usize>> {
    let mut place = 0;
    stack
        .layers
        .iter()
        .map(|layer| {
            layer.kind.is_copper().then(|| {
                place += 1;
                place
            })
        })
        .collect()
}

/// The spans of the drill pairs that start or end at the copper `place`.
fn spans_at(pairs: &[DrillPairRow], place: usize) -> Vec<(usize, usize)> {
    pairs
        .iter()
        .map(|pair| pair.span)
        .filter(|&(from, to)| from == place || to == place)
        .collect()
}

/// Each signal row with the plane row nearest it in the stack, the upper
/// one on a tie; `None` when the stack has no plane.
fn layer_pairs(stack: &LayerStack) -> Vec<(usize, Option<usize>)> {
    let rows_of = |kind| {
        stack
            .layers
            .iter()
            .enumerate()
            .filter(move |(_, layer)| layer.kind == kind)
            .map(|(row, _)| row)
    };
    let planes: Vec<usize> = rows_of(StackLayerKind::Plane).collect();
    rows_of(StackLayerKind::Signal)
        .map(|row| {
            let nearest = planes.iter().copied().min_by_key(|p| p.abs_diff(row));
            (row, nearest)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_length_reads_back_in_every_unit() {
        for unit in StackUnit::ALL {
            let back = unit.to_mm(unit.from_mm(0.035));
            assert!((back - 0.035).abs() < 1e-12, "{unit:?}");
        }
    }

    /// Mask, Top signal, dielectric, GND plane, dielectric, plane, dielectric,
    /// Bottom signal, mask: the mockup's four copper layers.
    fn four_layer() -> LayerStack {
        use StackLayerKind::{Dielectric, Plane, Signal, SolderMask};
        let kinds = [
            SolderMask, Signal, Dielectric, Plane, Dielectric, Plane, Dielectric, Signal,
            SolderMask,
        ];
        LayerStack {
            name: None,
            layers: kinds
                .into_iter()
                .map(signex_types::pcb_stack::StackLayer::new)
                .collect(),
        }
    }

    fn pair(span: (usize, usize)) -> DrillPairRow {
        DrillPairRow {
            name: String::new(),
            from: String::new(),
            to: String::new(),
            kind: String::new(),
            span,
            depth_mm: None,
            min_drill_mm: None,
            aspect_ratio: None,
            vias: 0,
        }
    }

    #[test]
    fn copper_rows_are_numbered_top_to_bottom_and_others_are_not() {
        let places = copper_places(&four_layer());
        assert_eq!(
            places,
            [
                None,
                Some(1),
                None,
                Some(2),
                None,
                Some(3),
                None,
                Some(4),
                None
            ]
        );
    }

    #[test]
    fn a_copper_place_lists_the_pairs_that_start_or_end_on_it() {
        let pairs = [pair((1, 4)), pair((1, 2)), pair((2, 3))];
        assert_eq!(spans_at(&pairs, 1), [(1, 4), (1, 2)]);
        assert_eq!(spans_at(&pairs, 2), [(1, 2), (2, 3)]);
        assert_eq!(spans_at(&pairs, 4), [(1, 4)]);
        assert!(spans_at(&pairs[..1], 2).is_empty(), "a pair passing by");
    }

    #[test]
    fn each_signal_layer_pairs_with_its_nearest_plane() {
        let pairs = layer_pairs(&four_layer());
        assert_eq!(pairs, [(1, Some(3)), (7, Some(5))]);
    }

    #[test]
    fn a_stack_without_a_plane_pairs_with_nothing() {
        let stack = LayerStack::derived(&[], 1.6);
        let pairs = layer_pairs(&stack);
        assert!(!pairs.is_empty());
        assert!(pairs.iter().all(|(_, plane)| plane.is_none()), "{pairs:?}");
    }

    #[test]
    fn one_mil_is_exactly_25_4_micrometres() {
        assert!((StackUnit::Mil.to_mm(1.0) - 0.0254).abs() < 1e-12);
        assert_eq!(StackUnit::Um.show(0.035), "35");
    }
}
