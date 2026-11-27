//! The placed part's own panel state (mockup rows 42, 50): the open ⋯ menu and the
//! hover link of its illustrations, with the messages that change them.
//! The menu holds the three part actions plus "Copy Library ID".

use super::super::*;
use crate::library::PlacedPartAction;
use crate::styles::{Spacing, TypeScale};
use iced::Widget;
use signex_i18n::{MessageId, ids, t};
use signex_widgets::property_illustration::{IllustrationHover, IllustrationMsg};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// What the part's Properties remember between frames.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PartPanelState {
    /// The part whose ⋯ menu is open.
    pub menu_for: Option<uuid::Uuid>,
    /// Where the ⋯ press was, window coordinates: the menu opens level
    /// with it.
    pub menu_at: (f32, f32),
    /// The field or callout under the pointer.
    pub hover: IllustrationHover,
    /// Which of [`PART_SECTION_KEYS`] the user had collapsed before a pin
    /// was focused; `None` while no pin is focused.
    pub pin_focus_saved: Option<Vec<&'static str>>,
}

/// The part sections that fold away while a pin is focused, so the pin
/// facts and the Pins table fit without scrolling (mockup row 50).
pub(crate) const PART_SECTION_KEYS: [&str; 5] = [
    "sel_general",
    "sel_location",
    "sel_graphical",
    "sel_parameters",
    super::footprint_section::FOOTPRINT_SECTION_KEY,
];

/// What happened in a placed part's Properties.
#[derive(Debug, Clone)]
pub enum PartPanelMsg {
    /// The ⋯ button of the part was pressed.
    MenuToggled(uuid::Uuid),
    /// A press outside the open ⋯ menu closed it.
    MenuDismissed,
    /// A part action was picked from the ⋯ menu.
    MenuActionPicked(PlacedPartAction),
    /// "Copy Library ID" was picked; carries the id.
    LibraryIdCopyPicked(String),
    /// "Show net" or the Net link of the focused pin was pressed.
    PinNetShowRequested(String),
    /// The pointer moved over a field or a drawing part.
    IllustrationHovered(IllustrationMsg),
}

/// What the app does after the state took a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PartPanelEffect {
    None,
    /// Run a part action on the selected part.
    Action(PlacedPartAction),
    /// Put the text on the clipboard.
    CopyToClipboard(String),
    /// Open the Net Inspector on the net.
    ShowNet(String),
}

impl PartPanelState {
    /// Take `msg`; the effect is left for the caller to run as a `Task`.
    pub fn update(&mut self, msg: PartPanelMsg) -> PartPanelEffect {
        match msg {
            PartPanelMsg::MenuToggled(part) => {
                self.menu_for = (self.menu_for != Some(part)).then_some(part);
                PartPanelEffect::None
            }
            PartPanelMsg::MenuDismissed => {
                self.menu_for = None;
                PartPanelEffect::None
            }
            PartPanelMsg::MenuActionPicked(action) => {
                self.menu_for = None;
                PartPanelEffect::Action(action)
            }
            PartPanelMsg::LibraryIdCopyPicked(id) => {
                self.menu_for = None;
                PartPanelEffect::CopyToClipboard(id)
            }
            PartPanelMsg::PinNetShowRequested(net) => PartPanelEffect::ShowNet(net),
            PartPanelMsg::IllustrationHovered(hover) => {
                self.hover.update(hover);
                PartPanelEffect::None
            }
        }
    }

    /// Fold the part sections when a pin focus starts and give the user's
    /// own collapse state back when it ends. Sections outside
    /// [`PART_SECTION_KEYS`] (the Pins table) keep whatever the user did.
    pub fn sync_pin_focus(&mut self, collapsed: &mut CollapsedSections, is_pin_focused: bool) {
        if is_pin_focused {
            if self.pin_focus_saved.is_none() {
                self.pin_focus_saved = Some(
                    PART_SECTION_KEYS
                        .into_iter()
                        .filter(|key| collapsed.contains(*key))
                        .collect(),
                );
                collapsed.extend(PART_SECTION_KEYS.map(str::to_string));
            }
        } else if let Some(saved) = self.pin_focus_saved.take() {
            for key in PART_SECTION_KEYS {
                if saved.contains(&key) {
                    collapsed.insert(key.to_string());
                } else {
                    collapsed.remove(key);
                }
            }
        }
    }
}

