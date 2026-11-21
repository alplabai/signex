//! The Unions slice's state (mockup row 158): which unions are hidden per sheet (a
//! view setting, never saved and never an undo step), the panel's filter,
//! scope and tag chips, the union picked in the panel, and the Create Union
//! popover. Plus the pure builder of what the panel shows.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use signex_types::schematic::{SchUnion, SchematicSheet, SelectedItem, SelectedKind};
use uuid::Uuid;

/// Which unions the panel lists.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum UnionScope {
    /// The sheet in view.
    #[default]
    ThisSheet,
    /// Every open sheet of the sheet's project, grouped by sheet.
    Project,
}

/// The Create Union popover: the objects it groups (on `sheet`), the name,
/// the tags and the half-typed tag, the lock box, and where it opened.
#[derive(Debug, Clone, PartialEq)]
pub struct CreateUnionPopover {
    pub sheet: PathBuf,
    pub members: Vec<Uuid>,
    pub name: String,
    pub tags: Vec<String>,
    pub tag_input: String,
    pub lock: bool,
    /// The window point it opened at.
    pub anchor: (f32, f32),
}

impl CreateUnionPopover {
    /// The tags with the half-typed one added.
    #[must_use]
    pub fn tags_with_input(&self) -> Vec<String> {
        let mut tags = self.tags.clone();
        if !self.tag_input.trim().is_empty() {
            tags.push(self.tag_input.clone());
        }
        signex_engine::unions::normalized_tags(&tags)
    }

    /// Create is live once the union has a name.
    #[must_use]
    pub fn can_create(&self) -> bool {
        !self.name.trim().is_empty() && !self.members.is_empty()
    }
}

/// What the Unions slice keeps. Lives in `UiState`.
#[derive(Debug, Clone, Default)]
pub struct UnionsState {
    /// Hidden unions by sheet. Canvas only: prints and exports are
    /// unchanged, nothing is saved, and hiding is not an undo step.
    pub hidden: HashMap<PathBuf, Vec<Uuid>>,
    pub filter: String,
    pub scope: UnionScope,
    /// Tag filter chips that are on; a union shows when it has any of them.
    pub tags_on: BTreeSet<String>,
    /// The union picked in the panel, by sheet.
    pub picked: Option<(PathBuf, Uuid)>,
    pub popover: Option<CreateUnionPopover>,
}

impl UnionsState {
    /// The unions hidden on `path`.
    #[must_use]
    pub fn hidden_in(&self, path: &Path) -> &[Uuid] {
        match self.hidden.get(path) {
            Some(hidden) => hidden,
            None => &[],
        }
    }

    #[must_use]
    pub fn is_hidden(&self, path: &Path, union: Uuid) -> bool {
        self.hidden_in(path).contains(&union)
    }

    /// Hide `union` on `path`, or show it again.
    pub fn toggle_hidden(&mut self, path: &Path, union: Uuid) {
        let hidden = self.hidden.entry(path.to_path_buf()).or_default();
        match hidden.iter().position(|id| *id == union) {
            Some(at) => {
                hidden.remove(at);
            }
            None => hidden.push(union),
        }
        if hidden.is_empty() {
            self.hidden.remove(path);
        }
    }
}

/// What happened in the Unions slice: the panel, the canvas menu rows, the
/// keymap and the Create Union popover.
#[derive(Debug, Clone)]
pub enum UnionsMsg {
    /// Create Union… (Ctrl+G, the menu row, the panel's + button): open
    /// the popover for the selection.
    CreateRequested,
    /// Add Selection to Union: the selection joins the picked union.
    AddSelectionRequested,
    /// Remove Selection from Union: the selection leaves its union.
    RemoveSelectionRequested,
    /// Break Union: the picked union, or the one the selection is.
    BreakRequested,
    Popover(UnionPopoverMsg),
    FilterEdited(String),
    ScopePicked(UnionScope),
    TagChipToggled(String),
    /// A panel row was clicked.
    RowPicked {
        sheet: PathBuf,
        union: Uuid,
    },
    /// A row's Select members button.
    MembersSelected {
        sheet: PathBuf,
        union: Uuid,
    },
    LockToggled {
        sheet: PathBuf,
        union: Uuid,
    },
    HideToggled {
        sheet: PathBuf,
        union: Uuid,
    },
    /// The panel foot's name field of the picked union changed.
    RenameEdited(String),
}

