//! Keyboard Shortcuts section — the profile picker, the grouped /
//! searchable shortcut table, the chord recorder card, and the shortcut
//! chip / profile-option / label helpers. Moved verbatim from the former
//! single-file `preferences` module.

use super::*;
use iced::widget::{Column, Space, button, column, container, row, text, text_input};
use iced::{Background, Border, Element, Length, Theme};

pub(super) fn content_keyboard_shortcuts<'a>(
    editor: &'a crate::keymap::KeymapEditorModel,
    status: &'a str,
    search: &'a str,
    recorder: Option<&'a crate::app::KeymapRecorderState>,
) -> Element<'a, PrefMsg> {
    let profiles = editor.profiles();
    let active = profiles.iter().find(|profile| profile.active);
    let active_option = active.map(KeymapProfileOption::from);
    let active_is_custom = active
        .map(|profile| profile.kind == crate::keymap::ShortcutProfileKind::Custom)
        .unwrap_or(false);
    let active_summary = active
        .map(|profile| format!("{} bindings", profile.binding_count))
        .unwrap_or_else(|| "No active profile".to_string());

    let profile_options: Vec<KeymapProfileOption> =
        profiles.iter().map(KeymapProfileOption::from).collect();

    // Delete is only wired for custom profiles — built-ins are
    // protected by the model, so the button is inert on a built-in.
    let delete_button = {
        let base =
            button(container(text("Delete").size(11)).padding([5, 12])).style(danger_button_style);
        if active_is_custom {
            base.on_press(PrefMsg::KeymapDeleteActiveProfile)
        } else {
            base
        }
    };

    let header = column![
        section_title("Keyboard Shortcuts"),
        Space::new().height(4),
        text("Configure keyboard shortcut profiles. Command names and categories come from Signex command metadata.")
            .size(11)
            .style(text_muted),
    ]
    .spacing(6);

    // Profile picker + recorder stay ABOVE the search box and groups.
    let profile_row = row![
        column![
            text("Profile").size(12).style(text_primary),
            text(active_summary).size(10).style(text_muted),
        ]
        .spacing(3)
        .width(160),
        iced::widget::pick_list(profile_options, active_option, |profile| {
            PrefMsg::KeymapProfileSelected(profile.id)
        })
        .text_size(12)
        .width(180),
        button(container(text("Create").size(11)).padding([5, 12]))
            .on_press(PrefMsg::KeymapCreateCustomProfile)
            .style(primary_button_style),
        delete_button,
        button(container(text("Import").size(11)).padding([5, 12]))
            .on_press(PrefMsg::KeymapImportProfile)
            .style(secondary_button_style),
        button(container(text("Export").size(11)).padding([5, 12]))
            .on_press(PrefMsg::KeymapExportProfile)
            .style(secondary_button_style),
        Space::new().width(Length::Fill),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    let conflicts = editor.active_conflicts();
    let conflict_count = conflicts.len();
    // A non-empty status (parse / save error, profile action) wins;
    // otherwise fall back to the conflict summary.
    let status_line = if !status.is_empty() {
        status.to_string()
    } else if conflict_count == 1 {
        "1 conflict in active profile".to_string()
    } else if conflict_count > 1 {
        format!("{conflict_count} conflicts in active profile")
    } else {
        "No conflicts detected in active keyboard shortcuts.".to_string()
    };

    // Search box — case-insensitive filter across every group. Uses the
    // default (theme-aware) text_input styling like the other panes.
    let search_box = text_input("Search shortcuts…", search)
        .on_input(PrefMsg::KeymapSearchChanged)
        .padding(6)
        .size(12)
        .width(Length::Fill);

    let table_header = row![
        container(text("Category").size(11).style(text_muted)).width(Length::FillPortion(2)),
        container(text("Command").size(11).style(text_muted)).width(Length::FillPortion(4)),
        container(text("Context").size(11).style(text_muted)).width(Length::FillPortion(2)),
        container(text("Shortcut").size(11).style(text_muted)).width(Length::FillPortion(2)),
        container(text("State").size(11).style(text_muted)).width(Length::FillPortion(2)),
    ]
    .spacing(8)
    .padding([4, 0]);

    let active_profile_is_custom = editor.active_profile_is_custom();
    let filtered = editor.filtered_rows(search);

    let mut content = column![
        header,
        profile_row,
        text(status_line).size(10).style(text_muted),
    ]
    .spacing(10)
    .padding(20);

    if let Some(recorder) = recorder {
        content = content.push(keymap_recorder_control(recorder));
    }

    content = content.push(search_box);
    content = content.push(h_sep());

    if filtered.is_empty() {
        let empty = if editor.rows().is_empty() {
            "No shortcuts are defined in the active profile.".to_string()
        } else {
            format!("No shortcuts match “{}”.", search.trim())
        };
        content = content.push(text(empty).size(11).style(text_muted));
        return content.into();
    }

    content = content.push(table_header);

    // One block per group, in the fixed CommandGroup::ALL display order.
    // Groups with no matching rows are skipped entirely.
    for &group in crate::keymap::CommandGroup::ALL {
        let group_rows: Vec<Element<'a, PrefMsg>> = filtered
            .iter()
            .filter(|row_model| row_model.group == group)
            .map(|row_model| keymap_table_row(row_model, &conflicts, active_profile_is_custom))
            .collect();
        if group_rows.is_empty() {
            continue;
        }
        content = content.push(Space::new().height(6));
        content = content.push(section_title(group.display_name()));
        content = content.push(Column::with_children(group_rows).spacing(2));
    }

    content.into()
}

