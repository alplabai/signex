//! Document tab bar — tabs for open schematic sheets and PCB.

use iced::widget::scrollable::{Direction, RelativeOffset, Scrollbar};
use iced::widget::{Row, container, mouse_area, row, scrollable, text, tooltip};
use iced::{Element, Length};
use signex_types::theme::ThemeTokens;
use signex_widgets::tab_pill::{AccentPosition, TabPill, TabPillStyle};

use crate::app::TabInfo;
use crate::styles::{self, Spacing, TypeScale};
/// Default type / spacing scales; equal to the old literals at the default size.
const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

#[derive(Debug, Clone)]
pub enum TabMessage {
    Select(usize),
    /// User pressed the mouse on this tab. Arms the tab for
    /// drag-to-undock — if the cursor later leaves the main window, the
    /// tab auto-detaches.
    ///
    /// Carries no press position: `mouse_area::on_press` hands back a
    /// fixed message with no cursor in it, so the handler reads the live
    /// position from `interaction_state.last_mouse_pos` instead.
    StartDrag(usize),
    /// Right-click on this tab — the app opens the per-tab context
    /// menu (Close \[filename\] / Close All Others / Close All / Open
    /// In New Window). Replaces the inline close-X / undock buttons
    /// that used to live on every tab.
    ContextMenu(usize),
}

/// Stable id of the tab strip's scrollable, so the active tab can be
/// scrolled into view from `update`.
const TAB_STRIP_ID: &str = "document-tab-strip";

/// Relative scroll offset that brings tab `active` of `count` into view.
/// Tab widths vary with their titles, so this is the index's proportional
/// position along the strip: exact at both ends (newly appended tab =
/// end), close enough in between for a tab to land on screen.
fn reveal_offset(active: usize, count: usize) -> RelativeOffset {
    let x = if count < 2 {
        0.0
    } else {
        active.min(count - 1) as f32 / (count - 1) as f32
    };
    RelativeOffset { x, y: 0.0 }
}

/// Task scrolling the tab strip so tab `active` (of `count`) is visible.
pub fn reveal_active_tab<M: Send + 'static>(active: usize, count: usize) -> iced::Task<M> {
    iced::widget::operation::snap_to(TAB_STRIP_ID, reveal_offset(active, count))
}

