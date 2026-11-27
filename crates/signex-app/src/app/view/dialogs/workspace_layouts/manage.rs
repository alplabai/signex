//! Manage Layouts (mockup row 166): the built-ins and your layouts on the left (a user
//! row drags by its grip to reorder), the selected layout's detail on the
//! right, the "Deleted layout" toast with Undo, and Import / Export.

use iced::widget::tooltip as tip;
use iced::widget::{Column, Row, Space, column, container, mouse_area, scrollable, stack};
use iced::{Background, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, Localized, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, text, text_input, tooltip};

use super::{GLYPH_THEME, ICON, SP, TS, caption, form_key, form_row, icon, shortcut_field};
use crate::app::handlers::workspace_layouts::RENAME_INPUT_ID;
use crate::app::{Message, Signex};
use crate::styles::{
    DialogButtonKind, PillTone, Radii, dialog_button, dialog_footer_split, ti, tokens,
};
use crate::workspace_layouts::{
    BuiltinLayout, LayoutId, Layouts, ManageLayoutsDialog, ManageLayoutsMsg, NameClash, Naming,
    WorkspaceLayout,
};

/// The mock's 280 px list column.
const LIST_WIDTH: f32 = 280.0;
/// The mock's 84 × 54 list thumbnail.
const SMALL_THUMB: (f32, f32) = (84.0, 54.0);
/// The detail thumbnail's height.
const BIG_THUMB_H: f32 = 200.0;
/// The detail form's key column.
const KEY_WIDTH: f32 = 110.0;
/// The greyed Shortcut field's width.
const SHORTCUT_WIDTH: f32 = 160.0;
/// Space between the toast and the dialog's bottom edge.
const TOAST_INSET: f32 = 8.0;

fn manage(msg: ManageLayoutsMsg) -> Message {
    msg.into()
}

fn semibold() -> iced::Font {
    iced::Font {
        weight: iced::font::Weight::Semibold,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    }
}

/// A list row's look: the selection fill when selected, the hover overlay
/// under the pointer.
fn row_style(
    tokens: &ThemeTokens,
    selected: bool,
) -> impl Fn(&Theme, iced::widget::button::Status) -> iced::widget::button::Style + 'static {
    let (selection, hover, ink) = (
        ti(tokens.selection),
        tokens::hover_overlay(tokens),
        ti(tokens.text),
    );
    move |_, status| iced::widget::button::Style {
        background: if selected {
            Some(Background::Color(selection))
        } else if matches!(
            status,
            iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed
        ) {
            Some(Background::Color(hover))
        } else {
            None
        },
        text_color: ink,
        border: Border {
            radius: Radii::GRAPHITE.small.into(),
            ..Border::default()
        },
        ..iced::widget::button::Style::default()
    }
}

/// A small outline button with an optional leading icon; `on_press: None`
/// greys it and `hint` explains why.
fn small_button<'a>(
    label: Localized,
    glyph: Option<iced::widget::svg::Handle>,
    on_press: Option<Message>,
    hint: Localized,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let enabled = on_press.is_some();
    let ink = if enabled {
        ti(tokens.text)
    } else {
        tokens::resolve(tokens).ink4
    };
    let mut face = Row::<Element<'a, Message>>::new()
        .spacing(SP.s)
        .align_y(iced::Alignment::Center);
    if let Some(handle) = glyph {
        face = face.push(icon(handle, ink, TS.body));
    }
    face = face.push(text(label).size(TS.caption).color(ink).boxed());
    let pressed = button(face)
        .padding([SP.xs, SP.m])
        .height(Length::Fixed(TS.caption * 2.0))
        .on_press_maybe(on_press)
        .style(crate::styles::outline_button(tokens));
    let r = tokens::resolve(tokens);
    let hint = container(text(hint).size(TS.caption))
        .padding([SP.xs, SP.s])
        .style(signex_widgets::active_bar_dropdown::tooltip_style(
            r.raised,
            r.line_strong,
        ));
    tooltip(pressed, hint, tip::Position::Bottom).boxed()
}

