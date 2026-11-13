//! The catalog and the id registry agree, and every locale on disk is sound.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use fluent_syntax::ast::{Entry, Expression, InlineExpression, Pattern, PatternElement, Resource};
use fluent_syntax::parser;

use crate::catalog::{AVAILABLE, EN_US, EN_US_SOURCES};
use crate::ids;
use crate::{I18n, LanguagePref};

/// message id -> the variables its pattern (and its attributes) read.
type Messages = BTreeMap<String, BTreeSet<String>>;

fn locales_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("locales")
}

fn parse_file(path: &Path) -> Resource<String> {
    let source = std::fs::read_to_string(path).unwrap();
    parser::parse(source)
        .unwrap_or_else(|(_, errors)| panic!("{} does not parse: {errors:?}", path.display()))
}

fn variables_of_pattern(pattern: &Pattern<String>, out: &mut BTreeSet<String>) {
    for element in &pattern.elements {
        if let PatternElement::Placeable { expression } = element {
            variables_of_expression(expression, out);
        }
    }
}

fn variables_of_expression(expression: &Expression<String>, out: &mut BTreeSet<String>) {
    match expression {
        Expression::Inline(inline) => variables_of_inline(inline, out),
        Expression::Select { selector, variants } => {
            variables_of_inline(selector, out);
            for variant in variants {
                variables_of_pattern(&variant.value, out);
            }
        }
    }
}

fn variables_of_inline(inline: &InlineExpression<String>, out: &mut BTreeSet<String>) {
    match inline {
        InlineExpression::VariableReference { id } => {
            out.insert(id.name.clone());
        }
        InlineExpression::Placeable { expression } => variables_of_expression(expression, out),
        InlineExpression::FunctionReference { arguments, .. }
        | InlineExpression::TermReference {
            arguments: Some(arguments),
            ..
        } => {
            for arg in arguments.positional.iter() {
                variables_of_inline(arg, out);
            }
            for named in &arguments.named {
                variables_of_inline(&named.value, out);
            }
        }
        _ => {}
    }
}

/// Every message of every `.ftl` file in `locales/<tag>/`; panics on a
/// parse error or a message id repeated across files.
fn messages_of(tag: &str) -> Messages {
    let mut files: Vec<PathBuf> = std::fs::read_dir(locales_dir().join(tag))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "ftl"))
        .collect();
    files.sort();
    let mut messages = Messages::new();
    for path in files {
        for entry in parse_file(&path).body {
            let Entry::Message(message) = entry else {
                continue;
            };
            let mut vars = BTreeSet::new();
            if let Some(pattern) = &message.value {
                variables_of_pattern(pattern, &mut vars);
            }
            for attribute in &message.attributes {
                variables_of_pattern(&attribute.value, &mut vars);
            }
            let id = message.id.name;
            assert!(
                messages.insert(id.clone(), vars).is_none(),
                "{tag}: message {id} is defined twice"
            );
        }
    }
    messages
}

fn locale_dirs() -> Vec<String> {
    let mut tags: Vec<String> = std::fs::read_dir(locales_dir())
        .unwrap()
        .map(|entry| entry.unwrap())
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    tags.sort();
    tags
}

/// The ids rendered from a built key rather than a constant: one
/// `language-name-<tag>` per locale negotiation may pick.
fn language_name_ids() -> BTreeSet<String> {
    AVAILABLE
        .iter()
        .map(|tag| format!("language-name-{}", tag.to_lowercase()))
        .collect()
}

/// Every defined id, across the id modules.
fn all_ids() -> Vec<crate::MessageId> {
    ids::ALL
        .iter()
        .chain(crate::command_ids::ALL)
        .chain(crate::menu_ids::ALL)
        .chain(crate::shortcut_ids::ALL)
        .copied()
        .chain(crate::area_ids::area_lists())
        .collect()
}