pub fn view<'a>(
    tabs: &[TabInfo],
    active: usize,
    dragging: Option<usize>,
    visible_paths: &std::collections::HashSet<std::path::PathBuf>,
    dirty_paths: &std::collections::HashSet<std::path::PathBuf>,
    read_only_paths: &std::collections::HashSet<std::path::PathBuf>,
    tokens: &ThemeTokens,
) -> Element<'a, TabMessage> {
    // Tabs sit flush against each other — Altium parity. The active
    // pill's accent underline distinguishes it from neighbours; no
    // gap or vertical divider needed.
    let mut bar = Row::new().spacing(0.0);

    let text_primary = styles::ti(tokens.text);
    let text_muted = styles::ti(tokens.text_secondary);

    // Collect the indices of tabs that should render in this window —
    // we need the count up front so each pill knows whether it's the
    // last one (which draws the right edge; the rest skip it so
    // adjacent tabs share their L/R divider as a single 1 px line).
    let visible_indices: Vec<usize> = tabs
        .iter()
        .enumerate()
        .filter_map(|(i, tab)| visible_paths.contains(&tab.path).then_some(i))
        .collect();
    let last_idx = visible_indices.last().copied();

    for &i in &visible_indices {
        let tab = &tabs[i];
        // Tab title. A document with unsaved changes also gets a dot
        // (the Projects tree row keeps its own); there is no close-X on
        // the tab any more, so the two no longer compete.
        let label = ellipsize_title(&tab.title, MAX_TAB_TITLE_CHARS);

        let is_active = i == active;
        let is_dragging = dragging == Some(i);
        let is_last = Some(i) == last_idx;
        let text_c = if is_active { text_primary } else { text_muted };

        // No inline close-X / undock buttons. Both actions live in the
        // right-click menu now (Altium parity) — the tab itself is the
        // entire hit target.
        //
        // The pill itself is a custom widget (`signex_widgets::TabPill`)
        // that paints its own bg + 3-sided border (top + L/R only) +
        // 2 px accent strip below. Iced's stock Border can't do "top
        // and sides only", and a stacked-bg fake leaked accent through
        // the rounded corners (visible on dark themes).
        let pill_style = TabPillStyle {
            fill: pill_fill(tokens, is_active, is_dragging),
            // Theme border colour — reads as a subtle divider on
            // each theme's chrome surface. Tried pure black; it
            // looked harsh on every theme.
            border: styles::ti(tokens.border),
            accent: styles::ti(tokens.accent),
            is_active,
            is_last,
            // Document tabs hang from the top of the editing area:
            // rounded top corners + accent stripe at the bottom.
            accent_position: AccentPosition::Bottom,
        };
        let mut content = row![]
            .spacing(TAB_FONT_SIZE * 0.5)
            .align_y(iced::Alignment::Center);
        if tab_is_dirty(tab, dirty_paths) {
            content = content.push(dirty_dot(tokens));
        }
        let is_read_only = read_only_paths.contains(&tab.path);
        if is_read_only {
            content = content.push(
                text(READ_ONLY_LABEL)
                    .size(TAB_FONT_SIZE)
                    .color(styles::ti(tokens.text_secondary)),
            );
        }
        let inner = container(content.push(text(label).size(TAB_FONT_SIZE).color(text_c)))
            .padding([SP.s, 10.0]);
        // Cursor: default Pointer on hover so the bar reads as
        // "clickable tabs", switching to Grabbing only while a tab
        // is in flight so the drag-to-reorder / drag-to-undock
        // gesture stays advertised. Always-on Grab on hover read
        // as the move cursor and felt heavy.
        let cursor = if dragging.is_some() {
            iced::mouse::Interaction::Grabbing
        } else {
            iced::mouse::Interaction::Pointer
        };
        let tab_el: Element<'_, TabMessage> = mouse_area(TabPill::new(inner, pill_style))
            .on_press(TabMessage::StartDrag(i))
            .on_release(TabMessage::Select(i))
            .on_right_press(TabMessage::ContextMenu(i))
            .interaction(cursor)
            .into();
        // The pill shows an ellipsized title; hovering names the full path.
        let mut tip = tab.path.display().to_string();
        if is_read_only {
            tip.push('\n');
            tip.push_str(READ_ONLY_TOOLTIP);
        }
        let tab_el: Element<'_, TabMessage> = tooltip(
            tab_el,
            container(text(tip).size(TAB_FONT_SIZE)).padding([SP.xs, 6.0]),
            tooltip::Position::Bottom,
        )
        .style(styles::context_menu(tokens))
        .into();
        bar = bar.push(tab_el);
    }

    // Strip baseline — 1 px theme-border line at the bottom of the
    // tab bar. Inactive tabs sit "on" the line; the active tab's
    // accent stripe overlays it at its x-range, visually punching
    // through. Color follows the theme's border token so it reads
    // as part of the chrome rather than a hard black edge.
    let baseline_color = styles::ti(tokens.border);
    let baseline = container(iced::widget::Space::new())
        .width(Length::Fill)
        .height(1)
        .style(move |_: &iced::Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(baseline_color)),
            ..iced::widget::container::Style::default()
        });
    container(iced::widget::column![
        // More tabs than fit scroll sideways (wheel / thin scrollbar) rather
        // than being clipped off the right edge.
        container(
            scrollable(bar)
                .id(TAB_STRIP_ID)
                .direction(Direction::Horizontal(
                    Scrollbar::new().width(3).scroller_width(3),
                ))
                .width(Length::Fill),
        )
        .width(Length::Fill)
        // Tabs hang from the top; the baseline below them sits at the
        // strip's fixed height.
        .height(Length::Fill)
        .padding(iced::Padding {
            top: 2.0,
            right: 6.0,
            bottom: 0.0,
            left: 6.0,
        }),
        baseline,
    ])
    .width(Length::Fill)
    .height(TAB_STRIP_HEIGHT)
    .style(styles::toolbar_strip(tokens))
    .into()
}

