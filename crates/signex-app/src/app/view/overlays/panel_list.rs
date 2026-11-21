//! The status bar's panel list (mockup row 7): the panels grouped under Project,
//! Inspect, Verify, Library and PCB captions, an open panel marked with a
//! check and the place it is open in. Panels that need a schematic or a
//! board nobody opened are left out, and so is a group left empty.

use super::*;
use iced::widget::{Column, container, row, svg};
use signex_i18n::{MessageId, UserText, ids, t};

use crate::dock::{PanelPosition, PanelSite};
use crate::panels::PanelKind;
use crate::panels::panel_groups::grouped_panels;

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// The check column at the left of a row (the mock's 14 px).
const CHECK_COLUMN: f32 = TS.body + SP.xs;
/// Side of the check mark.
const CHECK_ICON: f32 = TS.body;
/// Theme the check is fetched for: it is tinted with an ink token, which
/// hides the theme's accent swap, so any theme gives the same.
const GLYPH_THEME: signex_types::theme::ThemeId = signex_types::theme::ThemeId::GraphiteDark;

/// Where an open panel is: a dock region, floating over the window, or a
/// window of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PanelRegion {
    Left,
    Right,
    Bottom,
    Floating,
    Window,
}

impl PanelRegion {
    /// The caption at the right of the row.
    pub(super) fn caption(self) -> MessageId {
        match self {
            PanelRegion::Left => ids::PANELS_LIST_REGION_LEFT,
            PanelRegion::Right => ids::PANELS_LIST_REGION_RIGHT,
            PanelRegion::Bottom => ids::PANELS_LIST_REGION_BOTTOM,
            PanelRegion::Floating => ids::PANELS_LIST_REGION_FLOATING,
            PanelRegion::Window => ids::PANELS_LIST_REGION_WINDOW,
        }
    }

    /// The region of a panel found in the dock at `site`.
    pub(super) fn of_site(site: PanelSite) -> Self {
        match site {
            PanelSite::Docked(PanelPosition::Left, _) => PanelRegion::Left,
            PanelSite::Docked(PanelPosition::Right, _) => PanelRegion::Right,
            PanelSite::Docked(PanelPosition::Bottom, _) => PanelRegion::Bottom,
            PanelSite::Floating(_) => PanelRegion::Floating,
        }
    }
}

impl Signex {
    /// Where `kind` is open: in the dock, floating, or (open but in neither)
    /// detached into a window of its own. `None` while it is closed.
    pub(super) fn panel_region(&self, kind: PanelKind) -> Option<PanelRegion> {
        match self.document_state.dock.locate(kind) {
            Some(site) => Some(PanelRegion::of_site(site)),
            None => self.is_panel_open(kind).then_some(PanelRegion::Window),
        }
    }

    /// The panel list popover, anchored above the Panels button at the
    /// bottom right of the status bar. Clicking a row opens the panel, or
    /// brings an open one forward. Pushes the dismiss layer then the card.
    pub(in crate::app::view) fn panel_list_overlay(&self) -> Vec<Element<'_, Message>> {
        let ui = &self.ui_state;
        if !ui.panel_list_open {
            return Vec::new();
        }
        let document = &self.document_state;
        let tokens = &document.panel_ctx.tokens;
        let (has_sch, has_pcb) = (document.panel_ctx.has_schematic, document.panel_ctx.has_pcb);
        let groups = grouped_panels(|kind| {
            (!kind.needs_schematic() || has_sch) && (!kind.needs_pcb() || has_pcb)
        });
        let rows: usize = groups.iter().map(|(_, kinds)| kinds.len()).sum();
        let mut list: Column<Element<'_, Message>> = Column::new().spacing(0);
        for (group, kinds) in &groups {
            list = list.push(group_caption(&ui.i18n, group.caption(), tokens));
            for &kind in kinds {
                list = list.push(panel_row(kind, self.panel_region(kind), tokens, &ui.i18n));
            }
        }
        let popup = container(
            list.width(crate::status_bar::POPUP_WIDTH - 2.0 * crate::status_bar::POPUP_PADDING),
        )
        .padding(crate::status_bar::POPUP_PADDING)
        .style(crate::status_bar::popup(tokens));
        // Directly above the Panels button, its bottom edge just above the bar.
        let (ww, wh) = ui.window_size;
        let popup_h = crate::status_bar::panel_list_height(rows, groups.len());
        let status_h = crate::status_bar::status_bar_height(tokens);
        let left =
            (ww - crate::status_bar::POPUP_WIDTH - crate::status_bar::POPUP_RIGHT_MARGIN).max(0.0);
        let top = (wh - popup_h - status_h - crate::status_bar::POPUP_GAP).max(0.0);
        Self::dismiss_and_place(
            Message::Overlay(OverlayMsg::TogglePanelList),
            popup.boxed(),
            (left, top),
        )
    }
}

