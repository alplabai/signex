//! "Library Updates Available" modal — Stage 16 of
//! `v0.9-snxlib-as-file-plan.md` §3.5.
//!
//! Surfaces drift between a freshly-opened schematic's placed
//! `Symbol`s and their source library rows. Each placed Symbol that
//! went through the `.snxlib` picker carries `library_id`, `row_id`,
//! and `library_version` (Stage 16's schema additions on
//! `signex_types::schematic::Symbol`). On schematic open the
//! dispatcher walks every Symbol that has a `library_id` set and
//! compares its pinned version to the row's current version through
//! the mounted `LibrarySet`. Mismatches accumulate into a
//! [`LibraryUpdatesState`].
//!
//! In **Personal** workflow mode (the manifest's
//! `[workflow] mode = "personal"` default) drift is auto-applied to
//! every instance silently — no modal opens, just a `tracing::info`
//! line and a dirty mark on the schematic.
//!
//! In **Team** workflow mode the dispatcher opens this modal so the
//! user can review which updates to accept, with checkboxes
//! pre-toggled by `BumpKind`:
//!
//! - `Patch` — checked by default (compatible auto-update);
//! - `Minor` — unchecked by default (review before accepting);
//! - `Major` — unchecked + ⚠ warning (likely-breaking).
//!
//! "Update Selected Components" rewrites the picked Symbols to the
//! latest version + dirty-marks the schematic. "Skip All" leaves
//! everything pinned and the dispatcher records the schematic in
//! `LibraryState.skipped_updates_for` so the status bar can show a
//! persistent indicator until resolved.
//!
//! The matching `needs_overlay` predicate in
//! `app/view/mod.rs::view_main_for` includes
//! `library_updates.is_some()` — without it the modal's `collect_overlays`
//! contribution is discarded silently and clicks fall through (memory:
//! `[needs_overlay predicate gates modal rendering]`).

use std::path::PathBuf;

use iced::widget::{Space, button, checkbox, column, container, row, scrollable, text};
use iced::{Border, Element, Length, Theme};
use signex_library::RowId;
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;
use uuid::Uuid;

use super::messages::LibraryMessage;

/// Modal width — wider than the recovery dialogs (520 px) because the
/// drift list needs room for the ref-designator column + two version
/// strings + the bump-kind badge.
const MODAL_W: f32 = 640.0;
/// Cap the visible drift-list height so a 100-symbol schematic doesn't
/// produce a window-tall modal. The list scrolls past this height.
const LIST_MAX_H: f32 = 320.0;

/// Severity classification for a drift entry — drives the default
/// checkbox state and the warning glyph in the row label. Pure-data
/// (no semver crate dependency); see [`classify_bump`] for the
/// inference rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BumpKind {
    /// Patch bump (`1.0.0 → 1.0.1`) — same major + minor; treated as
    /// an opt-in default-on auto-update.
    Patch,
    /// Minor bump (`1.0.0 → 1.1.0`) — same major; treated as
    /// review-before-accepting (default off).
    Minor,
    /// Major bump (`1.0.0 → 2.0.0`) or any version pair where parsing
    /// the components fails — treated as likely-breaking. Default off
    /// + warning glyph.
    Major,
}

impl BumpKind {
    /// Default checkbox state for the "Update Selected Components"
    /// flow — patch-only is the safe default.
    pub fn default_checked(self) -> bool {
        matches!(self, BumpKind::Patch)
    }

    /// Short label for the badge column.
    pub fn label(self) -> &'static str {
        match self {
            BumpKind::Patch => "patch",
            BumpKind::Minor => "minor",
            BumpKind::Major => "major",
        }
    }
}

/// Classify a `(current, latest)` semver-style version pair into a
/// [`BumpKind`]. Both inputs are opaque-string-treated semver: the
/// rule splits on `.` and compares the leading two numeric segments.
/// Any parse failure or unequal-major pair upgrades to [`BumpKind::Major`].
pub fn classify_bump(current: &str, latest: &str) -> BumpKind {
    fn parts(s: &str) -> (Option<u32>, Option<u32>) {
        let mut iter = s.split('.');
        let major = iter.next().and_then(|s| s.parse::<u32>().ok());
        let minor = iter.next().and_then(|s| s.parse::<u32>().ok());
        (major, minor)
    }
    let (cur_maj, cur_min) = parts(current);
    let (lat_maj, lat_min) = parts(latest);
    match (cur_maj, lat_maj, cur_min, lat_min) {
        (Some(a), Some(b), _, _) if a != b => BumpKind::Major,
        (Some(_), Some(_), Some(a), Some(b)) if a != b => BumpKind::Minor,
        (Some(_), Some(_), Some(_), Some(_)) => BumpKind::Patch,
        _ => BumpKind::Major,
    }
}