/// A neutral pill: "yours" or "built-in".
fn pill<'a>(label: Localized, tokens: &ThemeTokens) -> Element<'a, Message> {
    container(text(label).size(TS.caption))
        .padding([0.0, SP.s + SP.xs])
        .style(crate::styles::pill(tokens, PillTone::Neutral))
        .boxed()
}

/// A panel chip of the detail's Panels row.
fn panel_chip<'a>(label: Localized, tokens: &ThemeTokens) -> Element<'a, Message> {
    let r = tokens::resolve(tokens);
    let (fill, ink) = (r.sunken, r.ink3);
    container(text(label).size(TS.caption).color(ink))
        .padding([SP.xs, SP.s + SP.xs])
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                radius: Radii::GRAPHITE.pill.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// A group caption of the list: "BUILT-IN", "YOUR LAYOUTS".
fn group_caption<'a>(
    i18n: &I18n,
    id: signex_i18n::MessageId,
    top: f32,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    container(
        text(t!(i18n, id))
            .size(TS.caption)
            .font(iced::Font {
                weight: iced::font::Weight::Medium,
                ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
            })
            .color(tokens::resolve(tokens).ink4),
    )
    .padding(iced::Padding {
        top,
        right: SP.m,
        bottom: SP.s,
        left: SP.m,
    })
    .boxed()
}

/// A layout's name and its caption, stacked.
fn name_block<'a>(name: String, under: Localized, tokens: &ThemeTokens) -> Element<'a, Message> {
    column![
        text(UserText::new(name))
            .size(TS.body)
            .color(ti(tokens.text)),
        text(under)
            .size(TS.caption)
            .color(tokens::resolve(tokens).ink4),
    ]
    .width(Length::Fill)
    .boxed()
}

/// One list row: `content` in a selectable button.
fn list_row<'a>(
    content: Row<Element<'a, Message>>,
    id: LayoutId,
    selected: bool,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    button(
        content
            .spacing(SP.m)
            .align_y(iced::Alignment::Center)
            .height(Length::Shrink),
    )
    .padding([SP.xs, SP.m])
    .width(Length::Fill)
    .height(Length::Shrink)
    .on_press(manage(ManageLayoutsMsg::RowSelected(id)))
    .style(row_style(tokens, selected))
    .boxed()
}

/// The user rows in the order a drag in progress would leave them, with
/// each row's index in the stored list.
fn shown_order(user: &[WorkspaceLayout], dialog: &ManageLayoutsDialog) -> Vec<usize> {
    let mut order: Vec<usize> = (0..user.len()).collect();
    if let Some(drag) = dialog.drag
        && drag.from < order.len()
    {
        let moved = order.remove(drag.from);
        order.insert(drag.over.min(order.len()), moved);
    }
    order
}

/// The "Saved" line of `layout`.
fn saved_line(layout: &WorkspaceLayout, layouts: &Layouts, i18n: &I18n) -> Localized {
    let date = layout.saved_at.clone().unwrap_or_default();
    match layout.builtin() {
        Some(builtin) if layouts.is_edited(builtin) => {
            t!(i18n, ids::WORKSPACE_LAYOUTS_SAVED_BUILTIN_EDITED, "date" => date)
        }
        Some(_) => t!(i18n, ids::WORKSPACE_LAYOUTS_SAVED_BUILTIN),
        None if layout.from_current => {
            t!(i18n, ids::WORKSPACE_LAYOUTS_SAVED_FROM_CURRENT, "date" => date)
        }
        None => t!(i18n, ids::WORKSPACE_LAYOUTS_SAVED_AT, "date" => date),
    }
}

