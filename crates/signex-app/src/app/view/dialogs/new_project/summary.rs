//! The project-setup summary row of New PCB and New Schematic (#2538): when
//! the project saved a setup, one sunken row says what it is and "Use
//! another profile..." unfolds the cards; unfolded, it reads "Keep project
//! setup".

use iced::widget::{Space, container, row};
use iced::{Alignment, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, Localized, ids, t};
use signex_types::project::{ProjectSetup, UserProfile};
use signex_types::schematic::SheetSize;
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, text};

use super::cards::mm;
use super::{SP, TS};
use crate::app::{DocumentSetupMsg, SetupKind};
use crate::styles::{Radii, ti, tokens};

const CHECK: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24"><path d="M5 12l5 5 9-10" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"/></svg>"#;

/// The name of the profile a saved setup is on: a saved profile's name, else
/// the card's catalog name.
fn profile_name(setup: &ProjectSetup, users: &[UserProfile], i18n: &I18n) -> String {
    setup
        .user_profile
        .as_ref()
        .and_then(|id| users.iter().find(|p| p.id == *id))
        .map(|p| p.name.clone())
        .unwrap_or_else(|| {
            let id = crate::app::new_project::profile_name_id(setup.profile);
            t!(i18n, id).into_string()
        })
}

/// The summary's value line: `Standard · 4 layers · 0.15 mm clearance ·
/// 0.2 mm tracks`, or for a sheet `A4 landscape · Signex default title
/// block · IEEE 315`.
pub(crate) fn summary_line(
    setup: &ProjectSetup,
    kind: SetupKind,
    users: &[UserProfile],
    i18n: &I18n,
) -> Localized {
    match kind {
        SetupKind::Pcb => t!(
            i18n,
            ids::SETUP_SUMMARY_PCB,
            "profile" => profile_name(setup, users, i18n),
            "layers" => u32::from(setup.pcb.copper_layers),
            "clearance" => mm(setup.pcb.clearance_mm),
            "track" => mm(setup.pcb.track_width_mm)
        ),
        SetupKind::Sheet => {
            let block = if setup.sheet.show_title_block {
                ids::SETUP_SUMMARY_BLOCK_DEFAULT
            } else {
                ids::SETUP_SUMMARY_BLOCK_NONE
            };
            let paper = if SheetSize::parse(&setup.sheet.paper_size).0.is_recognized() {
                setup.sheet_line()
            } else {
                setup.sheet.paper_size.clone()
            };
            t!(
                i18n,
                ids::SETUP_SUMMARY_SHEET,
                "paper" => paper,
                "block" => t!(i18n, block).into_string(),
                "designators" => setup.designators.label()
            )
        }
    }
}

/// The summary row: the check, the bold heading, the value line, and the
/// button that unfolds (or folds) the cards.
pub(crate) fn row_view<'a>(
    setup: &ProjectSetup,
    kind: SetupKind,
    open: bool,
    users: &[UserProfile],
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, DocumentSetupMsg> {
    let bold = iced::Font {
        weight: iced::font::Weight::Semibold,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    };
    let link = if open {
        ids::SETUP_SUMMARY_KEEP
    } else {
        ids::SETUP_SUMMARY_OTHER
    };
    let toggle = button(text(t!(i18n, link)).size(TS.body))
        .padding([SP.xs, SP.m])
        .on_press(DocumentSetupMsg::OtherProfileToggled)
        .style(crate::styles::ghost_button(tokens));
    let ok = ti(tokens.success);
    let content = row![
        super::card_parts::icon(CHECK, TS.label, ok),
        text(t!(i18n, ids::SETUP_SUMMARY_HEADING))
            .size(TS.body)
            .font(bold)
            .color(ti(tokens.text)),
        text(summary_line(setup, kind, users, i18n))
            .size(TS.caption)
            .font(super::super::graphite::mono())
            .color(ti(tokens.text_secondary)),
        Space::new().width(Length::Fill),
        toggle,
    ]
    .spacing(SP.m + SP.xs)
    .align_y(Alignment::Center);
    let fill = tokens::resolve(tokens).sunken;
    let line = ti(tokens.border);
    container(content)
        .padding([SP.m + SP.xs, SP.l])
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(fill.into()),
            border: Border {
                width: 1.0,
                color: line,
                radius: Radii::GRAPHITE.card.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}