/// What happened in the Create Union popover.
#[derive(Debug, Clone)]
pub enum UnionPopoverMsg {
    NameEdited(String),
    TagInputEdited(String),
    /// Enter in the tag field: the typed tag becomes a pill.
    TagSubmitted,
    TagRemoved(usize),
    LockToggled(bool),
    Cancelled,
    Confirmed,
}

/// One row of the Unions panel.
#[derive(Debug, Clone, PartialEq)]
pub struct UnionRow {
    pub sheet: PathBuf,
    pub uuid: Uuid,
    pub name: String,
    pub members: usize,
    pub tags: Vec<String>,
    pub locked: bool,
    pub hidden: bool,
    pub picked: bool,
}

/// The rows of one sheet; `title` is the sheet's file name in Project scope.
#[derive(Debug, Clone, PartialEq)]
pub struct UnionGroup {
    pub title: Option<String>,
    pub rows: Vec<UnionRow>,
}

/// The picked union, for the panel foot.
#[derive(Debug, Clone, PartialEq)]
pub struct UnionFoot {
    pub sheet: PathBuf,
    pub uuid: Uuid,
    pub name: String,
    pub members: usize,
    /// Designators of the member parts, then the text of member labels.
    pub pills: Vec<String>,
    pub wires: usize,
    /// Members that are neither parts, labels nor wires.
    pub others: usize,
    pub locked: bool,
    pub hidden: bool,
    /// Whether the selection holds objects of this sheet outside the union
    /// (Add selection) and members of it (Remove selection).
    pub can_add: bool,
    pub can_remove: bool,
}

/// What the Unions panel shows; built in `update`, copied into the panel
/// context.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct UnionsPanelView {
    /// A schematic sheet is in view.
    pub has_sheet: bool,
    pub filter: String,
    pub scope: UnionScope,
    /// Every tag in scope, and whether its chip is on.
    pub tags: Vec<(String, bool)>,
    pub groups: Vec<UnionGroup>,
    pub foot: Option<UnionFoot>,
    /// The selection can become a union (Create from selection).
    pub can_create: bool,
    /// The stamp of the app state this view was built from; see
    /// `Signex::sync_unions_panel`. `None` from [`panel_view`].
    pub built_from: Option<u64>,
}

/// One sheet the panel reads: its path, file name and document.
pub struct ScopeSheet<'a> {
    pub path: &'a Path,
    pub name: String,
    pub sheet: &'a SchematicSheet,
}

/// Whether `union` passes the filter text and the tag chips.
fn passes(union: &SchUnion, filter: &str, tags_on: &BTreeSet<String>) -> bool {
    if !tags_on.is_empty() && !union.tags.iter().any(|t| tags_on.contains(t)) {
        return false;
    }
    let filter = filter.trim().to_lowercase();
    if filter.is_empty() {
        return true;
    }
    match filter.strip_prefix('#') {
        Some(tag) => union.tags.iter().any(|t| t.to_lowercase().starts_with(tag)),
        None => {
            union.name.to_lowercase().contains(&filter)
                || union
                    .tags
                    .iter()
                    .any(|t| t.to_lowercase().contains(&filter))
        }
    }
}

