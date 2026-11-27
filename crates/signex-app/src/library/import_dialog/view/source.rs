//! Step 1, source and options (mockup row 211): Source with Browse…, the
//! Detected kind chips (and the converter chips when several read the
//! files), the database override, Name, Location, the mount scope and the
//! 3D models switch; or the not-installed notice.

use iced::widget::{Column, column, row};
use iced::{Alignment, Element, Length, Widget};
use signex_i18n::{I18n, Localized, RenderTranslatable, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{segmented, text, text_input};

use super::{
    ImportDialogContext, SP, TS, caption, chip, form_row, mono, outline_btn, problem, tag,
};
use crate::library::create_options::option_card;
use crate::library::import_dialog::source::Blocker;
use crate::library::import_dialog::{ImportDialogMsg, ImportDialogState, SourceKind, msg};
use crate::library::messages::LibraryMessage;
use crate::library::modal_chrome::{cancel_btn, footer_strip, notice_banner, primary_btn_if};
use crate::library_import::Discovery;
use crate::styles::PillTone;

pub(super) fn body<'a>(
    state: &'a ImportDialogState,
    ctx: ImportDialogContext<'a>,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let mut col: Column<Element<'a, LibraryMessage>> = column![].spacing(SP.l);
    match &state.discovery {
        None => {
            col = col.push(caption(
                t!(i18n, ids::LIBRARY_IMPORT_DIALOG_LOOKING),
                tokens,
            ))
        }
        Some(discovery) if discovery.converters.is_empty() => {
            col = col.push(not_installed(discovery, tokens, i18n));
        }
        Some(_) => {}
    }
    col = col.push(source_row(state, tokens, i18n));
    col = col.push(form_row(
        t!(i18n, ids::LIBRARY_IMPORT_DIALOG_DETECTED),
        detected(state, tokens, i18n),
        tokens,
    ));
    if state.has_database_input() {
        col = col.push(database_row(state, tokens, i18n));
    }
    col = col.push(name_and_location(state, tokens, i18n));
    col = col.push(options(state, ctx, tokens, i18n));
    if let Some(reason) = &state.target_problem {
        let words = text(reason.localized(i18n)).size(TS.body).boxed();
        col = col.push(notice_banner(PillTone::Err, words, Vec::new(), tokens));
    }
    if let Some(line) = blocker_line(state, i18n) {
        col = col.push(problem(line, tokens));
    }
    col.boxed()
}

/// The notice when no converter answered (mockup "Converter not
/// installed"): the install hint, why candidates were set aside, and
/// Look again.
fn not_installed<'a>(
    discovery: &Discovery,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let mut words = column![
        text(t!(i18n, ids::LIBRARY_IMPORT_DIALOG_NOT_INSTALLED_TITLE)).size(TS.body),
        text(
            Discovery::none_found_error()
                .to_translatable()
                .localized(i18n)
        )
        .size(TS.body),
    ]
    .spacing(SP.xs);
    for (_, why) in discovery.skipped.iter().take(3) {
        words = words.push(
            text(why.to_translatable().localized(i18n))
                .size(TS.caption)
                .boxed(),
        );
    }
    let actions = vec![
        outline_btn(
            t!(i18n, ids::LIBRARY_IMPORT_DIALOG_RESCAN),
            Some(msg(ImportDialogMsg::RescanPressed)),
            tokens,
        ),
        caption(
            t!(i18n, ids::LIBRARY_IMPORT_DIALOG_NOT_INSTALLED_NEXT),
            tokens,
        ),
    ];
    notice_banner(PillTone::Info, words.boxed(), actions, tokens)
}

