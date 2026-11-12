//! Keyboard Shortcuts section — the profile picker, the grouped /
//! searchable shortcut table, the chord recorder card, and the shortcut
//! chip / profile-option / label helpers. Moved verbatim from the former
//! single-file `preferences` module.

use super::*;
use iced::widget::{Column, Space, Widget, column, container, row};
use iced::{Background, Border, Element, Length, Theme};
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::{button, raw, text, text_input};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// What the last keymap load, save or profile action left to tell the user.
pub(super) struct KeymapFeedback<'a> {
    pub status: &'a str,
    pub load_error: Option<&'a str>,
    pub backup: Option<&'a str>,
}

pub(super) fn content_keyboard_shortcuts<'a>(
    i18n: &signex_i18n::I18n,
    editor: &'a crate::keymap::KeymapEditorModel,
    feedback: KeymapFeedback<'a>,
    search: &'a str,
    recorder: Option<&'a crate::app::KeymapRecorderState>,
    tokens: &signex_types::theme::ThemeTokens,
) -> Element<'a, PrefMsg> {
    let KeymapFeedback {
        status,
        load_error,
        backup,
    } = feedback;
    let profiles = editor.profiles();
    let active = profiles.iter().find(|profile| profile.active);
    let active_option = active.map(|profile| KeymapProfileOption::new(i18n, profile));
    let active_is_custom = active
        .map(|profile| profile.kind == crate::keymap::ShortcutProfileKind::Custom)
        .unwrap_or(false);
    let active_summary = active
        .map(|profile| {
            i18n.message(
                ids::PREF_KEYMAP_BINDINGS,
                &[("count", profile.binding_count.to_string().into())],
            )
        })
        .unwrap_or_else(|| i18n.message(ids::PREF_KEYMAP_NO_PROFILE, &[]));

    let profile_options: Vec<KeymapProfileOption> = profiles
        .iter()
        .map(|profile| KeymapProfileOption::new(i18n, profile))
        .collect();

    // Delete is only wired for custom profiles — built-ins are
    // protected by the model, so the button is inert on a built-in.
    let delete_button = {
        let base = button(
            container(text(i18n.message(ids::PREF_KEYMAP_DELETE, &[])).size(TS.body))
                .padding([5.0, SP.l]),
        )
        .style(danger_button_style);
        if active_is_custom {
            base.on_press(PrefMsg::KeymapDeleteActiveProfile)
        } else {
            base
        }
    };

    let header = column![
        section_title(&i18n.message_plain(command_ids::CMD_MENU_SHOW_CURRENT_COMMAND_HOTKEYS, &[])),
        Space::new().height(4),
        text(i18n.message(ids::PREF_KEYMAP_INTRO, &[]))
            .size(TS.body)
            .style(text_muted),
    ]
    .spacing(6);

    // Persistent banner for a shortcuts file that exists on disk but could
    // not be honoured at boot (#595). Distinct from `status_line` below,
    // which is transient per-action feedback: this one stays for the whole
    // session, because until the pane is saved the user's own profiles are
    // still in that file and NOT in the running set. Built as an
    // `Option<Element>` and extended in, so the `None` case adds nothing
    // rather than forking the column type.
    let load_error_banner: Option<Element<'a, PrefMsg>> = load_error.map(|error| {
        let location = crate::keymap::config_path()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| i18n.message_plain(ids::PREF_KEYMAP_USUAL_LOCATION, &[]));
        text(i18n.message(
            ids::PREF_KEYMAP_LOAD_ERROR,
            &[("location", location.into()), ("error", error.into())],
        ))
        .size(TS.caption)
        .style(text_warning)
        .boxed()
    });

    // #603 — the backup outlives the failure that produced it, so this
    // row is gated on the file existing rather than on `load_error`.
    // Before it, a successful save cleared the banner and with it the
    // only mention the app ever made of the backup: the user's profiles
    // sat in a file nothing could see, restore or remove.
    //
    // Restore reads the backup through the normal loader rather than
    // copying the file back — the backup IS the file that failed to
    // load, so a copy back would just reproduce the failure. Discard is
    // the only thing in the app that deletes it; no save path does.
    let backup_row: Option<Element<'a, PrefMsg>> = backup.map(|bak| {
        column![
            text(i18n.message(ids::PREF_KEYMAP_BACKUP, &[("path", bak.into())]))
                .size(TS.caption)
                .style(text_muted),
            row![
                button(
                    container(text(i18n.message(ids::PREF_KEYMAP_RESTORE, &[])).size(TS.body))
                        .padding([5.0, SP.l])
                )
                .on_press(PrefMsg::KeymapRestoreFromBackup)
                .style(secondary_button_style),
                button(
                    container(
                        text(i18n.message(ids::PREF_KEYMAP_DISCARD_BACKUP, &[])).size(TS.body)
                    )
                    .padding([5.0, SP.l])
                )
                .on_press(PrefMsg::KeymapDiscardBackup)
                .style(danger_button_style),
            ]
            .spacing(SP.m),
        ]
        .spacing(6)
        .boxed()
    });

    // Profile picker + recorder stay ABOVE the search box and groups.
    let profile_row = row![
        column![
            text(i18n.message(ids::PREF_KEYMAP_PROFILE, &[]))
                .size(TS.label)
                .style(text_primary),
            text(active_summary).size(TS.caption).style(text_muted),
        ]
        .spacing(3)
        .width(160),
        iced::widget::pick_list(
            active_option,
            profile_options,
            |profile: &KeymapProfileOption| { profile.to_string() }
        )
        .graphite()
        .on_select(|profile: KeymapProfileOption| PrefMsg::KeymapProfileSelected(profile.id))
        .width(180),
        button(
            container(text(i18n.message(ids::PREF_KEYMAP_CREATE, &[])).size(TS.body))
                .padding([5.0, SP.l])
        )
        .on_press(PrefMsg::KeymapCreateCustomProfile)
        .style(primary_button_style),
        delete_button,
        button(
            container(text(i18n.message(ids::PREF_KEYMAP_IMPORT, &[])).size(TS.body))
                .padding([5.0, SP.l])
        )
        .on_press(PrefMsg::KeymapImportProfile)
        .style(secondary_button_style),
        button(
            container(text(i18n.message(ids::PREF_KEYMAP_EXPORT, &[])).size(TS.body))
                .padding([5.0, SP.l])
        )
        .on_press(PrefMsg::KeymapExportProfile)
        .style(secondary_button_style),
        Space::new().width(Length::Fill),
    ]
    .spacing(SP.m)
    .align_y(iced::Alignment::Center);

    let conflicts = editor.active_conflicts();
    let conflict_count = conflicts.len();
    // A non-empty status (parse / save error, profile action) wins;
    // otherwise fall back to the conflict summary.
    let status_line = if !status.is_empty() {
        // Worded by the handler that set it.
        text(raw(status.to_string()))
    } else if conflict_count == 1 {
        text(i18n.message(ids::PREF_KEYMAP_CONFLICT_ONE, &[]))
    } else if conflict_count > 1 {
        text(i18n.message(
            ids::PREF_KEYMAP_CONFLICTS,
            &[("count", conflict_count.to_string().into())],
        ))
    } else {
        text(i18n.message(ids::PREF_KEYMAP_NO_CONFLICTS, &[]))
    };

    // Search box — case-insensitive filter across every group. Uses the
    // default (theme-aware) text_input styling like the other panes.
    let search_box = text_input(i18n.message(ids::PREF_KEYMAP_SEARCH, &[]), search)
        .on_input(PrefMsg::KeymapSearchChanged)
        .padding(6)
        .size(TS.label)
        .width(Length::Fill);

    let table_header = row![
        container(
            text(i18n.message(ids::PREF_KEYMAP_COL_CATEGORY, &[]))
                .size(TS.body)
                .style(text_muted)
        )
        .width(Length::FillPortion(2)),
        container(
            text(i18n.message(ids::PREF_KEYMAP_COL_COMMAND, &[]))
                .size(TS.body)
                .style(text_muted)
        )
        .width(Length::FillPortion(4)),
        container(
            text(i18n.message(ids::PREF_KEYMAP_COL_CONTEXT, &[]))
                .size(TS.body)
                .style(text_muted)
        )
        .width(Length::FillPortion(2)),
        container(
            text(i18n.message(ids::KEYMAP_COL_SHORTCUT, &[]))
                .size(TS.body)
                .style(text_muted)
        )
        .width(Length::FillPortion(2)),
        container(
            text(i18n.message(ids::PREF_KEYMAP_COL_STATE, &[]))
                .size(TS.body)
                .style(text_muted)
        )
        .width(Length::FillPortion(2)),
    ]
    .spacing(SP.m)
    .padding([SP.s, 0.0]);

    let active_profile_is_custom = editor.active_profile_is_custom();
    let filtered = editor.filtered_rows_in(i18n, search);

    let mut content = column![header].spacing(10).padding(20);
    content = content.extend(load_error_banner);
    content = content.extend(backup_row);
    content = content.push(profile_row.boxed());
    content = content.push(status_line.size(TS.caption).style(text_muted).boxed());

    if let Some(recorder) = recorder {
        content = content.push(keymap_recorder_control(i18n, recorder));
    }

    content = content.push(search_box.boxed());
    content = content.push(h_sep());

    if filtered.is_empty() {
        let empty = if editor.rows_in(i18n).is_empty() {
            i18n.message(ids::PREF_KEYMAP_EMPTY, &[])
        } else {
            i18n.message(
                ids::PREF_KEYMAP_NO_MATCH,
                &[("query", search.trim().into())],
            )
        };
        content = content.push(text(empty).size(TS.body).style(text_muted).boxed());
        return content.boxed();
    }

    content = content.push(table_header.boxed());

    // One block per group, in the fixed CommandGroup::ALL display order.
    // Groups with no matching rows are skipped entirely.
    for &group in crate::keymap::CommandGroup::ALL {
        let group_rows: Vec<Element<'a, PrefMsg>> = filtered
            .iter()
            .filter(|row_model| row_model.group == group)
            .map(|row_model| {
                keymap_table_row(
                    i18n,
                    row_model,
                    &conflicts,
                    active_profile_is_custom,
                    tokens,
                )
            })
            .collect();
        if group_rows.is_empty() {
            continue;
        }
        content = content.push(Space::new().height(6).boxed());
        content = content.push(section_title(&i18n.message_plain(group.title(), &[])));
        content = content.push(Column::with_children(group_rows).spacing(SP.xs).boxed());
    }

    content.boxed()
}