impl Signex {
    pub(super) fn view_manage_layouts_card<'a>(
        &'a self,
        dialog: &'a ManageLayoutsDialog,
    ) -> Element<'a, Message> {
        let tokens = &self.document_view.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let naming = self.layout_naming();
        let layouts = &self.ui_state.workspace_layouts.layouts;
        let close = manage(ManageLayoutsMsg::Closed);
        let header = crate::app::view::dialogs::graphite_header_closing(
            t!(i18n, ids::WORKSPACE_LAYOUTS_MANAGE_TITLE).into_string(),
            Some(t!(i18n, ids::WORKSPACE_LAYOUTS_MANAGE_SUBTITLE).into_string()),
            None,
            Some(close.clone()),
            crate::app::view::dialogs::DIALOG_HEADER_H,
            tokens,
        );
        let line_soft = tokens::resolve(tokens).line_soft;
        let list = container(scrollable(layout_list(
            layouts, dialog, &naming, i18n, tokens,
        )))
        .padding(SP.m)
        .width(Length::Fixed(LIST_WIDTH))
        .height(Length::Fill)
        .style(move |_: &Theme| container::Style {
            border: Border {
                right: iced::border::Side::default().width(1.0),
                color: line_soft,
                ..Border::default()
            },
            ..container::Style::default()
        });
        let detail = container(scrollable(self.layout_detail(dialog, &naming)))
            .padding([SP.l, SP.xl])
            .width(Length::Fill)
            .height(Length::Fill);
        let body = Row::<Element<'a, Message>>::new()
            .push(list.boxed())
            .push(detail.boxed())
            .height(Length::Fill);
        let has_user = !layouts.user().is_empty();
        let is_locked = self.ui_state.workspace_layouts.kept.list_unreadable();
        let banner = if is_locked {
            super::list_locked_banner(i18n, tokens)
        } else {
            Space::new().height(Length::Fixed(0.0)).boxed()
        };
        let footer = dialog_footer_split(
            vec![
                dialog_button(
                    t!(i18n, ids::WORKSPACE_LAYOUTS_IMPORT),
                    DialogButtonKind::Ghost,
                    (!is_locked).then(|| manage(ManageLayoutsMsg::ImportRequested)),
                    tokens,
                ),
                dialog_button(
                    t!(i18n, ids::WORKSPACE_LAYOUTS_EXPORT),
                    DialogButtonKind::Ghost,
                    has_user.then(|| manage(ManageLayoutsMsg::ExportRequested)),
                    tokens,
                ),
            ],
            vec![dialog_button(
                t!(i18n, ids::WORKSPACE_LAYOUTS_CLOSE),
                DialogButtonKind::Secondary,
                Some(close),
                tokens,
            )],
        );
        let width = super::super::widgets::modal_card_width(
            self.ui_state.window_size,
            super::MANAGE_LAYOUTS.0,
        );
        let card = container(column![header, banner, body, footer])
            .width(Length::Fixed(width))
            .height(Length::Fixed(super::MANAGE_LAYOUTS.1))
            .style(crate::styles::modal_card(tokens))
            .clip(true)
            .boxed();
        let Some(toast) = dialog.toast.as_ref() else {
            return card;
        };
        let toast_layer = container(undo_toast(&toast.deleted.layout.name, i18n, tokens))
            .width(Length::Fixed(width))
            .height(Length::Fixed(super::MANAGE_LAYOUTS.1))
            .padding(iced::Padding {
                bottom: TOAST_INSET,
                ..iced::Padding::ZERO
            })
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Bottom)
            .boxed();
        stack(vec![card, toast_layer]).boxed()
    }

    /// The right pane: the selected layout's name, pill and actions, its
    /// thumbnail, its facts, and Update from current / Apply.
    fn layout_detail<'a>(
        &'a self,
        dialog: &'a ManageLayoutsDialog,
        naming: &Naming,
    ) -> Element<'a, Message> {
        let tokens = &self.document_view.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let layouts = &self.ui_state.workspace_layouts.layouts;
        let id = &dialog.selected;
        let Some(layout) = layouts.get(id) else {
            return Space::new().boxed();
        };
        let name = layouts.name_of(id, naming).unwrap_or_default();
        let mine = id.builtin().is_none();

        let mut head = Row::<Element<'a, Message>>::new()
            .spacing(SP.m)
            .align_y(iced::Alignment::Center);
        let mut rename_note = None;
        match (&dialog.rename, mine) {
            (Some(draft), true) => {
                let taken = !draft.trim().is_empty()
                    && layouts.name_clash(draft, naming, Some(id)) != NameClash::Free;
                if taken {
                    rename_note = Some(t!(
                        i18n,
                        ids::WORKSPACE_LAYOUTS_NAME_TAKEN,
                        "name" => draft.trim()
                    ));
                }
                head = head.push(
                    text_input(t!(i18n, ids::WORKSPACE_LAYOUTS_NAME), draft)
                        .id(RENAME_INPUT_ID.clone())
                        .style(crate::panels::form_look::text_input_style)
                        .on_input(|value| manage(ManageLayoutsMsg::RenameEdited(value)))
                        .on_submit(manage(ManageLayoutsMsg::RenameCommitted))
                        .size(TS.heading)
                        .padding([SP.xs, SP.s])
                        .width(Length::Fill)
                        .boxed(),
                );
            }
            _ => {
                head = head
                    .push(
                        text(UserText::new(name.clone()))
                            .size(TS.heading)
                            .font(semibold())
                            .color(ti(tokens.text))
                            .boxed(),
                    )
                    .push(pill(
                        t!(
                            i18n,
                            if mine {
                                ids::WORKSPACE_LAYOUTS_PILL_YOURS
                            } else {
                                ids::WORKSPACE_LAYOUTS_PILL_BUILTIN
                            }
                        ),
                        tokens,
                    ))
                    .push(Space::new().width(Length::Fill).boxed());
            }
        }
        let (rename_hint, delete_hint) = if mine {
            (
                t!(i18n, ids::WORKSPACE_LAYOUTS_RENAME_TOOLTIP, "name" => name.as_str()),
                t!(i18n, ids::WORKSPACE_LAYOUTS_DELETE_TOOLTIP, "name" => name.as_str()),
            )
        } else {
            (
                t!(i18n, ids::WORKSPACE_LAYOUTS_RENAME_BUILTIN),
                t!(i18n, ids::WORKSPACE_LAYOUTS_DELETE_BUILTIN),
            )
        };
        let renaming = dialog.rename.is_some();
        head = head
            .push(small_button(
                t!(i18n, ids::WORKSPACE_LAYOUTS_RENAME),
                Some(crate::icons::icon_layouts_edit(GLYPH_THEME)),
                (mine && !renaming).then(|| manage(ManageLayoutsMsg::RenameStarted)),
                rename_hint,
                tokens,
            ))
            .push(small_button(
                t!(i18n, ids::WORKSPACE_LAYOUTS_DUPLICATE),
                None,
                Some(manage(ManageLayoutsMsg::DuplicateRequested)),
                t!(i18n, ids::WORKSPACE_LAYOUTS_DUPLICATE),
                tokens,
            ))
            .push(small_button(
                t!(i18n, ids::WORKSPACE_LAYOUTS_DELETE),
                Some(crate::icons::icon_layouts_trash(GLYPH_THEME)),
                mine.then(|| manage(ManageLayoutsMsg::DeleteRequested)),
                delete_hint,
                tokens,
            ));

        let mut pane = Column::<Element<'a, Message>>::new()
            .spacing(SP.m + SP.xs)
            .push(head.boxed());
        if let Some(note) = rename_note {
            pane = pane.push(
                text(note)
                    .size(TS.caption)
                    .color(ti(tokens.warning))
                    .boxed(),
            );
        }
        pane = pane
            .push(super::thumb::thumbnail(
                layout.dock.as_ref(),
                Length::Fill,
                BIG_THUMB_H,
                tokens,
            ))
            .push(layout_facts(&layout, layouts, i18n, tokens))
            .push(
                Row::<Element<'a, Message>>::new()
                    .spacing(SP.m)
                    .push(dialog_button(
                        t!(i18n, ids::WORKSPACE_LAYOUTS_UPDATE_FROM_CURRENT),
                        DialogButtonKind::Secondary,
                        Some(manage(ManageLayoutsMsg::UpdateFromCurrentRequested)),
                        tokens,
                    ))
                    .push(dialog_button(
                        t!(i18n, ids::WORKSPACE_LAYOUTS_APPLY),
                        DialogButtonKind::Primary,
                        Some(manage(ManageLayoutsMsg::ApplyRequested)),
                        tokens,
                    ))
                    .boxed(),
            );
        pane.boxed()
    }
}

