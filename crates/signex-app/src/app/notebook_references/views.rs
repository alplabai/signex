//! The cards and the list of notebook references (mockup rows 196 and 197):
//! a reference's hover card for the notebook's margin lane, a Notebook
//! badge's hover card for the schematic, the `@` picker list and the
//! status line of the notebook ("31 references · 3 stale"). Pure views;
//! the notebook tab and the canvas mount them.

use iced::widget::{Column, Row, container};
use iced::{Alignment, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, Localized, UserText, ids, t};
use signex_notebook::{
    Badge, BadgeTone, BlockMention, CardAction, CardFacts, FixOffer, PickerEntry, ReferenceCard,
    ReferenceReport, ReferenceState,
};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext::warning_color;
use signex_widgets::{button, text};

use super::NotebookReferenceMsg;
use crate::styles::{DialogButtonKind, Spacing, TypeScale, dialog_button, ti, tokens::resolve};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The text of a card action, or of a fix, as its button reads.
pub fn action_label(i18n: &I18n, action: CardAction, stale: usize) -> Localized {
    match action {
        CardAction::CrossProbe => t!(i18n, ids::NOTEBOOK_ACTION_CROSS_PROBE),
        CardAction::OpenSheet => t!(i18n, ids::NOTEBOOK_ACTION_OPEN_SHEET),
        CardAction::Timeline => t!(i18n, ids::NOTEBOOK_ACTION_TIMELINE),
        CardAction::HighlightNet => t!(i18n, ids::NOTEBOOK_ACTION_HIGHLIGHT_NET),
        CardAction::CopyReference => t!(i18n, ids::NOTEBOOK_ACTION_COPY_REFERENCE),
        CardAction::ShowInSource => t!(i18n, ids::NOTEBOOK_ACTION_SHOW_IN_SOURCE),
        CardAction::UpdateAllStale => t!(i18n, ids::NOTEBOOK_ACTION_UPDATE_ALL, "count" => stale),
    }
}

/// The text of a fix offer's button.
pub fn fix_label(i18n: &I18n, offer: &FixOffer) -> Localized {
    match offer {
        FixOffer::Update { to } => t!(i18n, ids::NOTEBOOK_ACTION_UPDATE_TO, "name" => to.name()),
        FixOffer::RemoveReference => t!(i18n, ids::NOTEBOOK_ACTION_REMOVE_REFERENCE),
        FixOffer::KeepAsPlainText => t!(i18n, ids::NOTEBOOK_ACTION_KEEP_AS_TEXT),
        FixOffer::PickAnotherPin { .. } => t!(i18n, ids::NOTEBOOK_ACTION_PICK_PIN),
    }
}

/// Why a stale reference no longer reads as the design does.
pub fn stale_reason(i18n: &I18n, card: &ReferenceCard, state: &ReferenceState) -> Localized {
    let name = card.reference.name();
    match state {
        ReferenceState::Renamed { now } => t!(
            i18n, ids::NOTEBOOK_CARD_STALE_RENAMED,
            "old" => name, "new" => now.name()
        ),
        ReferenceState::MissingPin { .. } => {
            let (part, pin) = match &card.reference {
                signex_notebook::Reference::Pin { component, pin } => {
                    (component.clone(), pin.clone())
                }
                other => (other.name(), String::new()),
            };
            t!(i18n, ids::NOTEBOOK_CARD_STALE_MISSING_PIN, "part" => part, "pin" => pin)
        }
        ReferenceState::Deleted | ReferenceState::Resolved => {
            t!(i18n, ids::NOTEBOOK_CARD_STALE_DELETED, "name" => name)
        }
    }
}

/// The status line of a notebook file: "31 references · all resolve", or
/// "31 references · 3 stale".
pub fn summary_line(i18n: &I18n, report: &ReferenceReport) -> Localized {
    let count = report.total_uses();
    match report.stale_count() {
        0 => t!(i18n, ids::NOTEBOOK_REF_SUMMARY_OK, "count" => count),
        stale => t!(i18n, ids::NOTEBOOK_REF_SUMMARY_STALE, "count" => count, "stale" => stale),
    }
}