/// Render one shortcut table row. Extracted so the grouped view can build
/// rows per [`crate::keymap::CommandGroup`] without duplicating the cell
/// layout. Text that sits directly on the (theme-neutral) modal surface
/// keeps the shared muted / primary constants; the trigger chip and Edit
/// button pull their colours from the active theme.
fn keymap_table_row<'a>(
    i18n: &signex_i18n::I18n,
    row_model: &crate::keymap::KeymapEditorRow,
    conflicts: &[crate::keymap::BindingConflict],
    active_profile_is_custom: bool,
    tokens: &signex_types::theme::ThemeTokens,
) -> Element<'a, PrefMsg> {
    let (has_conflict, reserved_by_system) =
        row_conflict_kinds(row_model, conflicts, crate::keymap::Platform::current());
    let overridden_by = shadow_note(i18n, &row_model.trigger_config);
    let word = |id| text(i18n.message(id, &[]));
    let state = if !row_model.trigger_valid {
        word(ids::PREF_KEYMAP_STATE_INVALID)
    } else if row_model.unbound {
        // The user's own unbind outranks the fixed-action note: the row
        // does nothing, whatever it would have been shadowed by.
        word(ids::PREF_KEYMAP_STATE_UNBOUND)
    } else if let Some(note) = overridden_by.clone() {
        // Composed from the catalog and the fixed key's action name.
        text(raw(note))
    } else if reserved_by_system {
        word(ids::PREF_KEYMAP_STATE_RESERVED)
    } else if has_conflict {
        word(ids::PREF_KEYMAP_STATE_CONFLICT)
    } else if row_model.unbound || row_model.trigger.trim().is_empty() {
        word(ids::PREF_KEYMAP_STATE_UNBOUND)
    } else if row_model.keyboard_editable && active_profile_is_custom {
        word(ids::PREF_KEYMAP_STATE_EDITABLE)
    } else if row_model.keyboard_editable {
        word(ids::PREF_KEYMAP_STATE_CREATE_CUSTOM)
    } else {
        word(ids::PREF_KEYMAP_STATE_GESTURE)
    };
    let state_warn =
        !row_model.trigger_valid || has_conflict || (overridden_by.is_some() && !row_model.unbound);
    let state_unbound = row_model.unbound || row_model.trigger.trim().is_empty();
    let state_style = move |theme: &Theme| text::Style {
        color: Some(if state_warn {
            theme.palette().warning.base.color
        } else if state_unbound {
            theme.palette().secondary.base.color
        } else {
            theme.palette().background.base.text
        }),
        ..text::Style::default()
    };

    // Keyboard-editable rows in a custom profile get an inline Edit button
    // that opens the chord recorder; everything else is read-only (built-in
    // profiles, pointer gestures).
    let trigger_cell: Element<'a, PrefMsg> =
        if active_profile_is_custom && row_model.keyboard_editable {
            if let Some(command) = row_model.command.clone() {
                let context = row_model.context;
                let label = row_model.label.clone();
                let trigger = row_model.trigger_config.clone();
                let tone = if state_warn {
                    ChipTone::Warning
                } else {
                    ChipTone::Neutral
                };
                row![
                    trigger_display(i18n, &row_model.trigger, tone, tokens),
                    button(
                        container(text(i18n.message(ids::PREF_KEYMAP_EDIT, &[])).size(TS.caption))
                            .padding([3.0, SP.m])
                    )
                    .on_press(PrefMsg::KeymapRecorderOpen {
                        slot: row_model.slot,
                        command,
                        label,
                        context,
                        trigger,
                    })
                    .style(secondary_button_style),
                ]
                .spacing(6)
                .align_y(iced::Alignment::Center)
                .boxed()
            } else {
                text(raw(row_model.trigger.clone()))
                    .size(TS.body)
                    .style(text_muted)
                    .boxed()
            }
        } else {
            trigger_display(i18n, &row_model.trigger, ChipTone::Neutral, tokens)
        };

    row![
        container(
            text(raw(category_title(i18n, &row_model.category)))
                .size(TS.body)
                .style(text_muted)
        )
        .width(Length::FillPortion(2)),
        container(
            text(raw(row_model.label.clone()))
                .size(TS.body)
                .style(text_primary)
        )
        .width(Length::FillPortion(4)),
        container(
            text(context_label(i18n, row_model.context))
                .size(TS.body)
                .style(text_muted)
        )
        .width(Length::FillPortion(2)),
        container(trigger_cell).width(Length::FillPortion(2)),
        container(state.size(TS.body).style(state_style)).width(Length::FillPortion(2)),
    ]
    .spacing(SP.m)
    .align_y(iced::Alignment::Center)
    .padding([5, 0])
    .boxed()
}

