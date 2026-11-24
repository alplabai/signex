//! The Materials sub-tab (mockup rows 105 and 177): the material library,
//! built-in entries and the person's own (`signex_types::material_library`),
//! with "Use for selected layer" and an Add material row. Using a material
//! copies its values into the layer selected in Stack as one board undo
//! step; the board keeps the copy (decision 2026-11-24).

use std::fmt;
use std::path::PathBuf;

use iced::widget::{Column, Row, column, container, pick_list, row, scrollable};
use iced::{Alignment, Element, Length, Widget as _};
use signex_i18n::{I18n, Localized, MessageId, UserText, ids, t};
use signex_types::material_library::{Material, MaterialKind};
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::{button, text, text_input};

use super::page::heading;
use super::{LayerStackFacts, LayerStackMsg, SP, TS, form_look, msg};
use crate::panels::graphite::hint_tooltip;
use crate::panels::pcb_primitive_properties::format_number;
use crate::panels::{PanelContext, PanelMsg};
use crate::styles::{ti, tokens::resolve};

/// Where a library entry comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaterialSource {
    BuiltIn,
    User,
}

/// A library entry by its list and place in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaterialPick {
    pub source: MaterialSource,
    pub index: usize,
}

/// The person's own library, read once from the config folder.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum UserLibrary {
    #[default]
    NotRead,
    Reading,
    Read(Vec<Material>),
    /// The file could not be read; nothing is written over it.
    Failed(String),
}

impl UserLibrary {
    /// The entries read; none while reading or after a failure.
    pub fn entries(&self) -> &[Material] {
        match self {
            Self::Read(entries) => entries,
            _ => &[],
        }
    }
}

/// A typed field of the Add material row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaterialField {
    Name,
    Thickness,
    DielectricConstant,
    LossTangent,
}

/// The Add material row as typed.
#[derive(Debug, Clone, PartialEq)]
pub struct MaterialDraft {
    pub name: String,
    pub kind: MaterialKind,
    pub thickness: String,
    pub dielectric_constant: String,
    pub loss_tangent: String,
}

impl Default for MaterialDraft {
    fn default() -> Self {
        Self {
            name: String::new(),
            kind: MaterialKind::Prepreg,
            thickness: String::new(),
            dielectric_constant: String::new(),
            loss_tangent: String::new(),
        }
    }
}

impl MaterialDraft {
    pub fn text_mut(&mut self, field: MaterialField) -> &mut String {
        match field {
            MaterialField::Name => &mut self.name,
            MaterialField::Thickness => &mut self.thickness,
            MaterialField::DielectricConstant => &mut self.dielectric_constant,
            MaterialField::LossTangent => &mut self.loss_tangent,
        }
    }
}

/// What the Materials sub-tab shows.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MaterialsFacts {
    pub built_in: Vec<Material>,
    pub library: UserLibrary,
    pub selected: Option<MaterialPick>,
    pub draft: MaterialDraft,
    /// Why the last write of the user library failed.
    pub write_error: Option<String>,
}

impl MaterialsFacts {
    /// The entry `pick` names, if it is still there.
    pub fn entry(&self, pick: MaterialPick) -> Option<&Material> {
        match pick.source {
            MaterialSource::BuiltIn => self.built_in.get(pick.index),
            MaterialSource::User => self.library.entries().get(pick.index),
        }
    }

    pub fn picked(&self) -> Option<&Material> {
        self.selected.and_then(|pick| self.entry(pick))
    }
}

/// An event of the Materials sub-tab.
#[derive(Debug, Clone, PartialEq)]
pub enum MaterialsMsg {
    Selected {
        board: PathBuf,
        pick: MaterialPick,
    },
    /// "Use for selected layer" pressed.
    Used {
        board: PathBuf,
    },
    DraftTyped {
        board: PathBuf,
        field: MaterialField,
        text: String,
    },
    DraftKindPicked {
        board: PathBuf,
        kind: MaterialKind,
    },
    /// Add material pressed: the draft goes into the user library.
    Added {
        board: PathBuf,
    },
    /// The selected user entry taken out of the user library.
    Removed {
        board: PathBuf,
    },
    /// The user library file was read, or why not.
    UserLibraryRead(Result<Vec<Material>, String>),
    /// The user library file was written, or why not.
    UserLibraryWritten(Result<(), String>),
}

