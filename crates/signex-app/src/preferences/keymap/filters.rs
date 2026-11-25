//! Which rows the Keyboard shortcuts table shows and in what order: the
//! context chips, the search, the Command sort and each row's state. The
//! rules are pure so they are tested without a view; the filter bar that
//! sets them is drawn at the end of this file.

use std::sync::LazyLock;

use iced::widget::{Row, container, row, svg};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Theme, Widget as _};
use signex_i18n::{I18n, MessageId, ids, shortcut_ids as sids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::chips::{self, ChipMetrics, ChipStyle};
use signex_widgets::text_input;

use super::{KeymapViewMsg, SP, TS};
use crate::keymap::{
    BindingConflict, KeymapEditorModel, KeymapEditorRow, Platform, ShortcutContext, ShortcutTrigger,
};
use crate::preferences::PrefMsg;
use crate::styles::{self, ti, tokens};

/// A context chip above the shortcut table. One is on at a time; `All` is
/// the default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KeymapContextFilter {
    #[default]
    All,
    Global,
    Schematic,
    Pcb,
    Footprint,
    Library,
    /// Rows that are in a conflict of the active profile.
    Conflicts,
}

impl KeymapContextFilter {
    /// The chips in the order the filter bar shows them.
    pub const ALL: [Self; 7] = [
        Self::All,
        Self::Global,
        Self::Schematic,
        Self::Pcb,
        Self::Footprint,
        Self::Library,
        Self::Conflicts,
    ];

    /// The chip's label.
    pub(super) fn label(self) -> MessageId {
        match self {
            Self::All => ids::PREFERENCES_KEYMAP_FILTER_ALL,
            Self::Global => sids::PREF_KEYMAP_CONTEXT_GLOBAL,
            Self::Schematic => sids::PREF_KEYMAP_CONTEXT_SCHEMATIC,
            Self::Pcb => sids::PREF_KEYMAP_CONTEXT_PCB,
            Self::Footprint => sids::PREF_KEYMAP_CONTEXT_FOOTPRINT,
            Self::Library => sids::PREF_KEYMAP_CONTEXT_LIBRARY,
            Self::Conflicts => ids::PREFERENCES_KEYMAP_FILTER_CONFLICTS,
        }
    }

    /// The chip a context's rows show under besides All. Contexts without
    /// a chip of their own (dialogs, text fields, the palette, placement)
    /// show under All only. The board 3D view's rows show under PCB.
    pub fn for_context(context: ShortcutContext) -> Option<Self> {
        match context {
            ShortcutContext::Global => Some(Self::Global),
            ShortcutContext::Schematic => Some(Self::Schematic),
            ShortcutContext::Pcb | ShortcutContext::Board3d => Some(Self::Pcb),
            ShortcutContext::Footprint => Some(Self::Footprint),
            ShortcutContext::Library => Some(Self::Library),
            ShortcutContext::Modal
            | ShortcutContext::TextInput
            | ShortcutContext::CommandPalette
            | ShortcutContext::Placement => None,
        }
    }

    /// Whether `row` shows while this chip is on. Conflicts are compared as
    /// strokes resolved for `platform` (see [`row_conflict_kinds`]).
    pub fn matches(
        self,
        row: &KeymapEditorRow,
        conflicts: &[BindingConflict],
        platform: Platform,
    ) -> bool {
        match self {
            Self::All => true,
            Self::Conflicts => row_conflict_kinds(row, conflicts, platform).0,
            chip => Self::for_context(row.context) == Some(chip),
        }
    }
}

/// The rows the table shows: those matching the search and the chip,
/// sorted by command name ascending (on-screen collation, stable, so rows
/// of one command keep their profile order).
#[cfg(test)]
pub(super) fn visible_rows(
    editor: &KeymapEditorModel,
    i18n: &I18n,
    search: &str,
    filter: KeymapContextFilter,
    conflicts: &[BindingConflict],
    platform: Platform,
) -> Vec<KeymapEditorRow> {
    let none = crate::keymap::ExtensionCommandNames::default();
    visible_rows_with(editor, i18n, &none, search, filter, conflicts, platform)
}

/// [`visible_rows`] with the installed extensions' commands named by
/// `extensions` (#2495).
pub(super) fn visible_rows_with(
    editor: &KeymapEditorModel,
    i18n: &I18n,
    extensions: &crate::keymap::ExtensionCommandNames,
    search: &str,
    filter: KeymapContextFilter,
    conflicts: &[BindingConflict],
    platform: Platform,
) -> Vec<KeymapEditorRow> {
    let mut rows: Vec<KeymapEditorRow> = editor
        .filtered_rows_in_with(i18n, extensions, search)
        .into_iter()
        .filter(|row| filter.matches(row, conflicts, platform))
        .collect();
    i18n.sort_for_display(&mut rows, |row| row.label.as_str());
    rows
}

/// What a row's State column says. The order of the checks is the order
/// of precedence: a user's own unbind outranks a conflict, a conflict
/// outranks "editable".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RowState {
    Invalid,
    Unbound,
    Reserved,
    Conflict,
    Editable,
    CreateCustom,
    Gesture,
}

impl RowState {
    pub(super) fn of(
        row: &KeymapEditorRow,
        conflicts: &[BindingConflict],
        platform: Platform,
        active_is_custom: bool,
    ) -> Self {
        let (has_conflict, reserved) = row_conflict_kinds(row, conflicts, platform);
        if !row.trigger_valid {
            Self::Invalid
        } else if row.unbound {
            Self::Unbound
        } else if reserved {
            Self::Reserved
        } else if has_conflict {
            Self::Conflict
        } else if row.trigger.trim().is_empty() {
            Self::Unbound
        } else if row.keyboard_editable && active_is_custom {
            Self::Editable
        } else if row.keyboard_editable {
            Self::CreateCustom
        } else {
            Self::Gesture
        }
    }