/// Colour intent for a shortcut chip. Resolved against the active theme
/// inside [`shortcut_chip`] so chips read correctly on light and dark
/// palettes alike.
#[derive(Debug, Clone, Copy)]
enum ChipTone {
    /// Regular trigger text — pairs with the chip's own background.
    Neutral,
    /// Invalid / conflicting / transient-modifier trigger.
    Warning,
}

/// A table row's trigger: key caps for a plain neutral stroke, the
/// chip otherwise (warnings, sequences, gestures).
fn trigger_display<'a>(
    i18n: &signex_i18n::I18n,
    label: &str,
    tone: ChipTone,
    tokens: &signex_types::theme::ThemeTokens,
) -> Element<'a, PrefMsg> {
    if matches!(tone, ChipTone::Neutral)
        && let Some(caps) = keycaps::key_caps(label, tokens)
    {
        return caps;
    }
    shortcut_chip(i18n, label, tone)
}

fn shortcut_chip<'a>(
    i18n: &signex_i18n::I18n,
    label: &str,
    tone: ChipTone,
) -> Element<'a, PrefMsg> {
    let label = if label.trim().is_empty() {
        i18n.message_plain(ids::PREF_KEYMAP_STATE_UNBOUND, &[])
    } else {
        label.to_string()
    };
    container(
        text(raw(label))
            .size(TS.body)
            .style(move |theme: &Theme| text::Style {
                color: Some(match tone {
                    ChipTone::Neutral => theme.palette().background.weak.text,
                    ChipTone::Warning => theme.palette().warning.base.color,
                }),
                ..text::Style::default()
            }),
    )
    .padding([3.0, SP.m])
    .width(Length::Shrink)
    .style(move |theme: &Theme| {
        let palette = theme.palette();
        container::Style {
            background: Some(Background::Color(palette.background.weak.color)),
            border: Border {
                width: 1.0,
                color: palette.background.strong.color,
                radius: 3.0.into(),
                ..Border::default()
            },
            ..container::Style::default()
        }
    })
    .boxed()
}

