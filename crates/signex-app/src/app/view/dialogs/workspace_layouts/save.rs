//! Save Layout (#166): the name (a free "Layout N" offered), what the layout
//! stores, and Save, which reads Replace when the name is one of your
//! layouts. A built-in's name is refused.

use iced::widget::{Column, Row, Space, column, container};
use iced::{Background, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{CheckboxLabel as _, checkbox, text, text_input};

use super::{GLYPH_THEME, ICON, SP, TS, form_key, form_row, icon, shortcut_field};
use crate::app::handlers::workspace_layouts::SAVE_NAME_INPUT_ID;
use crate::app::{Message, Signex};
use crate::styles::{DialogButtonKind, Radii, dialog_button, dialog_footer, tokens};
use crate::workspace_layouts::{NameClash, SaveLayoutDialog, SaveLayoutMsg};

/// The mock's 90 px key column.
const KEY_WIDTH: f32 = 90.0;

/// One "Store" tick box; `on` toggles it, `None` greys it.
fn store_tick<'a>(
    checked: bool,
    label: signex_i18n::Localized,
    on: Option<SaveLayoutMsg>,
) -> Element<'a, Message> {
    let tick = checkbox(checked)
        .style(crate::panels::form_look::checkbox_style)
        .size(TS.body + SP.xs)
        .labelled(label)
        .text_size(TS.body)
        .spacing(SP.m);
    match on {
        Some(msg) => tick.on_toggle(move |_| msg.clone().into()).boxed(),
        None => tick.boxed(),
    }
}

/// The info banner: soft info fill, the info icon, primary ink.
fn documents_note<'a>(i18n: &I18n, tokens: &ThemeTokens) -> Element<'a, Message> {
    let r = tokens::resolve(tokens);
    let fill = r.info_soft;
    let line = Row::<Element<'a, Message>>::new()
        .push(icon(
            crate::icons::icon_layouts_info(GLYPH_THEME),
            r.info,
            ICON,
        ))
        .push(
            text(t!(i18n, ids::WORKSPACE_LAYOUTS_DOCUMENTS_NOTE))
                .size(TS.body)
                .color(crate::styles::ti(tokens.text))
                .width(Length::Fill)
                .boxed(),
        )
        .spacing(SP.m);
    container(line)
        .padding([SP.m, SP.m + SP.xs])
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

impl Signex {
    pub(super) fn view_save_layout_card<'a>(
        &'a self,
        dialog: &'a SaveLayoutDialog,
    ) -> Element<'a, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let naming = self.layout_naming();
        let layouts = &self.ui_state.workspace_layouts.layouts;
        let clash = layouts.name_clash(&dialog.name, &naming, None);
        let has_split = self.current_document_split().is_some();
        let can_save = !dialog.name.trim().is_empty()
            && !matches!(clash, NameClash::Builtin(_))
            && self.save_layout_stores_something(dialog);
        let confirm = can_save.then(|| Message::from(SaveLayoutMsg::Confirmed));
        let cancel = Message::from(SaveLayoutMsg::Cancelled);
        let header = crate::app::view::dialogs::graphite_header_closing(
            t!(i18n, ids::WORKSPACE_LAYOUTS_SAVE_TITLE).into_string(),
            None,
            None,
            Some(cancel.clone()),
            crate::app::view::dialogs::DIALOG_HEADER_H,
            tokens,
        );

        let name = text_input(t!(i18n, ids::WORKSPACE_LAYOUTS_NAME), &dialog.name)
            .id(SAVE_NAME_INPUT_ID.clone())
            .style(crate::panels::form_look::text_input_style)
            .on_input(|value| SaveLayoutMsg::NameEdited(value).into())
            .on_submit_maybe(confirm.clone())
            .size(TS.body)
            .padding([SP.s, SP.m])
            .width(Length::Fill)
            .boxed();
        let mut grid: Column<Element<'a, Message>> =
            Column::new().spacing(SP.s + SP.xs).push(form_row(
                Some(form_key(i18n, ids::WORKSPACE_LAYOUTS_NAME, tokens)),
                name,
                KEY_WIDTH,
            ));
        let warn = crate::styles::ti(tokens.warning);
        let note = match &clash {
            NameClash::User(id) => Some(t!(
                i18n,
                ids::WORKSPACE_LAYOUTS_REPLACES,
                "name" => layouts.name_of(id, &naming).unwrap_or_default()
            )),
            NameClash::Builtin(builtin) => Some(t!(
                i18n,
                ids::WORKSPACE_LAYOUTS_BUILTIN_NAME,
                "name" => naming.builtin(*builtin)
            )),
            NameClash::Free => None,
        };
        if let Some(note) = note {
            grid = grid.push(form_row(
                None,
                text(note).size(TS.caption).color(warn).boxed(),
                KEY_WIDTH,
            ));
        }
        grid = grid.push(form_row(
            Some(form_key(i18n, ids::WORKSPACE_LAYOUTS_SHORTCUT, tokens)),
            shortcut_field(i18n, tokens, Length::Fill),
            KEY_WIDTH,
        ));

        let r = tokens::resolve(tokens);
        let windows = self.arrangement_window_count();
        let extra_windows = column![
            store_tick(false, t!(i18n, ids::WORKSPACE_LAYOUTS_STORE_WINDOWS), None),
            container(
                text(t!(i18n, ids::WORKSPACE_LAYOUTS_WINDOWS_COUNT, "count" => windows))
                    .size(TS.caption)
                    .color(r.ink4)
            )
            .padding(iced::Padding {
                left: TS.body + SP.xs + SP.m,
                ..iced::Padding::ZERO
            }),
        ]
        .spacing(SP.xs);
        let body = Column::<Element<'a, Message>>::new()
            .spacing(SP.m + SP.xs)
            .push(grid.boxed())
            .push(
                text(t!(i18n, ids::WORKSPACE_LAYOUTS_STORE))
                    .size(TS.section)
                    .color(r.ink3)
                    .boxed(),
            )
            .push(store_tick(
                dialog.store_panels,
                t!(i18n, ids::WORKSPACE_LAYOUTS_STORE_PANELS),
                Some(SaveLayoutMsg::PanelsToggled),
            ))
            .push(store_tick(
                dialog.store_split && has_split,
                t!(i18n, ids::WORKSPACE_LAYOUTS_STORE_SPLIT),
                has_split.then_some(SaveLayoutMsg::SplitToggled),
            ))
            .push(extra_windows.boxed())
            .push(documents_note(i18n, tokens))
            .push(Space::new().height(Length::Fixed(SP.xs)).boxed());

        let save_label = if matches!(clash, NameClash::User(_)) {
            ids::WORKSPACE_LAYOUTS_REPLACE
        } else {
            ids::WORKSPACE_LAYOUTS_SAVE
        };
        let footer = dialog_footer(vec![
            dialog_button(
                t!(i18n, ids::WORKSPACE_LAYOUTS_CANCEL),
                DialogButtonKind::Cancel,
                Some(cancel),
                tokens,
            ),
            dialog_button(
                t!(i18n, save_label),
                DialogButtonKind::Primary,
                confirm,
                tokens,
            ),
        ]);
        container(
            column![header, container(body).padding([SP.l, SP.xl]), footer].width(Length::Fixed(
                super::super::widgets::modal_card_width(
                    self.ui_state.window_size,
                    super::SAVE_LAYOUT.0,
                ),
            )),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true)
        .boxed()
    }
}