/// Render one shortcut table row. Extracted so the grouped view can build
/// rows per [`crate::keymap::CommandGroup`] without duplicating the cell
/// layout. Text that sits directly on the (theme-neutral) modal surface
/// keeps the shared muted / primary constants; the trigger chip and Edit
/// button pull their colours from the active theme.
fn keymap_table_row<'a>(
    row_model: &crate::keymap::KeymapEditorRow,
    conflicts: &[crate::keymap::BindingConflict],
    active_profile_is_custom: bool,
) -> Element<'a, PrefMsg> {
    let has_conflict = conflicts.iter().any(|conflict| {
        conflict.context == row_model.context
            && conflict.trigger == row_model.trigger
            && row_model.command.as_ref().is_some_and(|command| {
                command == &conflict.first_command || command == &conflict.second_command
            })
    });
    let state = if !row_model.trigger_valid {
        "Invalid"
    } else if has_conflict {
        "Conflict"
    } else if row_model.trigger.trim().is_empty() {
        "Unbound"
    } else if row_model.keyboard_editable && active_profile_is_custom {
        "Editable"
    } else if row_model.keyboard_editable {
        "Create custom"
    } else {
        "Gesture"
    };
    let state_warn = !row_model.trigger_valid || has_conflict;
    let state_unbound = row_model.trigger.trim().is_empty();
    let state_style = move |theme: &Theme| text::Style {
        color: Some(if state_warn {
            theme.palette().warning
        } else if state_unbound {
            theme.extended_palette().secondary.base.color
        } else {
            theme.extended_palette().background.base.text
        }),
    };

    // Keyboard-editable rows in a custom profile get an inline Edit button
    // that opens the chord recorder; everything else is read-only (built-in
    // profiles, pointer gestures).
    let trigger_cell: Element<'a, PrefMsg> =
        if active_profile_is_custom && row_model.keyboard_editable {
            if let Some(command) = row_model.command.clone() {
                let context = row_model.context;
                let label = row_model.label.clone();
                let trigger = row_model.trigger.clone();
                let tone = if state_warn {
                    ChipTone::Warning
                } else {
                    ChipTone::Neutral
                };
                row![
                    shortcut_chip(&row_model.trigger, tone),
                    button(container(text("Edit").size(10)).padding([3, 8]))
                        .on_press(PrefMsg::KeymapRecorderOpen {
                            command,
                            label,
                            context,
                            trigger,
                        })
                        .style(secondary_button_style),
                ]
                .spacing(6)
                .align_y(iced::Alignment::Center)
                .into()
            } else {
                text(row_model.trigger.clone())
                    .size(11)
                    .style(text_muted)
                    .into()
            }
        } else {
            shortcut_chip(&row_model.trigger, ChipTone::Neutral)
        };

    row![
        container(
            text(title_case(&row_model.category))
                .size(11)
                .style(text_muted)
        )
        .width(Length::FillPortion(2)),
        container(text(row_model.label.clone()).size(11).style(text_primary))
            .width(Length::FillPortion(4)),
        container(
            text(context_label(row_model.context))
                .size(11)
                .style(text_muted)
        )
        .width(Length::FillPortion(2)),
        container(trigger_cell).width(Length::FillPortion(2)),
        container(text(state).size(11).style(state_style)).width(Length::FillPortion(2)),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center)
    .padding([5, 0])
    .into()
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

