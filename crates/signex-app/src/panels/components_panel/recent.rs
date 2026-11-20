//! "Recently placed" of the Components panel (#2535): the parts placed from
//! this panel this session, shown in the details area while no part is
//! selected. Session-scoped; nothing is written to disk.

use iced::widget::{Column, column, container, row};
use iced::{Alignment, Border, Element, Length, Padding, Theme, Widget};
use signex_i18n::{I18n, ids, t};
use signex_library::ComponentRow;
use signex_types::theme::ThemeTokens;
use signex_widgets::text;

use super::{HINT_TEXT_SIZE, ROW_TEXT_SIZE, SP, look, thumbs};
use crate::library::messages::LibraryMessage;
use crate::library::state::{ComponentsPanelRowKey, ComponentsPanelState, LibraryState};
use crate::styles::ti;
use crate::styles::tokens::resolve;

/// Most parts the list keeps.
pub const MAX_RECENT: usize = 5;

/// One recently placed part and how often it was placed this session.
#[derive(Debug, Clone, PartialEq)]
pub struct RecentPart {
    pub key: ComponentsPanelRowKey,
    pub uses: u32,
}

/// The recently placed parts, most recent first, at most [`MAX_RECENT`].
#[derive(Debug, Clone, Default)]
pub struct RecentlyPlaced {
    parts: Vec<RecentPart>,
}

impl RecentlyPlaced {
    /// `key` was placed: it moves to the front with one more use.
    pub fn record(&mut self, key: ComponentsPanelRowKey) {
        let part = match self.parts.iter().position(|p| p.key == key) {
            Some(at) => {
                let mut part = self.parts.remove(at);
                part.uses = part.uses.saturating_add(1);
                part
            }
            None => RecentPart { key, uses: 1 },
        };
        self.parts.insert(0, part);
        self.parts.truncate(MAX_RECENT);
    }

    /// The parts, most recent first.
    pub fn parts(&self) -> &[RecentPart] {
        &self.parts
    }
}

/// Whether the details area shows Recently placed: only while no part is
/// selected.
pub(super) fn shows_recent(panel: &ComponentsPanelState) -> bool {
    panel.selected.is_none()
}

/// The in-memory row `key` names.
pub(super) fn row_of<'a>(
    state: &'a LibraryState,
    key: &ComponentsPanelRowKey,
) -> Option<&'a ComponentRow> {
    state
        .library_at(&key.library_path)?
        .tables
        .get(&key.table)?
        .iter()
        .find(|r| r.row_id == key.row_id.0)
}

/// Recent parts whose row is still mounted, with that row.
pub(super) fn listed<'a>(
    state: &'a LibraryState,
    panel: &'a ComponentsPanelState,
) -> Vec<(&'a RecentPart, &'a ComponentRow)> {
    panel
        .recent
        .parts()
        .iter()
        .filter_map(|part| Some((part, row_of(state, &part.key)?)))
        .collect()
}

/// The details area with nothing selected: Recently placed (or its hint
/// card), then the "select a component" line.
pub(super) fn view<'a>(
    state: &'a LibraryState,
    panel: &'a ComponentsPanelState,
    tokens: &'a ThemeTokens,
    thumb_look: thumbs::ThumbLook,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let r = resolve(tokens);
    let previews = panel.row_style.shows_previews();
    let parts = listed(state, panel);
    let mut col: Column<Element<'a, LibraryMessage>> = column![].spacing(0).width(Length::Fill);
    col = col.push(
        container(
            text(t!(i18n, ids::PANELS_COMPONENTS_RECENT_TITLE))
                .size(crate::styles::TypeScale::DEFAULT.section)
                .color(r.ink4),
        )
        .padding(Padding {
            top: SP.s,
            right: look::EDGE,
            bottom: SP.xs,
            left: look::EDGE,
        })
        .boxed(),
    );
    if parts.is_empty() {
        col = col.push(hint_card(tokens, i18n));
    }
    for (part, row_data) in parts {
        let thumb = previews
            .then(|| panel.thumbs.get(&row_data.symbol_ref).cloned())
            .flatten();
        col = col.push(recent_row(
            part,
            row_data,
            thumb.map(|lib| thumb_look.thumb(lib)),
            previews,
            tokens,
            i18n,
        ));
    }
    col = col.push(
        container(
            text(t!(i18n, ids::PANELS_COMPONENTS_DETAILS_SELECT))
                .size(HINT_TEXT_SIZE)
                .color(r.ink4),
        )
        .padding([SP.s, look::EDGE])
        .boxed(),
    );
    col.boxed()
}