/// The "overridden by <fixed action>" note for a binding whose stroke a
/// fixed app key claims first (F1, Esc), or `None`.
fn shadow_note(i18n: &signex_i18n::I18n, trigger: &str) -> Option<String> {
    crate::app::dispatch::input::shadowing_fixed_key_for_trigger(trigger)
        .map(|action| i18n.message_plain(ids::PREF_KEYMAP_OVERRIDDEN, &[("action", action.into())]))
}

/// Recorder warning for a recorded chord that a fixed app key would
/// swallow before the profile sees it.
#[cfg(test)]
fn recorder_shadow_warning(trigger: &str) -> Option<signex_i18n::Localized> {
    recorder_shadow_warning_for(signex_i18n::I18n::en_us_ref(), trigger, trigger)
}

/// [`recorder_shadow_warning`] for a trigger shown as `display` whose
/// parseable spelling is `config` (they differ on macOS).
fn recorder_shadow_warning_for(
    i18n: &signex_i18n::I18n,
    display: &str,
    config: &str,
) -> Option<signex_i18n::Localized> {
    crate::app::dispatch::input::shadowing_fixed_key_for_trigger(config).map(|action| {
        i18n.message(
            ids::PREF_KEYMAP_CLAIMED,
            &[("display", display.into()), ("action", action.into())],
        )
    })
}