/// One `label value` line of a card.
fn fact<'a, M: 'a>(label: Localized, value: String, tokens: &ThemeTokens) -> Element<'a, M> {
    let r = resolve(tokens);
    let line: Row<Element<'a, M>> = Row::new()
        .spacing(SP.s)
        .push(text(label).size(TS.caption).color(r.ink3).boxed())
        .push(
            text(UserText::new(value))
                .size(TS.caption)
                .color(ti(tokens.text))
                .width(Length::Fill)
                .boxed(),
        );
    line.boxed()
}

/// The facts a card lists, as `label value` lines.
fn fact_lines(i18n: &I18n, card: &ReferenceCard) -> Vec<(Localized, String)> {
    let list = |items: &[String]| items.join(", ");
    match &card.facts {
        CardFacts::Part {
            footprint,
            sheet,
            nets,
            fitted_in,
            variant_count,
            ..
        } => {
            let mut lines = vec![
                (t!(i18n, ids::NOTEBOOK_CARD_FOOTPRINT), footprint.clone()),
                (t!(i18n, ids::NOTEBOOK_CARD_SHEET), sheet.clone()),
                (t!(i18n, ids::NOTEBOOK_CARD_NETS), list(nets)),
            ];
            if *variant_count > 0 {
                let fitted = if fitted_in.len() == *variant_count {
                    t!(i18n, ids::NOTEBOOK_CARD_FITTED_ALL, "count" => *variant_count)
                } else {
                    t!(
                        i18n, ids::NOTEBOOK_CARD_FITTED_SOME,
                        "count" => fitted_in.len(), "total" => *variant_count
                    )
                };
                lines.push((t!(i18n, ids::NOTEBOOK_CARD_VARIANTS), fitted.into_string()));
            }
            lines
        }
        CardFacts::Pin {
            net,
            connected,
            sheet,
            ..
        } => {
            let connected: Vec<String> = connected.iter().map(ToString::to_string).collect();
            vec![
                (
                    t!(i18n, ids::NOTEBOOK_CARD_NET),
                    net.clone().unwrap_or_default(),
                ),
                (t!(i18n, ids::NOTEBOOK_CARD_CONNECTED), list(&connected)),
                (t!(i18n, ids::NOTEBOOK_CARD_SHEET), sheet.clone()),
            ]
        }
        CardFacts::Net { parts, sheets, .. } => vec![
            (t!(i18n, ids::NOTEBOOK_CARD_PARTS), list(parts)),
            (t!(i18n, ids::NOTEBOOK_CARD_SHEETS), list(sheets)),
        ],
        CardFacts::Stale { value, .. } => value
            .iter()
            .map(|value| {
                (
                    t!(i18n, ids::NOTEBOOK_CARD_SAME_PART, "value" => value.as_str()),
                    String::new(),
                )
            })
            .collect(),
    }
}

/// The second line of a card's head: a part's value, a pin's place on its
/// part, a net's pins and sheets.
fn subtitle(i18n: &I18n, card: &ReferenceCard) -> String {
    match &card.facts {
        CardFacts::Part {
            value, description, ..
        } => [value.as_str(), description.as_str()]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" · "),
        CardFacts::Pin { pin_name, .. } => match &card.reference {
            signex_notebook::Reference::Pin { component, pin } => {
                let place = t!(i18n, ids::NOTEBOOK_CARD_PIN_OF, "pin" => pin.as_str(), "part" => component.as_str());
                if pin_name.is_empty() {
                    place.into_string()
                } else {
                    format!("{pin_name} · {}", place.into_string())
                }
            }
            _ => pin_name.clone(),
        },
        CardFacts::Net {
            pin_count, sheets, ..
        } => t!(
            i18n, ids::NOTEBOOK_CARD_NET_SUMMARY,
            "pins" => *pin_count, "sheets" => sheets.len()
        )
        .into_string(),
        CardFacts::Stale { .. } => t!(i18n, ids::NOTEBOOK_CARD_STALE_TITLE).into_string(),
    }
}