/// The entries of the ⋯ menu, label first.
pub(super) fn menu_entries(lib_id: &str) -> Vec<(MessageId, PartPanelMsg)> {
    use PlacedPartAction as A;
    let mut entries: Vec<(MessageId, PartPanelMsg)> = [
        (ids::PROPERTIES_ACTION_OPEN_DATASHEET, A::OpenDatasheet),
        (ids::PROPERTIES_ACTION_EDIT_SYMBOL, A::EditSymbol),
        (ids::PROPERTIES_ACTION_CHANGE_COMPONENT, A::ChangeComponent),
    ]
    .into_iter()
    .map(|(label, action)| (label, PartPanelMsg::MenuActionPicked(action)))
    .collect();
    if !lib_id.is_empty() {
        entries.push((
            ids::PROPERTIES_ACTION_COPY_LIBRARY_ID,
            PartPanelMsg::LibraryIdCopyPicked(lib_id.to_string()),
        ));
    }
    entries
}

pub(super) fn part_msg(msg: PartPanelMsg) -> PanelMsg {
    PanelMsg::Part(msg)
}

/// A read-only row whose value is a link: ink, underlined, pressing it
/// sends `on_press` (Datasheet, a pin's Net).
pub(super) fn link_row<'a>(
    label: impl Into<signex_widgets::Label<'a>>,
    value: &str,
    on_press: PanelMsg,
    label_c: Color,
    tokens: &ThemeTokens,
) -> Element<'a, PanelMsg> {
    let ink = crate::styles::ti(tokens.text);
    let link: iced::widget::text::Rich<'a, (), PanelMsg> =
        signex_widgets::rich_text([signex_widgets::span(signex_i18n::UserText::new(value))
            .color(ink)
            .underline(true)
            .link(())])
        .size(TS.body)
        .on_link_click(move |()| on_press.clone());
    container(
        iced::widget::row![
            property_label(label, label_c),
            container(link)
                .padding([3, 6])
                .clip(true)
                .width(Length::FillPortion(PROPERTY_CONTROL_PORTION)),
        ]
        .spacing(SP.m)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .boxed()
}

/// A field row linked to an illustration: hovering it lights the drawing
/// parts of `field`, and a hovered callout of `field` lights the row.
pub(super) fn linked_row<'a>(
    row: Element<'a, PanelMsg>,
    field: signex_widgets::property_illustration::FieldId,
    ctx: &PanelContext,
) -> Element<'a, PanelMsg> {
    let lit = ctx.part_panel.hover.is_field_highlighted(&field);
    let glow = crate::styles::tokens::resolve(&ctx.tokens).accent_soft;
    let row = container(row)
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: lit.then_some(Background::Color(glow)),
            ..container::Style::default()
        });
    iced::widget::mouse_area(row)
        .on_enter(part_msg(PartPanelMsg::IllustrationHovered(
            IllustrationMsg::FieldHovered(field.clone()),
        )))
        .on_exit(part_msg(PartPanelMsg::IllustrationHovered(
            IllustrationMsg::FieldLeft(field),
        )))
        .boxed()
}

/// An illustration under a section header, mapped into the panel's
/// messages; the hover lives in [`PartPanelState`].
pub(super) fn illustration_box<'a>(
    drawing: signex_widgets::property_illustration::Illustration,
    ctx: &PanelContext,
) -> Element<'a, PanelMsg> {
    let drawing = signex_widgets::property_illustration::property_illustration(
        drawing,
        &ctx.part_panel.hover,
        &ctx.tokens,
        TS.caption,
    )
    .map(|m| part_msg(PartPanelMsg::IllustrationHovered(m)));
    container(drawing)
        .padding(iced::Padding {
            top: SP.xs,
            right: SP.m,
            bottom: SP.s,
            left: SP.m,
        })
        .width(Length::Fill)
        .boxed()
}

/// The open ⋯ menu of the selected part: where the press was and its rows.
/// The app draws it over the menu scrim like every other menu (#3063).
pub(crate) struct OpenPartMenu {
    pub at: (f32, f32),
    pub entries: Vec<(MessageId, PartPanelMsg)>,
}

/// The ⋯ menu `ctx` has open, while its part is still the selected one.
pub(crate) fn open_part_menu(ctx: &PanelContext) -> Option<OpenPartMenu> {
    let part = ctx.part_panel.menu_for?;
    if ctx.selected_uuid != Some(part) {
        return None;
    }
    let lib_id = super::rows::info_row(&ctx.selection_info, "Library ID");
    Some(OpenPartMenu {
        at: ctx.part_panel.menu_at,
        entries: menu_entries(&lib_id),
    })
}