/// Whether `row` is in a conflict, and whether that conflict is a chord the
/// OS reserves. Compared as strokes resolved for `platform`, not as text:
/// `Mod+K`, `Ctrl+K` and a typed `Cmd+K` all land on the same key on the OS
/// that runs them.
fn row_conflict_kinds(
    row: &crate::keymap::KeymapEditorRow,
    conflicts: &[crate::keymap::BindingConflict],
    platform: crate::keymap::Platform,
) -> (bool, bool) {
    let Some(crate::keymap::ShortcutTrigger::KeySequence(strokes)) =
        crate::keymap::ShortcutTrigger::parse(&row.trigger_config).ok()
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

fn keymap_recorder_control<'a>(
    i18n: &signex_i18n::I18n,
    recorder: &'a crate::app::KeymapRecorderState,
) -> Element<'a, PrefMsg> {
    // The recorder card carries its own themed surfaces, so its text uses
    // theme-aware helpers (base / secondary / warning) rather than the
    // modal's neutral constants — otherwise the labels would vanish on a
    // light theme where the card background flips light.
    let recorded: Element<'a, PrefMsg> = if recorder.strokes.is_empty() {
        text(i18n.message(ids::PREF_KEYMAP_RECORDER_HINT, &[]))
            .size(TS.label)
            .style(iced::widget::text::secondary)
            .boxed()
    } else {
        row(recorded_shortcut_chips(i18n, recorder))
            .spacing(6)
            .align_y(iced::Alignment::Center)
            .boxed()
    };

    let transient_modifiers = if recorder.recording
        && recorder.modifiers != crate::keymap::Modifiers::default()
        && recorder.strokes.len() < crate::app::KeymapRecorderState::MAX_STROKES
    {
        Some(shortcut_chip(
            i18n,
            &format!("{}...", modifiers_label(recorder.modifiers)),
            ChipTone::Warning,
        ))
    } else {
        None
    };
    let capture_status: Element<'a, PrefMsg> = if recorder.recording {
        text(i18n.message(ids::PREF_KEYMAP_RECORDING, &[]))
            .size(TS.body)
            .style(iced::widget::text::warning)
            .boxed()
    } else {
        text(i18n.message(ids::PREF_KEYMAP_RECORDED, &[]))
            .size(TS.body)
            .style(iced::widget::text::secondary)
            .boxed()
    };
    let mut capture_row = row![capture_status, recorded]
        .spacing(SP.m)
        .align_y(iced::Alignment::Center);
    if let Some(transient_modifiers) = transient_modifiers {
        capture_row = capture_row.push(transient_modifiers);
    }

    let recorder_warning: Element<'a, PrefMsg> = match recorder_shadow_warning_for(
        i18n,
        &recorder.trigger_text(),
        &recorder.trigger().config_text(),
    ) {
        Some(message) if !recorder.strokes.is_empty() => text(message)
            .size(TS.body)
            .style(|theme: &Theme| text::Style {
                color: Some(theme.palette().warning.base.color),
                ..text::Style::default()
            })
            .boxed(),
        _ => Space::new().boxed(),
    };

    let record_button = if recorder.recording {
        button(
            container(text(i18n.message(ids::PREF_KEYMAP_STOP, &[])).size(TS.body))
                .padding([5.0, SP.l]),
        )
        .on_press(PrefMsg::KeymapRecorderStop)
        .style(danger_button_style)
    } else {
        button(
            container(text(i18n.message(ids::PREF_KEYMAP_RECORD, &[])).size(TS.body))
                .padding([5.0, SP.l]),
        )
        .on_press(PrefMsg::KeymapRecorderStart)
        .style(primary_button_style)
    };

    container(
        column![
            row![
                column![
                    text(i18n.message(ids::PREF_KEYMAP_RECORDER_TITLE, &[]))
                        .size(TS.heading)
                        .style(iced::widget::text::base),
                    text(raw(recorder.command_label.clone()))
                        .size(TS.body)
                        .style(iced::widget::text::secondary),
                ]
                .spacing(3),
                Space::new().width(Length::Fill),
                button(
                    container(text(i18n.message(ids::PREF_KEYMAP_CANCEL, &[])).size(TS.body))
                        .padding([5.0, SP.l])
                )
                .on_press(PrefMsg::KeymapRecorderCancel)
                .style(secondary_button_style),
            ]
            .align_y(iced::Alignment::Center),
            container(
                column![
                    row![
                        text(context_label(i18n, recorder.context))
                            .size(TS.body)
                            .style(iced::widget::text::secondary),
                        text(i18n.message(ids::PREF_KEYMAP_CURRENT, &[]))
                            .size(TS.body)
                            .style(iced::widget::text::secondary),
                        shortcut_chip(i18n, &recorder.original_trigger, ChipTone::Neutral),
                    ]
                    .spacing(SP.m)
                    .align_y(iced::Alignment::Center),
                    capture_row,
                ]
                .spacing(10),
            )
            .padding(SP.l)
            .width(Length::Fill)
            .style(move |theme: &Theme| {
                let palette = theme.palette();
                container::Style {
                    background: Some(Background::Color(palette.background.weak.color)),
                    border: Border {
                        width: 1.0,
                        color: palette.background.strong.color,
                        radius: 4.0.into(),
                        ..Border::default()
                    },
                    ..container::Style::default()
                }
            }),
            text(i18n.message(ids::PREF_KEYMAP_RECORDER_HELP, &[]))
                .size(TS.caption)
                .style(iced::widget::text::secondary),
            recorder_warning,
            row![
                record_button,
                button(
                    container(text(i18n.message(ids::PREF_KEYMAP_CLEAR, &[])).size(TS.body))
                        .padding([5.0, SP.l])
                )
                .on_press(PrefMsg::KeymapRecorderClear)
                .style(secondary_button_style),
                Space::new().width(Length::Fill),
                button(
                    container(text(i18n.message(ids::PREF_KEYMAP_OK, &[])).size(TS.body))
                        .padding([5.0, SP.l])
                )
                .on_press(PrefMsg::KeymapRecorderApply)
                .style(primary_button_style),
            ]
            .spacing(SP.m)
            .align_y(iced::Alignment::Center),
        ]
        .spacing(SP.l),
    )
    .padding(SP.xl)
    .width(Length::Fill)
    .style(move |theme: &Theme| {
        let palette = theme.palette();
        container::Style {
            background: Some(Background::Color(palette.background.base.color)),
            border: Border {
                width: 1.0,
                color: palette.primary.base.color,
                radius: 6.0.into(),
                ..Border::default()
            },
            ..container::Style::default()
        }
    })
    .boxed()
}

