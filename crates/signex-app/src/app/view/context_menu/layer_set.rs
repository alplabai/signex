//! Menu of the layer set button at the left end of the layer strip (#2394):
//! the built-in sets, the user's saved sets, Save current as set... (a name
//! field in the same card) and Manage sets... (the saved sets with Delete).

use super::*;

use super::items::{dd_disabled, text};
use crate::app::contracts::LAYER_SET_NAME_INPUT_ID;
use crate::app::{LayerSetCard, LayerSetMenuMsg, PcbMsg};
use crate::layer_strip_prefs::LayerStripPrefs;
use crate::styles::tokens::resolve;
use iced::widget::button::{Status, Style as ButtonStyle};
use iced::widget::{Space, row};
use iced::{Background, Border, Length, Theme, Widget as _};
use signex_i18n::{Arg, I18n, Localized, ids, t};
use signex_renderer::pcb::{BuiltinLayerSet, LayerSetId};
use signex_types::theme::ThemeTokens;
use signex_widgets::active_bar_dropdown::{DropdownEntry, DropdownItem, RowPaint};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

fn set_msg(event: LayerSetMenuMsg) -> Message {
    Message::Pcb(PcbMsg::LayerSetMenu(event))
}

/// The rows to pick a set from: the built-in sets, the saved ones, then
/// Save current as set... and Manage sets.... `shown` is the set the board
/// view shows (`None`: every layer, the All set).
pub(super) fn layer_set_entries(
    i18n: &I18n,
    prefs: &LayerStripPrefs,
    shown: Option<&LayerSetId>,
) -> Vec<DropdownEntry<Message>> {
    let shown = shown
        .cloned()
        .unwrap_or(LayerSetId::Builtin(BuiltinLayerSet::All));
    let mut entries: Vec<DropdownEntry<Message>> = BuiltinLayerSet::ALL
        .into_iter()
        .map(|set| {
            let label = i18n.message_plain(ids::PCB_LAYER_SET, &[("set", Arg::from(set.token()))]);
            let on = shown == LayerSetId::Builtin(set);
            let item = DropdownItem::new(label, set_msg(LayerSetMenuMsg::BuiltinPicked(set)));
            DropdownEntry::Item(item.checked(on))
        })
        .collect();
    if !prefs.saved_sets.is_empty() {
        entries.push(DropdownEntry::Separator);
    }
    for saved in &prefs.saved_sets {
        let on = shown == LayerSetId::Saved(saved.name.clone());
        let picked = set_msg(LayerSetMenuMsg::SavedPicked(saved.name.clone()));
        entries.push(DropdownEntry::Item(
            DropdownItem::new(saved.name.clone(), picked).checked(on),
        ));
    }
    entries.push(DropdownEntry::Separator);
    entries.push(DropdownEntry::Item(DropdownItem::new(
        text(i18n, ids::PCB_LAYER_SET_SAVE),
        set_msg(LayerSetMenuMsg::SaveStarted),
    )));
    let manage = text(i18n, ids::PCB_LAYER_SET_MANAGE);
    entries.push(if prefs.saved_sets.is_empty() {
        match dd_disabled(None, &manage, None) {
            DropdownEntry::Item(item) => {
                DropdownEntry::Item(item.tooltip(text(i18n, ids::PCB_LAYER_SET_NONE_SAVED)))
            }
            other => other,
        }
    } else {
        DropdownEntry::Item(DropdownItem::new(
            manage,
            set_msg(LayerSetMenuMsg::ManageStarted),
        ))
    });
    entries
}

/// A small button of the card: `primary` fills it with the primary colour.
fn card_button(
    label: Localized,
    message: Message,
    primary: bool,
    tokens: &ThemeTokens,
) -> Element<'static, Message> {
    let r = resolve(tokens);
    let hover = RowPaint::of(tokens).hover;
    let ink = if primary {
        r.primary_ink
    } else {
        crate::styles::ti(tokens.text)
    };
    signex_widgets::button(signex_widgets::text(label).size(TS.body).color(ink))
        .padding([SP.xs, SP.s])
        .style(move |_: &Theme, status| ButtonStyle {
            background: if primary {
                Some(Background::Color(r.primary))
            } else {
                matches!(status, Status::Hovered | Status::Pressed)
                    .then_some(Background::Color(hover))
            },
            border: Border {
                width: if primary { 0.0 } else { 1.0 },
                color: r.line_soft,
                radius: crate::styles::Radii::GRAPHITE.small.into(),
            },
            ..ButtonStyle::default()
        })
        .on_press(message)
        .boxed()
}