/// Height of the document tab strip: two body-text lines for the pill plus
/// the strip's top padding, 26 px. Both the strip and the empty-window
/// placeholder take their height from here, so they cannot diverge and the
/// chrome does not jump when the first tab opens. It is derived from the
/// default type scale and spacing, the same constants the strip's text and
/// padding use; the app has no runtime UI scale for it to follow.
pub const TAB_STRIP_HEIGHT: f32 = TS.body * 2.0 + SP.s;

/// The strip drawn with no document open: same height and background as the
/// tab strip, nothing in it.
pub fn placeholder<'a, M: 'a>(tokens: &ThemeTokens) -> Element<'a, M> {
    container(iced::widget::Space::new())
        .width(Length::Fill)
        .height(TAB_STRIP_HEIGHT)
        .style(styles::toolbar_strip(tokens))
        .into()
}

/// Muted marker before the title of a read-only sheet (#1402).
const READ_ONLY_LABEL: &str = "read-only";

/// Tooltip line saying why a sheet is read-only.
const READ_ONLY_TOOLTIP: &str = "Read-only: written by a newer Signex. Edit it in that version.";

/// Tab label font size; the dirty dot is sized from it.
const TAB_FONT_SIZE: f32 = TS.body;

/// Dirty dot diameter as a fraction of the tab font size.
const DIRTY_DOT_EM: f32 = 0.6;

/// Dirty dot diameter for a tab label of `font_size`.
fn dirty_dot_diameter(font_size: f32) -> f32 {
    font_size * DIRTY_DOT_EM
}

/// Whether `tab` has unsaved changes: its own flag, or its path is in
/// `dirty_paths`. A Component Preview tab has a synthetic path that is
/// never in the app-wide set, so callers pass the set from
/// [`dirty_tab_paths`], which adds those tabs.
pub(crate) fn tab_is_dirty(
    tab: &TabInfo,
    dirty_paths: &std::collections::HashSet<std::path::PathBuf>,
) -> bool {
    tab.dirty || dirty_paths.contains(&tab.path)
}

/// The app-wide dirty set plus the paths of Component Preview tabs whose
/// editor (in `library.editors`) has unsaved edits. Those tabs are keyed
/// by row, not file, so only the library state knows they are dirty.
pub(crate) fn dirty_tab_paths(
    tabs: &[TabInfo],
    dirty_paths: &std::collections::HashSet<std::path::PathBuf>,
    library: &crate::library::state::LibraryState,
) -> std::collections::HashSet<std::path::PathBuf> {
    let mut out = dirty_paths.clone();
    for tab in tabs {
        if let crate::app::TabKind::ComponentEditor(ce) = &tab.kind {
            let address = crate::library::state::EditorAddress::new(
                ce.library_path.clone(),
                ce.table.clone(),
                ce.row_id,
            );
            if library
                .editors
                .get(&address)
                .is_some_and(|e| e.has_unsaved_edits())
            {
                out.insert(tab.path.clone());
            }
        }
    }
    out
}

/// Paths of the schematic tabs whose engine holds a read-only sheet, one
/// written by a newer Signex (#1402). Derived from the engines on every
/// view, so it can never disagree with what the gateway refuses.
pub(crate) fn read_only_tab_paths(
    tabs: &[TabInfo],
    engines: &std::collections::HashMap<std::path::PathBuf, signex_engine::Engine>,
) -> std::collections::HashSet<std::path::PathBuf> {
    tabs.iter()
        .filter(|tab| {
            engines
                .get(&tab.path)
                .is_some_and(|engine| engine.document().is_read_only())
        })
        .map(|tab| tab.path.clone())
        .collect()
}

/// Round dot in the theme's error colour, shown before a dirty tab's title.
fn dirty_dot<'a>(tokens: &ThemeTokens) -> Element<'a, TabMessage> {
    let colour = styles::ti(tokens.error);
    let d = dirty_dot_diameter(TAB_FONT_SIZE);
    container(iced::widget::Space::new())
        .width(d)
        .height(d)
        .style(move |_: &iced::Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(colour)),
            border: iced::Border {
                radius: (d / 2.0).into(),
                ..iced::Border::default()
            },
            ..iced::widget::container::Style::default()
        })
        .into()
}