/// A reference's hover card: the name in mono and its subtitle, the facts
/// by kind, the blocks that use it and the last measurement, then the
/// actions (a stale reference: why, and its fixes). `file` is the notebook
/// file the reference is in, which a fix edits.
pub fn reference_card<'a>(
    card: &'a ReferenceCard,
    file: &'a str,
    stale: usize,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, NotebookReferenceMsg> {
    let r = resolve(tokens);
    let is_stale = matches!(card.facts, CardFacts::Stale { .. });
    let name_ink = if is_stale {
        warning_color(tokens)
    } else {
        ti(tokens.text)
    };
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let mut column: Column<Element<'a, NotebookReferenceMsg>> = Column::new()
        .spacing(SP.xs)
        .width(Length::Fill)
        .push(
            text(UserText::new(card.reference.name()))
                .size(TS.body)
                .font(mono)
                .color(name_ink)
                .boxed(),
        )
        .push(
            text(UserText::new(subtitle(i18n, card)))
                .size(TS.caption)
                .color(r.ink3)
                .boxed(),
        );
    if let CardFacts::Stale { state, .. } = &card.facts {
        column = column.push(
            text(stale_reason(i18n, card, state))
                .size(TS.caption)
                .color(ti(tokens.text))
                .boxed(),
        );
    }
    for (label, value) in fact_lines(i18n, card) {
        column = column.push(fact(label, value, tokens));
    }
    if !card.blocks.is_empty() {
        let used: Vec<String> = card.blocks.iter().map(|b| b.title.clone()).collect();
        let uses = t!(i18n, ids::NOTEBOOK_CARD_USES, "count" => card.uses).into_string();
        let value = format!("{} ({uses})", used.join(", "));
        column = column.push(fact(t!(i18n, ids::NOTEBOOK_CARD_USED_IN), value, tokens));
    }
    if let Some(last) = &card.last_measurement {
        let when = last.date.map(|d| format!(" · {d}")).unwrap_or_default();
        let value = format!("{}{when}", last.title);
        column = column.push(fact(
            t!(i18n, ids::NOTEBOOK_CARD_LAST_MEASURED),
            value,
            tokens,
        ));
    }
    column = column.push(actions_row(card, file, stale, i18n, tokens));
    card_frame(column, tokens)
}

/// The buttons of a card: a stale reference's fixes first, then its
/// actions.
fn actions_row<'a>(
    card: &'a ReferenceCard,
    file: &'a str,
    stale: usize,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, NotebookReferenceMsg> {
    let mut row: Row<Element<'a, NotebookReferenceMsg>> = Row::new().spacing(SP.s);
    if let CardFacts::Stale { fixes, .. } = &card.facts {
        for offer in fixes {
            // Pick another pin opens the pin list; the others apply at once.
            let fix = match offer {
                FixOffer::Update { to } => Some(signex_notebook::Fix::UpdateTo { to: to.clone() }),
                FixOffer::RemoveReference => Some(signex_notebook::Fix::RemoveReference),
                FixOffer::KeepAsPlainText => Some(signex_notebook::Fix::KeepAsPlainText),
                FixOffer::PickAnotherPin { .. } => None,
            };
            let press = fix.map(|fix| NotebookReferenceMsg::FixPicked {
                file: file.to_string(),
                reference: card.reference.clone(),
                fix,
            });
            row = row.push(dialog_button(
                fix_label(i18n, offer),
                DialogButtonKind::Primary,
                press,
                tokens,
            ));
        }
    }
    for &action in &card.actions {
        let press = NotebookReferenceMsg::CardActionPicked {
            reference: card.reference.clone(),
            action,
        };
        row = row.push(dialog_button(
            action_label(i18n, action, stale),
            DialogButtonKind::Secondary,
            Some(press),
            tokens,
        ));
    }
    row.boxed()
}

