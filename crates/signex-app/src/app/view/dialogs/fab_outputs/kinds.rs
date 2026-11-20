//! The kinds of file: four groups in two columns, each group headed by its
//! name and how many of its kinds are ticked, each kind a short title with
//! a muted description under it.

use iced::widget::{Column, Row};
use iced::{Element, Length, Widget as _};
use signex_i18n::{I18n, MessageId, ids, t};
use signex_types::theme::ThemeTokens;

use super::{SP, fo, kit};
use crate::app::Message;
use crate::app::fab_outputs::{FabGroup, FabOutput, FabOutputKinds, FabOutputsMsg};

/// Gap between the two columns (24 px at the 11 px base).
const COLUMN_GAP: f32 = 2.0 * SP.l;

/// The catalog title of a kind.
pub(super) fn kind_label(kind: FabOutput) -> MessageId {
    match kind {
        FabOutput::Gerber => ids::FAB_OUTPUTS_GERBER,
        FabOutput::Drill => ids::FAB_OUTPUTS_DRILL,
        FabOutput::Centroid => ids::FAB_OUTPUTS_CENTROID,
        FabOutput::TestNetlist => ids::FAB_OUTPUTS_TEST_NETLIST,
        FabOutput::Ipc2581 => ids::FAB_OUTPUTS_IPC2581,
        FabOutput::Odb => ids::FAB_OUTPUTS_ODB,
        FabOutput::AssemblyPdf => ids::FAB_OUTPUTS_ASSEMBLY_PDF,
        FabOutput::AssemblySvg => ids::FAB_OUTPUTS_ASSEMBLY_SVG,
        FabOutput::Step => ids::FAB_OUTPUTS_STEP,
    }
}

/// The muted line under a kind's title; `None` for a kind whose title
/// says it all.
pub(super) fn kind_detail(kind: FabOutput) -> Option<MessageId> {
    match kind {
        FabOutput::Gerber => Some(ids::FAB_OUTPUTS_GERBER_DETAIL),
        FabOutput::Drill => Some(ids::FAB_OUTPUTS_DRILL_DETAIL),
        FabOutput::Ipc2581 => Some(ids::FAB_OUTPUTS_IPC2581_DETAIL),
        FabOutput::AssemblyPdf => Some(ids::FAB_OUTPUTS_ASSEMBLY_PDF_DETAIL),
        FabOutput::AssemblySvg => Some(ids::FAB_OUTPUTS_ASSEMBLY_SVG_DETAIL),
        FabOutput::Centroid | FabOutput::TestNetlist | FabOutput::Odb | FabOutput::Step => None,
    }
}

/// The catalog name of a group.
pub(super) fn group_label(group: FabGroup) -> MessageId {
    match group {
        FabGroup::Fabrication => ids::FAB_OUTPUTS_GROUP_FABRICATION,
        FabGroup::SingleFile => ids::FAB_OUTPUTS_GROUP_SINGLE_FILE,
        FabGroup::Assembly => ids::FAB_OUTPUTS_GROUP_ASSEMBLY,
        FabGroup::Model => ids::FAB_OUTPUTS_GROUP_MODEL,
    }
}

/// The four groups, two to a column.
pub(super) fn grid<'a>(
    ticked: &FabOutputKinds,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let mut columns: Row<Element<'a, Message>> = Row::new().spacing(COLUMN_GAP);
    for groups in FabGroup::COLUMNS {
        let mut column: Column<Element<'a, Message>> =
            Column::new().spacing(SP.xl).width(Length::FillPortion(1));
        for group in groups {
            column = column.push(group_view(group, ticked, i18n, tokens));
        }
        columns = columns.push(column.boxed());
    }
    columns.width(Length::Fill).boxed()
}

/// A group: its head with the live count, then its kinds.
fn group_view<'a>(
    group: FabGroup,
    ticked: &FabOutputKinds,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let (on, total) = group.count(ticked);
    let count = t!(i18n, ids::FAB_OUTPUTS_GROUP_COUNT, "ticked" => on, "total" => total);
    let mut col: Column<Element<'a, Message>> = Column::new().spacing(SP.s).push(
        kit::section_head(t!(i18n, group_label(group)), Some(count), i18n, tokens),
    );
    for kind in group.kinds() {
        col = col.push(kind_row(*kind, ticked, i18n, tokens));
        // The ODB++ form sits under its row, greyed until ODB++ is ticked.
        if *kind == FabOutput::Odb {
            let folder = kit::tick(
                ticked.odb_folder,
                t!(i18n, ids::FAB_OUTPUTS_ODB_FOLDER),
                ticked.odb.then(|| fo(FabOutputsMsg::OdbFolderToggled)),
            );
            col = col.push(kit::under_tick(kit::dependent(ticked.odb, folder, tokens)));
        }
    }
    col.boxed()
}

/// One kind: its tick box and title on the first line, the description
/// under the title.
fn kind_row<'a>(
    kind: FabOutput,
    ticked: &FabOutputKinds,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let tick = kit::tick(
        kind.is_on(ticked),
        t!(i18n, kind_label(kind)),
        Some(fo(FabOutputsMsg::OutputToggled(kind))),
    );
    match kind_detail(kind) {
        Some(detail) => Column::new()
            .push(tick)
            .push(kit::under_tick(kit::caption(t!(i18n, detail), tokens)))
            .boxed(),
        None => tick,
    }
}