#[test]
fn every_defined_id_exists_in_en_us() {
    let en_us = messages_of(EN_US);
    for id in all_ids() {
        assert!(
            en_us.contains_key(id.as_str()),
            "{} is not in en-US",
            id.as_str()
        );
    }
}

#[test]
fn every_en_us_message_is_a_defined_id() {
    let defined: BTreeSet<String> = all_ids()
        .iter()
        .map(|id| id.as_str().to_owned())
        .chain(language_name_ids())
        .collect();
    for id in messages_of(EN_US).keys() {
        assert!(
            defined.contains(id),
            "en-US message {id} has no id in `ids`"
        );
    }
}

#[test]
fn the_id_registry_has_no_duplicates() {
    let unique: BTreeSet<_> = all_ids().iter().map(|id| id.as_str()).collect();
    assert_eq!(unique.len(), all_ids().len());
}

#[test]
fn every_locale_on_disk_parses_and_its_variables_are_in_the_source() {
    let source = messages_of(EN_US);
    let tags = locale_dirs();
    assert!(tags.iter().any(|tag| tag == EN_US));
    for tag in tags.iter().filter(|tag| *tag != EN_US) {
        for (id, vars) in messages_of(tag) {
            let Some(source_vars) = source.get(&id) else {
                panic!("{tag}: message {id} is not in en-US");
            };
            assert!(
                vars.is_subset(source_vars),
                "{tag}: {id} uses {vars:?}, en-US offers {source_vars:?}"
            );
        }
    }
}

#[test]
fn every_id_renders_from_the_embedded_en_us_catalog() {
    let i18n = I18n::new(&LanguagePref::Locale(EN_US.into()));
    for id in all_ids() {
        let text = i18n.message(id, &[]).to_string();
        assert_ne!(
            text,
            id.as_str(),
            "{} is not in the embedded catalog",
            id.as_str()
        );
    }
    // Ids render with no arguments, so a message with a `{ $var }` logs a
    // format problem; only a fall-back or a missing id is a failure here.
    assert_eq!(
        i18n.lookup_problems_for_test(),
        0,
        "an id fell back or is missing"
    );
}

#[test]
fn a_message_with_a_variable_is_not_a_lookup_problem() {
    let i18n = I18n::en_us_from_sources_for_test(&["with-var = Zoom { $percent }\n"]);
    let id = crate::MessageId::new("with-var");
    let _ = i18n.message(id, &[]);
    assert_eq!(i18n.emitted_for_test(), 1, "the format problem is logged");
    assert_eq!(i18n.lookup_problems_for_test(), 0);
    let _ = i18n.message(crate::MessageId::new("absent"), &[]);
    assert_eq!(i18n.lookup_problems_for_test(), 1);
}

#[test]
fn every_en_us_file_on_disk_is_embedded() {
    let dir = locales_dir().join(EN_US);
    let mut on_disk = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|ext| ext != "ftl") {
            continue;
        }
        on_disk += 1;
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            EN_US_SOURCES.contains(&text.as_str()),
            "{} is not listed in EN_US_SOURCES",
            path.display()
        );
    }
    assert_eq!(
        on_disk,
        EN_US_SOURCES.len(),
        "EN_US_SOURCES lists a file twice or one that is gone"
    );
}

/// #2225: catalog text moved out of the views keeps its exact English. These
/// three once carried an extraction note after the value (#2298 review).
#[test]
fn moved_graphite_strings_keep_their_english() {
    let i18n = I18n::new(&LanguagePref::Locale(EN_US.into()));
    let plain = |s: String| s.replace(['\u{2068}', '\u{2069}'], "");
    assert_eq!(
        plain(
            i18n.message(ids::EDITORS_COMPONENT_UNSAVED_MARKER, &[])
                .to_string()
        ),
        "\u{2022} unsaved"
    );
    assert_eq!(
        plain(
            i18n.message(
                ids::LIBRARY_RANGE_FILTER_EXCLUDED,
                &[("count", 3usize.into()), ("unit", "Ω".into())]
            )
            .to_string()
        ),
        "3 excluded: not a measurement in Ω."
    );
    assert_eq!(
        plain(
            i18n.message(ids::LIBRARY_PREVIEW_OVERRIDE_PIN, &[("pin", "1".into())])
                .to_string()
        ),
        "Override pin 1 \u{2192}"
    );
    assert_eq!(
        plain(
            i18n.message(ids::PANELS_STATUS_BAR_SNAP_ON, &[])
                .to_string()
        ),
        "Snap on"
    );
    assert_eq!(
        plain(
            i18n.message(ids::PANELS_STATUS_BAR_SNAP_OFF, &[])
                .to_string()
        ),
        "Snap off"
    );
}