/// Longest tab title drawn in full; longer ones are cut and end in `…`.
const MAX_TAB_TITLE_CHARS: usize = 28;

/// `title` cut to at most `max` characters, ending in `…` when cut.
fn ellipsize_title(title: &str, max: usize) -> String {
    if title.chars().count() <= max {
        return title.to_owned();
    }
    let kept: String = title.chars().take(max.saturating_sub(1)).collect();
    format!("{kept}…")
}

/// Resolve the pill bg fill for the current state. Altium parity:
/// inactive tabs are dimmer than active so they read as "off" but
/// still show their tab body (transparent inactive made them look
/// like floating labels — the strip showed through). Active uses
/// `tokens.hover` at full alpha; inactive 0.35× the same; drag
/// tints with theme accent at 22 %.
fn pill_fill(tokens: &ThemeTokens, is_active: bool, is_dragging: bool) -> iced::Color {
    let tab_active = styles::ti(tokens.hover);
    let accent = styles::ti(tokens.accent);
    if is_dragging {
        iced::Color { a: 0.22, ..accent }
    } else if is_active {
        tab_active
    } else {
        iced::Color {
            a: tab_active.a * 0.35,
            ..tab_active
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        dirty_dot_diameter, dirty_tab_paths, ellipsize_title, read_only_tab_paths,
        reveal_active_tab, reveal_offset, tab_is_dirty,
    };
    use crate::app::{TabInfo, TabKind};
    use std::collections::HashSet;
    use std::path::PathBuf;

    /// The strip and the no-document placeholder are both exactly
    /// `TAB_STRIP_HEIGHT` tall, and that is the 26 px they always were.
    #[test]
    fn the_strip_and_its_placeholder_both_use_the_height_token() {
        use iced::Length;
        let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::Signex);
        let fixed = Length::Fixed(super::TAB_STRIP_HEIGHT);
        assert_eq!(super::TAB_STRIP_HEIGHT, 26.0);

        let placeholder = super::placeholder::<()>(&tokens);
        assert_eq!(placeholder.as_widget().size().height, fixed);

        let none = HashSet::new();
        let strip = super::view(&[], 0, None, &none, &none, &none, &tokens);
        assert_eq!(strip.as_widget().size().height, fixed);
    }

    fn tab(path: &str, dirty: bool) -> TabInfo {
        TabInfo {
            title: path.to_owned(),
            path: PathBuf::from(path),
            cached_document: None,
            dirty,
            project_id: None,
            kind: TabKind::Schematic,
        }
    }

    #[test]
    fn read_only_tab_paths_lists_only_newer_minor_sheets() {
        let mut newer = signex_types::schematic::SchematicSheet::default();
        newer.unknown.newer_minor_source = Some("snxsch/1.7".into());
        let engines = std::collections::HashMap::from([
            (
                PathBuf::from("new.snxsch"),
                signex_engine::Engine::new(newer).unwrap(),
            ),
            (
                PathBuf::from("cur.snxsch"),
                signex_engine::Engine::new(Default::default()).unwrap(),
            ),
        ]);
        // `none.snxsch` has no engine (a library editor tab, say).
        let tabs = [
            tab("new.snxsch", false),
            tab("cur.snxsch", false),
            tab("none.snxsch", false),
        ];
        let got = read_only_tab_paths(&tabs, &engines);
        assert_eq!(got, HashSet::from([PathBuf::from("new.snxsch")]));
    }

    #[test]
    fn a_tab_is_dirty_when_its_path_is_in_dirty_paths_or_its_flag_is_set() {
        let dirty: HashSet<PathBuf> = [PathBuf::from("a.snxsch")].into();
        assert!(tab_is_dirty(&tab("a.snxsch", false), &dirty));
        assert!(tab_is_dirty(&tab("b.snxsch", true), &dirty));
        assert!(!tab_is_dirty(&tab("b.snxsch", false), &dirty));
    }

    fn preview(lib: &std::path::Path) -> crate::library::state::ComponentPreviewState {
        use signex_library::{
            ComponentClass, ComponentRow, DatasheetRef, InternalPn, LifecycleState,
            ManufacturerPart, ParamMap, PlmReserved, PrimitiveRef,
        };
        use uuid::Uuid;
        let row = ComponentRow {
            alternate_symbols: Vec::new(),
            row_id: Uuid::new_v4(),
            internal_pn: InternalPn::new("TEST_PN"),
            class: ComponentClass::generic(),
            datasheet: DatasheetRef::default(),
            datasheet_label: String::new(),
            description: String::new(),
            more_datasheets: Vec::new(),
            state: LifecycleState::Draft,
            symbol_ref: PrimitiveRef::new(Uuid::new_v4(), Uuid::new_v4()),
            footprint_ref: None,
            sim_ref: None,
            pin_map_overrides: Vec::new(),
            primary_mpn: ManufacturerPart::draft("Acme", "ACM-001"),
            alternates: Vec::new(),
            supply: Vec::new(),
            parameters: ParamMap::new(),
            plm: PlmReserved::default(),
            version: "0.0.1".into(),
            released: false,
            symbol_version: String::new(),
            footprint_version: String::new(),
            sim_version: String::new(),
            created: chrono::Utc::now(),
            updated: chrono::Utc::now(),
            content_hash: [0u8; 32],
            server_revision: None,
        };
        crate::library::state::ComponentPreviewState::from_row(
            lib.to_path_buf(),
            "parts".into(),
            row,
        )
    }

    #[test]
    fn a_component_preview_tab_with_unsaved_edits_gets_the_dot() {
        use crate::app::ComponentEditorTab;
        use crate::library::state::{EditorAddress, LibraryState};
        let lib = PathBuf::from("lib.snxlib");
        let editor = preview(&lib);
        let row_id = signex_library::RowId(editor.row.row_id);
        let mut t = tab("lib.snxlib/parts.tsv#row", false);
        t.kind = TabKind::ComponentEditor(ComponentEditorTab {
            library_path: lib.clone(),
            table: "parts".to_owned(),
            row_id,
        });
        let mut library = LibraryState::default();
        let address = EditorAddress::new(lib, "parts".to_owned(), row_id);
        library.editors.insert(address.clone(), editor);
        let none = HashSet::new();
        let clean = dirty_tab_paths(std::slice::from_ref(&t), &none, &library);
        assert!(!tab_is_dirty(&t, &clean));
        library.editors.get_mut(&address).unwrap().mark_dirty();
        let dirty = dirty_tab_paths(std::slice::from_ref(&t), &none, &library);
        assert!(tab_is_dirty(&t, &dirty));
    }

    #[test]
    fn the_dirty_dot_scales_with_the_font_and_stays_smaller_than_it() {
        assert!(dirty_dot_diameter(22.0) > dirty_dot_diameter(11.0));
        assert!(dirty_dot_diameter(11.0) < 11.0);
    }

    #[test]
    fn reveal_offset_is_exact_at_both_ends_and_proportional_between() {
        assert_eq!(reveal_offset(0, 1).x, 0.0);
        assert_eq!(reveal_offset(0, 5).x, 0.0);
        assert_eq!(reveal_offset(4, 5).x, 1.0);
        assert_eq!(reveal_offset(2, 5).x, 0.5);
        assert_eq!(reveal_offset(9, 5).x, 1.0);
    }

    #[test]
    fn revealing_a_tab_emits_a_scroll_task() {
        assert_eq!(reveal_active_tab::<()>(1, 3).units(), 1);
    }

    #[test]
    fn short_titles_are_untouched_and_long_ones_end_in_an_ellipsis() {
        assert_eq!(ellipsize_title("power.snxsch", 28), "power.snxsch");
        let cut = ellipsize_title("a_very_long_sheet_name_that_overflows.snxsch", 10);
        assert_eq!(cut.chars().count(), 10);
        assert!(cut.ends_with('…'));
    }
}