/// One recent part: thumbnail (Previews only), part number, use count and
/// its Place button.
fn recent_row<'a>(
    part: &RecentPart,
    row_data: &'a ComponentRow,
    thumb: Option<Element<'a, LibraryMessage>>,
    previews: bool,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let r = resolve(tokens);
    let mut line = row![].spacing(SP.s).align_y(Alignment::Center);
    if previews {
        line = line.push(thumb.unwrap_or_else(look::thumb_placeholder));
    }
    line = line
        .push(
            text(signex_i18n::UserText::new(row_data.internal_pn.as_str()))
                .size(ROW_TEXT_SIZE)
                .font(look::mono())
                .color(ti(tokens.text))
                .width(Length::Fill)
                .wrapping(iced::widget::text::Wrapping::None)
                .boxed(),
        )
        .push(
            text(t!(i18n, ids::PANELS_COMPONENTS_RECENT_USES, "count" => i64::from(part.uses)))
                .size(HINT_TEXT_SIZE)
                .font(look::mono())
                .color(r.ink4)
                .boxed(),
        )
        .push(look::place_button(
            LibraryMessage::ComponentsPanelPlace {
                library_path: part.key.library_path.clone(),
                table: part.key.table.clone(),
                row_id: part.key.row_id,
            },
            tokens,
            i18n,
        ));
    container(line)
        .padding([SP.xs, look::EDGE])
        .width(Length::Fill)
        .boxed()
}

/// The card shown before anything was placed from the panel.
fn hint_card<'a>(tokens: &ThemeTokens, i18n: &I18n) -> Element<'a, LibraryMessage> {
    let r = resolve(tokens);
    let line = r.line_strong;
    let radius = crate::styles::Radii::GRAPHITE.card;
    container(
        container(
            text(t!(i18n, ids::PANELS_COMPONENTS_RECENT_EMPTY))
                .size(HINT_TEXT_SIZE)
                .color(r.ink3),
        )
        .padding([SP.m, SP.m])
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            border: Border {
                width: 1.0,
                color: line,
                radius: radius.into(),
                ..Border::default()
            },
            ..container::Style::default()
        }),
    )
    .padding([SP.xs, look::EDGE])
    .width(Length::Fill)
    .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(n: u128) -> ComponentsPanelRowKey {
        ComponentsPanelRowKey {
            library_path: "lib.snxlib".into(),
            table: "t".into(),
            row_id: signex_library::RowId::from_uuid(uuid::Uuid::from_u128(n)),
        }
    }

    #[test]
    fn recent_parts_are_most_recent_first_capped_and_counted() {
        let mut recent = RecentlyPlaced::default();
        for n in 1..=6 {
            recent.record(key(n));
        }
        let order: Vec<_> = recent.parts().iter().map(|p| p.key.clone()).collect();
        assert_eq!(
            order,
            [key(6), key(5), key(4), key(3), key(2)],
            "at most five"
        );

        recent.record(key(4));
        recent.record(key(4));
        let first = &recent.parts()[0];
        assert_eq!(first.key, key(4), "placing again moves it to the front");
        assert_eq!(first.uses, 3);
        assert_eq!(recent.parts().len(), MAX_RECENT);
        assert!(recent.parts()[1..].iter().all(|p| p.uses == 1));
    }

    #[test]
    fn recently_placed_shows_only_while_nothing_is_selected() {
        let mut panel = ComponentsPanelState::default();
        panel.recent.record(key(1));
        assert!(shows_recent(&panel));
        panel.selected = Some(key(1));
        assert!(!shows_recent(&panel));
    }

    #[test]
    fn a_recent_part_whose_row_is_gone_is_not_listed() {
        let mut state = LibraryState::default();
        state.components_panel.recent.record(key(1));
        assert!(listed(&state, &state.components_panel).is_empty());
    }
}