/// A group caption: the section size, muted, in capitals for the language.
fn group_caption<'a>(
    i18n: &signex_i18n::I18n,
    caption: MessageId,
    tokens: &signex_types::theme::ThemeTokens,
) -> Element<'a, Message> {
    let words = t!(i18n, caption).into_string();
    container(
        signex_widgets::text(UserText::new(i18n.upper_for_display(&words)))
            .size(TS.section)
            .style(crate::styles::section_label(tokens)),
    )
    .padding(crate::status_bar::popup_caption_padding())
    .height(crate::status_bar::popup_caption_height())
    .width(Length::Fill)
    .boxed()
}

/// One panel row: the check when open, the name, and where it is open.
fn panel_row<'a>(
    kind: PanelKind,
    region: Option<PanelRegion>,
    tokens: &signex_types::theme::ThemeTokens,
    i18n: &signex_i18n::I18n,
) -> Element<'a, Message> {
    let check_c = crate::status_bar::check_ink(tokens);
    let check: Element<'a, Message> = match region {
        Some(_) => svg(crate::icons::icon_chrome_check(GLYPH_THEME))
            .width(CHECK_ICON)
            .height(CHECK_ICON)
            .style(move |_: &iced::Theme, _| svg::Style {
                color: Some(check_c),
                ..Default::default()
            })
            .boxed(),
        None => iced::widget::Space::new().boxed(),
    };
    let mut line = row![
        container(check).width(CHECK_COLUMN),
        signex_widgets::text(t!(i18n, kind.message()))
            .size(TS.body)
            .color(crate::styles::ti(tokens.text))
            .width(Length::Fill),
    ]
    .spacing(SP.m)
    .align_y(iced::Alignment::Center);
    if let Some(region) = region {
        line = line.push(
            signex_widgets::text(t!(i18n, region.caption()))
                .size(TS.caption)
                .color(crate::styles::tokens::resolve(tokens).ink4)
                .boxed(),
        );
    }
    signex_widgets::button(line)
        .width(Length::Fill)
        .on_press(Message::Overlay(OverlayMsg::OpenPanel(kind)))
        .padding([0.0, SP.m])
        .height(crate::status_bar::popup_row_height())
        .style(crate::status_bar::popup_row(tokens))
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each place a panel can be open in has its own caption: left, right,
    /// bottom, floating, window.
    #[test]
    fn each_region_has_its_own_caption() {
        let i18n = signex_i18n::I18n::en_us();
        let shown: Vec<String> = [
            PanelRegion::Left,
            PanelRegion::Right,
            PanelRegion::Bottom,
            PanelRegion::Floating,
            PanelRegion::Window,
        ]
        .into_iter()
        .map(|r| t!(i18n, r.caption()).into_string())
        .collect();
        assert_eq!(shown, ["left", "right", "bottom", "floating", "window"]);
    }

    /// A docked panel is captioned with its region, a floating one as
    /// floating.
    #[test]
    fn a_dock_site_maps_to_its_region() {
        assert_eq!(
            PanelRegion::of_site(PanelSite::Docked(PanelPosition::Left, 0)),
            PanelRegion::Left
        );
        assert_eq!(
            PanelRegion::of_site(PanelSite::Docked(PanelPosition::Right, 2)),
            PanelRegion::Right
        );
        assert_eq!(
            PanelRegion::of_site(PanelSite::Docked(PanelPosition::Bottom, 1)),
            PanelRegion::Bottom
        );
        assert_eq!(
            PanelRegion::of_site(PanelSite::Floating(0)),
            PanelRegion::Floating
        );
    }

    /// The app's own region answer: a panel in the dock reads its region; a
    /// closed one has none.
    #[test]
    fn the_region_follows_the_dock() {
        let (mut app, _boot) = Signex::new();
        let kind = PanelKind::Properties;
        app.document_state.dock.close_panel(kind);
        assert_eq!(app.panel_region(kind), None);
        app.document_state
            .dock
            .add_panel(PanelPosition::Bottom, kind);
        assert_eq!(app.panel_region(kind), Some(PanelRegion::Bottom));
    }
}
