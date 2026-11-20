//! The recorder popover: title and command, the recording box with the
//! captured keycaps, the current shortcut, the help line, the warning for a
//! stroke a fixed app key swallows, and Clear / Stop / Cancel / OK. A key
//! change is held by the recorder until OK applies it to the working copy;
//! the dialog's Apply / OK then saves it.

use iced::widget::{Row, Space, column, container, row};
use iced::{Alignment, Background, Border, Element, Font, Length, Theme, Widget as _};
use signex_i18n::{I18n, MessageId, ids, shortcut_ids as sids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{raw, text};

use super::table::{ChipTone, shortcut_chip};
use super::{ButtonLook, SP, TS, page_button, stroke_labels};
use crate::app::KeymapRecorderState;
use crate::keymap::{ModifiersExt as _, Platform};
use crate::preferences::{PrefMsg, keycaps};
use crate::styles::{self, Radii, ti, tokens};

/// The popover's width: 300 px at the 12 px body.
const POPOVER_W: f32 = TS.body * 25.0;

/// One of the popover's buttons: its label, what it sends, and whether it
/// is the primary action.
#[derive(Debug, Clone)]
pub(super) struct RecorderAction {
    pub(super) label: MessageId,
    pub(super) message: PrefMsg,
    pub(super) is_primary: bool,
}

/// The buttons in order: Clear, then (after a gap) Stop or Record, Cancel
/// and OK. Stop while recording, Record once stopped.
pub(super) fn recorder_actions(recording: bool) -> [RecorderAction; 4] {
    let action = |label, message| RecorderAction {
        label,
        message,
        is_primary: false,
    };
    let toggle = if recording {
        action(sids::PREF_KEYMAP_STOP, PrefMsg::KeymapRecorderStop)
    } else {
        action(sids::PREF_KEYMAP_RECORD, PrefMsg::KeymapRecorderStart)
    };
    [
        action(sids::PREF_KEYMAP_CLEAR, PrefMsg::KeymapRecorderClear),
        toggle,
        action(sids::PREF_KEYMAP_CANCEL, PrefMsg::KeymapRecorderCancel),
        RecorderAction {
            is_primary: true,
            ..action(sids::PREF_KEYMAP_OK, PrefMsg::KeymapRecorderApply)
        },
    ]
}

fn semibold() -> Font {
    Font {
        weight: iced::font::Weight::Semibold,
        ..Font::DEFAULT
    }
}

/// The popover card: raised fill, hairline, card radius, floating shadow.
pub(super) fn recorder_popover<'a>(
    i18n: &I18n,
    recorder: &KeymapRecorderState,
    platform: Platform,
    t: &ThemeTokens,
) -> Element<'a, PrefMsg> {
    let r = tokens::resolve(t);
    let (ink, ink4) = (ti(t.text), r.ink4);
    let title = row![
        text(t!(i18n, sids::PREF_KEYMAP_RECORDER_TITLE))
            .size(TS.body)
            .font(semibold())
            .color(ink)
            .width(Length::Fill),
        // raw: the command label is rendered from the catalog when the
        // recorder opens (#2057).
        text(raw(recorder.command_label.clone()))
            .size(TS.caption)
            .color(ink4),
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center);

    let current_strokes = match crate::keymap::ShortcutTrigger::parse(&recorder.original_config) {
        Ok(crate::keymap::ShortcutTrigger::KeySequence(strokes)) => {
            stroke_labels(i18n, &strokes, platform)
        }
        _ => Vec::new(),
    };
    let then = t!(i18n, ids::PREFERENCES_KEYMAP_THEN);
    let current: Element<'a, PrefMsg> = keycaps::sequence_caps(&current_strokes, &then, t)
        .unwrap_or_else(|| {
            shortcut_chip(i18n, &recorder.original_trigger_in(i18n), ChipTone::Neutral)
        });

    let mut body = column![
        title,
        recording_box(i18n, recorder, platform, t),
        row![
            text(t!(i18n, sids::PREF_KEYMAP_CURRENT))
                .size(TS.caption)
                .color(ink4),
            current,
        ]
        .spacing(SP.m)
        .align_y(Alignment::Center),
        text(t!(i18n, sids::PREF_KEYMAP_RECORDER_HELP))
            .size(TS.caption)
            .color(ink4),
    ]
    .spacing(SP.s + SP.xs);
    if let Some(warning) = recorder_shadow_warning_for(
        i18n,
        &recorder.trigger_text_in(i18n),
        &recorder.trigger().config_text(),
    )
    .filter(|_| !recorder.strokes.is_empty())
    {
        body = body.push(text(warning).size(TS.caption).color(ti(t.warning)).boxed());
    }
    body = body.push(action_row(i18n, recorder.recording, t));

    let (raised, line) = (r.raised, ti(t.border));
    let shadow = styles::floating_shadow(t);
    container(body)
        .padding(SP.m + SP.xs)
        .width(Length::Fixed(POPOVER_W))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(raised)),
            text_color: Some(ink),
            border: Border {
                width: 1.0,
                color: line,
                radius: Radii::GRAPHITE.card.into(),
                ..Border::default()
            },
            shadow,
            ..container::Style::default()
        })
        .boxed()
}

