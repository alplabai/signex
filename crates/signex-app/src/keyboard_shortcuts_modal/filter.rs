//! What the sheet shows for a filter and a scope: pure functions over the
//! cached sections, so the view only lays the result out.

use signex_i18n::I18n;

use super::ShortcutSection;
use crate::keymap::{KeymapEditorRow, ShortcutContext, ShortcutTrigger};

/// Which editor's shortcuts the sheet lists.
///
/// The rule: Schematic hides the rows of the PCB, footprint and library
/// editors; PCB hides the schematic, footprint and library rows; All shows
/// every row. Global, modal, text-field, palette and placement rows show in
/// every scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShortcutScope {
    #[default]
    Schematic,
    Pcb,
    All,
}

impl ShortcutScope {
    /// The scopes in the order the segmented switch shows them.
    pub const ALL: [Self; 3] = [Self::Schematic, Self::Pcb, Self::All];

    /// The scope the sheet opens on, from the context of the editor in
    /// front: Schematic or PCB for those editors. Every other editor (the
    /// footprint and library editors, which neither scope lists) and no
    /// editor at all open on All, so the sheet never opens without the
    /// active editor's own shortcuts.
    pub fn for_active_editor(editor: Option<ShortcutContext>) -> Self {
        match editor {
            Some(ShortcutContext::Schematic) => Self::Schematic,
            Some(ShortcutContext::Pcb) => Self::Pcb,
            _ => Self::All,
        }
    }

    /// Whether a row bound in `context` is listed in this scope. Exhaustive
    /// on both sides: a new context or scope must be placed here.
    pub fn shows(self, context: ShortcutContext) -> bool {
        match context {
            ShortcutContext::Global
            | ShortcutContext::Modal
            | ShortcutContext::TextInput
            | ShortcutContext::CommandPalette
            | ShortcutContext::Placement => true,
            ShortcutContext::Footprint | ShortcutContext::Library => match self {
                Self::All => true,
                Self::Schematic | Self::Pcb => false,
            },
            ShortcutContext::Schematic => match self {
                Self::Schematic | Self::All => true,
                Self::Pcb => false,
            },
            ShortcutContext::Pcb | ShortcutContext::Board3d => match self {
                Self::Pcb | Self::All => true,
                Self::Schematic => false,
            },
        }
    }
}

/// The sheet's own state: the filter text and the scope. Set afresh each
/// time the sheet opens.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ShortcutSheet {
    pub filter: String,
    pub scope: ShortcutScope,
}

impl ShortcutSheet {
    /// A sheet just opened: no filter, on `scope`.
    pub fn opened(scope: ShortcutScope) -> Self {
        Self {
            filter: String::new(),
            scope,
        }
    }

    /// Whether a filter is typed (blanks do not count).
    pub fn is_filtered(&self) -> bool {
        !self.filter.trim().is_empty()
    }
}

/// One command as the sheet lists it (mockup row 13, #3047): one row with
/// every key the command has in the scope, in profile order. A key bound
/// in two contexts is listed once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShownEntry<'a> {
    pub rows: Vec<&'a KeymapEditorRow>,
}

impl<'a> ShownEntry<'a> {
    /// The command's label, as its first row writes it.
    pub fn label(&self) -> &'a str {
        self.rows
            .first()
            .copied()
            .map_or("", |row| row.label.as_str())
    }

    /// Whether `row` is a key of this entry's command: the same command
    /// id, or for rows without one the same label.
    fn takes(&self, row: &KeymapEditorRow) -> bool {
        self.rows
            .first()
            .is_some_and(|first| match (&first.command, &row.command) {
                (Some(a), Some(b)) => a == b,
                (None, None) => first.label == row.label,
                _ => false,
            })
    }
}

/// One category as the sheet shows it: the commands in scope that match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShownSection<'a> {
    pub category: &'a str,
    pub entries: Vec<ShownEntry<'a>>,
}

