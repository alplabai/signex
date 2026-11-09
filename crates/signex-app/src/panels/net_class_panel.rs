//! Properties panel — the project's net-class table (#1042).
//!
//! Pure view plus the message and state types it needs. Every edit is a
//! [`NetClassMsg`] that the dock handler turns into a
//! `signex_engine::NetClassCommand`; nothing here mutates the table.

use super::*;
use crate::icons;
use crate::styles::tokens::{hover_overlay, resolve};
use crate::styles::{Spacing, TypeScale, ti};
use signex_types::net_class::{NetClass, NetLineStyle, format_hex_color};
use signex_types::theme::ThemeId;

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// A text field of the selected class. Edited as a draft and committed on
/// Enter, so one edit is one undo step rather than one per keystroke.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetClassField {
    Name,
    Color,
    Width,
    Nets,
    Patterns,
}

#[derive(Debug, Clone)]
pub enum NetClassMsg {
    Add,
    RemoveSelected,
    Select(usize),
    Draft(NetClassField, String),
    Commit(NetClassField),
    /// The class's line style; `None` is Default (inherit the project's).
    SetLine(Option<NetLineStyle>),
    /// Put `net` in `class` (and out of every other class's exact list), or
    /// in no class for `None`, in `project`'s table. Sent from a selected net
    /// label's Properties; `project` is the one that label's sheet belongs
    /// to, which is not necessarily the active project.
    AssignNet {
        project: crate::app::ProjectId,
        net: String,
        class: Option<String>,
    },
    MoveUp,
    MoveDown,
    Undo,
    Redo,
}

/// Text buffers for the selected class's fields.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NetClassDraft {
    pub name: String,
    pub color: String,
    pub width: String,
    pub nets: String,
    pub patterns: String,
}

impl NetClassDraft {
    pub fn from_class(class: &NetClass) -> Self {
        Self {
            name: class.name.clone(),
            color: class.style.color.map(format_hex_color).unwrap_or_default(),
            width: class
                .style
                .wire_width_mm
                .map(|w| w.to_string())
                .unwrap_or_default(),
            nets: class.nets.join(", "),
            patterns: class.patterns.join(", "),
        }
    }

    pub fn field_mut(&mut self, field: NetClassField) -> &mut String {
        match field {
            NetClassField::Name => &mut self.name,
            NetClassField::Color => &mut self.color,
            NetClassField::Width => &mut self.width,
            NetClassField::Nets => &mut self.nets,
            NetClassField::Patterns => &mut self.patterns,
        }
    }
}

/// What the Net Classes section shows. `classes`, `can_undo` and `can_redo`
/// are copied from the active project on every panel refresh; `selected`,
/// `draft` and `notice` are the panel's own state and survive a refresh.
#[derive(Debug, Clone, Default)]
pub struct NetClassPanel {
    /// A project is active, so there is a table to edit.
    pub available: bool,
    /// The project `selected` and `draft` belong to. When the active project
    /// differs, both are reset so one project's edits never land on another.
    pub project: Option<crate::app::ProjectId>,
    pub classes: Vec<NetClass>,
    pub selected: usize,
    pub draft: NetClassDraft,
    pub can_undo: bool,
    pub can_redo: bool,
    /// Why the last edit was refused.
    pub notice: Option<String>,
    /// The net class picker of the selected net label, when its net is known.
    pub label: Option<LabelClassPick>,
}

/// What a selected net label's Net Class picker needs: the project its sheet
/// belongs to, the net the project netlist puts the label on, and that
/// project's class table.
#[derive(Debug, Clone, PartialEq)]
pub struct LabelClassPick {
    pub project: crate::app::ProjectId,
    /// The netlist's name for the label's net, which is what a class stores.
    pub net: String,
    pub classes: Vec<NetClass>,
}

/// One entry of a net label's Net Class picker. "No class" is its own variant,
/// not a reserved string, so a class a loaded `.snxprj` happens to name
/// `(none)` is still a class when picked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetClassChoice {
    NoClass,
    Class(String),
}

impl NetClassChoice {
    /// The class to assign, or `None` to clear the assignment.
    #[must_use]
    pub fn into_class(self) -> Option<String> {
        match self {
            Self::NoClass => None,
            Self::Class(name) => Some(name),
        }
    }
}

impl std::fmt::Display for NetClassChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoClass => f.write_str(signex_types::net_class::NO_CLASS_ENTRY),
            // A class literally named like the clear entry is quoted, so the
            // picker never shows two identical rows.
            Self::Class(name) if name == signex_types::net_class::NO_CLASS_ENTRY => {
                write!(f, "\"{name}\"")
            }
            Self::Class(name) => f.write_str(name),
        }
    }
}

