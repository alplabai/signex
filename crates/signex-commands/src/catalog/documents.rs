//! Document commands: create, open, save, revert, close and print a
//! document or project, plus the project options and report dialogs.

use super::operands::{Operand, OperandKind, optional, required};
use super::{
    CommandFlags, CommandGroup, CommandMetadata, DocumentKind, Enablement, operands,
    variant_operands,
};
use signex_i18n_ids::{command_ids, menu_ids};

/// A schematic printed to PDF (#1298, #2609, #2610, #2787): which sheet and
/// the file; the rest defaults as Export PDF opens on the sheet. `pages` is
/// `all`, `current` or sheet numbers and ranges (`1-2, 4`), the Pages
/// field's rule; `layers` is `all` or the ids of the PDF layers visible on
/// open (`PdfLayer::id`), a flat PDF when left out; `variant` names the
/// printed variant. `watermark` is `draft`, `confidential` or the text to
/// print; the other `watermark_*` operands style it (`watermark_pages` is
/// `all` or `first`). `links=false` writes no link annotations.
const SCHEMATIC_PDF: &[Operand] = &[
    required("document", OperandKind::Document),
    required("path", OperandKind::Path),
    optional("pages", OperandKind::Text),
    optional("layers", OperandKind::Text),
    optional("variant", OperandKind::Text),
    optional("watermark", OperandKind::Text),
    optional("watermark_color", OperandKind::Color),
    optional("watermark_opacity", OperandKind::Text),
    optional("watermark_angle", OperandKind::Text),
    optional("watermark_pages", OperandKind::Text),
    optional("watermark_revision", OperandKind::Bool),
    optional("links", OperandKind::Bool),
];

/// A block diagram written as SVG or PDF (#2796): the `.snxblk` and the
/// file; `kind` is `svg` or `pdf` (else the file's extension), `monochrome`
/// draws every mark in black, `title_block=false` leaves the PDF's off.
const BLOCK_DIAGRAM_EXPORT: &[Operand] = &[
    required("document", OperandKind::Document),
    required("path", OperandKind::Path),
    optional("kind", OperandKind::Text),
    optional("monochrome", OperandKind::Bool),
    optional("title_block", OperandKind::Bool),
    // A PDF's pages (`blocks,power_tree,budget`), its paper
    // (`a4|a3|a2|letter|tabloid`), `landscape|portrait`, and the variant
    // the power tree pages read (mockup row 171).
    optional("pages", OperandKind::Text),
    optional("paper", OperandKind::Text),
    optional("orientation", OperandKind::Text),
    optional("variant", OperandKind::Text),
];

