//! Properties of the components selected on a board (#2354): a header with
//! the selection summary, a row search and General / Parameters tabs; then
//! Location, Properties, Footprint, Swapping Options and Schematic Reference
//! Information. Several components show the values they share and leave a
//! differing one blank. A pure view over facts the panel context copies from
//! the board; every edit is a message the board handler runs as one
//! `BoardCommand` undo step.
//!
//! Rows the board cannot act on yet are drawn disabled with a
//! tooltip saying why, never with a made-up value.

mod footprint;
mod general;
mod header;
mod identity;
mod parameters;
mod rows;
mod settings;
pub(crate) use settings::type_label as component_type_label;
mod text;

use std::collections::BTreeMap;
use std::path::PathBuf;

use iced::widget::{Column, scrollable};
use iced::{Element, Length, Widget as _};
use signex_engine::pcb::component::LocationField;
use signex_engine::pcb::component_edit::{ComponentSetting, TypedSetting};
use signex_engine::pcb::designators::FootprintTextField;
use signex_types::pcb::{ComponentAttributes, ComponentType};
use signex_types::pcb_layer::BoardSide;
use uuid::Uuid;

use super::{PanelContext, PanelMsg};

pub use text::{FootprintEditability, PcbFootprintFacts, PcbFootprintTextMsg};

/// A value the selected components share, or `Mixed` when they differ.
#[derive(Debug, Clone, PartialEq)]
pub enum Shared<T> {
    Same(T),
    Mixed,
}

impl<T: PartialEq> Shared<T> {
    /// The shared value of `values`; `None` when there are none.
    pub fn of(values: impl IntoIterator<Item = T>) -> Option<Self> {
        let mut values = values.into_iter();
        let first = values.next()?;
        Some(if values.all(|v| v == first) {
            Self::Same(first)
        } else {
            Self::Mixed
        })
    }

    /// The value, when every component has it.
    pub fn same(&self) -> Option<&T> {
        match self {
            Self::Same(value) => Some(value),
            Self::Mixed => None,
        }
    }
}

/// A field whose typed text is a draft until Enter.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ComponentField {
    Location(LocationField),
    /// The parameter of this key; the Description row is one too.
    Parameter(String),
    /// A component setting typed as text (#2365).
    Setting(TypedSetting),
    /// The Comment of several components at once (#2369).
    Comment,
    /// The name of a parameter to add; applied by the Add button (#2369).
    NewParameterName,
    /// The value of a parameter to add.
    NewParameterValue,
}

/// The library a board footprint came from, as the Library row names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FootprintLibrary {
    /// An open library, by its display name.
    Open(String),
    /// A library that is not open, by its id.
    NotOpen(Uuid),
}

/// One row of the Parameters tab: a key the selected components all have.
#[derive(Debug, Clone, PartialEq)]
pub struct ParameterFacts {
    pub key: String,
    pub value: Shared<String>,
}

/// What the schematic says about a component, read through its link
/// (#2367).
#[derive(Debug, Clone, PartialEq)]
pub enum PartIdentity {
    /// The footprint has no schematic link.
    NotLinked,
    /// The linked part's sheet is not open, so it cannot be read.
    NotRead,
    Read {
        /// The designator on the schematic sheet.
        designator: String,
        /// The library part it came from: the row's part number, else the
        /// symbol's library name; `None` when it has neither.
        design_item_id: Option<String>,
        /// The open library holding the part; `None` when none does.
        source: Option<String>,
    },
}

/// The component settings as the panel shows them (#2365).
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentSettingFacts {
    pub component_type: Shared<ComponentType>,
    pub height_mm: Shared<f64>,
    pub body_opacity: Shared<f64>,
    pub lock_primitives: Shared<bool>,
    pub lock_strings: Shared<bool>,
    pub pin_swapping: Shared<bool>,
    pub part_swapping: Shared<bool>,
    pub channel_offset: Shared<u32>,
    pub description: Shared<String>,
}

impl ComponentSettingFacts {
    /// Every setting at its default, as a component without any has them.
    pub fn defaults() -> Self {
        let d = ComponentAttributes::default();
        Self {
            component_type: Shared::Same(d.component_type),
            height_mm: Shared::Same(d.height_mm),
            body_opacity: Shared::Same(d.body_opacity),
            lock_primitives: Shared::Same(d.lock_primitives),
            lock_strings: Shared::Same(d.lock_strings),
            pin_swapping: Shared::Same(d.pin_swapping),
            part_swapping: Shared::Same(d.part_swapping),
            channel_offset: Shared::Same(d.channel_offset),
            description: Shared::Same(d.description),
        }
    }
}