fn materials_msg(m: MaterialsMsg) -> PanelMsg {
    msg(LayerStackMsg::Materials(m))
}

/// The catalog name of a material kind.
pub fn material_kind_label(i18n: &I18n, kind: MaterialKind) -> Localized {
    let id = match kind {
        MaterialKind::Core => ids::PCB_STACK_MATERIAL_KIND_CORE,
        MaterialKind::Prepreg => ids::PCB_STACK_MATERIAL_KIND_PREPREG,
        MaterialKind::Copper => ids::PCB_STACK_MATERIAL_KIND_COPPER,
        MaterialKind::SolderMask => ids::PCB_STACK_MATERIAL_KIND_SOLDER_MASK,
    };
    t!(i18n, id)
}

/// A material kind in the Add row's Type dropdown.
#[derive(Debug, Clone, PartialEq, Eq)]
struct KindChoice {
    kind: MaterialKind,
    label: String,
}

impl fmt::Display for KindChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

/// The width of the library table in body-text ems.
const TABLE_EMS: f32 = 56.0;

/// The library columns: title and share of the row.
const COLUMNS: [(MessageId, u16); 6] = [
    (ids::PCB_STACK_COL_NAME, 6),
    (ids::PCB_STACK_COL_TYPE, 3),
    (ids::PCB_STACK_COL_THICKNESS, 3),
    (ids::PCB_STACK_COL_DK, 2),
    (ids::PCB_STACK_COL_DF, 2),
    (ids::PCB_STACK_COL_SOURCE, 3),
];

pub(super) fn view<'a>(ctx: &'a PanelContext, facts: &'a LayerStackFacts) -> Element<'a, PanelMsg> {
    let muted = resolve(&ctx.tokens).ink3;
    let note = |id| text(t!(ctx.i18n, id)).size(TS.dense).color(muted).boxed();
    let mut col = column![
        heading(
            ctx,
            ids::PCB_STACK_MATERIAL_LIBRARY,
            ids::PCB_STACK_MATERIAL_LIBRARY_HINT
        ),
        table(ctx, facts),
    ]
    .spacing(SP.m)
    .padding(SP.l)
    .width(Length::Fill);
    if let Some(status) = library_status(ctx, &facts.materials) {
        col = col.push(text(status).size(TS.dense).color(muted).boxed());
    }
    col = col.push(use_row(ctx, facts));
    col = col.push(add_row(ctx, facts));
    col = col.push(note(ids::PCB_STACK_MATERIAL_FLEX_NOTE));
    scrollable(col)
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed()
}

/// Reading, a failed read or write, or an empty user library; `None` once
/// the library reads with entries.
fn library_status(ctx: &PanelContext, facts: &MaterialsFacts) -> Option<Localized> {
    let i18n = &*ctx.i18n;
    if let Some(why) = &facts.write_error {
        return Some(t!(i18n, ids::PCB_STACK_MATERIAL_WRITE_FAILED, "why" => why.clone()));
    }
    match &facts.library {
        UserLibrary::NotRead | UserLibrary::Reading => {
            Some(t!(i18n, ids::PCB_STACK_MATERIAL_READING))
        }
        UserLibrary::Failed(why) => {
            Some(t!(i18n, ids::PCB_STACK_MATERIAL_READ_FAILED, "why" => why.clone()))
        }
        UserLibrary::Read(entries) if entries.is_empty() => {
            Some(t!(i18n, ids::PCB_STACK_MATERIAL_USER_EMPTY))
        }
        UserLibrary::Read(_) => None,
    }
}