/// Source: the Files / Database choice (when both are offered), the
/// picked files and Browse….
fn source_row<'a>(
    state: &'a ImportDialogState,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let mut line = row![].spacing(SP.m).align_y(Alignment::Center);
    if state.offers(SourceKind::Files) && state.offers(SourceKind::Database) {
        let style = crate::styles::tokens::resolve(tokens).segmented(TS.body);
        line = line.push(segmented(
            [
                (
                    SourceKind::Files,
                    t!(i18n, ids::LIBRARY_IMPORT_DIALOG_KIND_FILES).into(),
                ),
                (
                    SourceKind::Database,
                    t!(i18n, ids::LIBRARY_IMPORT_DIALOG_KIND_DATABASE).into(),
                ),
            ],
            state.source_kind,
            style,
            |kind| msg(ImportDialogMsg::SourceKindPicked(kind)),
        ));
    }
    let picked: Element<'a, LibraryMessage> = match state.sources.as_slice() {
        [] => caption(t!(i18n, ids::LIBRARY_IMPORT_DIALOG_NO_SOURCE), tokens),
        [one] => mono(one.display().to_string(), tokens),
        [first, rest @ ..] => text(t!(
            i18n,
            ids::LIBRARY_IMPORT_DIALOG_SOURCES_MANY,
            "first" => file_name(first),
            "count" => rest.len()
        ))
        .size(TS.body)
        .boxed(),
    };
    let can_browse = state
        .discovery
        .as_ref()
        .is_some_and(|d| !d.converters.is_empty());
    line = line.push(iced::widget::container(picked).width(Length::Fill).boxed());
    line = line.push(outline_btn(
        t!(i18n, ids::LIBRARY_IMPORT_DIALOG_BROWSE),
        can_browse.then(|| msg(ImportDialogMsg::BrowsePressed)),
        tokens,
    ));
    form_row(
        t!(i18n, ids::LIBRARY_IMPORT_DIALOG_SOURCE),
        line.boxed(),
        tokens,
    )
}

fn file_name(path: &std::path::Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

/// Detected: the kind chips and the converter that reads them, the
/// converter chips when several do, or why none does.
fn detected<'a>(
    state: &'a ImportDialogState,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    if let Some(path) = state.unreadable_source() {
        return problem(
            t!(i18n, ids::LIBRARY_IMPORT_DIALOG_UNREADABLE, "file" => file_name(path)),
            tokens,
        );
    }
    let readers = state.readers();
    if !state.sources.is_empty() && readers.is_empty() {
        return problem(t!(i18n, ids::LIBRARY_IMPORT_DIALOG_NO_COMMON), tokens);
    }
    let Some(chosen) = state.chosen_converter() else {
        return iced::widget::Space::new().boxed();
    };
    let mut kinds = row![].spacing(SP.s).align_y(Alignment::Center);
    for kind in state.detected_kinds() {
        kinds = kinds.push(tag(
            UserText::new(kind.label.clone()),
            PillTone::Accent,
            tokens,
        ));
    }
    let read_by = t!(
        i18n,
        ids::LIBRARY_IMPORT_DIALOG_READ_BY,
        "converter" => chosen.description.converter.as_str(),
        "version" => chosen.description.version.as_str()
    );
    kinds = kinds.push(caption(read_by, tokens));
    let mut col = column![kinds].spacing(SP.s);
    if readers.len() > 1 {
        let mut chips = row![caption(
            t!(i18n, ids::LIBRARY_IMPORT_DIALOG_CHOOSE_CONVERTER),
            tokens
        )]
        .spacing(SP.s)
        .align_y(Alignment::Center);
        for reader in readers {
            let label = format!(
                "{} {}",
                reader.description.converter, reader.description.version
            );
            chips = chips.push(chip(
                UserText::new(label),
                reader.path == chosen.path,
                PillTone::Accent,
                msg(ImportDialogMsg::ConverterPicked(reader.path.clone())),
                tokens,
            ));
        }
        col = col.push(chips.boxed());
    }
    col.boxed()
}

/// The database connection override for a database link file.
fn database_row<'a>(
    state: &'a ImportDialogState,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let input = text_input(
        t!(i18n, ids::LIBRARY_IMPORT_DIALOG_DATABASE_PLACEHOLDER),
        &state.database,
    )
    .style(crate::panels::form_look::text_input_style)
    .on_input(|v| msg(ImportDialogMsg::DatabaseEdited(v)))
    .size(TS.body)
    .padding(SP.s)
    .width(Length::Fill);
    let line = row![
        input,
        outline_btn(
            t!(i18n, ids::LIBRARY_IMPORT_DIALOG_DATABASE_CHANGE),
            Some(msg(ImportDialogMsg::DatabaseBrowsePressed)),
            tokens,
        )
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center);
    let content = column![
        line,
        caption(t!(i18n, ids::LIBRARY_IMPORT_DIALOG_DATABASE_HELP), tokens)
    ]
    .spacing(SP.xs);
    form_row(
        t!(i18n, ids::LIBRARY_IMPORT_DIALOG_DATABASE),
        content.boxed(),
        tokens,
    )
}