/// One row in the modal — drift between a placed Symbol's pinned
/// version and its source row's current version.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct LibraryUpdateEntry {
    /// Schematic-side UUID for the placed `Symbol` — used to look up
    /// the instance back on apply (the engine indexes by Symbol UUID).
    pub symbol_uuid: Uuid,
    /// Reference designator (e.g. `R12`, `U3`) — sort key + display
    /// label.
    pub ref_des: String,
    /// Source library that owns the row.
    pub library_id: Uuid,
    /// Library's display name — pulled from `OpenLibrary.display_name`
    /// at scan-time so the modal can show the user a friendly source
    /// when the bump dialog opens.
    pub library_name: String,
    /// Row identity inside the library.
    pub row_id: RowId,
    /// Library path (absolute `.snxlib` file path) — kept on the entry
    /// so the apply step can re-resolve through `LibraryState::set`
    /// even if the row's adapter changed mounts between scan and apply.
    pub library_path: PathBuf,
    /// Pinned version on the placed Symbol.
    pub current_version: String,
    /// Row's current version inside the library.
    pub latest_version: String,
    /// Severity hint — drives the checkbox default + warning badge.
    pub bump_kind: BumpKind,
    /// Live checkbox state — pre-set from `bump_kind.default_checked()`
    /// on modal open; the dispatcher toggles via
    /// [`LibraryMessage::LibraryUpdatesToggleSelection`].
    pub selected: bool,
}

/// Modal state — owned by [`crate::library::LibraryState::library_updates`].
/// `None` while closed; populated by the schematic-open scan. Sorted
/// by `ref_des` (lexicographic with natural-numeric tail handling
/// would be nicer but isn't worth the dep here — `R12` sorts before
/// `R2` in pure lex; the user can re-sort visually if it bites).
#[derive(Debug, Clone)]
pub struct LibraryUpdatesState {
    /// Schematic the scan ran against — drift applies into this
    /// schematic's engine when the user clicks Update Selected. Kept
    /// so the dispatcher can re-resolve `engines.get_mut(&path)` at
    /// apply time without hauling an extra arg through the message.
    pub schematic_path: PathBuf,
    /// Drift rows, sorted by `ref_des`.
    pub entries: Vec<LibraryUpdateEntry>,
}

impl LibraryUpdatesState {
    /// Build a state from a freshly-collected drift list. Auto-sorts
    /// by `ref_des` and applies the `bump_kind.default_checked()`
    /// rule to each entry's checkbox.
    pub fn new(schematic_path: PathBuf, mut entries: Vec<LibraryUpdateEntry>) -> Self {
        entries.sort_by(|a, b| a.ref_des.cmp(&b.ref_des));
        for entry in &mut entries {
            entry.selected = entry.bump_kind.default_checked();
        }
        Self {
            schematic_path,
            entries,
        }
    }

    /// Toggle the checkbox for one entry, addressed by `symbol_uuid`.
    /// No-op when the uuid isn't present (e.g. the user dismissed and
    /// re-scanned).
    pub fn toggle(&mut self, symbol_uuid: Uuid) {
        if let Some(e) = self.entries.iter_mut().find(|e| e.symbol_uuid == symbol_uuid) {
            e.selected = !e.selected;
        }
    }

    /// Number of entries the user has currently checked.
    pub fn selected_count(&self) -> usize {
        self.entries.iter().filter(|e| e.selected).count()
    }
}