/// The detail's form: Panels (chips), Windows, Shortcut (greyed) and Saved.
fn layout_facts<'a>(
    layout: &WorkspaceLayout,
    layouts: &Layouts,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let panels: Element<'a, Message> = match &layout.dock {
        Some(dock) => {
            let chips = dock
                .left
                .panels
                .iter()
                .chain(&dock.right.panels)
                .chain(&dock.bottom.panels)
                .map(|kind| panel_chip(t!(i18n, kind.message()), tokens));
            Row::<Element<'a, Message>>::new()
                .spacing(SP.s)
                .extend(chips)
                .wrap()
                .vertical_spacing(SP.s)
                .boxed()
        }
        None => caption(i18n, ids::WORKSPACE_LAYOUTS_NO_PANELS, tokens),
    };
    Column::<Element<'a, Message>>::new()
        .spacing(SP.s + SP.xs)
        .push(form_row(
            Some(form_key(i18n, ids::WORKSPACE_LAYOUTS_PANELS, tokens)),
            panels,
            KEY_WIDTH,
        ))
        .push(form_row(
            Some(form_key(i18n, ids::WORKSPACE_LAYOUTS_WINDOWS, tokens)),
            text(t!(i18n, ids::WORKSPACE_LAYOUTS_WINDOWS_MAIN))
                .size(TS.body)
                .color(ti(tokens.text))
                .boxed(),
            KEY_WIDTH,
        ))
        .push(form_row(
            Some(form_key(i18n, ids::WORKSPACE_LAYOUTS_SHORTCUT, tokens)),
            shortcut_field(i18n, tokens, Length::Fixed(SHORTCUT_WIDTH)),
            KEY_WIDTH,
        ))
        .push(form_row(
            Some(form_key(i18n, ids::WORKSPACE_LAYOUTS_SAVED, tokens)),
            text(saved_line(layout, layouts, i18n))
                .size(TS.caption)
                .color(tokens::resolve(tokens).ink4)
                .boxed(),
            KEY_WIDTH,
        ))
        .boxed()
}

