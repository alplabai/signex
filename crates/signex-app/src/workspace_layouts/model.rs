//! The workspace-layout list and its rules: the four built-ins with their
//! optional user overrides, the user's own layouts in order, and the active
//! one. Plain data, no iced and no IO; the names a person sees come in
//! through [`Naming`]. Design: docs/WORKSPACE_LAYOUTS.md.

use std::collections::{BTreeMap, BTreeSet};

use crate::dock::DockLayoutSnapshot;

/// One of the four layouts Signex ships.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BuiltinLayout {
    Schematic,
    Layout,
    Review,
    Notebook,
}

impl BuiltinLayout {
    /// Menu and list order.
    pub const ALL: [Self; 4] = [Self::Schematic, Self::Layout, Self::Review, Self::Notebook];

    /// The id `prefs.json` stores for it.
    pub fn key(self) -> &'static str {
        match self {
            Self::Schematic => "schematic",
            Self::Layout => "layout",
            Self::Review => "review",
            Self::Notebook => "notebook",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|b| b.key() == key)
    }

    /// The command that applies it.
    pub fn command_id(self) -> &'static str {
        match self {
            Self::Schematic => "apply_layout_schematic",
            Self::Layout => "apply_layout_board",
            Self::Review => "apply_layout_review",
            Self::Notebook => "apply_layout_notebook",
        }
    }

    fn index(self) -> usize {
        match self {
            Self::Schematic => 0,
            Self::Layout => 1,
            Self::Review => 2,
            Self::Notebook => 3,
        }
    }
}

/// Which layout: a built-in, or a user layout by its stored id.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LayoutId {
    Builtin(BuiltinLayout),
    User(String),
}

impl LayoutId {
    /// The id as `prefs.json` stores it: a built-in's key, else the user id.
    pub fn key(&self) -> &str {
        match self {
            Self::Builtin(b) => b.key(),
            Self::User(id) => id,
        }
    }

    pub fn from_key(key: &str) -> Self {
        BuiltinLayout::from_key(key).map_or_else(|| Self::User(key.to_string()), Self::Builtin)
    }

    pub fn builtin(&self) -> Option<BuiltinLayout> {
        match self {
            Self::Builtin(b) => Some(*b),
            Self::User(_) => None,
        }
    }
}

/// How the document area was split when the layout was saved (View ▸ Window
/// ▸ Tile).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentSplit {
    /// Tile Vertically.
    SideBySide,
    /// Tile Horizontally.
    Stacked,
}

impl DocumentSplit {
    pub fn key(self) -> &'static str {
        match self {
            Self::SideBySide => "side_by_side",
            Self::Stacked => "stacked",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "side_by_side" => Some(Self::SideBySide),
            "stacked" => Some(Self::Stacked),
            _ => None,
        }
    }
}

/// One layout. A built-in keeps an empty `name`: its name is catalog text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceLayout {
    pub id: LayoutId,
    pub name: String,
    /// The docked panels, their docks and sizes; `None` when the layout was
    /// saved without them (it then only splits the document area).
    pub dock: Option<DockLayoutSnapshot>,
    pub split: Option<DocumentSplit>,
    /// Local time of the save, `YYYY-MM-DD HH:MM`; `None` for a built-in as
    /// shipped and for an imported layout that carried none.
    pub saved_at: Option<String>,
    /// Saved from the arrangement on screen (Save Layout, Update from current).
    pub from_current: bool,
}

impl WorkspaceLayout {
    pub fn builtin(&self) -> Option<BuiltinLayout> {
        self.id.builtin()
    }
}

/// What Save Layout and Update from current take from the screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Captured {
    pub dock: Option<DockLayoutSnapshot>,
    pub split: Option<DocumentSplit>,
    pub saved_at: String,
}

/// Makes a name unique from a base name and a number ("Bench (2)").
pub type NthName = Box<dyn Fn(&str, usize) -> String>;