/// The class `net` belongs to under the table's first-match rule, as the
/// entry to show selected in a net label's Net Class picker.
pub fn net_class_choice(classes: &[NetClass], net: &str) -> NetClassChoice {
    signex_types::net_class::first_matching_class(classes, net)
        .map_or(NetClassChoice::NoClass, |c| {
            NetClassChoice::Class(c.name.clone())
        })
}

/// Entries for a net label's Net Class picker: none, then every class.
pub fn net_class_choices(classes: &[NetClass]) -> Vec<NetClassChoice> {
    std::iter::once(NetClassChoice::NoClass)
        .chain(
            classes
                .iter()
                .map(|c| NetClassChoice::Class(c.name.clone())),
        )
        .collect()
}

pub fn view_net_classes_section<'a>(ctx: &'a PanelContext) -> Column<Element<'a, PanelMsg>> {
    let panel = &ctx.net_classes;
    let tokens = &ctx.tokens;
    let theme_id = ctx.theme_id;
    let hint_ink = resolve(tokens).ink3;
    ctx.collapsible(
        "prop_net_classes",
        "Net Classes",
        &ctx.collapsed_sections,
        move || {
            let muted = hint_ink;
            let label = LabelInk::graphite(muted);
            let mut c = Column::new().spacing(SP.s).width(Length::Fill);
            if !panel.available {
                return c.push(
                    container(
                        text(raw("Open a project to define net classes."))
                            .size(TS.body)
                            .color(muted),
                    )
                    .padding([SP.s, SP.m])
                    .boxed(),
                );
            }
            let wrap = |m: NetClassMsg| PanelMsg::NetClass(m);

            c = c.push(
                container(
                    row![
                        add_class_button(theme_id, tokens),
                        Space::new().width(Length::Fill),
                        history_button(
                            icons::icon_undo(theme_id),
                            "Undo",
                            panel.can_undo.then(|| wrap(NetClassMsg::Undo)),
                            tokens,
                        ),
                        history_button(
                            icons::icon_redo(theme_id),
                            "Redo",
                            panel.can_redo.then(|| wrap(NetClassMsg::Redo)),
                            tokens,
                        ),
                    ]
                    .spacing(SP.s)
                    .align_y(iced::Alignment::Center),
                )
                .padding([SP.s, SP.m])
                .boxed(),
            );
            if panel.classes.is_empty() {
                return c.push(
                    container(
                        text(raw("No net classes. A net in no class draws as before."))
                            .size(TS.body)
                            .color(muted),
                    )
                    .padding([SP.s, SP.m])
                    .boxed(),
                );
            }

            let selected = panel.selected.min(panel.classes.len() - 1);
            let mut list = Column::new().spacing(SP.xs);
            for (idx, class) in panel.classes.iter().enumerate() {
                let is_selected = idx == selected;
                let btn = signex_widgets::button(
                    text(signex_i18n::UserText::new(class.name.clone())).size(TS.body),
                )
                .padding([SP.xs, SP.m])
                .width(Length::Fill)
                .on_press(wrap(NetClassMsg::Select(idx)))
                .style(form_look::option_tile(is_selected));
                list = list.push(btn.boxed());
            }
            c = c.push(container(list).padding([SP.xs, SP.m]).boxed());
            c = c.push(
                container(
                    text(raw("The first matching class wins; reorder to change it."))
                        .size(TS.caption)
                        .color(muted),
                )
                .padding([0.0, SP.m])
                .boxed(),
            );
            c = c.push(
                container(
                    row![
                        action_button("Move Up", (selected > 0).then(|| wrap(NetClassMsg::MoveUp))),
                        action_button(
                            "Move Down",
                            (selected + 1 < panel.classes.len())
                                .then(|| wrap(NetClassMsg::MoveDown))
                        ),
                        action_button("Delete", Some(wrap(NetClassMsg::RemoveSelected))),
                    ]
                    .spacing(SP.s),
                )
                .padding([SP.s, SP.m])
                .boxed(),
            );

            let field = |label_text: &str, hint: &str, f: NetClassField, value: &str| {
                commit_row(label_text, hint, value, label, f)
            };
            c = c.push(field(
                "Name",
                "Class name",
                NetClassField::Name,
                &panel.draft.name,
            ));
            c = c.push(field(
                "Colour",
                "#RRGGBB, empty = theme",
                NetClassField::Color,
                &panel.draft.color,
            ));
            c = c.push(field(
                "Width (mm)",
                "empty = wire's own",
                NetClassField::Width,
                &panel.draft.width,
            ));
            c = c.push(form_pick_row(
                "Line",
                ClassLine::options(),
                ClassLine(panel.classes[selected].style.line),
                |l: ClassLine| PanelMsg::NetClass(NetClassMsg::SetLine(l.0)),
                label,
            ));
            c = c.push(field(
                "Nets",
                "GND, +3V3",
                NetClassField::Nets,
                &panel.draft.nets,
            ));
            c = c.push(field(
                "Patterns",
                "VCC*, ?SDA",
                NetClassField::Patterns,
                &panel.draft.patterns,
            ));
            if let Some(notice) = &panel.notice {
                c = c.push(
                    container(
                        text(raw(notice.clone()))
                            .size(TS.caption)
                            .color(theme_ext::warning_color(&ctx.tokens)),
                    )
                    .padding([SP.xs, SP.m])
                    .boxed(),
                );
            }
            c
        },
    )
}