/// #2225 part 1, second wave: the strings moved out of the views #2291 had
/// open keep their English, plural and spacing included.
#[test]
fn moved_graphite_strings_of_the_second_wave_keep_their_english() {
    let i18n = I18n::new(&LanguagePref::Locale(EN_US.into()));
    let shown = |id, args: &[(&str, crate::Arg)]| {
        i18n.message(id, args)
            .to_string()
            .replace(['\u{2068}', '\u{2069}'], "")
    };
    let waiver = |hidden: usize| {
        shown(
            ids::PANELS_ERC_WAIVER_ROW,
            &[
                ("rule", "E101".into()),
                ("net", "GND".into()),
                ("hidden", hidden.into()),
            ],
        )
    };
    assert_eq!(waiver(0), "E101  net GND  (matches no finding)");
    assert_eq!(waiver(1), "E101  net GND  (1 finding hidden)");
    assert_eq!(waiver(3), "E101  net GND  (3 findings hidden)");
    let breadcrumb = |count: usize| {
        shown(
            ids::PANELS_FOOTPRINT_LIBRARY_BREADCRUMB,
            &[("file", "SOIC-8".into()), ("count", count.into())],
        )
    };
    assert_eq!(breadcrumb(1), "SOIC-8  (1 footprint)");
    assert_eq!(breadcrumb(0), "SOIC-8  (0 footprints)");
    assert_eq!(breadcrumb(2), "SOIC-8  (2 footprints)");
    assert_eq!(
        shown(
            ids::PANELS_SCH_LIBRARY_BREADCRUMB_IN_LIBRARY,
            &[
                ("library", "passives".into()),
                ("file", "R.snxsym".into()),
                ("count", 2usize.into()),
            ],
        ),
        "passives  \u{203A}  R.snxsym  (2 symbols)"
    );
    assert_eq!(
        shown(
            ids::PANELS_LIBRARY_ROW_SUMMARY,
            &[
                ("table", "Resistors".into()),
                ("class", "Resistor".into()),
                ("lifecycle", "Draft".into()),
                ("row", "c0ffee".into()),
            ],
        ),
        "table: Resistors  \u{00B7}  class: Resistor  \u{00B7}  Draft  \u{00B7}  row c0ffee"
    );
    assert_eq!(
        shown(
            ids::OVERLAY_PALETTE_MORE_RESULTS,
            &[("count", 181usize.into())]
        ),
        "181 more results \u{2014} refine query"
    );
    assert_eq!(
        shown(ids::PANELS_ERC_COUNT_ERRORS, &[("count", 2usize.into())]),
        "2 errors"
    );
    assert_eq!(
        shown(
            ids::PREFERENCES_APPEARANCE_ZOOM_STEP_VALUE,
            &[("percent", 15u32.into())]
        ),
        "15% per notch"
    );
    assert_eq!(
        shown(
            ids::DIALOG_ERC_PROFILE_UPDATE_HEADING,
            &[("name", "Strict".into()), ("count", 4usize.into())]
        ),
        "Update from profile \"Strict\" changes 4 entries:"
    );
    assert_eq!(
        shown(ids::PANELS_FOOTPRINT_LIBRARY_EMPTY, &[]),
        "Open a `.snxfpt` to see its footprints here. Right-click a `.snxlib` (or a project) \
         in the project tree and pick `Add New \u{25B8} Footprint Library` to create one."
    );
    assert_eq!(shown(ids::OVERLAY_PALETTE_GROUP_COMMANDS, &[]), "COMMANDS");
}

