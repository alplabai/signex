//! Compact group samples on both Graphite themes: DDR_DQ0-7 open with four
//! members (one selected, one out of band) and USB_D collapsed. Both theme
//! columns read one sample model, so a click in either updates both: the
//! pair rule of the tab and its dock panel.

use iced::widget::{Column, container, row};
use iced::{Background, Element, Length, Theme, Widget};
use signex_i18n::{UserText, ids, t};
use signex_types::coord::Unit;
use signex_types::theme::{ThemeId, theme_tokens};
use signex_widgets::LengthFormat;
use signex_widgets::compact_group::{
    CompactGroup, CompactGroupStyle, GroupHeader, Member, StatusPill, StatusTone, out_of_band_count,
};
use signex_widgets::theme_ext::{Spacing, TypeScale};

use crate::catalog::Message;
use crate::theme;

const BODY: f32 = TypeScale::DEFAULT.body;
const SP: Spacing = Spacing::DEFAULT;
/// Width of the compact dock panel, in em (300 px at 12 px).
const PANEL_EM: f32 = 25.0;

/// A sample group: name, target, tolerance and (name, length, caption).
type SampleGroup = (
    &'static str,
    f64,
    f64,
    &'static [(&'static str, f64, Option<&'static str>)],
);

const GROUPS: [SampleGroup; 2] = [
    (
        "DDR_DQ0-7",
        28.50,
        0.25,
        &[
            ("DDR_DQ0", 28.40, None),
            ("DDR_DQ1", 28.52, None),
            ("DDR_DQ6", 28.91, None),
            ("DDR_DQ7", 28.49, None),
        ],
    ),
    (
        "USB_D",
        12.40,
        0.15,
        &[
            ("USB_DP", 12.36, Some("USB_D")),
            ("USB_DM", 12.41, Some("USB_D")),
        ],
    ),
];

/// What a sample header or row sent.
#[derive(Debug, Clone)]
pub(crate) enum SampleMsg {
    GroupToggled(usize),
    MemberSelected(&'static str),
}

/// The open groups and the selected member, shared by both theme columns.
pub(crate) struct CompactGroupSample {
    open: [bool; 2],
    selected: Option<&'static str>,
}

impl Default for CompactGroupSample {
    fn default() -> Self {
        Self {
            open: [true, false],
            selected: Some("DDR_DQ1"),
        }
    }
}

fn format() -> LengthFormat<'static> {
    LengthFormat::new(crate::locale::en_us(), Unit::Mm)
}

impl CompactGroupSample {
    pub(crate) fn update(&mut self, message: SampleMsg) {
        match message {
            SampleMsg::GroupToggled(index) => {
                if let Some(open) = self.open.get_mut(index) {
                    *open = !*open;
                }
            }
            SampleMsg::MemberSelected(name) => self.selected = Some(name),
        }
    }

    fn group<'a>(&self, index: usize) -> CompactGroup<'a, usize, &'static str, SampleMsg> {
        let i18n = crate::locale::en_us();
        let fmt = format();
        let (name, target, tolerance, rows) = GROUPS[index];
        let members: Vec<Member<'a, &'static str>> = rows
            .iter()
            .map(|&(member, length, pair)| Member {
                key: member,
                name: UserText::new(member).into(),
                caption: pair
                    .map(|pair| t!(i18n, ids::PANELS_COMPACT_GROUP_PAIR, "pair" => pair).into()),
                length,
                target,
                tolerance,
                is_selected: self.selected == Some(member),
            })
            .collect();
        let out = out_of_band_count(&members);
        let status = if out > 0 {
            StatusPill {
                label: t!(i18n, ids::PANELS_COMPACT_GROUP_OUT_OF_BAND, "count" => out).into(),
                tone: StatusTone::Err,
            }
        } else {
            StatusPill {
                label: t!(i18n, ids::PANELS_COMPACT_GROUP_ALL_IN_BAND).into(),
                tone: StatusTone::Ok,
            }
        };
        let header = GroupHeader {
            name: UserText::new(name).into(),
            captions: vec![
                t!(i18n, ids::PANELS_COMPACT_GROUP_MEMBERS, "count" => members.len()).into(),
                t!(
                    i18n,
                    ids::PANELS_COMPACT_GROUP_BAND,
                    "target" => fmt.field_text(target),
                    "tolerance" => fmt.field_text(tolerance),
                )
                .into(),
            ],
            status: Some(status),
        };
        CompactGroup::new(
            index,
            header,
            members,
            self.open[index],
            SampleMsg::GroupToggled,
            SampleMsg::MemberSelected,
        )
    }

    fn panel<'a>(&self, theme_id: ThemeId) -> Element<'a, SampleMsg> {
        let tokens = theme_tokens(theme_id);
        let style = CompactGroupStyle::from_tokens(&tokens, BODY);
        let fmt = format();
        let mut body: Column<Element<'a, SampleMsg>> = Column::new().width(BODY * PANEL_EM);
        for index in 0..GROUPS.len() {
            body = body.push(self.group(index).view(&fmt, style));
        }
        let ground = theme::color(tokens.panel_bg);
        container(body)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(ground)),
                ..container::Style::default()
            })
            .boxed()
    }

    pub(crate) fn view<'a>(&self) -> Element<'a, Message> {
        row![
            self.panel(ThemeId::GraphiteLight),
            self.panel(ThemeId::GraphiteDark)
        ]
        .spacing(SP.xl)
        .width(Length::Shrink)
        .boxed()
        .map(Message::CompactGroup)
        .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sample_shows_one_selected_and_one_out_of_band_member() {
        let sample = CompactGroupSample::default();
        let open = sample.group(0);
        assert!(open.select_message(3).is_some());
        let rows = GROUPS[0].3;
        assert_eq!(rows.len(), 4);
        let out = rows
            .iter()
            .filter(|(_, length, _)| (length - GROUPS[0].1).abs() > GROUPS[0].2)
            .count();
        assert_eq!(out, 1);
        assert_eq!(sample.selected, Some("DDR_DQ1"));
        assert_eq!(sample.open, [true, false]);
    }

    #[test]
    fn toggling_and_selecting_update_the_shared_model() {
        let mut sample = CompactGroupSample::default();
        sample.update(SampleMsg::GroupToggled(1));
        assert_eq!(sample.open, [true, true]);
        sample.update(SampleMsg::MemberSelected("USB_DM"));
        assert_eq!(sample.selected, Some("USB_DM"));
        sample.update(SampleMsg::GroupToggled(9));
        assert_eq!(sample.open, [true, true]);
    }
}