/// The panel foot for `union` on `sheet`, with `selected` the selection of
/// the sheet in view when it is that sheet.
fn foot(
    path: &Path,
    sheet: &SchematicSheet,
    union: &SchUnion,
    hidden: bool,
    selected: Option<&[SelectedItem]>,
) -> UnionFoot {
    let items = signex_engine::unions::member_items(sheet, union);
    let mut pills = Vec::new();
    let (mut wires, mut others) = (0, 0);
    for item in &items {
        match item.kind {
            SelectedKind::Symbol => {
                if let Some(s) = sheet.symbols.iter().find(|s| s.uuid == item.uuid) {
                    pills.push(s.reference.clone());
                }
            }
            SelectedKind::Label => {
                if let Some(l) = sheet.labels.iter().find(|l| l.uuid == item.uuid) {
                    pills.push(l.text.clone());
                }
            }
            SelectedKind::Wire => wires += 1,
            _ => others += 1,
        }
    }
    let selected = selected.unwrap_or(&[]);
    let can_add = !union.locked
        && selected
            .iter()
            .any(|i| !union.members.contains(&i.uuid) && i.kind != SelectedKind::SheetPin);
    let can_remove = !union.locked && selected.iter().any(|i| union.members.contains(&i.uuid));
    UnionFoot {
        sheet: path.to_path_buf(),
        uuid: union.uuid,
        name: union.name.clone(),
        members: items.len(),
        pills,
        wires,
        others,
        locked: union.locked,
        hidden,
        can_add,
        can_remove,
    }
}

/// Build what the panel shows. `sheets` are the sheets in scope with the
/// one in view (`active`) first; `selected` is that sheet's selection.
#[must_use]
pub fn panel_view(
    state: &UnionsState,
    sheets: &[ScopeSheet<'_>],
    active: Option<&Path>,
    selected: &[SelectedItem],
) -> UnionsPanelView {
    let mut tags: BTreeSet<String> = BTreeSet::new();
    for s in sheets {
        for union in &s.sheet.unions {
            tags.extend(union.tags.iter().cloned());
        }
    }
    let active_sheet = sheets.iter().find(|s| Some(s.path) == active);
    // The picked union, else the one the canvas selection is.
    let picked = state
        .picked
        .clone()
        .filter(|(path, id)| {
            sheets
                .iter()
                .any(|s| s.path == path && s.sheet.unions.iter().any(|u| u.uuid == *id))
        })
        .or_else(|| {
            let s = active_sheet?;
            s.sheet
                .unions
                .iter()
                .find(|u| signex_engine::unions::selection_is_union(u, selected))
                .map(|u| (s.path.to_path_buf(), u.uuid))
        });
    let grouped = state.scope == UnionScope::Project && sheets.len() > 1;
    let mut groups = Vec::new();
    for s in sheets {
        let rows: Vec<UnionRow> = s
            .sheet
            .unions
            .iter()
            .filter(|u| passes(u, &state.filter, &state.tags_on))
            .map(|u| UnionRow {
                sheet: s.path.to_path_buf(),
                uuid: u.uuid,
                name: u.name.clone(),
                members: u.members.len(),
                tags: u.tags.clone(),
                locked: u.locked,
                hidden: state.is_hidden(s.path, u.uuid),
                picked: picked
                    .as_ref()
                    .is_some_and(|(p, id)| p.as_path() == s.path && *id == u.uuid),
            })
            .collect();
        if !rows.is_empty() || !grouped {
            groups.push(UnionGroup {
                title: grouped.then(|| s.name.clone()),
                rows,
            });
        }
    }
    let foot = picked.and_then(|(path, id)| {
        let s = sheets.iter().find(|s| s.path == path)?;
        let union = s.sheet.unions.iter().find(|u| u.uuid == id)?;
        let selection = (Some(s.path) == active).then_some(selected);
        Some(foot(
            s.path,
            s.sheet,
            union,
            state.is_hidden(s.path, id),
            selection,
        ))
    });
    UnionsPanelView {
        has_sheet: active_sheet.is_some(),
        filter: state.filter.clone(),
        scope: state.scope,
        tags: tags
            .into_iter()
            .map(|t| {
                let on = state.tags_on.contains(&t);
                (t, on)
            })
            .collect(),
        groups,
        foot,
        can_create: active_sheet.is_some() && !selected.is_empty(),
        built_from: None,
    }
}