/// #2225 part 1, third wave: the labels helpers used to take as `&str` keep
/// their English, plurals and placeables included.
#[test]
fn moved_graphite_strings_of_the_third_wave_keep_their_english() {
    let i18n = I18n::new(&LanguagePref::Locale(EN_US.into()));
    let shown = |id, args: &[(&str, crate::Arg)]| {
        i18n.message(id, args)
            .to_string()
            .replace(['\u{2068}', '\u{2069}'], "")
    };
    for (id, english) in [
        (ids::PANELS_DRAWING_START_X, "Start X"),
        (ids::PANELS_DRAWING_END_Y, "End Y"),
        (ids::PANELS_DRAWING_POSITION_X, "Position X"),
        (ids::PANELS_DRAWING_CENTER_Y, "Center Y"),
        (ids::PANELS_DRAWING_WIDTH, "Width"),
        (ids::PANELS_DRAWING_WIDTH_MM, "Width (mm)"),
        (ids::PANELS_DRAWING_HEIGHT_MM, "Height (mm)"),
        (ids::PANELS_DRAWING_BORDER, "Border"),
        (ids::PANELS_DRAWING_RADIUS, "Radius"),
        (ids::PANELS_DRAWING_START_ANGLE, "Start Angle"),
        (ids::PANELS_DRAWING_COLOR_AUTO, "Auto"),
        (ids::EDITORS_PAD_ROTATION, "Rotation (\u{B0})"),
        (ids::EDITORS_PAD_TEMPLATE_LIBRARY, "Library"),
        (ids::EDITORS_PAD_TOP_SIDE, "Top Side"),
        (ids::EDITORS_PAD_BOTTOM_FAB, "Bottom Fab"),
        (ids::EDITORS_PAD_SHAPE, "Shape"),
        (ids::EDITORS_PAD_SIZE_X, "X-Size"),
        (ids::EDITORS_PAD_SIZE_Y, "Y-Size"),
        (ids::EDITORS_PAD_RELIEF, "Relief"),
        (ids::EDITORS_PAD_CORNER_RADIUS, "Corner radius %"),
        (ids::EDITORS_PAD_HOLE_TOLERANCE_PLUS, "Tolerance + (mm)"),
        (
            ids::EDITORS_PAD_HOLE_TOLERANCE_MINUS,
            "Tolerance \u{2212} (mm)",
        ),
        (ids::EDITORS_PAD_COPPER_OFFSET_Y, "Copper offset Y (mm)"),
        (ids::EDITORS_PAD_BOTTOM_MASK_TENTED, "Bottom mask tented"),
        (ids::EDITORS_SILK_STROKE_WIDTH, "Stroke width (mm)"),
        (
            ids::EDITORS_SILK_USE_SKETCH,
            "Use Sketch mode for parametric editing",
        ),
        (
            ids::EDITORS_PATTERN_SKIP_LETTERS,
            "Skip I/O/Q/S/X/Z (IPC-7351)",
        ),
        (
            ids::EDITORS_COMPONENT_ALTERNATES_EMPTY,
            "No alternate drawings; the row uses its default symbol only.",
        ),
        (
            ids::EDITORS_COMPONENT_PIN_MAP_NO_PINS,
            "This symbol has no pins yet. Open the Symbol Editor to add some.",
        ),
        (ids::EDITORS_COMPONENT_PARTS_EMPTY, "No alternates yet."),
        (ids::OVERLAY_SYMBOL_CARD_DESIGNATOR, "Designator"),
        (ids::OVERLAY_SYMBOL_CARD_LIBRARY, "Library"),
        (
            ids::PREFERENCES_SECTION_ANNOTATION,
            "Schematic \u{2014} Annotation",
        ),
        (
            ids::PREFERENCES_SECTION_DISTRIBUTORS,
            "Library \u{2014} Distributor APIs",
        ),
        (ids::LIBRARY_ROW_NO_SYMBOL, "No symbol bound"),
        (
            ids::LIBRARY_ROW_SYMBOL_ABSENT,
            "Symbol unresolved (UUID not in mounted libraries)",
        ),
        (ids::LIBRARY_ROW_SYMBOL_READING, "Reading symbol\u{2026}"),
        (ids::LIBRARY_ROW_FOOTPRINT_BOUND, "Footprint bound"),
    ] {
        assert_eq!(shown(id, &[]), english, "{}", id.as_str());
    }
    let pins = |count: usize| shown(ids::LIBRARY_ROW_SYMBOL_PINS, &[("pins", count.into())]);
    assert_eq!(pins(0), "Symbol bound \u{2014} 0 pins");
    assert_eq!(pins(1), "Symbol bound \u{2014} 1 pin");
    assert_eq!(pins(4), "Symbol bound \u{2014} 4 pins");
    // Counts are grouped like every catalog count (#2291): 1,156, not 1156.
    assert_eq!(pins(1156), "Symbol bound \u{2014} 1,156 pins");
    let alternates = |count: usize| {
        shown(
            ids::LIBRARY_ROW_ALTERNATE_SYMBOLS,
            &[
                ("summary", "Symbol bound \u{2014} 4 pins".into()),
                ("count", count.into()),
            ],
        )
    };
    assert_eq!(
        alternates(1),
        "Symbol bound \u{2014} 4 pins (+1 alternate symbol)"
    );
    assert_eq!(
        alternates(3),
        "Symbol bound \u{2014} 4 pins (+3 alternate symbols)"
    );
    assert_eq!(
        shown(
            ids::LIBRARY_ROW_FOOTPRINT_FAILED,
            &[("error", "timed out".into())]
        ),
        "Footprint could not be read from its library \u{2014} timed out"
    );
    let delete = |id| shown(id, &[("name", "R".into())]);
    assert_eq!(
        delete(ids::LIBRARY_SYMBOL_DELETE_CHECKING),
        "Checking whether symbol \"R\" is in use"
    );
    assert_eq!(
        delete(ids::LIBRARY_SYMBOL_DELETE_MAY_BE_IN_USE),
        "Symbol \"R\" may still be in use"
    );
    assert_eq!(
        delete(ids::LIBRARY_SYMBOL_DELETE_IN_USE),
        "Symbol \"R\" is still in use"
    );
}