/// The selected components as the panel shows them.
#[derive(Debug, Clone, PartialEq)]
pub struct PcbComponentFacts {
    pub board: PathBuf,
    /// The selected footprints, sorted.
    pub footprints: Vec<Uuid>,
    pub x: Shared<f64>,
    pub y: Shared<f64>,
    pub rotation: Shared<f64>,
    pub locked: Shared<bool>,
    pub side: Shared<BoardSide>,
    pub designator: Shared<String>,
    /// `None` when a component has no designator text on the board.
    pub designator_shown: Option<Shared<bool>>,
    pub comment: Shared<String>,
    /// `None` when a component has no comment text on the board.
    pub comment_shown: Option<Shared<bool>>,
    /// The body's area in mm²; `None` inside when it cannot be measured.
    pub area_mm2: Shared<Option<f64>>,
    pub footprint_name: Shared<String>,
    /// The library the footprint came from; `None` inside when it has no
    /// library link.
    pub library: Shared<Option<FootprintLibrary>>,
    /// The schematic sheet path and part link; `None` inside when unlinked.
    pub schematic_path: Shared<Option<String>>,
    /// Type, height, locks and the other component settings.
    pub settings: ComponentSettingFacts,
    /// What the schematic says about the components.
    pub identity: Shared<PartIdentity>,
    /// How many selected components carry a designator other than the one
    /// the schematic gives them.
    pub designator_mismatches: usize,
    /// The keys every selected component has, in the first one's order.
    pub parameters: Vec<ParameterFacts>,
    /// Text typed and not applied yet, per field.
    pub drafts: BTreeMap<ComponentField, String>,
    /// The board opened read-only (a newer format).
    pub read_only: bool,
    /// The row filter typed into the header's search field.
    pub search: String,
}

impl PcbComponentFacts {
    /// Whether placement edits run: the board is writable and no selected
    /// component is locked or mixed-locked.
    pub fn is_editable(&self) -> bool {
        !self.read_only && self.locked == Shared::Same(false)
    }

    /// The typed text of `field`, if any.
    pub fn draft(&self, field: &ComponentField) -> Option<&String> {
        self.drafts.get(field)
    }

    /// The value of the parameter `key`, if every component has it.
    pub fn parameter(&self, key: &str) -> Option<&Shared<String>> {
        self.parameters
            .iter()
            .find(|p| p.key == key)
            .map(|p| &p.value)
    }
}

/// An edit or a view change in the Component view. Every edit names the
/// board and footprints it was made for, so a selection change in between
/// cannot send it to other components.
#[derive(Debug, Clone, PartialEq)]
pub enum PcbComponentMsg {
    /// The header's row filter changed.
    SearchChanged(String),
    FieldTyped {
        board: PathBuf,
        footprints: Vec<Uuid>,
        field: ComponentField,
        text: String,
    },
    FieldSubmitted {
        board: PathBuf,
        footprints: Vec<Uuid>,
        field: ComponentField,
    },
    LockToggled {
        board: PathBuf,
        footprints: Vec<Uuid>,
        locked: bool,
    },
    SidePicked {
        board: PathBuf,
        footprints: Vec<Uuid>,
        side: BoardSide,
    },
    TextShownToggled {
        board: PathBuf,
        footprints: Vec<Uuid>,
        field: FootprintTextField,
        shown: bool,
    },
    /// The Footprint Name row's picker button was pressed: the footprint
    /// picker opens to swap the footprint of these components (#2366).
    FootprintPickRequested {
        board: PathBuf,
        footprints: Vec<Uuid>,
    },
    /// A type was picked or a settings checkbox ticked (#2365).
    SettingPicked {
        board: PathBuf,
        footprints: Vec<Uuid>,
        setting: ComponentSetting,
    },
    /// The Add button of the Parameters tab, or Enter in its fields (#2369):
    /// adds the typed name and value.
    ParameterAdded {
        board: PathBuf,
        footprints: Vec<Uuid>,
    },
    /// A parameter row's remove button (#2369).
    ParameterRemoved {
        board: PathBuf,
        footprints: Vec<Uuid>,
        key: String,
    },
}

/// The Component view: header, then the General or the Parameters tab.
/// `single` carries the Designator and Comment drafts of a single component.
pub fn view<'a>(
    ctx: &'a PanelContext,
    facts: &'a PcbComponentFacts,
    single: Option<&'a PcbFootprintFacts>,
) -> Element<'a, PanelMsg> {
    let mut col: Column<Element<'a, PanelMsg>> = Column::new().spacing(0).width(Length::Fill);
    col = col.push(header::view(ctx, facts));
    col = if ctx.properties_tab == 1 {
        col.push(parameters::view(ctx, facts))
    } else {
        let col = general::push_sections(col, ctx, facts, single);
        footprint::push_sections(col, ctx, facts)
    };
    scrollable(col)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(super::form_look::scrollbar_style)
        .boxed()
}

#[cfg(test)]
#[path = "tests.rs"]
pub(in crate::panels) mod tests;