pub(super) const DOCUMENTS: &[CommandMetadata] = &[
    CommandMetadata {
        id: "new_document",
        category: "file",
        label: command_ids::CMD_NEW_DOCUMENT,
        menu_label: Some(command_ids::CMD_MENU_NEW_DOCUMENT),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::SAVE_PATH,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "new_project_with_profile",
        category: "file",
        label: command_ids::CMD_NEW_PROJECT_WITH_PROFILE,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::NEW_PROJECT,
        ..CommandMetadata::DEFAULT
    },
    // #2232: a new sheet in the project (File > New > Schematic, Projects >
    // Add New > Schematic). The GUI opens the New Schematic setup dialog,
    // whose Create asks for the file; a headless run names its `profile`.
    CommandMetadata {
        id: "new_schematic_sheet",
        category: "file",
        label: command_ids::CMD_NEW_SCHEMATIC_SHEET,
        menu_label: Some(command_ids::CMD_MENU_NEW_SCHEMATIC_SHEET),
        group: CommandGroup::Schematic,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES_NOT_UNDOABLE,
        operands: operands::NEW_SCHEMATIC,
        ..CommandMetadata::DEFAULT
    },
    // #1312: the project's one board, opened unsaved (File > New > PCB,
    // Projects > Add New > PCB). #2232: the GUI opens the New PCB setup
    // dialog first; a headless run names its `profile`.
    CommandMetadata {
        id: "new_pcb_document",
        category: "file",
        label: command_ids::CMD_NEW_PCB_DOCUMENT,
        menu_label: Some(command_ids::CMD_MENU_NEW_PCB_DOCUMENT),
        group: CommandGroup::Pcb,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES_NOT_UNDOABLE,
        operands: super::operands::NEW_PCB,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "open_document",
        category: "file",
        label: command_ids::CMD_OPEN_DOCUMENT,
        menu_label: Some(command_ids::CMD_MENU_OPEN_DOCUMENT),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::OPEN_PATH,
        ..CommandMetadata::DEFAULT
    },
    // #2896: the Gerber Viewer tab, empty or on the package `path` names (a
    // folder, a zip or one file). Palette only for now, no menu row; it
    // answers with what the tab shows.
    CommandMetadata {
        id: "open_gerber_viewer",
        category: "file",
        label: command_ids::CMD_OPEN_GERBER_VIEWER,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::OPTIONAL_PATH,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "save_document",
        category: "file",
        label: command_ids::CMD_SAVE_DOCUMENT,
        menu_label: Some(command_ids::CMD_MENU_SAVE_DOCUMENT),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "save_document_as",
        category: "file",
        label: command_ids::CMD_SAVE_DOCUMENT_AS,
        menu_label: Some(command_ids::CMD_MENU_SAVE_DOCUMENT_AS),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::SAVE_DOCUMENT_AS,
        ..CommandMetadata::DEFAULT
    },
    // #2844: File > Save All, on a key. It writes every unsaved document of
    // the session, so it stays a person's command.
    CommandMetadata {
        id: "save_all",
        category: "file",
        label: command_ids::CMD_SAVE_ALL,
        menu_label: Some(menu_ids::MENU_FILE_SAVE_ALL),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    // #2908: File > Save Project As…, a copy of the project in a new folder
    // through its dialog.
    CommandMetadata {
        id: "save_project_as",
        category: "file",
        label: command_ids::CMD_SAVE_PROJECT_AS,
        menu_label: Some(command_ids::CMD_MENU_SAVE_PROJECT_AS),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "revert",
        category: "edit",
        label: command_ids::CMD_REVERT,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES_NOT_UNDOABLE,
        operands: operands::REVERT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "close_active_document",
        category: "file",
        label: command_ids::CMD_CLOSE_ACTIVE_DOCUMENT,
        menu_label: Some(command_ids::CMD_MENU_CLOSE_ACTIVE_DOCUMENT),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "close_all_documents",
        category: "window",
        label: command_ids::CMD_CLOSE_ALL_DOCUMENTS,
        menu_label: Some(command_ids::CMD_MENU_CLOSE_ALL_DOCUMENTS),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "print",
        category: "file",
        label: command_ids::CMD_PRINT,
        menu_label: Some(command_ids::CMD_MENU_PRINT),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "open_project_options",
        category: "file",
        label: command_ids::CMD_OPEN_PROJECT_OPTIONS,
        menu_label: Some(command_ids::CMD_MENU_OPEN_PROJECT_OPTIONS),
        group: CommandGroup::Schematic,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "report_manager_bom",
        category: "reports",
        label: command_ids::CMD_REPORT_MANAGER_BOM,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    // #2388: open the Fabrication Outputs dialog on the board in view.
    // Nothing is written until the dialog's Generate. No default key.
    // #2470: named a board (`document`) and a folder (`path`), it writes what
    // Generate writes there and answers with the files.
    CommandMetadata {
        id: "fabrication_outputs",
        category: "file",
        label: command_ids::CMD_FABRICATION_OUTPUTS,
        menu_label: Some(command_ids::CMD_MENU_FABRICATION_OUTPUTS),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: super::operands::FABRICATION_OUTPUTS,
        ..CommandMetadata::DEFAULT
    },
    // #2404: the board reports. In a window each opens Board Reports on its
    // table; named a board (`document`), each answers with the table.
    // Read-only, no default key.
    CommandMetadata {
        id: "board_report_bom",
        category: "pcb",
        label: command_ids::CMD_BOARD_REPORT_BOM,
        menu_label: Some(command_ids::CMD_MENU_BOARD_REPORT_BOM),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: super::operands::DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "board_report_netlist",
        category: "pcb",
        label: command_ids::CMD_BOARD_REPORT_NETLIST,
        menu_label: Some(command_ids::CMD_MENU_BOARD_REPORT_NETLIST),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: super::operands::DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "board_report_net_length",
        category: "pcb",
        label: command_ids::CMD_BOARD_REPORT_NET_LENGTH,
        menu_label: Some(command_ids::CMD_MENU_BOARD_REPORT_NET_LENGTH),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: super::operands::DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "board_report_components",
        category: "pcb",
        label: command_ids::CMD_BOARD_REPORT_COMPONENTS,
        menu_label: Some(command_ids::CMD_MENU_BOARD_REPORT_COMPONENTS),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: super::operands::DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
    // #2404: the report in view (the BOM with none) as a CSV file; headless
    // it writes `kind` of `document` to `path`.
    CommandMetadata {
        id: "export_board_report",
        category: "reports",
        label: command_ids::CMD_EXPORT_BOARD_REPORT,
        menu_label: Some(command_ids::CMD_MENU_EXPORT_BOARD_REPORT),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: super::operands::BOARD_REPORT_EXPORT,
        ..CommandMetadata::DEFAULT
    },
    // #2402: the Board Print / PDF dialog (File > Print and Export > PDF on
    // a board open it too); headless it writes `document` to `path`.
    CommandMetadata {
        id: "export_board_pdf",
        category: "reports",
        label: command_ids::CMD_EXPORT_BOARD_PDF,
        menu_label: Some(command_ids::CMD_MENU_EXPORT_BOARD_PDF),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: super::operands::BOARD_PDF,
        ..CommandMetadata::DEFAULT
    },
    // #1298, #2609, #2610, #2787: Export PDF on a sheet (File > Export > PDF
    // opens it too); headless it writes `document`'s pages to `path`, with
    // the pages, layers, variant, watermark and links the operands set. No
    // default key.
    CommandMetadata {
        id: "export_schematic_pdf",
        category: "reports",
        label: command_ids::CMD_EXPORT_SCHEMATIC_PDF,
        menu_label: Some(command_ids::CMD_MENU_EXPORT_SCHEMATIC_PDF),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::NONE,
        operands: SCHEMATIC_PDF,
        ..CommandMetadata::DEFAULT
    },
    // #2486: the assembly drawings as SVG files in a folder the person
    // picks; headless it writes `document`'s drawings into `path`.
    CommandMetadata {
        id: "export_assembly_svg",
        category: "reports",
        label: command_ids::CMD_EXPORT_ASSEMBLY_SVG,
        menu_label: Some(command_ids::CMD_MENU_EXPORT_ASSEMBLY_SVG),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: super::operands::ASSEMBLY_SVG_EXPORT,
        ..CommandMetadata::DEFAULT
    },
    // #2486: one assembly drawing of the named board, answered as SVG text;
    // in a window it opens Board Print / PDF. Read-only, no default key.
    CommandMetadata {
        id: "board_assembly_drawing",
        category: "pcb",
        label: command_ids::CMD_BOARD_ASSEMBLY_DRAWING,
        menu_label: Some(command_ids::CMD_MENU_BOARD_ASSEMBLY_DRAWING),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: super::operands::ASSEMBLY_DRAWING,
        ..CommandMetadata::DEFAULT
    },
    // #2488: File > Export > STEP on a board asks where to save the board
    // and its parts as a STEP model; headless it writes `document` to
    // `path`.
    CommandMetadata {
        id: "export_board_step",
        category: "reports",
        label: command_ids::CMD_EXPORT_BOARD_STEP,
        menu_label: Some(command_ids::CMD_MENU_EXPORT_BOARD_STEP),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: super::operands::BOARD_STEP,
        ..CommandMetadata::DEFAULT
    },
    // #1299: the pages of the schematic's project as PNG files in a folder
    // the person picks; headless it writes `document`'s pages into `path`.
    CommandMetadata {
        id: "export_sheet_png",
        category: "reports",
        label: command_ids::CMD_EXPORT_SHEET_PNG,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::NONE,
        operands: super::operands::SHEET_PNG_EXPORT,
        ..CommandMetadata::DEFAULT
    },
    // #1299: an Export Job (`.snxout`) the person picks, or the one `path`
    // names, runs a profile, every enabled output or the one `name` names.
    CommandMetadata {
        id: "run_output_job",
        category: "reports",
        label: command_ids::CMD_RUN_OUTPUT_JOB,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: super::job_operands::OUTPUT_JOB_RUN,
        ..CommandMetadata::DEFAULT
    },
    // Mockup row 114: the Export Job tab's actions, each a command. A new
    // job in its tab, or written at `path`.
    CommandMetadata {
        id: "new_output_job",
        category: "file",
        label: command_ids::CMD_NEW_OUTPUT_JOB,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: super::job_operands::OUTPUT_JOB_PATH_OPTIONAL,
        ..CommandMetadata::DEFAULT
    },
    // View ▸ Panels ▸ Export Jobs: the project's job, or `path`, in its tab.
    CommandMetadata {
        id: "open_output_job",
        category: "panels",
        label: command_ids::CMD_OPEN_OUTPUT_JOB,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: super::job_operands::OUTPUT_JOB_PATH_OPTIONAL,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "read_output_job",
        category: "design",
        label: command_ids::CMD_READ_OUTPUT_JOB,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: super::job_operands::OUTPUT_JOB_PATH,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "add_job_output",
        category: "design",
        label: command_ids::CMD_ADD_JOB_OUTPUT,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: super::job_operands::JOB_OUTPUT_ADD,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "set_job_output",
        category: "design",
        label: command_ids::CMD_SET_JOB_OUTPUT,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: super::job_operands::JOB_OUTPUT_SET,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "set_job_profile",
        category: "design",
        label: command_ids::CMD_SET_JOB_PROFILE,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: super::job_operands::JOB_PROFILE_SET,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "set_job_triggers",
        category: "design",
        label: command_ids::CMD_SET_JOB_TRIGGERS,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: super::job_operands::JOB_TRIGGERS_SET,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "stop_output_job",
        category: "design",
        label: command_ids::CMD_STOP_OUTPUT_JOB,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: super::job_operands::OUTPUT_JOB_PATH,
        ..CommandMetadata::DEFAULT
    },
    // Mockup row 193b: a run's files on the Gerber Viewer.
    CommandMetadata {
        id: "view_output_job_run",
        category: "view",
        label: command_ids::CMD_VIEW_OUTPUT_JOB_RUN,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: super::job_operands::JOB_RUN_VIEW,
        ..CommandMetadata::DEFAULT
    },
    // #1299: a Gerber output's settings, one command each, as the Export
    // Job tab's Gerber fields set them.
    CommandMetadata {
        id: "set_job_gerber_units",
        category: "design",
        label: command_ids::CMD_SET_JOB_GERBER_UNITS,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: super::job_operands::JOB_GERBER_UNITS,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "set_job_gerber_format",
        category: "design",
        label: command_ids::CMD_SET_JOB_GERBER_FORMAT,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: super::job_operands::JOB_GERBER_FORMAT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "set_job_gerber_zeros",
        category: "design",
        label: command_ids::CMD_SET_JOB_GERBER_ZEROS,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: super::job_operands::JOB_GERBER_ZEROS,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "set_job_gerber_layers",
        category: "design",
        label: command_ids::CMD_SET_JOB_GERBER_LAYERS,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: super::job_operands::JOB_GERBER_LAYERS,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "set_job_gerber_apertures",
        category: "design",
        label: command_ids::CMD_SET_JOB_GERBER_APERTURES,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: super::job_operands::JOB_GERBER_APERTURES,
        ..CommandMetadata::DEFAULT
    },
    // #2796, mockup row 171: a block diagram (`.snxblk`) as an SVG or PDF
    // file. In a window it asks for the diagram and the file; headless it
    // writes `document` to `path`. No menu row: the diagram tab's toolbar
    // (mockup row 171) is its home.
    CommandMetadata {
        id: "export_block_diagram",
        category: "reports",
        label: command_ids::CMD_EXPORT_BLOCK_DIAGRAM,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: BLOCK_DIAGRAM_EXPORT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "project_settings",
        category: "design",
        label: command_ids::CMD_PROJECT_SETTINGS,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "project_move_sheet",
        category: "design",
        label: command_ids::CMD_PROJECT_MOVE_SHEET,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: operands::PROJECT_MOVE_SHEET,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "project_add_variant",
        category: "design",
        label: command_ids::CMD_PROJECT_ADD_VARIANT,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: operands::PROJECT_VARIANT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "project_rename_variant",
        category: "design",
        label: command_ids::CMD_PROJECT_RENAME_VARIANT,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: operands::PROJECT_RENAME_VARIANT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "project_remove_variant",
        category: "design",
        label: command_ids::CMD_PROJECT_REMOVE_VARIANT,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: operands::PROJECT_VARIANT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "project_set_active_variant",
        category: "design",
        label: command_ids::CMD_PROJECT_SET_ACTIVE_VARIANT,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: operands::PROJECT_ACTIVE_VARIANT,
        ..CommandMetadata::DEFAULT
    },
    // #2949: the Variant Manager's variant defaults, Compare variants and
    // Open in BOM Studio.
    CommandMetadata {
        id: "project_set_default_variant",
        category: "design",
        label: command_ids::CMD_PROJECT_SET_DEFAULT_VARIANT,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: operands::PROJECT_DEFAULT_VARIANT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "compare_variants",
        category: "design",
        label: command_ids::CMD_COMPARE_VARIANTS,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::COMPARE_VARIANTS,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "variant_bom",
        category: "design",
        label: command_ids::CMD_VARIANT_BOM,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::VARIANT_BOM,
        ..CommandMetadata::DEFAULT
    },
    // #3038: the Variant Manager's part edits and Duplicate, each a General
    // step of the project.
    CommandMetadata {
        id: "project_set_variant_fit",
        category: "design",
        label: command_ids::CMD_PROJECT_SET_VARIANT_FIT,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: variant_operands::PROJECT_VARIANT_FIT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "project_set_variant_alternate",
        category: "design",
        label: command_ids::CMD_PROJECT_SET_VARIANT_ALTERNATE,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: variant_operands::PROJECT_VARIANT_ALTERNATE,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "project_set_variant_override",
        category: "design",
        label: command_ids::CMD_PROJECT_SET_VARIANT_OVERRIDE,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: variant_operands::PROJECT_VARIANT_OVERRIDE,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "project_duplicate_variant",
        category: "design",
        label: command_ids::CMD_PROJECT_DUPLICATE_VARIANT,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: variant_operands::PROJECT_DUPLICATE_VARIANT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "project_move_library",
        category: "design",
        label: command_ids::CMD_PROJECT_MOVE_LIBRARY,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: operands::PROJECT_MOVE_LIBRARY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "project_set_annotate_undo",
        category: "design",
        label: command_ids::CMD_PROJECT_SET_ANNOTATE_UNDO,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: operands::PROJECT_ANNOTATE_UNDO,
        ..CommandMetadata::DEFAULT
    },
    // #1292: the project's annotation numbering scheme.
    CommandMetadata {
        id: "project_set_annotate_numbering",
        category: "design",
        label: command_ids::CMD_PROJECT_SET_ANNOTATE_NUMBERING,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: operands::PROJECT_ANNOTATE_NUMBERING,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "project_set_version_control",
        category: "design",
        label: command_ids::CMD_PROJECT_SET_VERSION_CONTROL,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: operands::PROJECT_VERSION_CONTROL,
        ..CommandMetadata::DEFAULT
    },
    // #2339: the Project tab's Requirements section. An edit is one General
    // step; the run shows its results in the tab (`check_requirements` is the
    // query that answers with the report).
    CommandMetadata {
        id: "project_add_requirement",
        category: "design",
        label: command_ids::CMD_PROJECT_ADD_REQUIREMENT,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: operands::PROJECT_ADD_REQUIREMENT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "project_set_requirement",
        category: "design",
        label: command_ids::CMD_PROJECT_SET_REQUIREMENT,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: operands::PROJECT_SET_REQUIREMENT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "project_remove_requirement",
        category: "design",
        label: command_ids::CMD_PROJECT_REMOVE_REQUIREMENT,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: operands::PROJECT_REQUIREMENT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "project_enable_requirement",
        category: "design",
        label: command_ids::CMD_PROJECT_ENABLE_REQUIREMENT,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: operands::PROJECT_ENABLE_REQUIREMENT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "project_run_requirements",
        category: "validation",
        label: command_ids::CMD_PROJECT_RUN_REQUIREMENTS,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
    // #2943: a power rail's voltage and tolerance, one General step.
    CommandMetadata {
        id: "project_set_rail",
        category: "design",
        label: command_ids::CMD_PROJECT_SET_RAIL,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: operands::PROJECT_SET_RAIL,
        ..CommandMetadata::DEFAULT
    },
];