/// #2225 part 1, last wave: the dialog button labels keep their English,
/// ellipses and the computed Update count included.
#[test]
fn moved_dialog_button_labels_keep_their_english() {
    let i18n = I18n::new(&LanguagePref::Locale(EN_US.into()));
    let shown = |id, args: &[(&str, crate::Arg)]| {
        i18n.message(id, args)
            .to_string()
            .replace(['\u{2068}', '\u{2069}'], "")
    };
    for (id, english) in [
        (ids::DIALOG_ANNOTATE_REPREFIX, "Re-prefix to standard"),
        (ids::DIALOG_ANNOTATE_RESET_ALL, "Reset All"),
        (ids::DIALOG_ANNOTATE_RESET_RENUMBER, "Reset & Renumber"),
        (ids::DIALOG_ANNOTATE_ACCEPT, "Accept Changes"),
        (ids::DIALOG_AUTOSAVE_RECOVERY_RESTORE, "Restore"),
        (ids::DIALOG_AUTOSAVE_RECOVERY_LATER, "Decide later"),
        (ids::DIALOG_BOM_EXPORT, "Export\u{2026}"),
        (ids::DIALOG_UNSAVED_DISCARD_ALL, "Discard All"),
        (ids::DIALOG_CREATE_CHILD_SHEET_OPEN_EXISTING, "Open it"),
        (ids::DIALOG_CREATE_CHILD_SHEET_BROWSE, "Browse..."),
        (ids::DIALOG_CREATE_CHILD_SHEET_LATER, "Later"),
        (
            ids::DIALOG_ERC_STORE_RULES,
            "Store ERC rules in this project",
        ),
        (ids::DIALOG_ERC_SAVE_AS_PROFILE, "Save as profile"),
        (ids::DIALOG_ERC_UPDATE_FROM_PROFILE, "Update from profile"),
        (ids::DIALOG_ERC_RUN, "Run ERC"),
        (ids::DIALOG_ERC_APPLY_UPDATE, "Apply update"),
        (ids::DIALOG_OVERWRITE, "Overwrite"),
        (ids::DIALOG_RENAME_SUBMIT, "Rename"),
        (ids::DIALOG_VERSION_CONTROL_ENABLE, "Enable"),
        (ids::DIALOG_VERSION_CONTROL_ENABLING, "Enabling\u{2026}"),
        (ids::DIALOG_NET_COLOR_USE, "Use Color"),
        (ids::DIALOG_PARAMETER_MANAGER_ADD_COLUMN, "Add column"),
        (ids::DIALOG_DISK_CONFLICT_RELOAD, "Reload from disk"),
        (ids::DIALOG_DISK_CONFLICT_RECREATE, "Re-create"),
        (
            ids::DIALOG_NETLIST_EXPORT_ANYWAY,
            "Export anyway (incomplete)",
        ),
        (ids::DIALOG_PRINT_PREVIEW_EXPORT_PDF, "Export PDF"),
        (ids::OVERLAY_PLACEMENT_PAUSED_RESUME, "Resume Placement"),
        (ids::PREFERENCES_RESTORE_DEFAULTS, "Restore defaults"),
        (ids::LIBRARY_CREATE_OPTIONS_CREATE, "Create Library"),
        (ids::LIBRARY_EDIT_ROW_RELOAD, "Reload"),
        (ids::LIBRARY_EDIT_ROW_KEEP_EDITING, "Keep editing"),
        (ids::LIBRARY_EDIT_ROW_RELOAD_SAVED, "Reload saved version"),
        (ids::LIBRARY_PICKER_NEXT, "Next"),
        (
            ids::LIBRARY_PRIMITIVE_PICKER_BROWSE,
            "Browse filesystem\u{2026}",
        ),
        (ids::LIBRARY_PRIMITIVE_PICKER_OPEN_LIBRARY, "Open library"),
        (ids::LIBRARY_PRIMITIVE_PICKER_DONT_COPY, "Don't copy"),
        (
            ids::LIBRARY_RECOVERY_REMOVE_FROM_PROJECT,
            "Remove from project",
        ),
        (ids::LIBRARY_RECOVERY_LOCATE, "Locate\u{2026}"),
        (ids::LIBRARY_RECOVERY_RESTORE_REMOTE, "Restore from remote"),
        (ids::LIBRARY_RECOVERY_SKIP_READ_ONLY, "Skip (read-only)"),
        (ids::LIBRARY_RECOVERY_REINIT, "Re-init (lose history)"),
        (ids::LIBRARY_RECOVERY_REMOVE_ROW, "Remove row"),
        (ids::LIBRARY_RECOVERY_REBIND, "Re-bind\u{2026}"),
        (ids::LIBRARY_DISTRIBUTOR_CONNECT_OAUTH, "Connect via OAuth"),
        (ids::LIBRARY_DISTRIBUTOR_TEST, "Test"),
        (ids::LIBRARY_STOCK_COPY_NEW_LIBRARY, "New Library\u{2026}"),
        (ids::LIBRARY_UPDATES_SKIP_ALL, "Skip All"),
        (ids::LIBRARY_CHANGE_PART_CONFIRM, "Change"),
        (ids::LIBRARY_CHANGE_PART_CONFIRM_ANYWAY, "Change anyway"),
        // Shared by dialog footer buttons.
        (ids::DIALOG_BUTTON_CANCEL, "Cancel"),
        (ids::DIALOG_BUTTON_OK, "OK"),
        (ids::DIALOG_BUTTON_APPLY, "Apply"),
        (ids::DIALOG_BUTTON_CLOSE, "Close"),
        (ids::DIALOG_BUTTON_SAVE, "Save"),
        (ids::DIALOG_BUTTON_SAVE_ALL, "Save All"),
        (ids::DIALOG_BUTTON_DISCARD, "Discard"),
        (ids::DIALOG_BUTTON_DELETE, "Delete"),
        (ids::DIALOG_BUTTON_CREATE, "Create"),
        (ids::DIALOG_BUTTON_BACK, "Back"),
        (ids::DIALOG_BUTTON_COPY, "Copy"),
        (ids::DIALOG_BUTTON_UNDO, "Undo"),
        (ids::DIALOG_BUTTON_REDO, "Redo"),
        (ids::DIALOG_BUTTON_PLACE, "Place"),
        // Reused: the same action under its own surface.
        (ids::EDITORS_SYMBOL_COPY_AGAIN, "Copy Again"),
        (ids::PANELS_LIBRARY_ROW_PICK_SYMBOL, "Pick Symbol\u{2026}"),
        (
            ids::PANELS_LIBRARY_ROW_PICK_FOOTPRINT,
            "Pick Footprint\u{2026}",
        ),
    ] {
        assert_eq!(shown(id, &[]), english, "{}", id.as_str());
    }
    // Counts are grouped like every catalog count (#2291): 1,156, not 1156.
    for (count, english) in [
        (0_usize, "Update Selected Components (0)"),
        (3, "Update Selected Components (3)"),
        (1156, "Update Selected Components (1,156)"),
    ] {
        assert_eq!(
            shown(ids::LIBRARY_UPDATES_APPLY, &[("count", count.into())]),
            english
        );
    }
    // The re-prefix count keeps "part(s)" for one part and many.
    for (count, english) in [
        (1_usize, "Re-prefix 1 part(s)"),
        (4, "Re-prefix 4 part(s)"),
        (1156, "Re-prefix 1,156 part(s)"),
    ] {
        assert_eq!(
            shown(
                ids::DIALOG_ANNOTATE_REPREFIX_PARTS,
                &[("count", count.into())]
            ),
            english
        );
    }
}