/// Ghost look; a disabled button loses its hover and mutes its label.
fn ghost_style(theme: &Theme, status: button::Status) -> button::Style {
    let mut style = form_look::ghost_button(theme, status);
    if status == button::Status::Disabled {
        style.text_color = form_look::FormLook::of(theme).ink4;
    }
    style
}

/// Text ghost button; disabled without a message.
fn action_button<'a>(label: &str, on_press: Option<PanelMsg>) -> Element<'a, PanelMsg> {
    button(text(raw(label.to_string())).size(TS.caption))
        .padding([3.0, SP.m])
        .on_press_maybe(on_press)
        .style(ghost_style)
        .boxed()
}

/// "+ Add Class": ghost button with the plus icon.
fn add_class_button<'a>(theme_id: ThemeId, tokens: &ThemeTokens) -> Element<'a, PanelMsg> {
    let ink = ti(tokens.text);
    let glyph = svg(icons::icon_plus(theme_id))
        .width(12)
        .height(12)
        .style(move |_: &Theme, _| svg::Style { color: Some(ink) });
    button(
        row![glyph, text(raw("Add Class")).size(TS.caption)]
            .spacing(SP.xs)
            .align_y(iced::Alignment::Center),
    )
    .padding([3.0, SP.m])
    .on_press(PanelMsg::NetClass(NetClassMsg::Add))
    .style(ghost_style)
    .boxed()
}

/// Undo / Redo: a ghost icon button with a tooltip; muted and inert when
/// `on_press` is `None`.
fn history_button<'a>(
    handle: svg::Handle,
    label: &str,
    on_press: Option<PanelMsg>,
    tokens: &ThemeTokens,
) -> Element<'a, PanelMsg> {
    let r = resolve(tokens);
    let ink = if on_press.is_some() {
        ti(tokens.text)
    } else {
        r.ink4
    };
    let glyph = svg(handle)
        .width(14)
        .height(14)
        .style(move |_: &Theme, _| svg::Style { color: Some(ink) });
    let btn = button(glyph)
        .padding([4.0, 6.0])
        .on_press_maybe(on_press)
        .style(ghost_style);
    let hint = container(
        text(raw(label.to_string()))
            .size(TS.caption)
            .color(ti(tokens.text)),
    )
    .padding([3, 8])
    .style(signex_widgets::active_bar_dropdown::tooltip_style(
        hover_overlay(tokens),
        ti(tokens.border),
    ));
    signex_widgets::tooltip(btn, hint, iced::widget::tooltip::Position::Bottom)
        .gap(4)
        .boxed()
}

/// Label | text input that edits a draft and commits on Enter.
fn commit_row<'a>(
    label: &str,
    hint: &str,
    value: &str,
    label_c: LabelInk,
    field: NetClassField,
) -> Element<'a, PanelMsg> {
    container(
        row![
            widgets::property_label(label.to_string(), label_c),
            signex_widgets::text_input(signex_widgets::raw(hint.to_string()), value.to_string())
                .on_input(move |s| PanelMsg::NetClass(NetClassMsg::Draft(field, s)))
                .on_submit(PanelMsg::NetClass(NetClassMsg::Commit(field)))
                .size(TS.body)
                .padding(SP.s)
                .width(Length::FillPortion(PROPERTY_CONTROL_PORTION)),
        ]
        .spacing(SP.m)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .boxed()
}

#[cfg(test)]
mod tests {
    use super::NetClassChoice;

    #[test]
    fn a_class_named_like_the_clear_entry_is_shown_distinctly() {
        let clear = NetClassChoice::NoClass.to_string();
        let lookalike = NetClassChoice::Class(clear.clone()).to_string();

        assert_ne!(clear, lookalike);
        assert_eq!(NetClassChoice::Class("Power".into()).to_string(), "Power");
    }
}

/// A class line style choice: `None` is Default, the project's line style
/// (#2026); Solid, Dashed and Dotted are explicit and override it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ClassLine(Option<NetLineStyle>);

impl ClassLine {
    fn options() -> Vec<Self> {
        std::iter::once(Self(None))
            .chain(NetLineStyle::ALL.iter().map(|line| Self(Some(*line))))
            .collect()
    }
}

impl std::fmt::Display for ClassLine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.map_or("Default", NetLineStyle::label))
    }
}