fn recorded_shortcut_chips<'a>(
    i18n: &signex_i18n::I18n,
    recorder: &'a crate::app::KeymapRecorderState,
) -> Vec<Element<'a, PrefMsg>> {
    recorder
        .strokes
        .iter()
        .map(|stroke| shortcut_chip(i18n, &stroke.to_string(), ChipTone::Neutral))
        .collect()
}

fn modifiers_label(modifiers: crate::keymap::Modifiers) -> String {
    modifiers.display_for(crate::keymap::Platform::current())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct KeymapProfileOption {
    id: String,
    label: String,
}

impl KeymapProfileOption {
    fn new(i18n: &signex_i18n::I18n, profile: &crate::keymap::KeymapEditorProfile) -> Self {
        let label = match profile.kind {
            crate::keymap::ShortcutProfileKind::BuiltIn => ids::PREF_KEYMAP_PROFILE_BUILT_IN,
            crate::keymap::ShortcutProfileKind::Custom => ids::PREF_KEYMAP_PROFILE_CUSTOM,
        };
        Self {
            id: profile.id.clone(),
            label: i18n.message_plain(label, &[("name", profile.name.as_str().into())]),
        }
    }
}

impl std::fmt::Display for KeymapProfileOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

fn context_label(
    i18n: &signex_i18n::I18n,
    context: crate::keymap::ShortcutContext,
) -> signex_i18n::Localized {
    i18n.message(
        match context {
            crate::keymap::ShortcutContext::Global => ids::PREF_KEYMAP_CONTEXT_GLOBAL,
            crate::keymap::ShortcutContext::Schematic => ids::PREF_KEYMAP_CONTEXT_SCHEMATIC,
            crate::keymap::ShortcutContext::Footprint => ids::PREF_KEYMAP_CONTEXT_FOOTPRINT,
            crate::keymap::ShortcutContext::Pcb => ids::PREF_KEYMAP_CONTEXT_PCB,
            crate::keymap::ShortcutContext::Library => ids::PREF_KEYMAP_CONTEXT_LIBRARY,
            crate::keymap::ShortcutContext::Modal => ids::PREF_KEYMAP_CONTEXT_MODAL,
            crate::keymap::ShortcutContext::TextInput => ids::PREF_KEYMAP_CONTEXT_TEXT_INPUT,
            crate::keymap::ShortcutContext::CommandPalette => {
                ids::PREF_KEYMAP_CONTEXT_COMMAND_PALETTE
            }
            crate::keymap::ShortcutContext::Placement => ids::PREF_KEYMAP_CONTEXT_PLACEMENT,
        },
        &[],
    )
}

use crate::keymap::category_title;
use signex_i18n::{command_ids, shortcut_ids as ids};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::KeymapEditorModel;

    #[test]
    fn classic_f1_and_escape_bindings_are_flagged_overridden() {
        let mut editor = KeymapEditorModel::built_ins().unwrap();
        editor.set_active_profile("classic").unwrap();
        let flagged = |command: &str| {
            editor
                .rows()
                .iter()
                .filter(|row| row.command.as_ref().map(|c| c.as_str()) == Some(command))
                .map(|row| shadow_note(signex_i18n::I18n::en_us_ref(), &row.trigger))
                .collect::<Vec<_>>()
        };
        // Classic binds F1 to zoom_in_at_cursor and Escape to cancel_current_tool.
        assert_eq!(flagged("zoom_in_at_cursor").iter().flatten().count(), 1);
        assert!(flagged("cancel_current_tool").iter().all(Option::is_some));
        assert!(!flagged("cancel_current_tool").is_empty());
        assert!(shadow_note(signex_i18n::I18n::en_us_ref(), "Ctrl+F1").is_some());
        assert_eq!(shadow_note(signex_i18n::I18n::en_us_ref(), "P W"), None);
        assert_eq!(
            shadow_note(signex_i18n::I18n::en_us_ref(), "DoubleClick"),
            None
        );
    }

    #[test]
    fn recorder_warns_only_for_shadowed_strokes() {
        assert!(recorder_shadow_warning("Esc").is_some());
        assert!(recorder_shadow_warning("X Esc").is_some());
        assert!(recorder_shadow_warning("Ctrl+K").is_none());
    }

    fn editor_with(
        platform: crate::keymap::Platform,
        bindings: &[(&str, &str)],
    ) -> KeymapEditorModel {
        use crate::keymap::{
            AppCommandId, ShortcutBinding, ShortcutBindingAction, ShortcutContext, ShortcutProfile,
            ShortcutProfileKind, ShortcutTrigger,
        };
        KeymapEditorModel::from(ShortcutProfile {
            id: "t".to_string(),
            name: "T".to_string(),
            kind: ShortcutProfileKind::Custom,
            schema_version: 1,
            description: None,
            base_profile: None,
            bindings: bindings
                .iter()
                .map(|(trigger, id)| ShortcutBinding {
                    action: ShortcutBindingAction::Command(AppCommandId::new(*id).unwrap()),
                    context: ShortcutContext::Global,
                    triggers: vec![ShortcutTrigger::parse(trigger).unwrap()],
                    platform: None,
                })
                .collect(),
        })
        .with_platform(platform)
    }

    /// `Mod+K` and a literal key that lands on the same stroke are one
    /// conflict, whatever each row's text says (m1).
    #[test]
    fn a_mod_row_and_a_literal_row_on_one_key_both_show_the_conflict() {
        use crate::keymap::Platform;
        // The second row is typed as a draft in a spelling that is neither
        // `Mod+K` nor how the row is shown, so only a stroke comparison sees
        // that it is the same key.
        for (platform, typed) in [
            (Platform::MacOs, "Cmd+K"),
            (Platform::Windows, "Control+K"),
            (Platform::Linux, "Control+K"),
        ] {
            let mut editor =
                editor_with(platform, &[("Mod+K", "find"), ("F9", "open_preferences")]);
            let slot = editor.rows()[1].slot;
            editor.edit_active_trigger(slot, typed.to_string()).unwrap();
            let conflicts = editor.active_conflicts();
            assert_eq!(conflicts.len(), 1, "{platform:?}");
            for row in editor.rows() {
                assert_eq!(
                    row_conflict_kinds(&row, &conflicts, platform),
                    (true, false),
                    "{platform:?} {}",
                    row.trigger
                );
            }
        }
    }

    /// m2: a chord macOS owns is a conflict marked as the system's.
    #[test]
    fn a_macos_reserved_chord_is_marked_reserved_on_its_row() {
        use crate::keymap::Platform;
        let editor = editor_with(
            Platform::MacOs,
            &[("Mod+Q", "cycle_unit"), ("Mod+S", "save_document")],
        );
        let conflicts = editor.active_conflicts();
        let kinds: Vec<_> = editor
            .rows()
            .iter()
            .map(|row| row_conflict_kinds(row, &conflicts, Platform::MacOs))
            .collect();
        assert_eq!(kinds, [(true, true), (false, false)]);
        let elsewhere = editor_with(Platform::Windows, &[("Mod+Q", "cycle_unit")]);
        assert!(elsewhere.active_conflicts().is_empty());
    }
}