/// The ⋯ button, right-aligned; its menu is an app overlay
/// ([`open_part_menu`]).
pub(super) fn push_part_menu<'a>(
    col: Column<Element<'a, PanelMsg>>,
    ctx: &'a PanelContext,
    part: uuid::Uuid,
) -> Column<Element<'a, PanelMsg>> {
    let i18n = &ctx.i18n;
    let more = signex_widgets::button(
        signex_widgets::text(t!(i18n, ids::PROPERTIES_PART_MORE)).size(TS.body),
    )
    .padding([0.0, SP.s])
    .on_press(part_msg(PartPanelMsg::MenuToggled(part)))
    .style(super::super::form_look::ghost_button);
    let more = signex_widgets::tooltip(
        more,
        container(signex_widgets::text(t!(i18n, ids::PROPERTIES_PART_MORE_TIP)).size(TS.body))
            .style(crate::styles::context_menu(&ctx.tokens)),
        iced::widget::tooltip::Position::Left,
    );
    col.push(
        container(more)
            .padding([0.0, SP.m])
            .width(Length::Fill)
            .align_x(iced::alignment::Horizontal::Right)
            .boxed(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_menu_toggles_for_one_part_and_closes_on_a_pick() {
        let part = uuid::Uuid::now_v7();
        let mut state = PartPanelState::default();
        assert_eq!(
            state.update(PartPanelMsg::MenuToggled(part)),
            PartPanelEffect::None
        );
        assert_eq!(state.menu_for, Some(part));
        let picked = state.update(PartPanelMsg::MenuActionPicked(PlacedPartAction::EditSymbol));
        assert_eq!(
            picked,
            PartPanelEffect::Action(PlacedPartAction::EditSymbol)
        );
        assert_eq!(state.menu_for, None);
        state.update(PartPanelMsg::MenuToggled(part));
        state.update(PartPanelMsg::MenuToggled(part));
        assert_eq!(state.menu_for, None, "a second press closes it");
    }

    #[test]
    fn copy_library_id_puts_the_id_on_the_clipboard() {
        let mut state = PartPanelState {
            menu_for: Some(uuid::Uuid::now_v7()),
            ..PartPanelState::default()
        };
        let effect = state.update(PartPanelMsg::LibraryIdCopyPicked("lib:sym".into()));
        assert_eq!(effect, PartPanelEffect::CopyToClipboard("lib:sym".into()));
        assert_eq!(state.menu_for, None);
    }

    #[test]
    fn the_menu_holds_the_three_actions_and_copy_library_id() {
        let en = signex_i18n::I18n::en_us();
        let labels: Vec<String> = menu_entries("lib:sym")
            .iter()
            .map(|(label, _)| en.message_plain(*label, &[]))
            .collect();
        assert_eq!(
            labels,
            [
                "Open Datasheet",
                "Edit Symbol",
                "Change Component\u{2026}",
                "Copy Library ID"
            ]
        );
        assert_eq!(menu_entries("").len(), 3, "no id, nothing to copy");
    }

    #[test]
    fn a_pin_focus_folds_the_part_sections_then_restores_the_users_state() {
        let mut state = PartPanelState::default();
        let mut collapsed: CollapsedSections =
            ["sel_location".to_string(), "other".to_string()].into();
        state.sync_pin_focus(&mut collapsed, true);
        for key in PART_SECTION_KEYS {
            assert!(
                collapsed.contains(key),
                "{key} folds while the pin is focused"
            );
        }
        // The user opens General while the pin is focused; a refresh with the
        // pin still focused leaves it open.
        collapsed.remove("sel_general");
        state.sync_pin_focus(&mut collapsed, true);
        assert!(!collapsed.contains("sel_general"));
        state.sync_pin_focus(&mut collapsed, false);
        let mut now: Vec<_> = collapsed.iter().cloned().collect();
        now.sort();
        assert_eq!(
            now,
            ["other", "sel_location"],
            "the user's own state is back"
        );
        assert_eq!(state.pin_focus_saved, None);
    }

    #[test]
    fn without_a_pin_focus_nothing_folds() {
        let mut state = PartPanelState::default();
        let mut collapsed = CollapsedSections::new();
        state.sync_pin_focus(&mut collapsed, false);
        assert!(collapsed.is_empty());
    }

    #[test]
    fn a_hover_reaches_the_illustration_state() {
        let mut state = PartPanelState::default();
        let field = signex_widgets::property_illustration::FieldId::new("designator");
        state.update(PartPanelMsg::IllustrationHovered(
            IllustrationMsg::FieldHovered(field.clone()),
        ));
        assert!(state.hover.is_field_highlighted(&field));
    }
}