/// Save current as set...: the name field and Save.
pub(super) fn naming_entries(
    i18n: &I18n,
    name: &str,
    tokens: &ThemeTokens,
) -> Vec<DropdownEntry<Message>> {
    let field = signex_widgets::text_input(
        t!(i18n, ids::PCB_LAYER_SET_NAME_PLACEHOLDER),
        name.to_owned(),
    )
    .id(LAYER_SET_NAME_INPUT_ID.clone())
    .style(crate::panels::form_look::text_input_style)
    .on_input(|text| set_msg(LayerSetMenuMsg::NameEdited(text)))
    .on_submit(set_msg(LayerSetMenuMsg::NameSubmitted))
    .size(TS.body)
    .padding(SP.xs)
    .width(Length::Fill);
    let save = card_button(
        t!(i18n, ids::PCB_LAYER_SET_SAVE_BUTTON),
        set_msg(LayerSetMenuMsg::NameSubmitted),
        true,
        tokens,
    );
    let line = row![field.boxed(), save]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center)
        .padding([SP.xs, SP.s]);
    vec![
        DropdownEntry::Header(text(i18n, ids::PCB_LAYER_SET_SAVE)),
        DropdownEntry::Custom(line.boxed()),
    ]
}

/// Manage sets...: each saved set with Delete.
pub(super) fn managing_entries(
    i18n: &I18n,
    prefs: &LayerStripPrefs,
    tokens: &ThemeTokens,
) -> Vec<DropdownEntry<Message>> {
    let mut entries = vec![DropdownEntry::Header(text(i18n, ids::PCB_LAYER_SET_MANAGE))];
    if prefs.saved_sets.is_empty() {
        entries.push(dd_disabled(
            None,
            &text(i18n, ids::PCB_LAYER_SET_NONE_SAVED),
            None,
        ));
    }
    for saved in &prefs.saved_sets {
        let delete = card_button(
            t!(i18n, ids::PCB_LAYER_SET_DELETE),
            set_msg(LayerSetMenuMsg::DeletePicked(saved.name.clone())),
            false,
            tokens,
        );
        let name = signex_widgets::text(signex_i18n::UserText::new(saved.name.clone()))
            .size(TS.body)
            .color(crate::styles::ti(tokens.text));
        let line = row![
            name.boxed(),
            Space::new().width(Length::Fill).boxed(),
            delete
        ]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center)
        .padding([SP.xs, SP.s]);
        entries.push(DropdownEntry::Custom(line.boxed()));
    }
    entries
}

impl Signex {
    /// The open layer set menu with its click-outside backdrop, in the
    /// window it was opened in; empty otherwise.
    pub(super) fn layer_set_menu_overlay(
        &self,
        window_id: iced::window::Id,
    ) -> Vec<Element<'_, Message>> {
        let Some(menu) = self.interaction_state.pcb_layer_set_menu.as_ref() else {
            return Vec::new();
        };
        if !self.overlay_belongs_to(menu.window, window_id) {
            return Vec::new();
        }
        let i18n = &self.ui_state.i18n;
        let prefs = &self.ui_state.layer_strip;
        let panel_ctx = &self.document_state.panel_ctx;
        let tokens = &panel_ctx.tokens;
        let entries = match &menu.card {
            LayerSetCard::Sets => {
                let view = self.pcb_layer_view_of_window(menu.window);
                layer_set_entries(i18n, prefs, view.layer_set().map(|set| &set.id))
            }
            LayerSetCard::Naming(name) => naming_entries(i18n, name, tokens),
            LayerSetCard::Managing => managing_entries(i18n, prefs, tokens),
        };
        let card = super::placement::menu_card(entries, tokens, &panel_ctx.ui_font_name);
        Self::menu_overlay(
            set_msg(LayerSetMenuMsg::Dismissed),
            card.element,
            (menu.x, menu.y),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layer_strip_prefs::LayerStripPrefs;

    fn labels(entries: &[DropdownEntry<Message>]) -> Vec<(String, bool, bool)> {
        entries
            .iter()
            .filter_map(|e| match e {
                DropdownEntry::Item(item) => {
                    Some((item.label.clone(), item.checked, item.on_press.is_some()))
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn the_menu_lists_the_built_in_sets_then_the_saved_ones() {
        let i18n = I18n::en_us();
        let mut prefs = LayerStripPrefs::default();
        let none = labels(&layer_set_entries(&i18n, &prefs, None));
        let names: Vec<&str> = none.iter().map(|(l, _, _)| l.as_str()).collect();
        assert_eq!(
            names,
            [
                "All Layers",
                "Signal Layers",
                "Plane Layers",
                "Non-signal Layers",
                "Mechanical Layers",
                "Save current as set...",
                "Manage sets...",
            ]
        );
        assert!(none[0].1, "All is checked without a set");
        assert!(!none[6].2, "nothing to manage without a saved set");

        prefs.save_set("Assembly", Vec::new());
        let shown = LayerSetId::Saved("Assembly".into());
        let with = labels(&layer_set_entries(&i18n, &prefs, Some(&shown)));
        let assembly = with.iter().find(|(l, _, _)| l == "Assembly");
        assert!(assembly.is_some_and(|(_, on, _)| *on));
        assert!(with.last().is_some_and(|(_, _, live)| *live));
    }
}
