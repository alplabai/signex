//! Above the table: the shortcuts-file banner (#595), the backup row (#603),
//! the profile row and the conflict banner.

use std::sync::LazyLock;

use iced::widget::{Row, Space, column, container, row, svg};
use iced::{Alignment, Background, Border, Element, Length, Padding, Theme, Widget as _};
use signex_i18n::{I18n, ids, shortcut_ids as sids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::text;

use super::{ButtonLook, KeymapViewMsg, SP, TS, command_label, page_button, stroke_labels};
use crate::keymap::{BindingConflict, KeymapEditorModel, Platform, ShortcutProfileKind};
use crate::preferences::{PrefMsg, keycaps};
use crate::styles::{Radii, ti, tokens};

/// The profile picker's width: 240 px at the 12 px body.
const PICKER_W: f32 = TS.body * 20.0;

/// A 24 x 24 stroke warning triangle, drawn in the warn ink.
const GLYPH_WARN: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3l10 18H2z"/><path d="M12 10v5M12 18h.01"/></svg>"##;

/// The triangle parsed once; an immutable handle, not app state.
static WARN_HANDLE: LazyLock<svg::Handle> = LazyLock::new(|| svg::Handle::from_memory(GLYPH_WARN));

/// The mock's `.banner.warn`: warn-soft fill, button radius, the warn
/// triangle, then `body` (ink text) taking the rest of the line.
fn warn_banner<'a>(
    body: Element<'a, PrefMsg>,
    action: Option<Element<'a, PrefMsg>>,
    t: &ThemeTokens,
) -> Element<'a, PrefMsg> {
    let fill = tokens::resolve(t).warn_soft;
    let warn = ti(t.warning);
    let glyph = TS.body + SP.xs;
    let icon = svg(WARN_HANDLE.clone())
        .width(Length::Fixed(glyph))
        .height(Length::Fixed(glyph))
        .style(move |_: &Theme, _| svg::Style {
            color: Some(warn),
            ..Default::default()
        });
    let mut line: Row<Element<'a, PrefMsg>> = Row::new()
        .spacing(SP.m)
        .align_y(Alignment::Center)
        .push(icon.boxed())
        .push(container(body).width(Length::Fill).boxed());
    if let Some(action) = action {
        line = line.push(action);
    }
    container(line)
        .padding([SP.m, SP.l])
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                radius: Radii::GRAPHITE.button.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// The persistent banner for a shortcuts file that exists on disk but could
/// not be honoured at boot (#595). It stays for the whole session, because
/// until the page is saved the user's own profiles are still in that file
/// and not in the running set. Distinct from the transient status line.
pub(super) fn load_error_banner<'a>(
    i18n: &I18n,
    load_error: Option<&str>,
    t: &ThemeTokens,
) -> Option<Element<'a, PrefMsg>> {
    let error = load_error?;
    let location = crate::keymap::config_path()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| i18n.message_plain(sids::PREF_KEYMAP_USUAL_LOCATION, &[]));
    let message = text(t!(
        i18n,
        sids::PREF_KEYMAP_LOAD_ERROR,
        "location" => location,
        "error" => error
    ))
    .size(TS.body)
    .color(ti(t.text));
    Some(warn_banner(message.boxed(), None, t))
}

/// The backup row (#603). The backup outlives the failure that produced it,
/// so the row is gated on the file existing rather than on the load error.
/// Restore reads the backup through the normal loader rather than copying
/// the file back (the backup is the file that failed to load); Delete is
/// the only thing in the app that removes it.
pub(super) fn backup_row<'a>(
    i18n: &I18n,
    backup: Option<&str>,
    t: &ThemeTokens,
) -> Option<Element<'a, PrefMsg>> {
    let path = backup?;
    let ink3 = tokens::resolve(t).ink3;
    Some(
        column![
            text(t!(i18n, sids::PREF_KEYMAP_BACKUP, "path" => path))
                .size(TS.caption)
                .color(ink3),
            row![
                page_button(
                    t!(i18n, sids::PREF_KEYMAP_RESTORE),
                    Some(PrefMsg::KeymapRestoreFromBackup),
                    ButtonLook::Normal,
                    t,
                ),
                page_button(
                    t!(i18n, sids::PREF_KEYMAP_DISCARD_BACKUP),
                    Some(PrefMsg::KeymapDiscardBackup),
                    ButtonLook::Danger,
                    t,
                ),
            ]
            .spacing(SP.m),
        ]
        .spacing(SP.s)
        .boxed(),
    )
}

/// Whether Delete acts: only on a custom profile. Built-ins are protected
/// by the model as well; the button says so by being inert.
pub(super) fn delete_enabled(editor: &KeymapEditorModel) -> bool {
    editor.active_profile_is_custom()
}