/// The names a person sees, from the catalog of the app's language.
pub struct Naming {
    /// The four built-in names, in [`BuiltinLayout::ALL`] order.
    pub builtins: [String; 4],
    /// "Layout 3": the name Save Layout offers.
    pub numbered: Box<dyn Fn(usize) -> String>,
    /// "Bench copy": the name of a duplicate.
    pub copy_of: Box<dyn Fn(&str) -> String>,
    /// "Bench (2)": a name made unique.
    pub nth: NthName,
}

impl Naming {
    pub fn builtin(&self, b: BuiltinLayout) -> &str {
        &self.builtins[b.index()]
    }
}

/// What a name typed into Save Layout or Rename would hit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NameClash {
    Free,
    /// One of the user's layouts: Save replaces it.
    User(LayoutId),
    /// A built-in: refused.
    Builtin(BuiltinLayout),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveOutcome {
    Added(LayoutId),
    Replaced(LayoutId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveRefusal {
    EmptyName,
    BuiltinName(BuiltinLayout),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenameRefusal {
    Builtin,
    EmptyName,
    Taken,
    Unknown,
}

/// What Delete hands back so Undo can put the layout where it was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeletedLayout {
    pub layout: WorkspaceLayout,
    pub index: usize,
    pub was_active: bool,
}

/// The whole list.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Layouts {
    overrides: BTreeMap<BuiltinLayout, WorkspaceLayout>,
    user: Vec<WorkspaceLayout>,
    active: Option<LayoutId>,
    /// Ids of stored entries this build could not read: a new layout never
    /// takes one, so the kept entry is not overwritten.
    reserved_ids: BTreeSet<String>,
}

/// Names compare trimmed and case-blind: "bench" replaces "Bench".
fn same_name(a: &str, b: &str) -> bool {
    a.trim().to_lowercase() == b.trim().to_lowercase()
}

impl Layouts {
    /// A list as the store read it. Duplicate user ids keep the first.
    pub fn from_parts(
        overrides: BTreeMap<BuiltinLayout, WorkspaceLayout>,
        user: Vec<WorkspaceLayout>,
        active: Option<LayoutId>,
    ) -> Self {
        let mut layouts = Self {
            overrides,
            user: Vec::new(),
            active: None,
            reserved_ids: BTreeSet::new(),
        };
        for layout in user {
            if layouts.find_user(&layout.id).is_none() {
                layouts.user.push(layout);
            }
        }
        layouts.active = active.filter(|id| layouts.get(id).is_some());
        layouts
    }

    /// Keep `ids` (stored entries this build could not read) from being
    /// handed to a new layout.
    pub fn reserve_ids(&mut self, ids: impl IntoIterator<Item = String>) {
        self.reserved_ids.extend(ids);
    }

    pub fn user(&self) -> &[WorkspaceLayout] {
        &self.user
    }

    pub fn overrides(&self) -> &BTreeMap<BuiltinLayout, WorkspaceLayout> {
        &self.overrides
    }

    pub fn active(&self) -> Option<&LayoutId> {
        self.active.as_ref()
    }

    /// Mark `id` as the layout on screen; an unknown id clears the mark.
    pub fn set_active(&mut self, id: Option<LayoutId>) {
        self.active = id.filter(|id| self.get(id).is_some());
    }

    /// Whether the built-in carries a user override.
    pub fn is_edited(&self, b: BuiltinLayout) -> bool {
        self.overrides.contains_key(&b)
    }

    /// The active built-in when it carries an override (the menu's Reset row).
    pub fn resettable(&self) -> Option<BuiltinLayout> {
        self.active
            .as_ref()
            .and_then(LayoutId::builtin)
            .filter(|b| self.is_edited(*b))
    }

    /// The built-in as it applies now: its override, else as shipped.
    pub fn builtin_layout(&self, b: BuiltinLayout) -> WorkspaceLayout {
        self.overrides
            .get(&b)
            .cloned()
            .unwrap_or_else(|| super::builtins::shipped(b))
    }

    fn find_user(&self, id: &LayoutId) -> Option<usize> {
        self.user.iter().position(|l| &l.id == id)
    }

    pub fn get(&self, id: &LayoutId) -> Option<WorkspaceLayout> {
        match id {
            LayoutId::Builtin(b) => Some(self.builtin_layout(*b)),
            LayoutId::User(_) => self.find_user(id).map(|i| self.user[i].clone()),
        }
    }

    /// The name a person sees.
    pub fn name_of(&self, id: &LayoutId, naming: &Naming) -> Option<String> {
        match id {
            LayoutId::Builtin(b) => Some(naming.builtin(*b).to_string()),
            LayoutId::User(_) => self.find_user(id).map(|i| self.user[i].name.clone()),
        }
    }

    /// What `name` hits; `except` is left out (the layout being renamed).
    pub fn name_clash(&self, name: &str, naming: &Naming, except: Option<&LayoutId>) -> NameClash {
        if let Some(b) = BuiltinLayout::ALL
            .into_iter()
            .find(|b| same_name(naming.builtin(*b), name))
        {
            return NameClash::Builtin(b);
        }
        self.user
            .iter()
            .find(|l| Some(&l.id) != except && same_name(&l.name, name))
            .map_or(NameClash::Free, |l| NameClash::User(l.id.clone()))
    }

    fn is_taken(&self, name: &str, naming: &Naming) -> bool {
        self.name_clash(name, naming, None) != NameClash::Free
    }

    /// The first "Layout N" no layout uses, from 1.
    pub fn first_free_name(&self, naming: &Naming) -> String {
        (1..)
            .map(|n| (naming.numbered)(n))
            .find(|name| !self.is_taken(name, naming))
            .unwrap_or_default()
    }

    /// `base`, or `base (2)`, `base (3)`, … whichever is free first.
    pub fn unique_name(&self, base: &str, naming: &Naming) -> String {
        if !self.is_taken(base, naming) {
            return base.to_string();
        }
        (2..)
            .map(|n| (naming.nth)(base, n))
            .find(|name| !self.is_taken(name, naming))
            .unwrap_or_default()
    }

    /// An id no user layout has and no kept entry holds: `user-1`, `user-2`, …
    fn fresh_id(&self) -> LayoutId {
        (1..)
            .map(|n| LayoutId::User(format!("user-{n}")))
            .find(|id| self.find_user(id).is_none() && !self.reserved_ids.contains(id.key()))
            .unwrap_or_else(|| LayoutId::User(String::from("user")))
    }

    /// Save Layout: a new user layout named `name`, or the user layout of that
    /// name replaced in place (same id, same position). A built-in's name is
    /// refused.
    pub fn save(
        &mut self,
        name: &str,
        captured: Captured,
        naming: &Naming,
    ) -> Result<SaveOutcome, SaveRefusal> {
        let name = name.trim();
        if name.is_empty() {
            return Err(SaveRefusal::EmptyName);
        }
        match self.name_clash(name, naming, None) {
            NameClash::Builtin(b) => Err(SaveRefusal::BuiltinName(b)),
            NameClash::User(id) => {
                let index = self.find_user(&id).ok_or(SaveRefusal::EmptyName)?;
                let layout = &mut self.user[index];
                layout.name = name.to_string();
                fill(layout, captured);
                Ok(SaveOutcome::Replaced(id))
            }
            NameClash::Free => {
                let id = self.fresh_id();
                let mut layout = WorkspaceLayout {
                    id: id.clone(),
                    name: name.to_string(),
                    dock: None,
                    split: None,
                    saved_at: None,
                    from_current: true,
                };
                fill(&mut layout, captured);
                self.user.push(layout);
                Ok(SaveOutcome::Added(id))
            }
        }
    }

    /// Update from current: a user layout takes the arrangement on screen; a
    /// built-in stores it as its override ("edited"). `false` for an unknown id.
    pub fn update_from_current(&mut self, id: &LayoutId, captured: Captured) -> bool {
        match id {
            LayoutId::Builtin(b) => {
                let mut layout = self.builtin_layout(*b);
                fill(&mut layout, captured);
                self.overrides.insert(*b, layout);
                true
            }
            LayoutId::User(_) => match self.find_user(id) {
                Some(index) => {
                    fill(&mut self.user[index], captured);
                    true
                }
                None => false,
            },
        }
    }

    /// Reset: drop a built-in's override. `false` when it had none.
    pub fn reset_builtin(&mut self, b: BuiltinLayout) -> bool {
        self.overrides.remove(&b).is_some()
    }

    /// Duplicate: a user layout named "<name> copy" (made unique), right after
    /// a user source, or last for a built-in.
    pub fn duplicate(&mut self, id: &LayoutId, naming: &Naming) -> Option<LayoutId> {
        let source = self.get(id)?;
        let source_name = self.name_of(id, naming)?;
        let name = self.unique_name(&(naming.copy_of)(&source_name), naming);
        let new_id = self.fresh_id();
        let copy = WorkspaceLayout {
            id: new_id.clone(),
            name,
            ..source
        };
        let at = self.find_user(id).map_or(self.user.len(), |i| i + 1);
        self.user.insert(at, copy);
        Some(new_id)
    }

    /// Rename a user layout. Built-ins keep their names; a name another
    /// layout uses is refused.
    pub fn rename(
        &mut self,
        id: &LayoutId,
        name: &str,
        naming: &Naming,
    ) -> Result<(), RenameRefusal> {
        if id.builtin().is_some() {
            return Err(RenameRefusal::Builtin);
        }
        let index = self.find_user(id).ok_or(RenameRefusal::Unknown)?;
        let name = name.trim();
        if name.is_empty() {
            return Err(RenameRefusal::EmptyName);
        }
        if self.name_clash(name, naming, Some(id)) != NameClash::Free {
            return Err(RenameRefusal::Taken);
        }
        self.user[index].name = name.to_string();
        Ok(())
    }

    /// Delete a user layout at once; the token puts it back.
    pub fn delete(&mut self, id: &LayoutId) -> Option<DeletedLayout> {
        let index = self.find_user(id)?;
        let layout = self.user.remove(index);
        let was_active = self.active.as_ref() == Some(id);
        if was_active {
            self.active = None;
        }
        Some(DeletedLayout {
            layout,
            index,
            was_active,
        })
    }

    /// Undo a delete: the layout returns at its index (or last, if the list
    /// is shorter now). A name taken meanwhile is made unique; an id taken
    /// meanwhile is replaced by a fresh one.
    pub fn undo_delete(&mut self, deleted: DeletedLayout, naming: &Naming) -> LayoutId {
        let mut layout = deleted.layout;
        if self.find_user(&layout.id).is_some() {
            layout.id = self.fresh_id();
        }
        layout.name = self.unique_name(&layout.name, naming);
        let id = layout.id.clone();
        let at = deleted.index.min(self.user.len());
        self.user.insert(at, layout);
        if deleted.was_active {
            self.active = Some(id.clone());
        }
        id
    }

    /// Move the user layout at `from` to `to` (an index of the final order).
    pub fn reorder(&mut self, from: usize, to: usize) -> bool {
        if from >= self.user.len() || from == to {
            return false;
        }
        let layout = self.user.remove(from);
        let to = to.min(self.user.len());
        self.user.insert(to, layout);
        true
    }

    /// Import: each layout is added last with a fresh id; a name in use gets
    /// " (2)", " (3)", ….
    pub fn import(&mut self, incoming: Vec<WorkspaceLayout>, naming: &Naming) -> Vec<LayoutId> {
        let mut added = Vec::new();
        for mut layout in incoming {
            layout.id = self.fresh_id();
            let base = if layout.name.trim().is_empty() {
                self.first_free_name(naming)
            } else {
                layout.name.trim().to_string()
            };
            layout.name = self.unique_name(&base, naming);
            added.push(layout.id.clone());
            self.user.push(layout);
        }
        added
    }
}

fn fill(layout: &mut WorkspaceLayout, captured: Captured) {
    layout.dock = captured.dock;
    layout.split = captured.split;
    layout.saved_at = Some(captured.saved_at);
    layout.from_current = true;
}
