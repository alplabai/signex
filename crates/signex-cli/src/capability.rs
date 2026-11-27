//! The capability groups a run may use (#1923): every catalog command needs
//! exactly one, and a run holds `read,edit` unless it names more.

use std::collections::BTreeSet;
use std::fmt;

use serde::Serialize;
use signex_app::keymap::CommandMetadata;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Capability {
    /// Inspect or select; changes no document.
    Read,
    /// Change a document in memory (undoable edits, undo, redo).
    Edit,
    /// Write a document or project to disk, or create one.
    Save,
    /// Write a report or an export file.
    Export,
    /// Remove files. No catalog command does yet; the group exists so the
    /// first one cannot run unnamed.
    Delete,
    /// Change a library.
    LibraryWrite,
}

impl Capability {
    pub const ALL: [Self; 6] = [
        Self::Read,
        Self::Edit,
        Self::Save,
        Self::Export,
        Self::Delete,
        Self::LibraryWrite,
    ];

    /// What a run holds when it names nothing.
    pub const DEFAULT: [Self; 2] = [Self::Read, Self::Edit];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Edit => "edit",
            Self::Save => "save",
            Self::Export => "export",
            Self::Delete => "delete",
            Self::LibraryWrite => "library-write",
        }
    }

    /// Whether a path operand of a command in this group is written to or
    /// removed, rather than read.
    pub const fn writes_files(self) -> bool {
        matches!(
            self,
            Self::Save | Self::Export | Self::Delete | Self::LibraryWrite
        )
    }
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// `read,edit` style list; a name that is not a capability is an error.
pub fn parse_list(text: &str) -> Result<BTreeSet<Capability>, String> {
    text.split(',')
        .map(|name| {
            Capability::ALL
                .into_iter()
                .find(|cap| cap.name() == name.trim())
                .ok_or_else(|| {
                    let all: Vec<_> = Capability::ALL.iter().map(|cap| cap.name()).collect();
                    format!(
                        "`{}` is not a capability (one of {})",
                        name.trim(),
                        all.join(", ")
                    )
                })
        })
        .collect()
}

/// Commands that write a file or create a project. The catalog has no flag
/// for it (a `file` command is not `mutates_doc`), so they are named; a test
/// keeps the list in step with the catalog.
const SAVES: &[&str] = &[
    "save_document",
    "save_document_as",
    // Writes every unsaved document (#2844).
    "save_all",
    // Writes a copy of the project into a new folder (#2908).
    "save_project_as",
    "new_document",
    "new_project_with_profile",
    // Writes the blank sheet and lists it in the saved project (#2232).
    "new_schematic_sheet",
    // Writes the block's file in the project's `reuse-blocks` folder (#2410).
    "save_board_reuse_block",
    // Writes the snippet's file in the project's or the library's `snippets`
    // folder (#1283).
    "save_snippet",
    // Writes the named layer set into the user's preferences (#2394).
    "save_layer_set",
    // Writes the named workspace layout into the user's preferences (mockup row 166).
    "save_workspace_layout",
    // Write an Export Job file: a new one, or an edit of one no tab holds
    // (#1299, mockup row 114).
    "new_output_job",
    "add_job_output",
    "set_job_output",
    "set_job_profile",
    "set_job_triggers",
    "set_job_gerber_units",
    "set_job_gerber_format",
    "set_job_gerber_zeros",
    "set_job_gerber_layers",
    "set_job_gerber_apertures",
    // Writes the design checklist as a template into the user's config
    // folder (#1291, mockup row 206).
    "project_save_checklist_template",
    // Writes the copy of a stock primitive into a library (#3013).
    "copy_stock_to_library",
    // May re-create a library's Git history on disk (#3013).
    "recover_library",
    // Write the project's block diagram file (#2968); a regenerate without
    // `apply` lists its changes and writes nothing.
    "generate_block_diagram",
    "regenerate_block_diagram",
    // Writes the part values into the notebook file (mockup row 199).
    "mark_notebook_attachment_valid",
    // Copies a file into notebook/attachments/ and writes the notebook file
    // (mockup row 199).
    "attach_notebook_file",
    // With `save`, writes notebook/generated/design-report.snxnbk (mockup
    // row 201).
    "generate_design_notebook",
    // Copies the font file into the project's `assets/fonts` folder (#1178).
    "project_add_font",
    // Write a binding into a custom profile of `keyboard_shortcuts.toml` (#2495).
    "keymap_assign",
    "keymap_clear",
];