/// One line: the Profile caption, the picker (each entry with its binding
/// count), Create and Delete, then Import… and Export… at the right end.
pub(super) fn profile_row<'a>(
    i18n: &I18n,
    editor: &KeymapEditorModel,
    t: &ThemeTokens,
) -> Element<'a, PrefMsg> {
    let profiles = editor.profiles();
    let options: Vec<KeymapProfileOption> = profiles
        .iter()
        .map(|profile| KeymapProfileOption::new(i18n, profile))
        .collect();
    let active = profiles
        .iter()
        .find(|profile| profile.active)
        .map(|profile| KeymapProfileOption::new(i18n, profile));
    let ink4 = tokens::resolve(t).ink4;

    let picker = iced::widget::pick_list(active, options, |profile: &KeymapProfileOption| {
        profile.to_string()
    })
    .graphite()
    .on_select(|profile: KeymapProfileOption| PrefMsg::KeymapProfileSelected(profile.id))
    .padding([SP.s, SP.m])
    .width(Length::Fixed(PICKER_W));
    let delete = delete_enabled(editor).then_some(PrefMsg::KeymapDeleteActiveProfile);

    let line = row![
        text(t!(i18n, sids::PREF_KEYMAP_PROFILE))
            .size(TS.caption)
            .color(ink4),
        picker,
        page_button(
            t!(i18n, sids::PREF_KEYMAP_CREATE),
            Some(PrefMsg::KeymapCreateCustomProfile),
            ButtonLook::Normal,
            t,
        ),
        page_button(
            t!(i18n, sids::PREF_KEYMAP_DELETE),
            delete,
            ButtonLook::Normal,
            t,
        ),
        Space::new().width(Length::Fill),
        page_button(
            t!(i18n, ids::PREFERENCES_KEYMAP_IMPORT),
            Some(PrefMsg::KeymapImportProfile),
            ButtonLook::Normal,
            t,
        ),
        page_button(
            t!(i18n, ids::PREFERENCES_KEYMAP_EXPORT),
            Some(PrefMsg::KeymapExportProfile),
            ButtonLook::Normal,
            t,
        ),
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center);
    // 12 px above, 10 px below at the 12 px body.
    container(line)
        .padding(Padding {
            top: SP.l,
            bottom: SP.m + SP.xs,
            left: 0.0,
            right: 0.0,
        })
        .width(Length::Fill)
        .boxed()
}

/// "1 conflict in active profile: Ctrl Shift V is bound to both Smart
/// Paste and Place Via.", naming the first clash, with Show turning the
/// Conflicts chip on. Nothing when the active profile has no conflict.
pub(super) fn conflict_banner<'a>(
    i18n: &I18n,
    conflicts: &[BindingConflict],
    platform: Platform,
    t: &ThemeTokens,
) -> Option<Element<'a, PrefMsg>> {
    let first = conflicts.first()?;
    let ink = ti(t.text);
    let lead = text(t!(
        i18n,
        ids::PREFERENCES_KEYMAP_CONFLICT_LEAD,
        "count" => conflicts.len()
    ))
    .size(TS.body)
    .color(ink);
    let first_label = command_label(i18n, &first.first_command);
    let tail = if first.system_reserved {
        t!(
            i18n,
            ids::PREFERENCES_KEYMAP_CONFLICT_RESERVED,
            "command" => first_label
        )
    } else {
        t!(
            i18n,
            ids::PREFERENCES_KEYMAP_CONFLICT_PAIR,
            "first" => first_label,
            "second" => command_label(i18n, &first.second_command)
        )
    };
    let then = t!(i18n, ids::PREFERENCES_KEYMAP_THEN);
    let strokes = stroke_labels(i18n, &first.sequence, platform);
    let keys: Element<'a, PrefMsg> =
        keycaps::sequence_caps(&strokes, &then, t).unwrap_or_else(|| {
            // raw: a key sequence shown with the catalog's key names (#2314).
            text(signex_widgets::raw(first.trigger.clone()))
                .size(TS.body)
                .color(ink)
                .boxed()
        });
    let sentence = Row::new()
        .spacing(SP.s)
        .align_y(Alignment::Center)
        .push(lead.boxed())
        .push(keys)
        .push(text(tail).size(TS.body).color(ink).boxed())
        .wrap()
        .vertical_spacing(SP.xs);
    let show = page_button(
        t!(i18n, ids::PREFERENCES_KEYMAP_SHOW),
        Some(PrefMsg::KeymapView(KeymapViewMsg::ConflictsShown)),
        ButtonLook::Small,
        t,
    );
    Some(warn_banner(sentence.boxed(), Some(show), t))
}

/// A picker entry: the profile id it selects and its label,
/// "<name> (custom) · 214 bindings" or "<name> · 214 bindings".
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct KeymapProfileOption {
    pub(super) id: String,
    pub(super) label: String,
}

impl KeymapProfileOption {
    pub(super) fn new(i18n: &I18n, profile: &crate::keymap::KeymapEditorProfile) -> Self {
        let label = match profile.kind {
            ShortcutProfileKind::BuiltIn => ids::PREFERENCES_KEYMAP_PROFILE_OPTION_BUILT_IN,
            ShortcutProfileKind::Custom => ids::PREFERENCES_KEYMAP_PROFILE_OPTION_CUSTOM,
        };
        Self {
            id: profile.id.clone(),
            label: i18n.message_plain(
                label,
                &[
                    ("name", profile.name.as_str().into()),
                    ("count", profile.binding_count.into()),
                ],
            ),
        }
    }
}

impl std::fmt::Display for KeymapProfileOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}