/// The header and one selectable line per entry, built-in first.
fn table<'a>(ctx: &'a PanelContext, facts: &'a LayerStackFacts) -> Element<'a, PanelMsg> {
    let muted = resolve(&ctx.tokens).ink3;
    let unit = facts.unit.label(&ctx.i18n).into_string();
    let mut header: Row<Element<'a, PanelMsg>> = Row::new().spacing(SP.s);
    for (id, portion) in COLUMNS {
        let title = if id == ids::PCB_STACK_COL_THICKNESS {
            t!(ctx.i18n, id, "unit" => unit.clone())
        } else {
            t!(ctx.i18n, id)
        };
        let cell = text(title).size(TS.dense).color(muted);
        header = header.push(cell.width(Length::FillPortion(portion)).boxed());
    }
    let mut lines: Column<Element<'a, PanelMsg>> = Column::new()
        .spacing(SP.xs)
        .push(container(header).padding([0.0, SP.s]).boxed());
    let materials = &facts.materials;
    let built_in = (0..materials.built_in.len()).map(|index| MaterialPick {
        source: MaterialSource::BuiltIn,
        index,
    });
    let user = (0..materials.library.entries().len()).map(|index| MaterialPick {
        source: MaterialSource::User,
        index,
    });
    for pick in built_in.chain(user) {
        if let Some(entry) = materials.entry(pick) {
            lines = lines.push(entry_line(ctx, facts, pick, entry));
        }
    }
    let sized = container(lines).width(Length::Fixed(TS.body * TABLE_EMS));
    scrollable(sized)
        .direction(iced::widget::scrollable::Direction::Horizontal(
            iced::widget::scrollable::Scrollbar::new().spacing(SP.xs),
        ))
        .width(Length::Fill)
        .boxed()
}

/// One entry as a button that selects it.
fn entry_line<'a>(
    ctx: &'a PanelContext,
    facts: &LayerStackFacts,
    pick: MaterialPick,
    entry: &Material,
) -> Element<'a, PanelMsg> {
    let source = match pick.source {
        MaterialSource::BuiltIn => ids::PCB_STACK_MATERIAL_BUILT_IN,
        MaterialSource::User => ids::PCB_STACK_MATERIAL_USER,
    };
    let dash = || t!(ctx.i18n, ids::PCB_STACK_NONE).into_string();
    let insulator = entry.kind != MaterialKind::Copper;
    let number = |v: f64| if insulator { format_number(v) } else { dash() };
    let cells = [
        entry.name.clone(),
        material_kind_label(&ctx.i18n, entry.kind).into_string(),
        facts.unit.show(entry.thickness_mm),
        number(entry.dielectric_constant),
        number(entry.loss_tangent),
        t!(ctx.i18n, source).into_string(),
    ];
    let mut line: Row<Element<'a, PanelMsg>> = Row::new().spacing(SP.s);
    for (cell, (_, portion)) in cells.into_iter().zip(COLUMNS) {
        let cell = text(UserText::new(cell))
            .size(TS.dense)
            .color(ti(ctx.tokens.text))
            .width(Length::FillPortion(portion));
        line = line.push(cell.boxed());
    }
    let selected = facts.materials.selected == Some(pick);
    button(line)
        .padding([SP.xs, SP.s])
        .width(Length::Fill)
        .style(form_look::option_tile(selected))
        .on_press(materials_msg(MaterialsMsg::Selected {
            board: facts.board.clone(),
            pick,
        }))
        .boxed()
}

/// Whether the picked entry can be used for the selected stack row.
pub(super) fn can_use(facts: &LayerStackFacts) -> bool {
    let row = facts.selected.and_then(|row| facts.stack.layers.get(row));
    match (facts.materials.picked(), row) {
        (Some(entry), Some(row)) => !facts.read_only && entry.kind.fits(row.kind),
        _ => false,
    }
}

/// "Use for selected layer" and the layer it would go to.
fn use_row<'a>(ctx: &'a PanelContext, facts: &'a LayerStackFacts) -> Element<'a, PanelMsg> {
    let live = can_use(facts);
    let tip = if live {
        ids::PCB_STACK_MATERIAL_USE_TIP
    } else {
        ids::PCB_STACK_MATERIAL_USE_UNAVAILABLE
    };
    let pressed = materials_msg(MaterialsMsg::Used {
        board: facts.board.clone(),
    });
    let face = button(text(t!(ctx.i18n, ids::PCB_STACK_MATERIAL_USE)).size(TS.dense))
        .padding([SP.xs, SP.s])
        .style(form_look::ghost_button)
        .on_press_maybe(live.then_some(pressed));
    let layer = match facts.selected.and_then(|row| facts.names.get(row)) {
        Some(name) => t!(ctx.i18n, ids::PCB_STACK_MATERIAL_LAYER, "name" => name.clone()),
        None => t!(ctx.i18n, ids::PCB_STACK_MATERIAL_NO_LAYER),
    };
    row![
        hint_tooltip(
            face.boxed(),
            t!(ctx.i18n, tip),
            &ctx.tokens,
            iced::widget::tooltip::Position::Bottom,
        ),
        text(layer).size(TS.dense).color(resolve(&ctx.tokens).ink3),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center)
    .boxed()
}