    pub(super) fn label(self) -> MessageId {
        match self {
            Self::Invalid => sids::PREF_KEYMAP_STATE_INVALID,
            Self::Unbound => sids::PREF_KEYMAP_STATE_UNBOUND,
            Self::Reserved => sids::PREF_KEYMAP_STATE_RESERVED,
            Self::Conflict => sids::PREF_KEYMAP_STATE_CONFLICT,
            Self::Editable => sids::PREF_KEYMAP_STATE_EDITABLE,
            Self::CreateCustom => sids::PREF_KEYMAP_STATE_CREATE_CUSTOM,
            Self::Gesture => sids::PREF_KEYMAP_STATE_GESTURE,
        }
    }
}

/// Whether `row` is in a conflict, and whether that conflict is a chord the
/// OS reserves. Compared as strokes resolved for `platform`, not as text:
/// `Mod+K`, `Ctrl+K` and a typed `Cmd+K` all land on the same key on the OS
/// that runs them.
pub(super) fn row_conflict_kinds(
    row: &KeymapEditorRow,
    conflicts: &[BindingConflict],
    platform: Platform,
) -> (bool, bool) {
    let Some(ShortcutTrigger::KeySequence(strokes)) =
        ShortcutTrigger::parse(&row.trigger_config).ok()
    else {
        return (false, false);
    };
    let sequence: Vec<_> = strokes.iter().map(|s| s.resolved_for(platform)).collect();
    let mut hits = conflicts.iter().filter(|conflict| {
        conflict.context == row.context
            && conflict.sequence == sequence
            && row.command.as_ref().is_some_and(|command| {
                command == &conflict.first_command || command == &conflict.second_command
            })
    });
    let first = hits.next();
    let reserved = first.is_some_and(|c| c.system_reserved) || hits.any(|c| c.system_reserved);
    (first.is_some(), reserved)
}

/// The search field's width: 260 px at the 12 px body.
const SEARCH_W: f32 = TS.body * 260.0 / 12.0;

/// A 24 x 24 stroke magnifier, drawn in the field's placeholder ink.
const GLYPH_SEARCH: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round"><circle cx="11" cy="11" r="7"/><path d="M20 20l-4-4"/></svg>"##;

/// The magnifier parsed once; an immutable handle, not app state.
static SEARCH_HANDLE: LazyLock<svg::Handle> =
    LazyLock::new(|| svg::Handle::from_memory(GLYPH_SEARCH));

/// The search field and the context chips, on one line.
pub(super) fn filter_bar<'a>(
    i18n: &I18n,
    search: &'a str,
    filter: KeymapContextFilter,
    conflict_count: usize,
    t: &ThemeTokens,
) -> Element<'a, PrefMsg> {
    let r = tokens::resolve(t);
    let (ink4, ink) = (r.ink4, ti(t.text));
    let glyph = TS.body;
    let icon = svg(SEARCH_HANDLE.clone())
        .width(Length::Fixed(glyph))
        .height(Length::Fixed(glyph))
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink4),
            ..Default::default()
        });
    let input = text_input(t!(i18n, sids::PREF_KEYMAP_SEARCH), search)
        .on_input(PrefMsg::KeymapSearchChanged)
        .padding(Padding::ZERO)
        .size(TS.body)
        .width(Length::Fill)
        .style(move |_: &Theme, _| text_input::Style {
            background: Background::Color(Color::TRANSPARENT),
            border: Border::default(),
            placeholder: ink4,
            value: ink,
            selection: Color { a: 0.4, ..ink },
        });
    let field = container(row![icon, input].spacing(SP.s).align_y(Alignment::Center))
        .padding([0.0, SP.m])
        .height(Length::Fixed(styles::ChromeMetrics::DEFAULT.field))
        .center_y(Length::Fixed(styles::ChromeMetrics::DEFAULT.field))
        .width(Length::Fixed(SEARCH_W))
        .style(styles::field(t, false));

    let metrics = ChipMetrics::from_text_size(TS.body);
    let style = chip_style(t);
    let mut chip_row: Row<Element<'a, PrefMsg>> = Row::new().spacing(metrics.row_gap);
    for chip in KeymapContextFilter::ALL {
        let count = (chip == KeymapContextFilter::Conflicts)
            .then(|| u32::try_from(conflict_count).unwrap_or(u32::MAX));
        chip_row = chip_row.push(chips::chip(
            t!(i18n, chip.label()),
            count,
            chip == filter,
            style,
            metrics,
            i18n,
            Some(PrefMsg::KeymapView(KeymapViewMsg::ContextFilterPicked(
                chip,
            ))),
        ));
    }

    row![field, chip_row.wrap().vertical_spacing(metrics.row_gap)]
        .spacing(SP.m)
        .align_y(Alignment::Center)
        .boxed()
}

/// Chip colours from the theme: field fill and hairline at rest, the hover
/// token on hover, accent-soft when on.
fn chip_style(t: &ThemeTokens) -> ChipStyle {
    let r = tokens::resolve(t);
    ChipStyle {
        fill: r.field,
        edge: ti(t.border),
        hover: tokens::composite_over(tokens::hover_overlay(t), r.field),
        ink: r.ink3,
        on_fill: r.accent_soft,
        on_ink: r.accent_soft_ink,
    }
}