fn shortcut_chip<'a>(label: &str, tone: ChipTone) -> Element<'a, PrefMsg> {
    let label = if label.trim().is_empty() {
        "Unbound".to_string()
    } else {
        label.to_string()
    };
    container(
        text(label)
            .size(11)
            .style(move |theme: &Theme| text::Style {
                color: Some(match tone {
                    ChipTone::Neutral => theme.extended_palette().background.weak.text,
                    ChipTone::Warning => theme.palette().warning,
                }),
            }),
    )
    .padding([3, 8])
    .width(Length::Shrink)
    .style(move |theme: &Theme| {
        let palette = theme.extended_palette();
        container::Style {
            background: Some(Background::Color(palette.background.weak.color)),
            border: Border {
                width: 1.0,
                color: palette.background.strong.color,
                radius: 3.0.into(),
            },
            ..container::Style::default()
        }
    })
    .into()
}

fn keymap_recorder_control<'a>(
    recorder: &'a crate::app::KeymapRecorderState,
) -> Element<'a, PrefMsg> {
    // The recorder card carries its own themed surfaces, so its text uses
    // theme-aware helpers (base / secondary / warning) rather than the
    // modal's neutral constants — otherwise the labels would vanish on a
    // light theme where the card background flips light.
    let recorded: Element<'a, PrefMsg> = if recorder.strokes.is_empty() {
        text("Press Record, then type a shortcut")
            .size(12)
            .style(iced::widget::text::secondary)
            .into()
    } else {
        row(recorded_shortcut_chips(recorder))
            .spacing(6)
            .align_y(iced::Alignment::Center)
            .into()
    };

    let transient_modifiers = if recorder.recording
        && recorder.modifiers != crate::keymap::Modifiers::default()
        && recorder.strokes.len() < crate::app::KeymapRecorderState::MAX_STROKES
    {
        Some(shortcut_chip(
            &format!("{}...", modifiers_label(recorder.modifiers)),
            ChipTone::Warning,
        ))
    } else {
        None
    };
    let capture_status: Element<'a, PrefMsg> = if recorder.recording {
        text("Recording")
            .size(11)
            .style(iced::widget::text::warning)
            .into()
    } else {
        text("Recorded")
            .size(11)
            .style(iced::widget::text::secondary)
            .into()
    };
    let mut capture_row = row![capture_status, recorded]
        .spacing(8)
        .align_y(iced::Alignment::Center);
    if let Some(transient_modifiers) = transient_modifiers {
        capture_row = capture_row.push(transient_modifiers);
    }

    let record_button = if recorder.recording {
        button(container(text("Stop").size(11)).padding([5, 12]))
            .on_press(PrefMsg::KeymapRecorderStop)
            .style(danger_button_style)
    } else {
        button(container(text("Record").size(11)).padding([5, 12]))
            .on_press(PrefMsg::KeymapRecorderStart)
            .style(primary_button_style)
    };

    container(
        column![
            row![
                column![
                    text("Edit Shortcut").size(15).style(iced::widget::text::base),
                    text(recorder.command_label.clone())
                        .size(11)
                        .style(iced::widget::text::secondary),
                ]
                .spacing(3),
                Space::new().width(Length::Fill),
                button(container(text("Cancel").size(11)).padding([5, 12]))
                    .on_press(PrefMsg::KeymapRecorderCancel)
                    .style(secondary_button_style),
            ]
            .align_y(iced::Alignment::Center),
            container(
                column![
                    row![
                        text(context_label(recorder.context))
                            .size(11)
                            .style(iced::widget::text::secondary),
                        text("Current").size(11).style(iced::widget::text::secondary),
                        shortcut_chip(&recorder.original_trigger, ChipTone::Neutral),
                    ]
                    .spacing(8)
                    .align_y(iced::Alignment::Center),
                    capture_row,
                ]
                .spacing(10),
            )
            .padding(12)
            .width(Length::Fill)
            .style(move |theme: &Theme| {
                let palette = theme.extended_palette();
                container::Style {
                    background: Some(Background::Color(palette.background.weak.color)),
                    border: Border {
                        width: 1.0,
                        color: palette.background.strong.color,
                        radius: 4.0.into(),
                    },
                    ..container::Style::default()
                }
            }),
            text("Press the exact keystroke to record it. ESC is recorded as a key; use Cancel to close.")
                .size(10)
                .style(iced::widget::text::secondary),
            row![
                record_button,
                button(container(text("Clear").size(11)).padding([5, 12]))
                    .on_press(PrefMsg::KeymapRecorderClear)
                    .style(secondary_button_style),
                Space::new().width(Length::Fill),
                button(container(text("OK").size(11)).padding([5, 12]))
                    .on_press(PrefMsg::KeymapRecorderApply)
                    .style(primary_button_style),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
        ]
        .spacing(12),
    )
    .padding(16)
    .width(Length::Fill)
    .style(move |theme: &Theme| {
        let palette = theme.extended_palette();
        container::Style {
            background: Some(Background::Color(palette.background.base.color)),
            border: Border {
                width: 1.0,
                color: palette.primary.base.color,
                radius: 6.0.into(),
            },
            ..container::Style::default()
        }
    })
    .into()
}