/// Commands outside the `reports` category that write an export file.
const EXPORTS: &[&str] = &[
    "print",
    // A `file` command (File ▸ Export ▸ Design Notebook PDF…, mockup row
    // 202) that writes the PDF.
    "export_notebook_pdf",
    // Writes the board's fabrication files into a folder (#2470).
    "fabrication_outputs",
];

/// `reports` commands that write no file: they answer with what they read.
const READS: &[&str] = &[
    // Compiles the notebook and answers with its status (mockup row 195).
    "notebook_compile",
];

/// The one capability `command` needs, from its catalog flags.
pub fn capability_of(command: &CommandMetadata) -> Capability {
    if SAVES.contains(&command.id) {
        Capability::Save
    } else if READS.contains(&command.id) {
        Capability::Read
    } else if command.category == "reports" || EXPORTS.contains(&command.id) {
        Capability::Export
    } else if command.category == "library" && command.flags.mutates_doc {
        Capability::LibraryWrite
    } else if command.flags.mutates_doc {
        Capability::Edit
    } else {
        Capability::Read
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_app::keymap::{all_command_ids, metadata_for_str};

    fn cap(id: &str) -> Capability {
        capability_of(metadata_for_str(id).expect("a catalog command"))
    }

    #[test]
    fn only_the_file_writing_groups_write_files() {
        let writers: Vec<_> = Capability::ALL
            .into_iter()
            .filter(|cap| cap.writes_files())
            .collect();
        assert_eq!(
            writers,
            [
                Capability::Save,
                Capability::Export,
                Capability::Delete,
                Capability::LibraryWrite
            ]
        );
    }

    #[test]
    fn every_catalog_command_maps_to_one_named_capability() {
        let mut seen = BTreeSet::new();
        for id in all_command_ids() {
            seen.insert(cap(id));
        }
        // The names round-trip, so `--allow <name>` can ask for each one.
        for found in &seen {
            assert_eq!(parse_list(found.name()).unwrap(), BTreeSet::from([*found]));
        }
        assert!(seen.contains(&Capability::Read) && seen.contains(&Capability::Edit));
    }

    /// An agent's edit list and its revert change a document in memory only:
    /// `edit`, never `save`.
    #[test]
    fn an_agents_edit_list_and_its_revert_need_the_edit_capability() {
        assert_eq!(cap("apply_edits"), Capability::Edit);
        assert_eq!(cap("revert"), Capability::Edit);
    }

    /// #2936, #2937: the footprint edits change a document in memory as one
    /// undo step; the assignment query only reads.
    #[test]
    fn the_footprint_edits_need_edit_and_their_query_read() {
        for id in [
            "set_footprint_position",
            "set_footprint_rotation",
            "set_footprint_side",
            "assign_footprint",
            "change_footprint",
        ] {
            assert_eq!(cap(id), Capability::Edit, "{id}");
        }
        assert_eq!(cap("query_footprint_assignments"), Capability::Read);
    }

    /// #2457, #2941: the rooms commands change the board in memory, one
    /// undo step each; none writes a file.
    #[test]
    fn the_rooms_commands_need_the_edit_capability() {
        for id in [
            "place_board_room",
            "create_rooms_from_sheets",
            "arrange_in_room",
            "auto_place",
        ] {
            assert_eq!(cap(id), Capability::Edit, "{id}");
        }
    }

    /// #2462, #2460: comparing the board with the schematic and reading the
    /// board's status change nothing; applying differences edits the board
    /// and the sheets in memory (a closed sheet is loaded, not written).
    #[test]
    fn the_board_against_schematic_reads_and_its_apply_form_edits() {
        assert_eq!(cap("check_board_against_schematic"), Capability::Read);
        assert_eq!(cap("board_status"), Capability::Read);
        assert_eq!(cap("apply_board_schematic_differences"), Capability::Edit);
    }

    /// #2506: Repour All clears `unfilled` on the board's pours as an undo
    /// step, so a read-only grant may not run it.
    #[test]
    fn repour_all_needs_the_edit_capability() {
        assert_eq!(cap("repour_all"), Capability::Edit);
    }

    /// #3013: a library dialog's command that writes a file needs `save`; one
    /// that changes a project or a sheet in memory needs `edit`; Document
    /// Options, a view setting held for the session, needs `read`.
    #[test]
    fn the_library_dialog_commands_need_their_capability() {
        assert_eq!(cap("copy_stock_to_library"), Capability::Save);
        assert_eq!(cap("recover_library"), Capability::Save);
        assert_eq!(cap("new_library"), Capability::Edit);
        assert_eq!(cap("apply_library_updates"), Capability::Edit);
        assert_eq!(cap("set_library_document_options"), Capability::Read);
    }

    /// #2470: New PCB opens the board unsaved and the numbering commands
    /// change documents in memory: none writes a file, so none needs `save`.
    #[test]
    fn a_new_board_and_the_numbering_commands_need_the_edit_capability() {
        for id in [
            "new_pcb_document",
            "annotate_schematic_quietly",
            "force_annotate_all_schematics",
            "reannotate_board",
            "back_annotate_schematics",
        ] {
            assert_eq!(cap(id), Capability::Edit, "{id}");
        }
    }

    /// #2470: a route or a via from operands is an undo step on the board in
    /// memory, never a write: `edit`.
    #[test]
    fn the_routing_tools_need_the_edit_capability() {
        for id in [
            "place_board_route",
            "place_board_track",
            "place_board_quick_route",
            "place_board_diff_pair",
            "place_board_multi_route",
            "place_board_via",
        ] {
            assert_eq!(cap(id), Capability::Edit, "{id}");
        }
    }

    /// #2470: the area tools add an area to the board in memory: `edit`.
    #[test]
    fn the_area_tools_need_the_edit_capability() {
        for id in [
            "place_board_fill",
            "place_board_region",
            "place_board_polygon_pour",
            "place_board_keepout",
            "place_board_pour_cutout",
        ] {
            assert_eq!(cap(id), Capability::Edit, "{id}");
        }
    }

    /// #2940: a DRC fix edits the board in memory as an undo step, its dry
    /// run included: `edit`, never `save`.
    #[test]
    fn the_drc_fixes_need_the_edit_capability() {
        assert_eq!(cap("fix_drc"), Capability::Edit);
        assert_eq!(cap("fix_drc_violation"), Capability::Edit);
    }

    /// #2942: setting the layout plan is a board undo step, so it needs
    /// `edit`; reading it and its status only read.
    #[test]
    fn the_layout_plan_is_set_with_edit_and_read_with_read() {
        assert_eq!(cap("layout_plan_set"), Capability::Edit);
        assert_eq!(cap("layout_plan_get"), Capability::Read);
        assert_eq!(cap("layout_plan_status"), Capability::Read);
    }

    /// #2938, #1297: the alternates and the swap change a project or a sheet
    /// in memory (`edit`, written only by a save); the priced BOM and the
    /// part search read cached data and write nothing (`read`, not the
    /// `export` of a BOM file).
    #[test]
    fn the_bom_and_part_search_commands_need_edit_or_read() {
        for id in [
            "bom_add_alternate",
            "bom_remove_alternate",
            "bom_set_alternate_order",
            "change_component",
        ] {
            assert_eq!(cap(id), Capability::Edit, "{id}");
        }
        assert_eq!(cap("query_bom"), Capability::Read);
        assert_eq!(cap("search_parts"), Capability::Read);
    }

    #[test]
    fn the_catalogs_writers_and_exporters_are_pinned() {
        let non_default: BTreeSet<_> = all_command_ids()
            .filter(|id| !Capability::DEFAULT.contains(&cap(id)))
            .map(|id| (cap(id), id))
            .collect();
        let pinned = BTreeSet::from([
            (Capability::Save, "new_document"),
            (Capability::Save, "new_project_with_profile"),
            (Capability::Save, "new_schematic_sheet"),
            (Capability::Save, "save_document"),
            (Capability::Save, "save_document_as"),
            (Capability::Save, "save_all"),
            (Capability::Save, "save_project_as"),
            (Capability::Save, "save_board_reuse_block"),
            (Capability::Save, "save_snippet"),
            (Capability::Save, "save_layer_set"),
            (Capability::Save, "save_workspace_layout"),
            (Capability::Save, "project_save_checklist_template"),
            (Capability::Save, "copy_stock_to_library"),
            (Capability::Save, "recover_library"),
            (Capability::Save, "generate_block_diagram"),
            (Capability::Save, "regenerate_block_diagram"),
            (Capability::Save, "mark_notebook_attachment_valid"),
            (Capability::Save, "attach_notebook_file"),
            (Capability::Save, "generate_design_notebook"),
            (Capability::Save, "project_add_font"),
            (Capability::Save, "keymap_assign"),
            (Capability::Save, "keymap_clear"),
            (Capability::Export, "print"),
            (Capability::Export, "report_manager_bom"),
            (Capability::Export, "export_board_report"),
            (Capability::Export, "export_board_pdf"),
            (Capability::Export, "export_schematic_pdf"),
            (Capability::Export, "export_assembly_svg"),
            (Capability::Export, "export_board_step"),
            // #1299: PNG pages, and an Export Job's outputs.
            (Capability::Export, "export_sheet_png"),
            (Capability::Export, "run_output_job"),
            // Mockup row 114: the Export Job edits write its file.
            (Capability::Save, "new_output_job"),
            (Capability::Save, "add_job_output"),
            (Capability::Save, "set_job_output"),
            (Capability::Save, "set_job_profile"),
            (Capability::Save, "set_job_triggers"),
            (Capability::Save, "set_job_gerber_units"),
            (Capability::Save, "set_job_gerber_format"),
            (Capability::Save, "set_job_gerber_zeros"),
            (Capability::Save, "set_job_gerber_layers"),
            (Capability::Save, "set_job_gerber_apertures"),
            (Capability::Export, "project_export_checklist_csv"),
            (Capability::Export, "export_block_diagram"),
            (Capability::Export, "export_notebook_timeline"),
            (Capability::Export, "export_notebook_pdf"),
            (Capability::Export, "fabrication_outputs"),
        ]);
        assert_eq!(non_default, pinned);
    }

    #[test]
    fn every_save_command_is_in_the_catalog_and_none_is_missed() {
        for id in SAVES {
            assert!(metadata_for_str(id).is_some(), "{id} left the catalog");
        }
        for id in all_command_ids().filter(|id| id.contains("save")) {
            assert_eq!(cap(id), Capability::Save, "{id} writes: name it in SAVES");
        }
    }

    #[test]
    fn a_list_names_capabilities_and_refuses_others() {
        assert_eq!(
            parse_list("save, export").unwrap(),
            BTreeSet::from([Capability::Save, Capability::Export])
        );
        let err = parse_list("read,nuke").unwrap_err();
        assert!(err.contains("`nuke` is not a capability"), "{err}");
    }
}