/// Whether the user library can be changed: read, and not failed.
fn library_writable(facts: &MaterialsFacts) -> bool {
    matches!(facts.library, UserLibrary::Read(_))
}

/// Name, Type, Thickness, Dk and Df typed in a row, then Add material and
/// Remove.
fn add_row<'a>(ctx: &'a PanelContext, facts: &'a LayerStackFacts) -> Element<'a, PanelMsg> {
    let materials = &facts.materials;
    let draft = &materials.draft;
    let writable = library_writable(materials);
    let unit = facts.unit.label(&ctx.i18n).into_string();
    let thickness = t!(ctx.i18n, ids::PCB_STACK_COL_THICKNESS, "unit" => unit);
    let fields = [
        (
            MaterialField::Name,
            t!(ctx.i18n, ids::PCB_STACK_MATERIAL_NAME),
            &draft.name,
            4,
        ),
        (MaterialField::Thickness, thickness, &draft.thickness, 3),
        (
            MaterialField::DielectricConstant,
            t!(ctx.i18n, ids::PCB_STACK_COL_DK),
            &draft.dielectric_constant,
            2,
        ),
        (
            MaterialField::LossTangent,
            t!(ctx.i18n, ids::PCB_STACK_COL_DF),
            &draft.loss_tangent,
            2,
        ),
    ];
    let mut line: Row<Element<'a, PanelMsg>> = Row::new().spacing(SP.s).align_y(Alignment::Center);
    line = line.push(kind_list(ctx, facts, writable));
    for (field, placeholder, value, portion) in fields {
        let mut input = text_input(placeholder, value.clone())
            .size(TS.dense)
            .padding(SP.xs)
            .style(form_look::text_input_style)
            .width(Length::FillPortion(portion));
        if writable {
            let board = facts.board.clone();
            input = input.on_input(move |text| {
                materials_msg(MaterialsMsg::DraftTyped {
                    board: board.clone(),
                    field,
                    text,
                })
            });
        }
        line = line.push(input.boxed());
    }
    line = line.push(library_button(
        ctx,
        ids::PCB_STACK_MATERIAL_ADD,
        ids::PCB_STACK_MATERIAL_ADD_TIP,
        writable.then(|| MaterialsMsg::Added {
            board: facts.board.clone(),
        }),
    ));
    let removable = writable
        && materials
            .selected
            .is_some_and(|pick| pick.source == MaterialSource::User);
    line = line.push(library_button(
        ctx,
        ids::PCB_STACK_MATERIAL_REMOVE,
        ids::PCB_STACK_MATERIAL_REMOVE_TIP,
        removable.then(|| MaterialsMsg::Removed {
            board: facts.board.clone(),
        }),
    ));
    line.boxed()
}

/// The Add row's Type dropdown.
fn kind_list<'a>(
    ctx: &'a PanelContext,
    facts: &'a LayerStackFacts,
    writable: bool,
) -> Element<'a, PanelMsg> {
    let choice = |kind| KindChoice {
        kind,
        label: material_kind_label(&ctx.i18n, kind).into_string(),
    };
    let options: Vec<KindChoice> = MaterialKind::ALL.into_iter().map(choice).collect();
    let current = choice(facts.materials.draft.kind);
    let list = pick_list(Some(current), options, |c: &KindChoice| c.to_string())
        .graphite()
        .text_size(TS.dense)
        .padding(SP.xs)
        .width(Length::FillPortion(3));
    if !writable {
        return list.boxed();
    }
    let board = facts.board.clone();
    list.on_select(move |c: KindChoice| {
        materials_msg(MaterialsMsg::DraftKindPicked {
            board: board.clone(),
            kind: c.kind,
        })
    })
    .boxed()
}

/// A ghost button with a hint; inert without an event.
fn library_button<'a>(
    ctx: &'a PanelContext,
    label: MessageId,
    tip: MessageId,
    event: Option<MaterialsMsg>,
) -> Element<'a, PanelMsg> {
    let face = button(text(t!(ctx.i18n, label)).size(TS.dense))
        .padding([SP.xs, SP.s])
        .style(form_look::ghost_button)
        .on_press_maybe(event.map(materials_msg));
    hint_tooltip(
        face.boxed(),
        t!(ctx.i18n, tip),
        &ctx.tokens,
        iced::widget::tooltip::Position::Bottom,
    )
}