/// The raised card the hover cards sit in.
fn card_frame<'a, M: 'a>(content: Column<Element<'a, M>>, tokens: &ThemeTokens) -> Element<'a, M> {
    container(content)
        .padding(SP.m)
        .width(Length::Fill)
        .style(crate::styles::card(tokens))
        .boxed()
}

/// The words of a badge's colour.
pub fn tone_label(i18n: &I18n, tone: BadgeTone) -> Localized {
    match tone {
        BadgeTone::Notes => t!(i18n, ids::NOTEBOOK_BADGE_TONE_NOTES),
        BadgeTone::Pass => t!(i18n, ids::NOTEBOOK_BADGE_TONE_PASS),
        BadgeTone::OpenIssue => t!(i18n, ids::NOTEBOOK_BADGE_TONE_OPEN_ISSUE),
        BadgeTone::Failing => t!(i18n, ids::NOTEBOOK_BADGE_TONE_FAILING),
    }
}

/// A Notebook badge's hover card: the part or net with the block count,
/// what the colour says, one line per block (kind, title, status, date);
/// a click opens the notebook at the first block.
pub fn badge_card<'a>(
    badge: &'a Badge,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, NotebookReferenceMsg> {
    let r = resolve(tokens);
    let title = t!(
        i18n, ids::NOTEBOOK_BADGE_CARD_TITLE,
        "name" => badge.target.name(), "count" => badge.count()
    );
    let mut column: Column<Element<'a, NotebookReferenceMsg>> = Column::new()
        .spacing(SP.xs)
        .width(Length::Fill)
        .push(text(title).size(TS.body).color(ti(tokens.text)).boxed())
        .push(
            text(tone_label(i18n, badge.tone))
                .size(TS.caption)
                .color(r.ink3)
                .boxed(),
        );
    for block in &badge.blocks {
        column = column.push(block_line(block, i18n, tokens));
    }
    column = column.push(
        text(t!(i18n, ids::NOTEBOOK_BADGE_CARD_HINT))
            .size(TS.caption)
            .color(r.ink3)
            .boxed(),
    );
    let frame = card_frame(column, tokens);
    match badge.blocks.first() {
        Some(first) => button(frame)
            .padding(0)
            .style(button::text)
            .on_press(NotebookReferenceMsg::BlockOpened {
                file: first.file.clone(),
                line: first.line,
            })
            .boxed(),
        None => frame,
    }
}

/// One block of a badge card: kind, title, status and date.
fn block_line<'a>(
    block: &'a BlockMention,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, NotebookReferenceMsg> {
    let r = resolve(tokens);
    let mut line: Row<Element<'a, NotebookReferenceMsg>> = Row::new()
        .spacing(SP.s)
        .align_y(Alignment::Center)
        .push(
            text(crate::panels::annotations::kind_label(i18n, block.kind))
                .size(TS.caption)
                .color(r.ink3)
                .boxed(),
        )
        .push(
            text(UserText::new(block.title.as_str()))
                .size(TS.caption)
                .color(ti(tokens.text))
                .width(Length::Fill)
                .boxed(),
        );
    if let Some(status) = block.status {
        line = line.push(
            text(crate::panels::annotations::status_label(i18n, status))
                .size(TS.caption)
                .color(r.ink3)
                .boxed(),
        );
    }
    if let Some(date) = block.date {
        line = line.push(
            text(UserText::new(date.to_string()))
                .size(TS.caption)
                .color(r.ink3)
                .boxed(),
        );
    }
    line.boxed()
}

/// The `@` picker: one row per entry (the call's name in mono and its
/// detail), `selected` highlighted; a click picks the entry. An empty list
/// says nothing matches.
pub fn picker_list<'a>(
    entries: &'a [PickerEntry],
    selected: usize,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, PickerEntry> {
    let r = resolve(tokens);
    let mut column: Column<Element<'a, PickerEntry>> = Column::new().width(Length::Fill);
    if entries.is_empty() {
        column = column.push(
            text(t!(i18n, ids::NOTEBOOK_PICKER_EMPTY))
                .size(TS.caption)
                .color(r.ink3)
                .boxed(),
        );
    }
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let (selection, ink) = (ti(tokens.selection), ti(tokens.text));
    for (index, entry) in entries.iter().enumerate() {
        let row: Row<Element<'a, PickerEntry>> = Row::new()
            .spacing(SP.m)
            .push(
                text(UserText::new(entry.reference.name()))
                    .size(TS.body)
                    .font(mono)
                    .color(ink)
                    .boxed(),
            )
            .push(
                text(UserText::new(entry.detail.as_str()))
                    .size(TS.caption)
                    .color(r.ink3)
                    .boxed(),
            );
        let chosen = index == selected;
        column = column.push(
            button(row)
                .padding([SP.xs, SP.s])
                .width(Length::Fill)
                .on_press(entry.clone())
                .style(move |_: &Theme, _status| button::Style {
                    background: chosen.then_some(iced::Background::Color(selection)),
                    text_color: ink,
                    ..button::Style::default()
                })
                .boxed(),
        );
    }
    card_frame(column, tokens)
}
