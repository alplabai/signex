//! Properties panel — the project's net-class table (#1042).
//!
//! Pure view plus the message and state types it needs. Every edit is a
//! [`NetClassMsg`] that the dock handler turns into a
//! `signex_engine::NetClassCommand`; nothing here mutates the table.

use super::*;
use signex_types::net_class::{NetClass, NetLineStyle, format_hex_color};

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
    SetLine(NetLineStyle),
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
}

pub fn view_net_classes_section<'a>(
    ctx: &'a PanelContext,
    palette: PanelPalette,
) -> Column<'a, PanelMsg> {
    let PanelPalette {
        muted,
        primary,
        border: border_c,
        ..
    } = palette;
    let panel = &ctx.net_classes;
    collapsible_section(
        "prop_net_classes",
        "Net Classes",
        &ctx.collapsed_sections,
        primary,
        border_c,
        move || {
            let mut c = Column::new().spacing(4).width(Length::Fill);
            if !panel.available {
                return c.push(
                    container(
                        text("Open a project to define net classes.")
                            .size(11)
                            .color(muted),
                    )
                    .padding([4, 8]),
                );
            }
            let wrap = |m: NetClassMsg| PanelMsg::NetClass(m);

            let undo = iced::widget::button(text("Undo").size(10)).padding([3, 8]);
            let redo = iced::widget::button(text("Redo").size(10)).padding([3, 8]);
            let undo = if panel.can_undo {
                undo.on_press(wrap(NetClassMsg::Undo))
            } else {
                undo
            };
            let redo = if panel.can_redo {
                redo.on_press(wrap(NetClassMsg::Redo))
            } else {
                redo
            };
            c = c.push(
                container(
                    row![
                        iced::widget::button(text("+ Add Class").size(10))
                            .padding([3, 8])
                            .on_press(wrap(NetClassMsg::Add)),
                        undo,
                        redo,
                    ]
                    .spacing(4),
                )
                .padding([4, 8]),
            );
            if panel.classes.is_empty() {
                return c.push(
                    container(
                        text("No net classes. A net in no class draws as before.")
                            .size(11)
                            .color(muted),
                    )
                    .padding([4, 8]),
                );
            }

            let selected = panel.selected.min(panel.classes.len() - 1);
            let mut list = Column::new().spacing(2);
            for (idx, class) in panel.classes.iter().enumerate() {
                let btn = iced::widget::button(text(class.name.clone()).size(11))
                    .padding([2, 8])
                    .width(Length::Fill)
                    .on_press(wrap(NetClassMsg::Select(idx)));
                list = list.push(if idx == selected {
                    btn.style(iced::widget::button::primary)
                } else {
                    btn.style(iced::widget::button::secondary)
                });
            }
            c = c.push(container(list).padding([2, 8]));
            c = c.push(
                container(
                    text("The first matching class wins; reorder to change it.")
                        .size(10)
                        .color(muted),
                )
                .padding([0, 8]),
            );
            c = c.push(
                container(
                    row![
                        iced::widget::button(text("Move Up").size(10))
                            .padding([3, 8])
                            .on_press_maybe((selected > 0).then(|| wrap(NetClassMsg::MoveUp))),
                        iced::widget::button(text("Move Down").size(10))
                            .padding([3, 8])
                            .on_press_maybe(
                                (selected + 1 < panel.classes.len())
                                    .then(|| wrap(NetClassMsg::MoveDown))
                            ),
                        iced::widget::button(text("Delete").size(10))
                            .padding([3, 8])
                            .on_press(wrap(NetClassMsg::RemoveSelected)),
                    ]
                    .spacing(4),
                )
                .padding([4, 8]),
            );

            let field = |label: &str, hint: &str, f: NetClassField, value: &str| {
                commit_row(label, hint, value, muted, f)
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
                NetLineStyle::ALL.to_vec(),
                panel.classes[selected].style.line,
                |l| PanelMsg::NetClass(NetClassMsg::SetLine(l)),
                muted,
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
                        text(notice.clone())
                            .size(10)
                            .color(theme_ext::warning_color(&ctx.tokens)),
                    )
                    .padding([2, 8]),
                );
            }
            c
        },
    )
}

/// Label | text input that edits a draft and commits on Enter.
fn commit_row<'a>(
    label: &str,
    hint: &str,
    value: &str,
    label_c: Color,
    field: NetClassField,
) -> Element<'a, PanelMsg> {
    container(
        row![
            widgets::property_label(label.to_string(), label_c),
            iced::widget::text_input(hint, value)
                .on_input(move |s| PanelMsg::NetClass(NetClassMsg::Draft(field, s)))
                .on_submit(PanelMsg::NetClass(NetClassMsg::Commit(field)))
                .size(11)
                .padding(4)
                .width(Length::FillPortion(PROPERTY_CONTROL_PORTION)),
        ]
        .spacing(8.0)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .into()
}
