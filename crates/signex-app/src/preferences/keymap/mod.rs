//! Keyboard shortcuts page (mockup row 145): the profile row, the conflict banner,
//! the search and context chips, the shortcut table and the recorder
//! popover anchored to the row under edit. The page's own view state (the
//! chip that is on) is [`KeymapViewState`]; everything else it shows lives
//! in the keymap working copy and the recorder on `UiState`.

mod filters;
mod profile_row;
mod recorder;
mod table;
#[cfg(test)]
mod tests;

use iced::alignment::Vertical;
use iced::widget::container;
use iced::{Background, Border, Color, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, Localized, ids, shortcut_ids as sids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, raw, text};

use super::{PrefMsg, PrefsView, setting_row};
use crate::keymap::Platform;
use crate::styles::{self, Radii, ti, tokens};

pub use filters::KeymapContextFilter;

#[cfg(test)]
use filters::row_conflict_kinds;
#[cfg(test)]
use recorder::recorder_shadow_warning;
#[cfg(test)]
use table::shadow_note;

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// What the Keyboard shortcuts page shows beyond the keymap itself: which
/// context chip is on. View state only; it never marks the dialog dirty and
/// is reset each time Preferences opens.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct KeymapViewState {
    pub context_filter: KeymapContextFilter,
}

impl KeymapViewState {
    /// Apply a page message.
    pub fn apply(&mut self, msg: KeymapViewMsg) {
        match msg {
            KeymapViewMsg::ContextFilterPicked(filter) => self.context_filter = filter,
            KeymapViewMsg::ConflictsShown => {
                self.context_filter = KeymapContextFilter::Conflicts;
            }
        }
    }

    /// Whether the Conflicts chip is on.
    pub fn shows_conflicts(&self) -> bool {
        self.context_filter == KeymapContextFilter::Conflicts
    }
}

/// Events of the Keyboard shortcuts page's own view state.
#[derive(Debug, Clone, PartialEq)]
pub enum KeymapViewMsg {
    /// A context chip was picked.
    ContextFilterPicked(KeymapContextFilter),
    /// Show in the conflict banner was pressed: the Conflicts chip turns on.
    ConflictsShown,
}

/// The Keyboard shortcuts page.
pub(super) fn content_keyboard_shortcuts<'a>(v: PrefsView<'a>) -> Element<'a, PrefMsg> {
    let i18n = v.i18n;
    let editor = v.keymap_editor;
    let tokens = v.panel_tokens;
    let platform = Platform::current();
    let conflicts = editor.active_conflicts();
    let filter = v.keymap_view.context_filter;
    let rows = filters::page_rows(
        editor,
        i18n,
        v.extension_commands,
        v.keymap_search,
        filter,
        &conflicts,
        platform,
    );

    let mut items: Vec<Element<'a, PrefMsg>> = vec![setting_row::page_heading(
        t!(i18n, ids::PREFERENCES_PAGE_KEYBOARD),
        t!(i18n, sids::PREF_KEYMAP_INTRO),
        tokens,
    )];
    items.extend(profile_row::load_error_banner(
        i18n,
        v.keymap_load_error,
        tokens,
    ));
    items.extend(profile_row::backup_row(i18n, v.keymap_backup, tokens));
    items.push(profile_row::profile_row(i18n, editor, tokens));
    items.extend(profile_row::conflict_banner(
        i18n, &conflicts, platform, tokens,
    ));
    items.extend(status_line(i18n, v.keymap_status, tokens));
    items.push(filters::filter_bar(
        i18n,
        v.keymap_search,
        filter,
        conflicts.len(),
        tokens,
    ));
    let empty = rows
        .is_empty()
        .then(|| empty_table_message(i18n, editor, v.keymap_search, filter));
    items.push(table::shortcut_table(table::TableInput {
        i18n,
        rows: &rows,
        empty,
        conflicts: &conflicts,
        platform,
        active_is_custom: editor.active_profile_is_custom(),
        recorder: v.keymap_recorder,
        tokens,
    }));

    setting_row::page(items)
}

/// What the table says when it shows no row: the profile has none, the
/// search matches none, or the chip leaves none.
fn empty_table_message(
    i18n: &I18n,
    editor: &crate::keymap::KeymapEditorModel,
    search: &str,
    filter: KeymapContextFilter,
) -> Localized {
    if editor.rows_in(i18n).is_empty() {
        t!(i18n, sids::PREF_KEYMAP_EMPTY)
    } else if !search.trim().is_empty() && filter == KeymapContextFilter::All {
        t!(i18n, sids::PREF_KEYMAP_NO_MATCH, "query" => search.trim())
    } else {
        t!(i18n, ids::PREFERENCES_KEYMAP_NO_ROWS_IN_FILTER)
    }
}

/// The transient result of the last profile action, load, save or edit, in
/// the interface language; nothing when there is none.
fn status_line<'a>(
    i18n: &I18n,
    status: &crate::app::KeymapStatus,
    tokens: &ThemeTokens,
) -> Option<Element<'a, PrefMsg>> {
    if status.is_empty() {
        return None;
    }
    let ink3 = tokens::resolve(tokens).ink3;
    // raw: a catalog message with its arguments, rendered here (#3050).
    Some(
        text(raw(status.render(i18n)))
            .size(TS.caption)
            .color(ink3)
            .boxed(),
    )
}