/// Render the modal card. Returns an `Element<LibraryMessage>` so the
/// caller can `.map(Message::Library)`.
pub fn view<'a>(
    state: &'a LibraryUpdatesState,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);

    let title_label = state
        .schematic_path
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| state.schematic_path.display().to_string());

    let header = container(
        row![
            text("Library Updates Available").size(14).color(text_c),
            Space::new().width(Length::Fill),
            text(title_label).size(11).color(muted),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([10, 14])
    .style(crate::styles::modal_header_strip(tokens));

    // Build the drift list. Each row is checkbox + ref_des + library
    // name + version pair + bump-kind badge.
    let mut rows: iced::widget::Column<'a, LibraryMessage> = column![].spacing(2);
    for entry in &state.entries {
        let row_view = render_entry_row(entry, text_c, muted, border);
        rows = rows.push(row_view);
    }

    let body = column![
        text(format!(
            "{} placed component{} drifted from the library since this schematic was last saved.",
            state.entries.len(),
            if state.entries.len() == 1 { "" } else { "s" }
        ))
        .size(11)
        .color(muted),
        Space::new().height(8),
        scrollable(rows).height(Length::Fixed(LIST_MAX_H)),
    ]
    .spacing(2);

    let selected = state.selected_count();
    let update_label = format!("Update Selected Components ({})", selected);

    let footer = container(
        row![
            secondary_btn(
                "Skip All",
                LibraryMessage::LibraryUpdatesSkipAll,
                text_c,
                border,
            ),
            Space::new().width(Length::Fill),
            secondary_btn(
                "Cancel",
                LibraryMessage::LibraryUpdatesCancel,
                text_c,
                border,
            ),
            Space::new().width(8),
            primary_btn(&update_label, LibraryMessage::LibraryUpdatesApply, selected),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([10, 14])
    .style(crate::styles::modal_footer_strip(tokens));

    container(
        column![header, container(body).padding([14, 16]), footer].width(Length::Fixed(MODAL_W)),
    )
    .style(crate::styles::modal_card(tokens))
    .clip(true)
    .into()
}

fn render_entry_row<'a>(
    entry: &'a LibraryUpdateEntry,
    text_c: iced::Color,
    muted: iced::Color,
    _border: iced::Color,
) -> Element<'a, LibraryMessage> {
    // Warning glyph for major bumps — leaves room for users to back
    // out before the click. Patch / minor get a neutral dot.
    let prefix = match entry.bump_kind {
        BumpKind::Major => "\u{26A0} ",
        _ => "  ",
    };
    let badge_color = match entry.bump_kind {
        BumpKind::Major => iced::Color::from_rgb(0.86, 0.40, 0.36),
        BumpKind::Minor => iced::Color::from_rgb(0.93, 0.65, 0.30),
        BumpKind::Patch => iced::Color::from_rgb(0.42, 0.74, 0.42),
    };

    let symbol_uuid = entry.symbol_uuid;
    let cb: Element<'a, LibraryMessage> = checkbox(entry.selected)
        .size(14)
        .on_toggle(move |_| LibraryMessage::LibraryUpdatesToggleSelection(symbol_uuid))
        .into();

    container(
        row![
            cb,
            Space::new().width(8),
            container(text(format!("{}{}", prefix, entry.ref_des)).size(12).color(text_c))
                .width(Length::Fixed(96.0)),
            container(text(&entry.library_name).size(11).color(muted))
                .width(Length::Fixed(160.0)),
            container(
                text(format!("{} \u{2192} {}", entry.current_version, entry.latest_version))
                    .size(11)
                    .color(text_c),
            )
            .width(Length::Fill),
            container(text(entry.bump_kind.label()).size(10).color(badge_color))
                .width(Length::Fixed(60.0)),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([4, 4])
    .into()
}

fn secondary_btn<'a>(
    label: &'a str,
    message: LibraryMessage,
    text_color: iced::Color,
    border: iced::Color,
) -> Element<'a, LibraryMessage> {
    button(container(text(label.to_string()).size(11).color(text_color)).padding([4, 14]))
        .on_press(message)
        .style(move |_: &Theme, _| iced::widget::button::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                1.0, 1.0, 1.0, 0.04,
            ))),
            text_color,
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: border,
            },
            ..iced::widget::button::Style::default()
        })
        .into()
}