fn name_and_location<'a>(
    state: &'a ImportDialogState,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let name = text_input(
        t!(i18n, ids::LIBRARY_IMPORT_DIALOG_NAME_PLACEHOLDER),
        &state.name,
    )
    .style(crate::panels::form_look::text_input_style)
    .on_input(|v| msg(ImportDialogMsg::NameEdited(v)))
    .size(TS.body)
    .padding(SP.s)
    .width(Length::Fill);
    let location = text_input(UserText::new(String::new()), &state.location)
        .style(crate::panels::form_look::text_input_style)
        .on_input(|v| msg(ImportDialogMsg::LocationEdited(v)))
        .size(TS.body)
        .padding(SP.s)
        .width(Length::Fill);
    let location_line = row![
        location,
        outline_btn(
            t!(i18n, ids::LIBRARY_IMPORT_DIALOG_BROWSE),
            Some(msg(ImportDialogMsg::LocationBrowsePressed)),
            tokens,
        )
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center);
    column![
        form_row(
            t!(i18n, ids::LIBRARY_IMPORT_DIALOG_NAME),
            name.boxed(),
            tokens
        ),
        form_row(
            t!(i18n, ids::LIBRARY_IMPORT_DIALOG_LOCATION),
            column![
                location_line,
                caption(
                    t!(i18n, ids::LIBRARY_IMPORT_DIALOG_LOCATION_CAPTION),
                    tokens
                )
            ]
            .spacing(SP.xs)
            .boxed(),
            tokens,
        ),
    ]
    .spacing(SP.m)
    .boxed()
}

/// Mount for all projects (forced on with no project open) and the 3D
/// models switch.
fn options<'a>(
    state: &'a ImportDialogState,
    ctx: ImportDialogContext<'a>,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let mount_help = if ctx.project_open {
        t!(i18n, ids::LIBRARY_IMPORT_DIALOG_MOUNT_ALL_HELP)
    } else {
        t!(i18n, ids::LIBRARY_IMPORT_DIALOG_MOUNT_NO_PROJECT)
    };
    let mount = option_card(
        state.mount_for_all || !ctx.project_open,
        ctx.project_open,
        t!(i18n, ids::LIBRARY_IMPORT_DIALOG_MOUNT_ALL),
        mount_help,
        msg(ImportDialogMsg::MountForAllToggled),
        tokens,
    );
    let veil = signex_widgets::enable_section::dependents_veil(
        crate::styles::ti(tokens.paper),
        ctx.project_open,
    );
    let mount = match veil {
        Some(veil) => signex_widgets::inert::Inert::new(mount, veil).boxed(),
        None => mount,
    };
    let models = option_card(
        state.include_models,
        true,
        t!(i18n, ids::LIBRARY_IMPORT_DIALOG_MODELS),
        t!(i18n, ids::LIBRARY_IMPORT_DIALOG_MODELS_HELP),
        msg(ImportDialogMsg::ModelsToggled),
        tokens,
    );
    column![mount, models].spacing(SP.m).boxed()
}

/// The words for what keeps Next off, for the blockers the body does not
/// already show.
pub(crate) fn blocker_line(state: &ImportDialogState, i18n: &I18n) -> Option<Localized> {
    Some(match state.blocker()? {
        Blocker::InvalidName => t!(i18n, ids::LIBRARY_IMPORT_DIALOG_NAME_INVALID),
        Blocker::NoLocation => t!(i18n, ids::LIBRARY_IMPORT_DIALOG_LOCATION_EMPTY),
        Blocker::LocationNotAbsolute => t!(i18n, ids::LIBRARY_IMPORT_DIALOG_LOCATION_NOT_ABSOLUTE),
        Blocker::Looking
        | Blocker::NotInstalled
        | Blocker::NoSource
        | Blocker::Unreadable(_)
        | Blocker::NoCommonConverter
        | Blocker::TargetRefused
        | Blocker::Checking => return None,
    })
}

pub(super) fn footer<'a>(
    state: &'a ImportDialogState,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let next_label = if state.checking_target {
        t!(i18n, ids::LIBRARY_IMPORT_DIALOG_CHECKING)
    } else {
        t!(i18n, ids::LIBRARY_IMPORT_DIALOG_NEXT)
    };
    footer_strip(
        vec![caption(
            t!(i18n, ids::LIBRARY_IMPORT_DIALOG_SNAPSHOT),
            tokens,
        )],
        vec![
            cancel_btn(
                t!(i18n, ids::DIALOG_BUTTON_CANCEL),
                msg(ImportDialogMsg::CancelPressed),
                tokens,
            ),
            primary_btn_if(
                next_label,
                msg(ImportDialogMsg::NextPressed),
                state.blocker().is_none(),
                tokens,
            ),
        ],
        tokens,
    )
}