fn recorded_shortcut_chips<'a>(
    recorder: &'a crate::app::KeymapRecorderState,
) -> Vec<Element<'a, PrefMsg>> {
    recorder
        .strokes
        .iter()
        .map(|stroke| shortcut_chip(&stroke.to_string(), ChipTone::Neutral))
        .collect()
}

fn modifiers_label(modifiers: crate::keymap::Modifiers) -> String {
    let mut parts = Vec::new();
    if modifiers.ctrl {
        parts.push("Ctrl");
    }
    if modifiers.command && !modifiers.ctrl {
        parts.push("Cmd");
    }
    if modifiers.alt {
        parts.push("Alt");
    }
    if modifiers.shift {
        parts.push("Shift");
    }
    parts.join("+")
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct KeymapProfileOption {
    id: String,
    label: String,
}

impl From<&crate::keymap::KeymapEditorProfile> for KeymapProfileOption {
    fn from(profile: &crate::keymap::KeymapEditorProfile) -> Self {
        let kind = match profile.kind {
            crate::keymap::ShortcutProfileKind::BuiltIn => "built-in",
            crate::keymap::ShortcutProfileKind::Custom => "custom",
        };
        Self {
            id: profile.id.clone(),
            label: format!("{} ({kind})", profile.name),
        }
    }
}

impl std::fmt::Display for KeymapProfileOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

fn context_label(context: crate::keymap::ShortcutContext) -> &'static str {
    match context {
        crate::keymap::ShortcutContext::Global => "Global",
        crate::keymap::ShortcutContext::Schematic => "Schematic",
        crate::keymap::ShortcutContext::Footprint => "Footprint",
        crate::keymap::ShortcutContext::Pcb => "PCB",
        crate::keymap::ShortcutContext::Library => "Library",
        crate::keymap::ShortcutContext::Modal => "Modal",
        crate::keymap::ShortcutContext::TextInput => "Text Input",
        crate::keymap::ShortcutContext::CommandPalette => "Command Palette",
        crate::keymap::ShortcutContext::Placement => "Placement",
    }
}

fn title_case(value: &str) -> String {
    value
        .split(['_', '-', ' '])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
