//! One requirement's card: a header row (the Enabled switch, the kind, its
//! id, a value summary, the outcome pill, Remove and expand), then, when
//! open, its fields in a three-column grid and the last run's findings.

use iced::widget::{Column, Row, Space, column, container, pick_list, row};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Theme, Widget as _};
use serde_json::Value;
use signex_i18n::{I18n, UserText, ids, t};
use signex_types::check::{CheckReport, Finding, Severity};
use signex_types::project::{Requirement, RequirementEntry, RequirementField, RequirementProblem};
use signex_types::theme::ThemeTokens;
use signex_widgets::chips::{ChipMetrics, ChipTone, kind_chip};
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::enable_section::DEPENDENT_VEIL_ALPHA;
use signex_widgets::inert::unless_read_only_veiled;
use signex_widgets::{ToggleSwitchStyle, checkbox, text, text_input, toggle_switch};

use super::super::super::look::{Glyph, err_box, icon_button, pill};
use super::super::super::table::{SP, TS, problem};
use super::super::words::{field_words, kind_words, problem_words};
use super::super::{EntryKey, RequirementsMsg, RequirementsTabState, message};
use super::summary::summary_words;
use super::{Choice, Verdict, findings_of, plain, stored_text, verdict};
use crate::app::Message;
use crate::app::state::ProjectId;
use crate::styles::tokens::{self as ink, resolve};
use crate::styles::{HAIRLINE, PillTone, Radii, ti};

/// Fields per row of a card's grid.
const GRID_COLUMNS: usize = 3;

/// Left inset of a card's fields, in `body`-size characters: they line up
/// under the kind name, past the switch.
const BODY_INSET_EMS: f32 = 4.2;

/// How much of the error colour a failed card's border mixes into the
/// border colour.
const FAILED_EDGE_MIX: f32 = 0.45;

/// Gap between the parts of a card's header row.
const HEADER_GAP: f32 = SP.m + SP.xs;

/// Whether a card shows its fields. A passed card is closed and every other
/// one open; a user flip (`toggled`) inverts that. A card holding a draft or
/// a refusal stays open, so what was typed and why it was refused stay in
/// sight.
pub(in crate::app::project_tab::requirements) fn is_open(
    verdict: Option<Verdict>,
    has_draft_or_problem: bool,
    toggled: bool,
) -> bool {
    if has_draft_or_problem {
        return true;
    }
    let passed = verdict == Some(Verdict::Passed);
    passed == toggled
}

/// One entry's card.
pub(super) struct Card<'s> {
    pub(super) project_id: ProjectId,
    pub(super) index: usize,
    /// The entry's key: its drafts', its refusal's and its flip's.
    pub(super) key: Option<EntryKey>,
    /// Whether the last refusal is this entry's.
    pub(super) refused_here: bool,
    pub(super) tab: Option<&'s RequirementsTabState>,
    pub(super) report: Option<&'s CheckReport>,
    pub(super) i18n: &'s I18n,
    pub(super) tokens: &'s ThemeTokens,
}