/// The left list: the built-ins with their lock, then your layouts with
/// their grips. A release anywhere over the list drops a dragged row.
fn layout_list<'a>(
    layouts: &Layouts,
    dialog: &ManageLayoutsDialog,
    naming: &Naming,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let ink4 = tokens::resolve(tokens).ink4;
    let thumb = |layout: &WorkspaceLayout| {
        super::thumb::thumbnail(
            layout.dock.as_ref(),
            Length::Fixed(SMALL_THUMB.0),
            SMALL_THUMB.1,
            tokens,
        )
    };
    let mut list = Column::<Element<'a, Message>>::new()
        .spacing(SP.xs)
        .width(Length::Fill)
        .push(group_caption(
            i18n,
            ids::WORKSPACE_LAYOUTS_GROUP_BUILTIN,
            SP.xs,
            tokens,
        ));
    for builtin in BuiltinLayout::ALL {
        let id = LayoutId::Builtin(builtin);
        let layout = layouts.builtin_layout(builtin);
        let under = if layouts.is_edited(builtin) {
            t!(i18n, ids::WORKSPACE_LAYOUTS_CAPTION_EDITED)
        } else {
            t!(i18n, ids::WORKSPACE_LAYOUTS_CAPTION_BUILTIN)
        };
        let content = Row::<Element<'a, Message>>::new()
            .push(thumb(&layout))
            .push(name_block(
                naming.builtin(builtin).to_string(),
                under,
                tokens,
            ))
            .push(icon(
                crate::icons::icon_layouts_lock(GLYPH_THEME),
                ink4,
                ICON,
            ));
        list = list.push(list_row(content, id.clone(), dialog.selected == id, tokens));
    }
    list = list.push(group_caption(
        i18n,
        ids::WORKSPACE_LAYOUTS_GROUP_YOURS,
        SP.m + SP.xs,
        tokens,
    ));
    let user = layouts.user();
    if user.is_empty() {
        list = list.push(
            container(caption(i18n, ids::WORKSPACE_LAYOUTS_NONE_YET, tokens))
                .padding([SP.xs, SP.m])
                .boxed(),
        );
    }
    for (shown_at, stored_at) in shown_order(user, dialog).into_iter().enumerate() {
        let layout = &user[stored_at];
        let grip = mouse_area(icon(
            crate::icons::icon_layouts_grip(GLYPH_THEME),
            ink4,
            ICON,
        ))
        .on_press(manage(ManageLayoutsMsg::GripPressed(stored_at)))
        .interaction(iced::mouse::Interaction::Grab)
        .boxed();
        let content = Row::<Element<'a, Message>>::new()
            .push(grip)
            .push(thumb(layout))
            .push(name_block(
                layout.name.clone(),
                t!(i18n, ids::WORKSPACE_LAYOUTS_CAPTION_NO_SHORTCUT),
                tokens,
            ));
        let row = list_row(
            content,
            layout.id.clone(),
            dialog.selected == layout.id,
            tokens,
        );
        list = list.push(
            mouse_area(row)
                .on_enter(manage(ManageLayoutsMsg::RowEntered(shown_at)))
                .boxed(),
        );
    }
    mouse_area(list)
        .on_release(manage(ManageLayoutsMsg::GripReleased))
        .boxed()
}