/// The look of a page button: the mock's `.btn`, `.btn.sm` and
/// `.btn.sm.primary`, and a danger ink for deleting a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ButtonLook {
    Normal,
    Danger,
    Small,
    SmallPrimary,
}

/// A page button. `None` draws it faded and inert.
fn page_button<'a>(
    label: Localized,
    on_press: Option<PrefMsg>,
    look: ButtonLook,
    t: &ThemeTokens,
) -> Element<'a, PrefMsg> {
    let small = matches!(look, ButtonLook::Small | ButtonLook::SmallPrimary);
    // `.btn.sm` is 22 px tall at the 12 px body; `.btn` is a field high.
    let (height, size, pad, radius) = if small {
        (
            TS.body * 22.0 / 12.0,
            TS.caption,
            SP.m,
            Radii::GRAPHITE.small,
        )
    } else {
        (
            styles::ChromeMetrics::DEFAULT.field,
            TS.body,
            SP.m + SP.xs,
            Radii::GRAPHITE.button,
        )
    };
    button(
        container(text(label).size(size))
            .height(Length::Fill)
            .align_y(Vertical::Center),
    )
    .padding([0.0, pad])
    .height(Length::Fixed(height))
    .on_press_maybe(on_press)
    .style(button_style(t, look, radius))
    .boxed()
}

fn faded(c: Color, alpha: f32) -> Color {
    Color {
        a: c.a * alpha,
        ..c
    }
}

fn button_style(
    t: &ThemeTokens,
    look: ButtonLook,
    radius: f32,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    let r = tokens::resolve(t);
    let (fill, ink, edge) = match look {
        ButtonLook::SmallPrimary => (r.primary, r.primary_ink, Color::TRANSPARENT),
        ButtonLook::Danger => (r.field, ti(t.error), ti(t.border)),
        ButtonLook::Normal | ButtonLook::Small => (r.field, ti(t.text), ti(t.border)),
    };
    let hover = tokens::composite_over(tokens::overlay(t, tokens::Overlay::Hover), fill);
    let pressed = tokens::composite_over(tokens::overlay(t, tokens::Overlay::Pressed), fill);
    move |_: &Theme, status: button::Status| {
        let (bg, alpha) = match status {
            button::Status::Hovered => (hover, 1.0),
            button::Status::Pressed => (pressed, 1.0),
            button::Status::Disabled => (fill, styles::DISABLED_OPACITY),
            button::Status::Active => (fill, 1.0),
        };
        button::Style {
            background: Some(Background::Color(faded(bg, alpha))),
            text_color: faded(ink, alpha),
            border: Border {
                width: 1.0,
                color: faded(edge, alpha),
                radius: radius.into(),
                ..Border::default()
            },
            ..button::Style::default()
        }
    }
}

/// The name of a shortcut context, as the table writes it.
fn context_label(i18n: &I18n, context: crate::keymap::ShortcutContext) -> Localized {
    i18n.message(
        match context {
            crate::keymap::ShortcutContext::Global => sids::PREF_KEYMAP_CONTEXT_GLOBAL,
            crate::keymap::ShortcutContext::Schematic => sids::PREF_KEYMAP_CONTEXT_SCHEMATIC,
            crate::keymap::ShortcutContext::Footprint => sids::PREF_KEYMAP_CONTEXT_FOOTPRINT,
            crate::keymap::ShortcutContext::Pcb => sids::PREF_KEYMAP_CONTEXT_PCB,
            crate::keymap::ShortcutContext::Library => sids::PREF_KEYMAP_CONTEXT_LIBRARY,
            crate::keymap::ShortcutContext::Modal => sids::PREF_KEYMAP_CONTEXT_MODAL,
            crate::keymap::ShortcutContext::TextInput => sids::PREF_KEYMAP_CONTEXT_TEXT_INPUT,
            crate::keymap::ShortcutContext::CommandPalette => {
                sids::PREF_KEYMAP_CONTEXT_COMMAND_PALETTE
            }
            crate::keymap::ShortcutContext::Placement => sids::PREF_KEYMAP_CONTEXT_PLACEMENT,
            crate::keymap::ShortcutContext::Board3d => sids::PREF_KEYMAP_CONTEXT_BOARD_3D,
        },
        &[],
    )
}

/// The labels of a key sequence's strokes as `platform` writes them.
fn stroke_labels(
    i18n: &I18n,
    strokes: &[crate::keymap::KeyStroke],
    platform: Platform,
) -> Vec<String> {
    strokes
        .iter()
        .map(|stroke| stroke.display_in(platform, i18n))
        .collect()
}

/// The command's name in the message language, as the table shows it.
fn command_label(i18n: &I18n, command: &crate::keymap::AppCommandId) -> String {
    use crate::keymap::CatalogTextExt as _;
    crate::keymap::metadata_for(command)
        .map(|metadata| metadata.label_in(i18n))
        .unwrap_or_else(|| crate::keymap::fallback_label(command))
}