/// The sections the sheet shows, how many commands they hold and how many
/// commands the scope has without the filter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listing<'a> {
    pub sections: Vec<ShownSection<'a>>,
    pub shown: usize,
    pub total: usize,
}

/// The commands of `sections` in `sheet`'s scope with a key that matches
/// its filter, by [`KeymapEditorRow::matches_query_in`] (label, key or
/// command id). A matching command shows all its keys; a category with no
/// such command is left out.
pub fn listing<'a>(
    i18n: &I18n,
    sections: &'a [ShortcutSection],
    sheet: &ShortcutSheet,
) -> Listing<'a> {
    let (mut shown, mut total) = (0, 0);
    let mut out = Vec::new();
    for section in sections {
        let mut entries = commands_in_scope(section, sheet.scope);
        total += entries.len();
        entries.retain(|entry| {
            entry
                .rows
                .iter()
                .any(|row| row.matches_query_in(i18n, &sheet.filter))
        });
        shown += entries.len();
        if !entries.is_empty() {
            out.push(ShownSection {
                category: &section.category,
                entries,
            });
        }
    }
    Listing {
        sections: out,
        shown,
        total,
    }
}

/// The rows of `section` that `scope` shows, one entry per command.
fn commands_in_scope(section: &ShortcutSection, scope: ShortcutScope) -> Vec<ShownEntry<'_>> {
    let mut entries: Vec<ShownEntry<'_>> = Vec::new();
    for row in section.rows.iter().filter(|r| scope.shows(r.context)) {
        match entries.iter_mut().find(|entry| entry.takes(row)) {
            Some(entry) => {
                if !entry
                    .rows
                    .iter()
                    .any(|key| key.trigger_config == row.trigger_config)
                {
                    entry.rows.push(row);
                }
            }
            None => entries.push(ShownEntry { rows: vec![row] }),
        }
    }
    entries
}

/// Where the right column starts: the left column takes whole sections
/// until it holds at least half the commands, so the two columns end near
/// the same height.
pub fn column_split(sections: &[ShownSection<'_>]) -> usize {
    let total: usize = sections.iter().map(|s| s.entries.len()).sum();
    let mut left = 0;
    for (index, section) in sections.iter().enumerate() {
        if 2 * left >= total {
            return index;
        }
        left += section.entries.len();
    }
    sections.len()
}

/// The key caps of a row: one per stroke of a key sequence (`P W` gives
/// two), one for a combo (`Shift+Space`) or a pointer gesture.
pub fn trigger_caps(row: &KeymapEditorRow) -> Vec<&str> {
    caps_of(&row.trigger_config, &row.trigger)
}

/// [`trigger_caps`] of a trigger in profile spelling (`config`) shown as
/// `shown`. The shown text splits on blanks only when it has one part per
/// stroke; anything else stays one cap.
fn caps_of<'t>(config: &str, shown: &'t str) -> Vec<&'t str> {
    let shown = shown.trim();
    if let Ok(ShortcutTrigger::KeySequence(strokes)) = ShortcutTrigger::parse(config) {
        let parts: Vec<&str> = shown.split_whitespace().collect();
        if parts.len() == strokes.len() {
            return parts;
        }
    }
    vec![shown]
}

/// Whether the key cap `cap` matches `filter` (drawn with the accent
/// border), both folded for search in the message language, as a
/// translated key name is.
pub fn cap_matches(i18n: &I18n, cap: &str, filter: &str) -> bool {
    let wanted = i18n.fold_for_search(filter.trim());
    !wanted.is_empty() && i18n.fold_for_search(cap).contains(&wanted)
}