impl Card<'_> {
    pub(super) fn view<'a>(&self, entry: &RequirementEntry) -> Element<'a, Message> {
        let verdict = verdict(self.report, entry, self.index);
        let mut col: Column<Element<'a, Message>> = Column::new();
        match entry {
            RequirementEntry::Known { requirement, raw } => {
                let held = self.has_draft_or_problem();
                let toggled = self
                    .tab
                    .zip(self.key.as_ref())
                    .is_some_and(|(tab, key)| tab.toggled.contains(key));
                let open = is_open(verdict, held, toggled);
                let chevron = (!held).then_some(open);
                col = col.push(self.header(requirement, raw, verdict, chevron));
                if open {
                    let findings = findings_of(self.report, entry, self.index);
                    let body = self.body(requirement, raw, &findings);
                    col = col.push(self.veiled(requirement.enabled, body));
                }
            }
            RequirementEntry::Unreadable { reason, .. } => {
                col = col.push(self.unreadable_header(entry, verdict));
                let words =
                    t!(self.i18n, ids::REQUIREMENTS_UNREADABLE, "reason" => reason.as_str());
                col = col.push(self.inset(problem(words, self.tokens)));
            }
        }
        if let Some(refused) = self.card_problem(entry) {
            let words = problem_words(refused, self.i18n);
            col = col.push(self.inset(problem(words, self.tokens)));
        }
        container(col)
            .width(Length::Fill)
            .style(card_style(self.tokens, verdict == Some(Verdict::Failed)))
            .boxed()
    }

    /// The switch, the kind, its id and value summary, the outcome, Remove
    /// and, when the card can close (`chevron`: open or not), expand.
    fn header<'a>(
        &self,
        requirement: &Requirement,
        raw: &Value,
        verdict: Option<Verdict>,
        chevron: Option<bool>,
    ) -> Element<'a, Message> {
        let mut line: Row<Element<'a, Message>> = row![
            self.switch(requirement.enabled),
            self.veiled(requirement.enabled, self.header_info(requirement, raw)),
        ]
        .spacing(HEADER_GAP)
        .align_y(Alignment::Center);
        if let Some(verdict) = verdict {
            line = line.push(self.outcome_pill(verdict));
        }
        line = line.push(self.remove());
        if let Some(open) = chevron {
            line = line.push(self.expand_button(open));
        }
        container(line)
            .padding([SP.m, SP.l])
            .width(Length::Fill)
            .boxed()
    }

    /// The kind, its id chip, the requirement id and the value summary,
    /// filling the header between the switch and the outcome.
    fn header_info<'a>(&self, requirement: &Requirement, raw: &Value) -> Element<'a, Message> {
        let r = resolve(self.tokens);
        let name = requirement.kind.name();
        let mut info: Row<Element<'a, Message>> = row![
            text(t!(self.i18n, kind_words(name)))
                .size(TS.body)
                .font(semibold())
                .color(ti(self.tokens.text)),
            kind_chip(
                UserText::new(name.to_string()),
                ChipTone {
                    fill: r.info_soft,
                    ink: r.info,
                },
                ChipMetrics::from_text_size(TS.body),
            ),
            text(UserText::new(requirement.id.clone()))
                .size(TS.caption)
                .font(mono())
                .color(r.ink4),
        ]
        .spacing(HEADER_GAP)
        .align_y(Alignment::Center);
        if let Some(words) = summary_words(requirement, raw, self.i18n) {
            info = info.push(text(words).size(TS.caption).color(r.ink3).boxed());
        }
        info.push(Space::new().width(Length::Fill).boxed())
            .width(Length::Fill)
            .boxed()
    }

    /// The expand button: it flips the card, `open` or not.
    fn expand_button<'a>(&self, open: bool) -> Element<'a, Message> {
        let (glyph, tip) = if open {
            (Glyph::ChevronDown, ids::REQUIREMENTS_COLLAPSE)
        } else {
            (Glyph::ChevronRight, ids::REQUIREMENTS_EXPAND)
        };
        let flipped = message(
            self.project_id,
            RequirementsMsg::ExpandToggled { index: self.index },
        );
        icon_button(glyph, t!(self.i18n, tip), Some(flipped), self.tokens)
    }

    /// An entry this build cannot read: its id or list place, the outcome
    /// and Remove.
    fn unreadable_header<'a>(
        &self,
        entry: &RequirementEntry,
        verdict: Option<Verdict>,
    ) -> Element<'a, Message> {
        let label = entry
            .id()
            .map_or_else(|| format!("#{}", self.index + 1), str::to_string);
        let mut line: Row<Element<'a, Message>> = row![
            text(UserText::new(label))
                .size(TS.body)
                .font(semibold())
                .color(ti(self.tokens.text))
                .width(Length::Fill),
        ]
        .spacing(HEADER_GAP)
        .align_y(Alignment::Center);
        if let Some(verdict) = verdict {
            line = line.push(self.outcome_pill(verdict));
        }
        line = line.push(self.remove());
        container(line)
            .padding([SP.m, SP.l])
            .width(Length::Fill)
            .boxed()
    }

    /// The Enabled switch: a press turns the requirement the other way.
    fn switch<'a>(&self, enabled: bool) -> Element<'a, Message> {
        let r = resolve(self.tokens);
        let toggled = message(
            self.project_id,
            RequirementsMsg::EnabledToggled {
                index: self.index,
                on: !enabled,
            },
        );
        let style = ToggleSwitchStyle::new(
            ink::accent(self.tokens),
            r.line_strong,
            ink::on_accent(self.tokens),
        );
        let tip = container(
            text(t!(self.i18n, ids::REQUIREMENTS_ENABLED))
                .size(TS.caption)
                .color(ti(self.tokens.text)),
        )
        .padding([SP.xs, SP.s])
        .style(signex_widgets::active_bar_dropdown::tooltip_style(
            r.raised,
            r.line_strong,
        ));
        signex_widgets::tooltip(
            toggle_switch(enabled, style, Some(toggled)),
            tip,
            signex_widgets::tooltip::Position::Top,
        )
        .boxed()
    }

    fn outcome_pill<'a>(&self, verdict: Verdict) -> Element<'a, Message> {
        let (words, tone) = match verdict {
            Verdict::Passed => (ids::REQUIREMENTS_PASSED, PillTone::Ok),
            Verdict::Failed => (ids::REQUIREMENTS_FAILED, PillTone::Err),
            Verdict::NotChecked => (ids::REQUIREMENTS_NOT_CHECKED, PillTone::Neutral),
            Verdict::Off => (ids::REQUIREMENTS_OFF, PillTone::Neutral),
        };
        pill(t!(self.i18n, words), tone, None, self.tokens)
    }

    fn remove<'a>(&self) -> Element<'a, Message> {
        let removed = message(
            self.project_id,
            RequirementsMsg::Removed { index: self.index },
        );
        icon_button(
            Glyph::Trash,
            t!(self.i18n, ids::REQUIREMENTS_REMOVE),
            Some(removed),
            self.tokens,
        )
    }

    /// The open card's fields, three to a row with the caption above each,
    /// then one box per finding of the last run. The id is not a field
    /// here: it shows read-only in the header.
    fn body<'a>(
        &self,
        requirement: &Requirement,
        raw: &Value,
        findings: &[&Finding],
    ) -> Element<'a, Message> {
        let mut cells = vec![self.severity(requirement.severity)];
        for field in RequirementField::of_kind(requirement.kind.name()) {
            cells.push(self.kind_field(*field, raw));
        }
        let mut col: Column<Element<'a, Message>> = Column::new().spacing(SP.m);
        let mut cells = cells.into_iter();
        let rows = cells.len().div_ceil(GRID_COLUMNS);
        for _ in 0..rows {
            let mut line: Row<Element<'a, Message>> = Row::new().spacing(SP.l);
            for _ in 0..GRID_COLUMNS {
                let cell = cells.next().unwrap_or_else(|| Space::new().boxed());
                line = line.push(container(cell).width(Length::FillPortion(1)).boxed());
            }
            col = col.push(line.boxed());
        }
        for finding in findings {
            col = col.push(self.evidence(finding));
        }
        container(col)
            .padding(Padding {
                top: SP.s,
                right: SP.l,
                bottom: SP.l,
                left: TS.body * BODY_INSET_EMS,
            })
            .width(Length::Fill)
            .boxed()
    }

    /// A grid cell: the caption, the field, and the reason its last value
    /// was refused.
    fn cell<'a>(
        &self,
        field: RequirementField,
        value: Element<'a, Message>,
    ) -> Element<'a, Message> {
        let caption = text(t!(self.i18n, field_words(field)))
            .size(TS.caption)
            .color(resolve(self.tokens).ink3);
        let mut col: Column<Element<'a, Message>> = column![caption, value].spacing(SP.xs);
        if let Some(refused) = self.field_problem(field) {
            col = col.push(problem(problem_words(refused, self.i18n), self.tokens));
        }
        col.width(Length::Fill).boxed()
    }

    fn severity<'a>(&self, current: Severity) -> Element<'a, Message> {
        let (project_id, index) = (self.project_id, self.index);
        let choices: Vec<Choice<&'static str>> = [
            ("error", ids::PROJECT_ERC_SEVERITY_ERROR),
            ("warning", ids::PROJECT_ERC_SEVERITY_WARNING),
            ("info", ids::PROJECT_ERC_SEVERITY_INFO),
        ]
        .into_iter()
        .map(|(value, words)| Choice {
            value,
            label: t!(self.i18n, words).to_string(),
        })
        .collect();
        let token = match current {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        };
        let selected = choices.iter().find(|c| c.value == token).cloned();
        let picker = pick_list(selected, choices, Choice::to_string)
            .graphite()
            .on_select(move |choice: Choice<&'static str>| {
                message(
                    project_id,
                    RequirementsMsg::FieldPicked {
                        index,
                        field: RequirementField::Severity,
                        value: choice.value.to_string(),
                    },
                )
            })
            .width(Length::Fill);
        self.cell(RequirementField::Severity, picker.boxed())
    }

    fn kind_field<'a>(&self, field: RequirementField, raw: &Value) -> Element<'a, Message> {
        if field != RequirementField::AllowWarnings {
            return self.text_field(field, raw);
        }
        let (project_id, index) = (self.project_id, self.index);
        let on = raw
            .get(field.key())
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let flag = checkbox(on)
            .style(crate::panels::form_look::checkbox_style)
            .size(TS.title)
            .on_toggle(move |on| {
                let value = on.to_string();
                message(
                    project_id,
                    RequirementsMsg::FieldPicked {
                        index,
                        field,
                        value,
                    },
                )
            });
        self.cell(field, flag.boxed())
    }

    /// A field's box: what was typed, else the stored value; Enter commits.
    /// A refused value keeps its box in the error colours.
    fn text_field<'a>(&self, field: RequirementField, raw: &Value) -> Element<'a, Message> {
        let (project_id, index) = (self.project_id, self.index);
        let draft = self
            .tab
            .zip(self.key.clone())
            .and_then(|(tab, key)| tab.drafts.get(&(key, field)))
            .cloned();
        let shown = draft.unwrap_or_else(|| stored_text(field, raw));
        let hint = if field.is_optional() {
            t!(self.i18n, ids::REQUIREMENTS_NOT_SET)
        } else {
            t!(self.i18n, field_words(field))
        };
        let refused = self.field_problem(field).is_some();
        let (err, err_soft) = (ink::danger(self.tokens), resolve(self.tokens).err_soft);
        let input = text_input(hint, shown)
            .on_input(move |text| {
                message(
                    project_id,
                    RequirementsMsg::FieldEdited { index, field, text },
                )
            })
            .on_submit(message(
                project_id,
                RequirementsMsg::FieldSubmitted { index, field },
            ))
            .style(move |theme: &Theme, status| {
                let mut look = crate::panels::form_look::text_input_style(theme, status);
                if refused {
                    look.border.color = err;
                    look.background = Background::Color(err_soft);
                    look.value = err;
                }
                look
            })
            .font(mono())
            .size(TS.body)
            .padding(SP.xs)
            .width(Length::Fill);
        self.cell(field, input.boxed())
    }

    /// A finding of the last run: its message and the measured values.
    fn evidence<'a>(&self, finding: &Finding) -> Element<'a, Message> {
        let mut parts = vec![finding.message.clone()];
        if let Value::Object(map) = &finding.evidence {
            parts.extend(
                map.iter()
                    .filter(|(key, _)| key.as_str() != "kind")
                    .map(|(key, value)| format!("{key}: {}", plain(value))),
            );
        }
        let words = text(UserText::new(parts.join(" · ")))
            .size(TS.body)
            .color(ink::danger(self.tokens))
            .width(Length::Fill);
        err_box(words.boxed(), self.tokens)
    }

    /// `content` padded like the card's body.
    fn inset<'a>(&self, content: Element<'a, Message>) -> Element<'a, Message> {
        container(content)
            .padding(Padding {
                top: 0.0,
                right: SP.l,
                bottom: SP.l,
                left: SP.l,
            })
            .width(Length::Fill)
            .boxed()
    }

    /// `content` dimmed and taking no input when the requirement is off.
    fn veiled<'a>(&self, enabled: bool, content: Element<'a, Message>) -> Element<'a, Message> {
        let veil = Color {
            a: DEPENDENT_VEIL_ALPHA,
            ..ti(self.tokens.panel_bg)
        };
        unless_read_only_veiled(!enabled, veil, content)
    }

    fn has_draft_or_problem(&self) -> bool {
        self.refused_here
            || self
                .tab
                .zip(self.key.as_ref())
                .is_some_and(|(tab, key)| tab.drafts.keys().any(|(at, _)| at == key))
    }

    /// The refusal of this card's `field`, shown under its box.
    fn field_problem(&self, field: RequirementField) -> Option<&RequirementProblem> {
        let tab = self.tab?;
        match &tab.problem {
            Some((Some(_), refused))
                if self.refused_here && tab.problem_field == Some(field) && shows(field) =>
            {
                Some(refused)
            }
            _ => None,
        }
    }

    /// A refusal of this card that no field box shows: a switch, a remove,
    /// an id, or any edit of an entry that cannot be read.
    fn card_problem(&self, entry: &RequirementEntry) -> Option<&RequirementProblem> {
        let tab = self.tab?;
        let Some((Some(_), refused)) = &tab.problem else {
            return None;
        };
        let in_box =
            matches!(entry, RequirementEntry::Known { .. }) && tab.problem_field.is_some_and(shows);
        (self.refused_here && !in_box).then_some(refused)
    }
}

/// Whether `field` has a box on the card: every field but the id, which
/// the header shows read-only.
fn shows(field: RequirementField) -> bool {
    field != RequirementField::Id
}

/// The card frame: hairline border, card radius; a failed card's border
/// leans to the error colour.
fn card_style(tokens: &ThemeTokens, failed: bool) -> impl Fn(&Theme) -> container::Style + 'static {
    let line = ti(tokens.border);
    let edge = if failed {
        let err = Color {
            a: FAILED_EDGE_MIX,
            ..ink::danger(tokens)
        };
        ink::composite_over(err, line)
    } else {
        line
    };
    let fill = ti(tokens.panel_bg);
    move |_| container::Style {
        background: Some(Background::Color(fill)),
        border: Border {
            width: HAIRLINE,
            color: edge,
            radius: Radii::GRAPHITE.card.into(),
            ..Border::default()
        },
        ..container::Style::default()
    }
}

/// The UI face at semibold, for a card's kind name.
fn semibold() -> iced::Font {
    iced::Font {
        weight: iced::font::Weight::Semibold,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    }
}

/// The mono face, for ids and values.
fn mono() -> iced::Font {
    crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
}