/// The recording box: accent border on the field fill, "● Recording" (or
/// "Recorded" once stopped) over the captured keycaps, and the held
/// modifiers while a stroke is being pressed.
fn recording_box<'a>(
    i18n: &I18n,
    recorder: &KeymapRecorderState,
    platform: Platform,
    t: &ThemeTokens,
) -> Element<'a, PrefMsg> {
    let r = tokens::resolve(t);
    let status = if recorder.recording {
        text(t!(i18n, ids::PREFERENCES_KEYMAP_RECORDING_LIVE))
            .size(TS.caption)
            .color(r.accent_text)
    } else {
        text(t!(i18n, sids::PREF_KEYMAP_RECORDED))
            .size(TS.caption)
            .color(r.ink4)
    };
    let then = t!(i18n, ids::PREFERENCES_KEYMAP_THEN);
    let captured: Element<'a, PrefMsg> = if recorder.strokes.is_empty() {
        text(t!(i18n, sids::PREF_KEYMAP_RECORDER_HINT))
            .size(TS.caption)
            .color(r.ink4)
            .boxed()
    } else {
        let labels = stroke_labels(i18n, &recorder.strokes, platform);
        keycaps::sequence_caps(&labels, &then, t).unwrap_or_else(|| {
            shortcut_chip(i18n, &recorder.trigger_text_in(i18n), ChipTone::Neutral)
        })
    };
    let mut keys: Row<Element<'a, PrefMsg>> = Row::new()
        .spacing(SP.s)
        .align_y(Alignment::Center)
        .push(captured);
    if recorder.recording
        && recorder.modifiers != crate::keymap::Modifiers::default()
        && recorder.strokes.len() < KeymapRecorderState::MAX_STROKES
    {
        let held = recorder.modifiers.display_in(platform, i18n);
        keys = keys.push(shortcut_chip(
            i18n,
            &format!("{held}..."),
            ChipTone::Warning,
        ));
    }
    let (field, accent) = (r.field, ti(t.accent));
    container(
        column![status, keys]
            .spacing(SP.s)
            .align_x(Alignment::Center),
    )
    .padding(SP.m + SP.xs)
    .width(Length::Fill)
    .center_x(Length::Fill)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(field)),
        border: Border {
            width: 1.0,
            color: accent,
            radius: Radii::GRAPHITE.control.into(),
            ..Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}

/// Clear at the left; Stop or Record, Cancel and OK at the right.
fn action_row<'a>(i18n: &I18n, recording: bool, t: &ThemeTokens) -> Element<'a, PrefMsg> {
    let [clear, toggle, cancel, ok] = recorder_actions(recording);
    let button = |action: RecorderAction| {
        let look = if action.is_primary {
            ButtonLook::SmallPrimary
        } else {
            ButtonLook::Small
        };
        page_button(t!(i18n, action.label), Some(action.message), look, t)
    };
    row![
        button(clear),
        Space::new().width(Length::Fill),
        button(toggle),
        button(cancel),
        button(ok),
    ]
    .spacing(SP.s + SP.xs)
    .align_y(Alignment::Center)
    .boxed()
}

/// Recorder warning for a recorded chord that a fixed app key would
/// swallow before the profile sees it.
#[cfg(test)]
pub(super) fn recorder_shadow_warning(trigger: &str) -> Option<signex_i18n::Localized> {
    recorder_shadow_warning_for(signex_i18n::I18n::en_us_ref(), trigger, trigger)
}

/// [`recorder_shadow_warning`] for a trigger shown as `display` whose
/// parseable spelling is `config` (they differ on macOS).
fn recorder_shadow_warning_for(
    i18n: &I18n,
    display: &str,
    config: &str,
) -> Option<signex_i18n::Localized> {
    crate::app::dispatch::input::shadowing_fixed_key_for_trigger(config).map(|action| {
        i18n.message(
            sids::PREF_KEYMAP_CLAIMED,
            &[("display", display.into()), ("action", action.into())],
        )
    })
}
