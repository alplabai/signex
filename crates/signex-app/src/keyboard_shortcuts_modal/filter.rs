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

    /// Whether a row bound in `context` is listed in this scope.
    pub fn shows(self, context: ShortcutContext) -> bool {
        match (self, context) {
            (Self::All, _) => true,
            (_, ShortcutContext::Footprint | ShortcutContext::Library) => false,
            (Self::Schematic, ShortcutContext::Pcb) => false,
            (Self::Pcb, ShortcutContext::Schematic) => false,
            _ => true,
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

/// One category as the sheet shows it: the rows in scope that match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShownSection<'a> {
    pub category: &'a str,
    pub rows: Vec<&'a KeymapEditorRow>,
}

/// The sections the sheet shows, how many rows they hold and how many rows
/// the scope has without the filter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listing<'a> {
    pub sections: Vec<ShownSection<'a>>,
    pub shown: usize,
    pub total: usize,
}

/// The rows of `sections` in `sheet`'s scope that match its filter, by
/// [`KeymapEditorRow::matches_query_in`] (label, key or command id). A
/// category with no such row is left out.
pub fn listing<'a>(
    i18n: &I18n,
    sections: &'a [ShortcutSection],
    sheet: &ShortcutSheet,
) -> Listing<'a> {
    let (mut shown, mut total) = (0, 0);
    let mut out = Vec::new();
    for section in sections {
        let mut rows = Vec::new();
        for row in section.rows.iter().filter(|r| sheet.scope.shows(r.context)) {
            total += 1;
            if row.matches_query_in(i18n, &sheet.filter) {
                rows.push(row);
            }
        }
        shown += rows.len();
        if !rows.is_empty() {
            out.push(ShownSection {
                category: &section.category,
                rows,
            });
        }
    }
    Listing {
        sections: out,
        shown,
        total,
    }
}

/// Where the right column starts: the left column takes whole sections
/// until it holds at least half the rows, so the two columns end near the
/// same height.
pub fn column_split(sections: &[ShownSection<'_>]) -> usize {
    let total: usize = sections.iter().map(|s| s.rows.len()).sum();
    let mut left = 0;
    for (index, section) in sections.iter().enumerate() {
        if 2 * left >= total {
            return index;
        }
        left += section.rows.len();
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

    fn rows<'a>(listing: &'a Listing<'a>) -> Vec<&'a KeymapEditorRow> {
        listing
            .sections
            .iter()
            .flat_map(|s| s.rows.iter().copied())
            .collect()
    }

    #[test]
    fn the_filter_matches_by_label_and_highlights_it() {
        let sections = sections();
        let listing = listing(en(), &sections, &sheet("undo", ShortcutScope::All));
        let found = rows(&listing);
        assert!(!found.is_empty());
        assert_eq!(listing.shown, found.len());
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
        let every: usize = sections.iter().map(|s| s.rows.len()).sum();
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
                rows: s.rows.iter().collect(),
            })
            .collect();
        // 10 rows: the 5-row section fills the left half.
        assert_eq!(column_split(&shown), 1);
        assert_eq!(column_split(&shown[1..]), 3);
        assert_eq!(column_split(&[]), 0);
    }
}