/// #2262: the Pad Properties header of a selected pad, singular and plural.
#[test]
fn the_pad_heading_names_the_pad_and_its_place_among_the_pads() {
    let i18n = I18n::new(&LanguagePref::Locale(EN_US.into()));
    let heading = |index: usize, pads: usize| {
        i18n.message(
            ids::EDITORS_FOOTPRINT_PAD_HEADING,
            &[
                ("number", "11".into()),
                ("kind", "SMD".into()),
                ("index", index.into()),
                ("pads", pads.into()),
            ],
        )
        .to_string()
        .replace(['\u{2068}', '\u{2069}'], "")
    };
    assert_eq!(heading(1, 11), "Pad 11 \u{00B7} SMD \u{00B7} 1 of 11 pads");
    assert_eq!(heading(1, 1), "Pad 11 \u{00B7} SMD \u{00B7} 1 of 1 pad");
    // The pad kinds keep the English the header showed as data before.
    for (id, english) in [
        (ids::EDITORS_PAD_KIND_SMD, "SMD"),
        (ids::EDITORS_PAD_KIND_THROUGH_HOLE, "Through-hole"),
        (ids::EDITORS_PAD_KIND_NPT_HOLE, "NPT hole"),
        (ids::EDITORS_PAD_KIND_CONNECTOR, "Connector"),
        (ids::EDITORS_PAD_KIND_CASTELLATED, "Castellated"),
        (ids::EDITORS_PAD_KIND_FIDUCIAL, "Fiducial"),
        (ids::EDITORS_PAD_KIND_UNKNOWN, "Unknown"),
    ] {
        assert_eq!(i18n.message(id, &[]).to_string(), english);
    }
}