/// `label` as runs of plain and matched text: the first contiguous
/// occurrence of `filter`, folded for search per character so the runs line
/// up with the label. No occurrence (a match on the key or the command id)
/// gives one plain run.
pub fn label_runs(i18n: &I18n, label: &str, filter: &str) -> Vec<(String, bool)> {
    let fold = |c: char| i18n.fold_char_for_search(c);
    let chars: Vec<char> = label.chars().collect();
    let folded: Vec<char> = chars.iter().copied().map(fold).collect();
    let wanted: Vec<char> = filter.trim().chars().map(fold).collect();
    let hit = (!wanted.is_empty() && wanted.len() <= folded.len())
        .then(|| {
            (0..=folded.len() - wanted.len()).find(|&i| folded[i..i + wanted.len()] == wanted[..])
        })
        .flatten();
    let Some(start) = hit else {
        return vec![(label.to_string(), false)];
    };
    let end = start + wanted.len();
    [
        (chars[..start].iter().collect::<String>(), false),
        (chars[start..end].iter().collect::<String>(), true),
        (chars[end..].iter().collect::<String>(), false),
    ]
    .into_iter()
    .filter(|(run, _)| !run.is_empty())
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::{KeymapEditorModel, ShortcutProfileSet};

    fn en() -> &'static I18n {
        I18n::en_us_ref()
    }

    fn sections() -> Vec<ShortcutSection> {
        let set = ShortcutProfileSet::built_ins().expect("built-ins load");
        super::super::build_sections(en(), &set)
    }

    fn sheet(filter: &str, scope: ShortcutScope) -> ShortcutSheet {
        ShortcutSheet {
            filter: filter.to_string(),
            scope,
        }
    }

    fn entries<'a>(listing: &'a Listing<'a>) -> Vec<&'a ShownEntry<'a>> {
        listing.sections.iter().flat_map(|s| &s.entries).collect()
    }

    fn rows<'a>(listing: &'a Listing<'a>) -> Vec<&'a KeymapEditorRow> {
        entries(listing)
            .into_iter()
            .flat_map(|e| e.rows.iter().copied())
            .collect()
    }

    /// How many commands `section` has: rows of one command id, or without
    /// one of one label, count once.
    fn commands_of(section: &ShortcutSection) -> usize {
        let mut seen = std::collections::BTreeSet::new();
        for row in &section.rows {
            seen.insert(match &row.command {
                Some(command) => (command.as_str().to_string(), String::new()),
                None => (String::new(), row.label.clone()),
            });
        }
        seen.len()
    }

    #[test]
    fn the_filter_matches_by_label_and_highlights_it() {
        let sections = sections();
        let listing = listing(en(), &sections, &sheet("undo", ShortcutScope::All));
        let found = rows(&listing);
        assert!(!found.is_empty());
        assert_eq!(listing.shown, entries(&listing).len());
        let undo = found
            .iter()
            .find(|r| r.label.to_lowercase().contains("undo"))
            .expect("Undo is listed");
        let runs = label_runs(en(), &undo.label, "undo");
        let hits: Vec<&str> = runs
            .iter()
            .filter(|(_, hit)| *hit)
            .map(|(run, _)| run.as_str())
            .collect();
        assert_eq!(hits.len(), 1, "{runs:?}");
        assert_eq!(hits[0].to_lowercase(), "undo");
        let joined: String = runs.iter().map(|(run, _)| run.as_str()).collect();
        assert_eq!(joined, undo.label, "the runs spell the label");
    }

    #[test]
    fn the_filter_matches_by_key_and_marks_the_cap() {
        let sections = sections();
        let all = listing(en(), &sections, &sheet("", ShortcutScope::All));
        let row = rows(&all)
            .into_iter()
            .find(|r| trigger_caps(r).len() == 1 && r.trigger.chars().count() > 1)
            .expect("a row with a one-cap combo");
        let key = row.trigger.clone();
        let listing = listing(en(), &sections, &sheet(&key, ShortcutScope::All));
        assert!(
            rows(&listing).iter().any(|r| r.trigger == key),
            "filtering by {key} lists its row"
        );
        assert!(
            trigger_caps(row)
                .iter()
                .any(|cap| cap_matches(en(), cap, &key))
        );
        assert!(!cap_matches(en(), "Ctrl+Z", "wire"));
        assert!(!cap_matches(en(), "Ctrl+Z", "  "));
        assert!(cap_matches(en(), "Shift+Space", "space"));
        // A key match leaves the label plain.
        assert_eq!(
            label_runs(en(), "Undo", "Ctrl+Z"),
            vec![("Undo".into(), false)]
        );
    }

    #[test]
    fn an_empty_filter_lists_every_row_of_the_scope() {
        let sections = sections();
        let listing = listing(en(), &sections, &sheet("  ", ShortcutScope::All));
        let every: usize = sections.iter().map(commands_of).sum();
        assert_eq!(listing.shown, every);
        assert_eq!(listing.total, every);
        assert!(!sheet("  ", ShortcutScope::All).is_filtered());
        assert!(sheet("w", ShortcutScope::All).is_filtered());
    }

    #[test]
    fn the_scope_hides_the_other_editors_rows() {
        use ShortcutContext as C;
        let schematic = ShortcutScope::Schematic;
        let pcb = ShortcutScope::Pcb;
        assert!(schematic.shows(C::Schematic) && !schematic.shows(C::Pcb));
        assert!(pcb.shows(C::Pcb) && !pcb.shows(C::Schematic));
        for scope in [schematic, pcb] {
            assert!(!scope.shows(C::Footprint) && !scope.shows(C::Library));
            for shared in [
                C::Global,
                C::Modal,
                C::TextInput,
                C::CommandPalette,
                C::Placement,
            ] {
                assert!(scope.shows(shared), "{scope:?} {shared:?}");
            }
        }
        for context in [C::Schematic, C::Pcb, C::Footprint, C::Library, C::Global] {
            assert!(ShortcutScope::All.shows(context));
        }

        let sections = sections();
        for scope in ShortcutScope::ALL {
            let listing = listing(en(), &sections, &sheet("", scope));
            assert!(rows(&listing).iter().all(|r| scope.shows(r.context)));
            assert_eq!(listing.shown, listing.total);
        }
        let all = listing(en(), &sections, &sheet("", ShortcutScope::All)).total;
        let sch = listing(en(), &sections, &sheet("", schematic)).total;
        assert!(sch < all, "Schematic hides some rows: {sch} of {all}");
    }

    #[test]
    fn the_sheet_opens_on_the_active_editors_scope_with_no_filter() {
        assert_eq!(
            ShortcutScope::for_active_editor(Some(ShortcutContext::Pcb)),
            ShortcutScope::Pcb
        );
        assert_eq!(
            ShortcutScope::for_active_editor(Some(ShortcutContext::Schematic)),
            ShortcutScope::Schematic
        );
        for other in [
            Some(ShortcutContext::Footprint),
            Some(ShortcutContext::Library),
            None,
        ] {
            assert_eq!(
                ShortcutScope::for_active_editor(other),
                ShortcutScope::All,
                "{other:?}"
            );
        }
        let opened = ShortcutSheet::opened(ShortcutScope::Pcb);
        assert!(opened.filter.is_empty());
        assert_eq!(opened.scope, ShortcutScope::Pcb);
    }

    #[test]
    fn a_filter_with_no_match_lists_nothing() {
        let sections = sections();
        let listing = listing(en(), &sections, &sheet("zzzqqq", ShortcutScope::Schematic));
        assert!(listing.sections.is_empty());
        assert_eq!(listing.shown, 0);
        assert!(listing.total > 0);
    }

    #[test]
    fn key_caps_split_a_sequence_but_not_a_combo_or_a_gesture() {
        assert_eq!(caps_of("P W", "P W"), vec!["P", "W"]);
        assert_eq!(caps_of("Shift+Space", "Shift+Space"), vec!["Shift+Space"]);
        assert_eq!(caps_of("Ctrl+Click", "Ctrl+Click"), vec!["Ctrl+Click"]);
        assert_eq!(caps_of("Ctrl+K S", "Ctrl+K S"), vec!["Ctrl+K", "S"]);
        // Shown text that does not split one part per stroke stays one cap.
        assert_eq!(caps_of("P", "Page Up"), vec!["Page Up"]);
        // A real two-stroke row of a built-in profile.
        let model = KeymapEditorModel::built_ins().expect("built-ins load");
        let rows = model.rows();
        let sequence = rows
            .iter()
            .find(|r| r.trigger_config.split_whitespace().count() == 2)
            .expect("a two-stroke binding");
        assert_eq!(trigger_caps(sequence).len(), 2, "{:?}", sequence.trigger);
    }

    #[test]
    fn the_columns_split_near_half_the_rows() {
        let row = KeymapEditorModel::built_ins().expect("built-ins").rows()[0].clone();
        let owned: Vec<ShortcutSection> = [5usize, 1, 1, 1, 1, 1]
            .iter()
            .enumerate()
            .map(|(i, n)| ShortcutSection {
                category: format!("c{i}"),
                rows: vec![row.clone(); *n],
            })
            .collect();
        let shown: Vec<ShownSection<'_>> = owned
            .iter()
            .map(|s| ShownSection {
                category: &s.category,
                entries: s
                    .rows
                    .iter()
                    .map(|row| ShownEntry { rows: vec![row] })
                    .collect(),
            })
            .collect();
        // 10 rows: the 5-row section fills the left half.
        assert_eq!(column_split(&shown), 1);
        assert_eq!(column_split(&shown[1..]), 3);
        assert_eq!(column_split(&[]), 0);
    }

    /// A section where the first row's command has a second key
    /// (`F12 F11`), the same key again in another context, and one other
    /// command.
    fn two_key_section() -> ShortcutSection {
        let rows = KeymapEditorModel::built_ins().expect("built-ins").rows();
        let first = rows
            .iter()
            .find(|r| {
                r.command.is_some()
                    && r.context == ShortcutContext::Schematic
                    && !r.trigger_config.trim().is_empty()
            })
            .expect("a schematic command")
            .clone();
        let other = rows
            .iter()
            .find(|r| r.command.is_some() && r.command != first.command)
            .expect("another command")
            .clone();
        let mut second_key = first.clone();
        second_key.trigger = "F12 F11".into();
        second_key.trigger_english = "F12 F11".into();
        second_key.trigger_config = "F12 F11".into();
        let mut same_key_again = first.clone();
        same_key_again.context = ShortcutContext::Global;
        ShortcutSection {
            category: first.category.clone(),
            rows: vec![first, second_key, same_key_again, other],
        }
    }

    /// Mockup row 13, #3047: a command with two keys is one row showing
    /// both, a key bound in two contexts shows once, and the count counts
    /// commands.
    #[test]
    fn a_command_with_two_keys_is_one_row_with_both_keys() {
        let sections = vec![two_key_section()];
        let first = &sections[0].rows[0];
        let listed = listing(en(), &sections, &sheet("", ShortcutScope::All));
        assert_eq!((listed.shown, listed.total), (2, 2));
        let commands = entries(&listed);
        assert_eq!(commands[0].label(), first.label);
        let keys: Vec<&str> = commands[0]
            .rows
            .iter()
            .map(|r| r.trigger_config.as_str())
            .collect();
        assert_eq!(keys, [first.trigger_config.as_str(), "F12 F11"]);
        assert_eq!(commands[1].rows.len(), 1);

        // A filter on the second key lists the command with both keys.
        let by_key = listing(en(), &sections, &sheet("F12 F11", ShortcutScope::All));
        assert_eq!((by_key.shown, by_key.total), (1, 2));
        assert_eq!(entries(&by_key)[0].rows.len(), 2);
    }
}