fn primary_btn<'a>(
    label: &str,
    message: LibraryMessage,
    selected: usize,
) -> Element<'a, LibraryMessage> {
    let bg = if selected == 0 {
        iced::Color::from_rgba(0.0, 0.47, 0.84, 0.4)
    } else {
        iced::Color::from_rgb(0.00, 0.47, 0.84)
    };
    let fg = iced::Color::WHITE;
    let mut btn = button(container(text(label.to_string()).size(11).color(fg)).padding([4, 14]));
    if selected > 0 {
        btn = btn.on_press(message);
    }
    btn.style(move |_: &Theme, _| iced::widget::button::Style {
        background: Some(iced::Background::Color(bg)),
        text_color: fg,
        border: Border {
            width: 0.0,
            radius: 3.0.into(),
            ..Border::default()
        },
        ..iced::widget::button::Style::default()
    })
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// `classify_bump` distinguishes patch / minor / major by the
    /// leading two numeric segments. Anything that fails to parse
    /// upgrades to Major — defensive default since mismatched-format
    /// pairs likely indicate breaking schema drift.
    #[test]
    fn classify_bump_distinguishes_three_buckets() {
        assert_eq!(classify_bump("1.0.0", "1.0.1"), BumpKind::Patch);
        assert_eq!(classify_bump("1.0.0", "1.1.0"), BumpKind::Minor);
        assert_eq!(classify_bump("1.0.0", "2.0.0"), BumpKind::Major);
        // Unparseable → Major (defensive).
        assert_eq!(classify_bump("nightly", "main"), BumpKind::Major);
        // Mixed parse — only major component present in latest.
        assert_eq!(classify_bump("1", "2"), BumpKind::Major);
    }

    /// Default checkbox follows the bump kind: patch on, others off.
    #[test]
    fn default_checked_only_patches() {
        assert!(BumpKind::Patch.default_checked());
        assert!(!BumpKind::Minor.default_checked());
        assert!(!BumpKind::Major.default_checked());
    }

    /// `LibraryUpdatesState::new` sorts by `ref_des` and applies the
    /// patch-default-checked rule.
    #[test]
    fn new_sorts_by_ref_des_and_pre_checks_patches() {
        let path = PathBuf::from("/tmp/x.standard_sch");
        let library_path = PathBuf::from("/tmp/x.snxlib");
        let mk = |refdes: &str, kind: BumpKind| LibraryUpdateEntry {
            symbol_uuid: Uuid::new_v4(),
            ref_des: refdes.to_string(),
            library_id: Uuid::nil(),
            library_name: "Lib".to_string(),
            row_id: RowId::from_uuid(Uuid::nil()),
            library_path: library_path.clone(),
            current_version: "1.0.0".to_string(),
            latest_version: match kind {
                BumpKind::Patch => "1.0.1",
                BumpKind::Minor => "1.1.0",
                BumpKind::Major => "2.0.0",
            }
            .to_string(),
            bump_kind: kind,
            selected: false,
        };
        let state = LibraryUpdatesState::new(
            path,
            vec![
                mk("U3", BumpKind::Major),
                mk("R1", BumpKind::Patch),
                mk("C2", BumpKind::Minor),
            ],
        );
        // Sorted ascending by ref_des: C2 < R1 < U3.
        assert_eq!(state.entries[0].ref_des, "C2");
        assert_eq!(state.entries[1].ref_des, "R1");
        assert_eq!(state.entries[2].ref_des, "U3");
        // Only patch is pre-checked.
        assert!(!state.entries[0].selected); // minor
        assert!(state.entries[1].selected); // patch
        assert!(!state.entries[2].selected); // major
        assert_eq!(state.selected_count(), 1);
    }

    /// `toggle` flips the checkbox for a matching `symbol_uuid` and
    /// is a no-op for an unknown uuid.
    #[test]
    fn toggle_flips_only_matching_uuid() {
        let path = PathBuf::from("/tmp/x.standard_sch");
        let library_path = PathBuf::from("/tmp/x.snxlib");
        let id = Uuid::new_v4();
        let other = Uuid::new_v4();
        let mut state = LibraryUpdatesState::new(
            path,
            vec![LibraryUpdateEntry {
                symbol_uuid: id,
                ref_des: "R1".to_string(),
                library_id: Uuid::nil(),
                library_name: "Lib".to_string(),
                row_id: RowId::from_uuid(Uuid::nil()),
                library_path: library_path.clone(),
                current_version: "1.0.0".to_string(),
                latest_version: "1.0.1".to_string(),
                bump_kind: BumpKind::Patch,
                selected: false,
            }],
        );
        // Started pre-checked because of the patch default.
        assert!(state.entries[0].selected);
        state.toggle(id);
        assert!(!state.entries[0].selected);
        // Unknown uuid is a no-op.
        state.toggle(other);
        assert!(!state.entries[0].selected);
    }
}