/// The dark toast: "Deleted layout “X”" and Undo.
fn undo_toast<'a>(name: &str, i18n: &I18n, tokens: &ThemeTokens) -> Element<'a, Message> {
    let r = tokens::resolve(tokens);
    let (fill, ink) = (r.primary, r.primary_ink);
    let shadow = crate::styles::floating_shadow(tokens);
    let undo = button(
        text(t!(i18n, ids::WORKSPACE_LAYOUTS_UNDO))
            .size(TS.body)
            .font(semibold())
            .color(ink),
    )
    .padding(0)
    .on_press(manage(ManageLayoutsMsg::UndoDeleteRequested))
    .style(move |_: &Theme, _| iced::widget::button::Style {
        background: None,
        text_color: ink,
        ..iced::widget::button::Style::default()
    });
    let line = Row::<Element<'a, Message>>::new()
        .spacing(SP.m + SP.xs)
        .align_y(iced::Alignment::Center)
        .push(
            text(t!(i18n, ids::WORKSPACE_LAYOUTS_DELETED, "name" => name))
                .size(TS.body)
                .color(ink)
                .boxed(),
        )
        .push(undo.boxed());
    container(line)
        .padding([SP.s + SP.xs, SP.l])
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            text_color: Some(ink),
            border: Border {
                radius: Radii::GRAPHITE.card.into(),
                ..Border::default()
            },
            shadow,
            ..container::Style::default()
        })
        .boxed()
}
