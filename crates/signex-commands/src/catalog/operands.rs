//! The operands a command takes besides its id (#1934): what a CLI or an agent
//! can pass in place of a pointer click or a dialog answer. One naming
//! convention, kept by a test over every catalog row (the app's
//! `OPERAND_CONVENTION`).

/// The shape of one operand's value, as written in text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperandKind {
    /// `x,y` in mm.
    Point,
    /// A multiple of 90 degrees within ±360, stored as 0, 90, 180 or 270:
    /// the pre-placement form's choices.
    Angle,
    /// Any angle within ±360 degrees, clockwise positive: a turn of the
    /// view, not the pre-placement form's right angles (#2316).
    Turn,
    /// Free text (a label text, a net name, a library id).
    Text,
    /// An absolute path on the host running the command (a GUI file dialog's
    /// answer).
    Path,
    /// `true` or `false`.
    Bool,
    /// An opaque colour, `#rrggbb` (the leading `#` is optional).
    Color,
    /// A document: its id (a hyphenated uuid) or an absolute path (#2172).
    Document,
    /// A length in mm, above zero: a grid step (#2408).
    Length,
    /// The items a command acts on, by uuid, comma separated (#2326): at
    /// least one and at most [`MAX_TARGETS`]; a repeated uuid counts once.
    Targets,
}

/// The most uuids one [`OperandKind::Targets`] operand names. A handler finds
/// each by a scan of the document's items, so this bounds one call's work
/// (and its text, about 370 kB); a person's selection is far smaller.
pub const MAX_TARGETS: usize = 10_000;

impl OperandKind {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Point => "point",
            Self::Angle => "angle",
            Self::Turn => "turn",
            Self::Text => "text",
            Self::Path => "path",
            Self::Bool => "bool",
            Self::Color => "color",
            Self::Document => "document",
            Self::Length => "length",
            Self::Targets => "targets",
        }
    }

    /// The unit the value is written in; `None` for a kind without one.
    pub const fn unit(self) -> Option<&'static str> {
        match self {
            Self::Point | Self::Length => Some("mm"),
            Self::Angle | Self::Turn => Some("deg"),
            Self::Text | Self::Path | Self::Bool | Self::Color | Self::Document | Self::Targets => {
                None
            }
        }
    }
}

/// One named operand of a command. A command's operands are the inputs it
/// needs; everything else (line width, fill, wire mode, label shape, font and
/// justification) comes from the pre-placement form and the project defaults,
/// as for a click.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Operand {
    /// snake_case, one of the convention's names.
    pub name: &'static str,
    pub kind: OperandKind,
    pub required: bool,
}

const fn required(name: &'static str, kind: OperandKind) -> Operand {
    Operand {
        name,
        kind,
        required: true,
    }
}

const fn optional(name: &'static str, kind: OperandKind) -> Operand {
    Operand {
        name,
        kind,
        required: false,
    }
}

// A placement takes an optional `document`: it then runs on that sheet and
// never on the active tab (#2199); without it, on the sheet the edit targets.

/// Place one object at a point.
pub const AT: &[Operand] = &[
    required("at", OperandKind::Point),
    optional("document", OperandKind::Document),
];
/// Draw from one point to another.
pub const SEGMENT: &[Operand] = &[
    required("start", OperandKind::Point),
    required("end", OperandKind::Point),
    optional("document", OperandKind::Document),
];
/// A No-ERC directive (#1699): its point and, comma separated, the ERC rule
/// ids it silences there; left out or empty silences every rule.
pub const NO_ERC: &[Operand] = &[
    required("at", OperandKind::Point),
    optional("rules", OperandKind::Text),
    optional("document", OperandKind::Document),
];
/// A No-ERC rule list edit (#1699): the directive `id` and the rule ids it
/// silences, comma separated; left out or empty (a required text operand
/// cannot be empty) silences every rule.
pub const EDIT_NO_ERC_RULES: &[Operand] = &[
    required("id", OperandKind::Text),
    optional("rules", OperandKind::Text),
    optional("document", OperandKind::Document),
];
/// A Blanket (#2420): its two corners, the net class it gives the nets
/// inside it and the ERC rule ids, comma separated, it does not report there.
pub const BLANKET: &[Operand] = &[
    required("start", OperandKind::Point),
    required("end", OperandKind::Point),
    optional("class", OperandKind::Text),
    optional("exclude", OperandKind::Text),
    optional("document", OperandKind::Document),
];
/// A Blanket edit (#2420): the directive `id` and what to set; an operand
/// left out keeps its value, an empty `class` or `exclude` clears it.
pub const EDIT_BLANKET: &[Operand] = &[
    required("id", OperandKind::Text),
    optional("start", OperandKind::Point),
    optional("end", OperandKind::Point),
    optional("class", OperandKind::Text),
    optional("exclude", OperandKind::Text),
    optional("document", OperandKind::Document),
];
/// A Net Parameters directive (#1489): the point on a wire it sits on, the
/// net class and diff-pair class it gives that net, and its routing rules
/// (lengths in mm, `current` in A, information only).
pub const NET_PARAMETERS: &[Operand] = &[
    required("at", OperandKind::Point),
    optional("class", OperandKind::Text),
    optional("diff_pair_class", OperandKind::Text),
    optional("width", OperandKind::Text),
    optional("clearance", OperandKind::Text),
    optional("via_diameter", OperandKind::Text),
    optional("via_hole", OperandKind::Text),
    optional("current", OperandKind::Text),
    optional("document", OperandKind::Document),
];
/// A wire run, checked against the sheet before it lands (#2199): `allow_merge`
/// lets it join two differently named nets, `allow_open` leaves an end
/// unconnected, `dry_run` reports without changing anything.
pub const WIRE: &[Operand] = &[
    required("start", OperandKind::Point),
    required("end", OperandKind::Point),
    optional("allow_merge", OperandKind::Bool),
    optional("allow_open", OperandKind::Bool),
    optional("dry_run", OperandKind::Bool),
    optional("document", OperandKind::Document),
];
/// A junction at a point: `allow_merge` lets it join two differently named
/// nets (#2199).
pub const JUNCTION: &[Operand] = &[
    required("at", OperandKind::Point),
    optional("allow_merge", OperandKind::Bool),
    optional("document", OperandKind::Document),
];
/// A bus run: as [`WIRE`]; `allow_merge` lets it join two differently named
/// buses.
pub const BUS: &[Operand] = &[
    required("start", OperandKind::Point),
    required("end", OperandKind::Point),
    optional("allow_merge", OperandKind::Bool),
    optional("allow_open", OperandKind::Bool),
    optional("dry_run", OperandKind::Bool),
    optional("document", OperandKind::Document),
];
/// A label: its text at a point. `allow_merge` lets it name a net that already
/// has another name (#2199).
pub const LABEL: &[Operand] = &[
    required("at", OperandKind::Point),
    required("text", OperandKind::Text),
    optional("rotation", OperandKind::Angle),
    optional("allow_merge", OperandKind::Bool),
    optional("document", OperandKind::Document),
];
/// A text note at a point.
pub const TEXT: &[Operand] = &[
    required("at", OperandKind::Point),
    required("text", OperandKind::Text),
    optional("document", OperandKind::Document),
];
/// A power port: the net it names and its library symbol.
pub const POWER: &[Operand] = &[
    required("at", OperandKind::Point),
    required("net_name", OperandKind::Text),
    required("lib_id", OperandKind::Text),
    optional("allow_merge", OperandKind::Bool),
    optional("document", OperandKind::Document),
];
/// A library part (#2199): the row of an open library, where, and which unit;
/// the designator, value, rotation and mirror a click's form would give it.
/// A connection point landing on anything is refused (no `allow_merge`).
pub const PART: &[Operand] = &[
    required("library", OperandKind::Text),
    required("part", OperandKind::Text),
    required("at", OperandKind::Point),
    optional("unit", OperandKind::Text),
    optional("designator", OperandKind::Text),
    optional("value", OperandKind::Text),
    optional("rotation", OperandKind::Angle),
    optional("mirror", OperandKind::Text),
    optional("document", OperandKind::Document),
];
/// A sheet symbol between two corners, referring to its child `file` (relative
/// to the sheet, inside the project folder; never written). `allow_merge` lets
/// the nets the file brings in short or rename project nets (#2199).
pub const SHEET_SYMBOL: &[Operand] = &[
    required("start", OperandKind::Point),
    required("end", OperandKind::Point),
    required("name", OperandKind::Text),
    required("file", OperandKind::Text),
    optional("allow_merge", OperandKind::Bool),
    optional("document", OperandKind::Document),
];
/// An entry on the border of the sheet symbol named `sheet`.
pub const SHEET_ENTRY: &[Operand] = &[
    required("sheet", OperandKind::Text),
    required("at", OperandKind::Point),
    required("name", OperandKind::Text),
    optional("direction", OperandKind::Text),
    optional("document", OperandKind::Document),
];
/// The entries of the sheet symbol named `sheet` matched to its child file.
pub const SYNC_SHEET: &[Operand] = &[
    required("sheet", OperandKind::Text),
    optional("document", OperandKind::Document),
];
/// A sheet symbol for the sheet file `path` with an entry for every port
/// (#1276): its top-left corner at `at`, else under the sheet's content.
pub const SHEET_SYMBOL_FROM_SHEET: &[Operand] = &[
    required("path", OperandKind::Path),
    optional("at", OperandKind::Point),
    optional("allow_merge", OperandKind::Bool),
    optional("document", OperandKind::Document),
];
/// The sync of the sheet symbol `symbol` (an id or a name) and its sheet,
/// `direction` `to_symbol` or `to_sheet` (#1276).
pub const SYNC_SHEET_ENTRIES: &[Operand] = &[
    required("symbol", OperandKind::Text),
    required("direction", OperandKind::Text),
    optional("document", OperandKind::Document),
];
/// An absolute move of the placed object `id` (#2418): `at` is where its
/// position goes (a part's origin, a wire's start, a sheet symbol's top-left).
pub const OBJECT_POSITION: &[Operand] = &[
    required("id", OperandKind::Text),
    required("at", OperandKind::Point),
    optional("document", OperandKind::Document),
];
/// An absolute turn of the part or label `id`.
pub const OBJECT_ROTATION: &[Operand] = &[
    required("id", OperandKind::Text),
    required("rotation", OperandKind::Angle),
    optional("document", OperandKind::Document),
];
/// The width and height of the sheet symbol `id`, written `w,h` in mm.
pub const SHEET_SYMBOL_SIZE: &[Operand] = &[
    required("id", OperandKind::Text),
    required("size", OperandKind::Point),
    optional("document", OperandKind::Document),
];
/// One parameter of the placed part `id`: its `name` and `value` (left out or
/// empty: the parameter stays, with no value).
pub const SYMBOL_PARAMETER: &[Operand] = &[
    required("id", OperandKind::Text),
    required("name", OperandKind::Text),
    optional("value", OperandKind::Text),
    optional("document", OperandKind::Document),
];
/// One sheet variable: its `name` and `value` (left out or empty: the
/// variable stays, with no value), or `remove=true` to delete it.
pub const SHEET_VARIABLE: &[Operand] = &[
    required("name", OperandKind::Text),
    optional("value", OperandKind::Text),
    optional("remove", OperandKind::Bool),
    optional("document", OperandKind::Document),
];
/// The placed part `id` on the sheet `document` (#1667): named, since an
/// agent has no selection.
pub const PART_ON_DOCUMENT: &[Operand] = &[
    required("id", OperandKind::Text),
    required("document", OperandKind::Document),
];
/// The content of the file `path` placed with its centre on `at` (#1667):
/// the dialog's answer and the click.
pub const PLACE_FILE: &[Operand] = &[
    required("path", OperandKind::Path),
    required("at", OperandKind::Point),
    optional("document", OperandKind::Document),
];
/// An open dialog's answer.
pub const OPEN_PATH: &[Operand] = &[required("path", OperandKind::Path)];
/// A save dialog's answer; `overwrite` lets it replace an existing file.
pub const SAVE_PATH: &[Operand] = &[
    required("path", OperandKind::Path),
    optional("overwrite", OperandKind::Bool),
];

/// A new project from the dialog's choices (#2232): where, which profile
/// and `key=value` overrides.
pub const NEW_PROJECT: &[Operand] = &[
    required("name", OperandKind::Text),
    required("location", OperandKind::Path),
    optional("make_folder", OperandKind::Bool),
    optional("profile", OperandKind::Text),
    optional("overrides", OperandKind::Text),
];

/// Update PCB without the dialog (#1312): `apply=true` executes the rows
/// ticked by default.
pub const APPLY: &[Operand] = &[required("apply", OperandKind::Bool)];

/// New PCB without the dialog (#2232): `profile` is `project` (the default,
/// the project's own setup), a built-in token or a saved user profile.
pub const NEW_PCB: &[Operand] = &[optional("profile", OperandKind::Text)];

/// New Schematic without the dialog (#2232): `profile` as for `NEW_PCB`;
/// `name` is the file stem in the project folder (default `Sheet<n>`).
pub const NEW_SCHEMATIC: &[Operand] = &[
    optional("profile", OperandKind::Text),
    optional("name", OperandKind::Text),
];

/// Turn the board view by `degrees` (90 when left out), clockwise positive
/// (#2316).
pub const TURN: &[Operand] = &[optional("degrees", OperandKind::Turn)];

/// One layer of the named board by its name in the board file, e.g.
/// `BottomCopper` (#2394).
pub const LAYER: &[Operand] = &[
    required("layer", OperandKind::Text),
    required("document", OperandKind::Document),
];

/// A kind of layer of the named board: `signal`, `plane`, `overlay`,
/// `mask`, `paste`, `mechanical` or `all` (#2394).
pub const LAYER_GROUP: &[Operand] = &[
    required("group", OperandKind::Text),
    required("document", OperandKind::Document),
];

/// A layer set on the named board: a built-in one (`all`, `signal`,
/// `plane`, `non_signal`, `mechanical`) or a saved one by its name (#2394).
pub const LAYER_SET: &[Operand] = &[
    required("name", OperandKind::Text),
    required("document", OperandKind::Document),
];

/// A saved layer set by its name (#2394): a per-user setting, no board.
pub const LAYER_SET_NAME: &[Operand] = &[required("name", OperandKind::Text)];

/// How the layer tabs name their layers: `short`, `medium` or `long`
/// (#2394).
pub const LAYER_TAB_NAMES: &[Operand] = &[required("style", OperandKind::Text)];

/// One layer of the named board's view shown (`on=true`) or hidden (#2363).
pub const LAYER_VISIBLE: &[Operand] = &[
    required("layer", OperandKind::Text),
    required("on", OperandKind::Bool),
    required("document", OperandKind::Document),
];

/// One layer's colour in the named board's view (#2363); without `color`
/// the board palette's again.
pub const LAYER_COLOR: &[Operand] = &[
    required("layer", OperandKind::Text),
    optional("color", OperandKind::Color),
    required("document", OperandKind::Document),
];

/// A switch of the named board's view (#2363).
pub const ON_BOARD: &[Operand] = &[
    required("on", OperandKind::Bool),
    required("document", OperandKind::Document),
];

/// A kind of object of the named board's view shown or hidden (#2363):
/// `pads`, `vias`, `arcs`, `tracks`, `polygons`, `regions`, `fills`,
/// `texts`, `dimensions`, `keepouts`, or `all`.
pub const OBJECT_VISIBLE: &[Operand] = &[
    required("kind", OperandKind::Text),
    required("on", OperandKind::Bool),
    required("document", OperandKind::Document),
];

/// How transparent a kind of object of the named board's view is drawn
/// (#2363): `value` in percent, 0 to 100; `kind` is `pads`, `vias`, `arcs`,
/// `tracks`, `polygons`, `regions` or `fills`.
pub const OBJECT_TRANSPARENCY: &[Operand] = &[
    required("kind", OperandKind::Text),
    required("value", OperandKind::Text),
    required("document", OperandKind::Document),
];

/// DXF import onto a layer (#2410): the file, the layer token (the active
/// layer when left out) and the board.
pub const IMPORT_DXF: &[Operand] = &[
    required("path", OperandKind::Path),
    optional("layer", OperandKind::Text),
    optional("document", OperandKind::Document),
];
/// DXF import as the board outline (#2410): the file and the board.
pub const IMPORT_DXF_OUTLINE: &[Operand] = &[
    required("path", OperandKind::Path),
    optional("document", OperandKind::Document),
];
/// Length tuning on the named board (#2389): `net` names the net to tune
/// (else the nets of the selected tracks), `value` the target length in mm
/// (else the longest of those nets).
pub const TUNE_LENGTH: &[Operand] = &[
    required("document", OperandKind::Document),
    optional("net", OperandKind::Text),
    optional("value", OperandKind::Text),
];

/// Match a differential pair's lengths on the named board (#2389): `net`
/// names either net of the pair (else the selected track's).
pub const TUNE_PAIR: &[Operand] = &[
    required("document", OperandKind::Document),
    optional("net", OperandKind::Text),
];

/// Colour the net under a point of the named document's sheet.
pub const COLOR_NET: &[Operand] = &[
    required("at", OperandKind::Point),
    required("color", OperandKind::Color),
    required("document", OperandKind::Document),
];
/// The document a command acts on, by id or path. A headless or CLI run names
/// it every time and never falls back to the active one (#2172).
pub const DOCUMENT: &[Operand] = &[required("document", OperandKind::Document)];
/// A sheet edit that takes nothing but the sheet: the one in view, or the one
/// `document` names (a CLI or an agent's edit list).
pub const OPTIONAL_DOCUMENT: &[Operand] = &[optional("document", OperandKind::Document)];
/// A board report written as CSV (#2404): which board, which report
/// (`bom`, `netlist`, `net_length`, `components`) and the file.
pub const BOARD_REPORT_EXPORT: &[Operand] = &[
    required("document", OperandKind::Document),
    required("kind", OperandKind::Text),
    required("path", OperandKind::Path),
];
/// A board printed to PDF (#2402): which board and the file; the rest
/// defaults as the dialog opens. `layers` lists sets by id (`top`,
/// `bottom`, `inner`, `mechanical`, `assembly_top`, `assembly_bottom`),
/// `pages` is `per_set` or `composite`, `scale` is `fit` or `one_to_one`.
pub const BOARD_PDF: &[Operand] = &[
    required("document", OperandKind::Document),
    required("path", OperandKind::Path),
    optional("layers", OperandKind::Text),
    optional("pages", OperandKind::Text),
    optional("scale", OperandKind::Text),
    optional("monochrome", OperandKind::Bool),
    optional("mirror_bottom", OperandKind::Bool),
    optional("outline", OperandKind::Bool),
    optional("drill_marks", OperandKind::Bool),
    optional("title_block", OperandKind::Bool),
];
/// The assembly drawings as SVG files (#2486): which board and the folder
/// they go in. `sides` lists `top` and `bottom` (both when left out).
pub const ASSEMBLY_SVG_EXPORT: &[Operand] = &[
    required("document", OperandKind::Document),
    required("path", OperandKind::Path),
    optional("sides", OperandKind::Text),
    optional("mirror_bottom", OperandKind::Bool),
    optional("monochrome", OperandKind::Bool),
];
/// One assembly drawing as SVG text (#2486): which board and `side`
/// (`top` or `bottom`).
pub const ASSEMBLY_DRAWING: &[Operand] = &[
    required("document", OperandKind::Document),
    required("side", OperandKind::Text),
    optional("mirror_bottom", OperandKind::Bool),
    optional("monochrome", OperandKind::Bool),
];
/// A board written as a STEP model (#2488): which board and the file;
/// `schema` is `ap214` (the default) or `ap242`.
pub const BOARD_STEP: &[Operand] = &[
    required("document", OperandKind::Document),
    required("path", OperandKind::Path),
    optional("schema", OperandKind::Text),
];
/// `add_board_graphic` (#2424): a drawing of `kind` (`line`, `arc`,
/// `circle`, `rectangle`, `text_frame`) on the board `document` names. A
/// line, rectangle or text frame takes `start` and `end`; an arc its centre
/// `at`, `start` and `end`; a circle its centre `at` and a point `end` on it.
/// `layer` is a layer name (`TopSilkscreen`), else the active layer;
/// `text` is a text frame's text.
pub const BOARD_GRAPHIC_ADD: &[Operand] = &[
    required("document", OperandKind::Document),
    required("kind", OperandKind::Text),
    optional("at", OperandKind::Point),
    optional("start", OperandKind::Point),
    optional("end", OperandKind::Point),
    optional("layer", OperandKind::Text),
    optional("text", OperandKind::Text),
];
/// `set_board_graphic` (#2424): one `field` (`layer`, `width`, `start_x`,
/// `start_y`, `end_x`, `end_y`, `center_x`, `center_y`, `radius`, `text`,
/// `text_height`) of the drawing `id` set to `value`; lengths in mm, X and
/// Y from the board origin with Y up, as Properties shows them.
pub const BOARD_GRAPHIC_SET: &[Operand] = &[
    required("document", OperandKind::Document),
    required("id", OperandKind::Text),
    required("field", OperandKind::Text),
    required("value", OperandKind::Text),
];
/// `set_footprint_text` (#2368): one `field` (`x`, `y`, `rotation`,
/// `height`, `justify`) of the `text` (`designator` or `comment`) of the
/// component `designator` set to `value`; X and Y in mm from the board origin
/// with Y up, as Properties shows them, `justify` one of `left`, `center`,
/// `right`.
pub const FOOTPRINT_TEXT_SET: &[Operand] = &[
    required("document", OperandKind::Document),
    required("designator", OperandKind::Text),
    required("text", OperandKind::Text),
    required("field", OperandKind::Text),
    required("value", OperandKind::Text),
];
/// `swap_pins` (#2410): two pins of one component on the board `document`,
/// as `pins` = `a,b` (pad numbers). The component is named by `ids` (its
/// uuid) or by `designator`, exactly one of them (#2326). `swap_schematic_pins`
/// (#1261) reads the same on a sheet: `ids` is a placed unit of the part.
pub const SWAP_PINS: &[Operand] = &[
    required("document", OperandKind::Document),
    optional("ids", OperandKind::Targets),
    optional("designator", OperandKind::Text),
    required("pins", OperandKind::Text),
];
/// `swap_gates` (#2410): two gates of one component, named as for
/// `swap_pins`, as `units` = `a,b` (unit numbers).
pub const SWAP_GATES: &[Operand] = &[
    required("document", OperandKind::Document),
    optional("ids", OperandKind::Targets),
    optional("designator", OperandKind::Text),
    required("units", OperandKind::Text),
];
/// `save_board_reuse_block` (#2410): the block `name` made of `ids`
/// (designators, or footprint, track and via ids, comma separated).
pub const SAVE_REUSE_BLOCK: &[Operand] = &[
    required("document", OperandKind::Document),
    required("name", OperandKind::Text),
    required("ids", OperandKind::Text),
];
/// `place_board_reuse_block` (#2410): the block `name` around one component,
/// named by `ids` (its uuid) or by `designator` (exactly one, #2326), turned
/// by `degrees`.
pub const PLACE_REUSE_BLOCK: &[Operand] = &[
    required("document", OperandKind::Document),
    required("name", OperandKind::Text),
    optional("ids", OperandKind::Targets),
    optional("designator", OperandKind::Text),
    optional("degrees", OperandKind::Turn),
];
/// A save dialog's answer for the named document.
pub const SAVE_DOCUMENT_AS: &[Operand] = &[
    required("document", OperandKind::Document),
    required("path", OperandKind::Path),
    optional("overwrite", OperandKind::Bool),
];
/// `resolve`: the uuid to say what it is.
pub const RESOLVE: &[Operand] = &[required("id", OperandKind::Text)];
/// Which documents: the filters `list_documents` and `find` share.
pub const DOCUMENT_FILTERS: &[Operand] = &[
    optional("kind", OperandKind::Text),
    optional("project", OperandKind::Text),
    optional("name", OperandKind::Text),
    optional("dirty", OperandKind::Bool),
];
/// `find`: documents by the shared filters, or elements by what a person
/// reads on them, in one document or a whole project.
pub const FIND: &[Operand] = &[
    optional("document", OperandKind::Document),
    optional("kind", OperandKind::Text),
    optional("project", OperandKind::Text),
    optional("name", OperandKind::Text),
    optional("dirty", OperandKind::Bool),
    optional("designator", OperandKind::Text),
    optional("value", OperandKind::Text),
    optional("net", OperandKind::Text),
    optional("text", OperandKind::Text),
];
/// `read_document`: the named document's data, narrowed and paged.
pub const READ_DOCUMENT: &[Operand] = &[
    required("document", OperandKind::Document),
    optional("ids", OperandKind::Text),
    optional("kinds", OperandKind::Text),
    optional("region", OperandKind::Text),
    optional("fields", OperandKind::Text),
    optional("offset", OperandKind::Text),
    optional("limit", OperandKind::Text),
];
/// `query_pins`: the pins, with their tip positions, of the part a designator
/// names on one sheet.
pub const QUERY_PINS: &[Operand] = &[
    required("document", OperandKind::Document),
    required("designator", OperandKind::Text),
];
/// `check_erc` (#2199): ERC on the project of the named sheet, answered as a
/// check report; `baseline` is an earlier answer's `run`.
pub const CHECK_ERC: &[Operand] = &[
    required("document", OperandKind::Document),
    optional("baseline", OperandKind::Text),
];
/// `check_drc` (#2199): DRC on the named board, answered as a check report;
/// `baseline` is an earlier answer's `run` on the same board.
pub const CHECK_DRC: &[Operand] = &[
    required("document", OperandKind::Document),
    optional("baseline", OperandKind::Text),
];
/// `check_requirements` (#2339): the requirements of the project the named
/// document is (or belongs to), answered as a check report; `baseline` is an
/// earlier answer's `run` on the same project.
pub const CHECK_REQUIREMENTS: &[Operand] = &[
    required("document", OperandKind::Document),
    optional("baseline", OperandKind::Text),
];
/// Select Net and Highlight Net on a board (#2403): the net by name; without
/// it, the net of the selection or of what is under the cursor.
pub const BOARD_NET: &[Operand] = &[optional("net", OperandKind::Text)];
/// `measure_board` (#2403): point to point from `start` to `end`, or edge to
/// edge between the two items `ids` names.
pub const MEASURE_BOARD: &[Operand] = &[
    required("document", OperandKind::Document),
    optional("start", OperandKind::Point),
    optional("end", OperandKind::Point),
    optional("ids", OperandKind::Targets),
];
/// `query_board_nets` (#2403): every net of the named board, or one.
pub const QUERY_BOARD_NETS: &[Operand] = &[
    required("document", OperandKind::Document),
    optional("net", OperandKind::Text),
];
/// `find_similar_items` (#2403): what agrees with the item `id` on the
/// properties `fields` lists.
pub const FIND_SIMILAR_ITEMS: &[Operand] = &[
    required("document", OperandKind::Document),
    required("id", OperandKind::Text),
    optional("fields", OperandKind::Text),
];
/// Put a net, by its name in the project netlist, in a class of the project
/// the named sheet belongs to; no `class` takes it out of its listed class.
pub const ASSIGN_NET_CLASS: &[Operand] = &[
    required("document", OperandKind::Document),
    required("net_name", OperandKind::Text),
    optional("class", OperandKind::Text),
];
/// `set_selection_filters` (#2417): the schematic categories to leave on, as
/// comma-separated tokens (`wires,net_labels`), or `all` or `none`.
pub const SELECTION_FILTER_KINDS: &[Operand] = &[required("kinds", OperandKind::Text)];
/// `apply_selection_filter_preset` (#2417): a saved preset, by its name.
pub const SELECTION_FILTER_PRESET: &[Operand] = &[required("name", OperandKind::Text)];

/// One finding of the named board's last DRC run, by its waiver key `id`
/// (#2399).
pub const DRC_VIOLATION: &[Operand] = &[
    required("document", OperandKind::Document),
    required("id", OperandKind::Text),
];
/// Waive a finding of the named board's last run; `text` is the note and
/// `author` who accepted it (AV-13).
pub const WAIVE_DRC_VIOLATION: &[Operand] = &[
    required("document", OperandKind::Document),
    required("id", OperandKind::Text),
    optional("text", OperandKind::Text),
    optional("author", OperandKind::Text),
];
/// What the DRC results show: one rule id, one copper layer token, one
/// severity (`error`, `warning`, `info`), and whether waived findings show.
/// A filter left out is cleared; `all` clears one too.
pub const DRC_FILTER: &[Operand] = &[
    required("document", OperandKind::Document),
    optional("rule", OperandKind::Text),
    optional("layer", OperandKind::Text),
    optional("severity", OperandKind::Text),
    optional("waived", OperandKind::Bool),
];
/// A new net class in the named board's project, `name` or the next free
/// `Class <n>` (#2400).
pub const ADD_NET_CLASS: &[Operand] = &[
    required("document", OperandKind::Document),
    optional("name", OperandKind::Text),
];
/// Rename `class` of the named board's project to `name`.
pub const RENAME_NET_CLASS: &[Operand] = &[
    required("document", OperandKind::Document),
    required("class", OperandKind::Text),
    required("name", OperandKind::Text),
];
/// One class of the named board's project.
pub const NET_CLASS: &[Operand] = &[
    required("document", OperandKind::Document),
    required("class", OperandKind::Text),
];
/// The colour and board rules of `class`: lengths in mm, an empty one is
/// unset; one left out keeps its value.
pub const NET_CLASS_RULES: &[Operand] = &[
    required("document", OperandKind::Document),
    required("class", OperandKind::Text),
    optional("color", OperandKind::Color),
    optional("width", OperandKind::Text),
    optional("clearance", OperandKind::Text),
    optional("via_diameter", OperandKind::Text),
    optional("via_drill", OperandKind::Text),
    optional("diff_pair_gap", OperandKind::Text),
    optional("diff_pair_width", OperandKind::Text),
];
/// Put a board net, by name, in `class` of the named board's project (out of
/// every other class's list); no `class` takes it out of its listed class.
pub const ASSIGN_BOARD_NET_CLASS: &[Operand] = &[
    required("document", OperandKind::Document),
    required("net_name", OperandKind::Text),
    optional("class", OperandKind::Text),
];

/// `apply_edits` (#2178): a JSON list of catalog edit commands, applied to the
/// named document as one undo step if its revision is still `expect_revision`.
/// An edit that names another sheet of the project makes the call one group on
/// every sheet it touches; `expect_revisions` then holds each such sheet's
/// revision (a JSON object, document to revision).
pub const APPLY_EDITS: &[Operand] = &[
    required("document", OperandKind::Document),
    required("expect_revision", OperandKind::Text),
    required("edits", OperandKind::Text),
    optional("summary", OperandKind::Text),
    optional("expect_revisions", OperandKind::Text),
];
/// `revert` (#2178): take back the agent step `group` of the named document,
/// while it is that document's newest step.
pub const REVERT: &[Operand] = &[
    required("document", OperandKind::Document),
    required("group", OperandKind::Text),
    optional("documents", OperandKind::Text),
];

// The `rules_*` commands (#2398): `document` is the project or any of its
// documents; `id` a rule's name or `user::<name>`.

/// `rules_list`: the rules, narrowed by kind or on / off.
pub const RULES_LIST: &[Operand] = &[
    required("document", OperandKind::Document),
    optional("kind", OperandKind::Text),
    optional("enabled", OperandKind::Bool),
];
/// `rules_get`, `rules_remove`: one rule.
pub const RULES_ONE: &[Operand] = &[
    required("document", OperandKind::Document),
    required("id", OperandKind::Text),
];
/// `rules_add`: a rule of `kind` (a rule target: clearance, track, via, hole,
/// edge, silk, mask, paste, courtyard) with its limits.
pub const RULES_ADD: &[Operand] = &[
    required("document", OperandKind::Document),
    required("kind", OperandKind::Text),
    required("values", OperandKind::Text),
    optional("scope", OperandKind::Text),
    optional("layer", OperandKind::Text),
    optional("pair", OperandKind::Text),
    optional("severity", OperandKind::Text),
    optional("name", OperandKind::Text),
];
/// `rules_set`: one field of one rule; no `value` clears it.
pub const RULES_SET: &[Operand] = &[
    required("document", OperandKind::Document),
    required("id", OperandKind::Text),
    required("field", OperandKind::Text),
    optional("value", OperandKind::Text),
];
/// `rules_enable`: one rule on or off.
pub const RULES_ENABLE: &[Operand] = &[
    required("document", OperandKind::Document),
    required("id", OperandKind::Text),
    required("on", OperandKind::Bool),
];
/// `rules_effective`: the value that governs a net, and why.
pub const RULES_EFFECTIVE: &[Operand] = &[
    required("document", OperandKind::Document),
    required("net", OperandKind::Text),
    optional("kind", OperandKind::Text),
    optional("layer", OperandKind::Text),
];
/// `rules_explain`: why a finding of the last DRC run asks its value.
pub const RULES_EXPLAIN: &[Operand] = &[
    required("document", OperandKind::Document),
    required("violation", OperandKind::Text),
];
/// `stitch_vias` (#2408): the area as a zone (`id`) or a rectangle (`start`,
/// `end`), the net by name (the zone's when left out) and the grid step
/// (`pitch`, the default step when left out).
pub const STITCH: &[Operand] = &[
    required("document", OperandKind::Document),
    optional("id", OperandKind::Text),
    optional("start", OperandKind::Point),
    optional("end", OperandKind::Point),
    optional("net", OperandKind::Text),
    optional("pitch", OperandKind::Length),
];
/// Add or remove teardrops (#2408): on the net named, or on every net.
pub const TEARDROPS: &[Operand] = &[
    required("document", OperandKind::Document),
    optional("net", OperandKind::Text),
];
/// `arrange_in_box` (#2409): the components and the box's two corners.
pub const ARRANGE_BOX: &[Operand] = &[
    required("document", OperandKind::Document),
    required("ids", OperandKind::Targets),
    required("start", OperandKind::Point),
    required("end", OperandKind::Point),
];
/// `rules_matrix_set`: one class-matrix cell; no `value` clears it.
pub const RULES_MATRIX_SET: &[Operand] = &[
    required("document", OperandKind::Document),
    required("class_a", OperandKind::Text),
    required("class_b", OperandKind::Text),
    optional("object_pair", OperandKind::Text),
    optional("layer", OperandKind::Text),
    optional("value", OperandKind::Text),
];
/// `rules_set_create`: a named rule set of `values`.
pub const RULES_SET_CREATE: &[Operand] = &[
    required("document", OperandKind::Document),
    required("name", OperandKind::Text),
    required("values", OperandKind::Text),
    optional("severity", OperandKind::Text),
];
/// `rules_set_edit`: new limits for a set; its rules follow.
pub const RULES_SET_EDIT: &[Operand] = &[
    required("document", OperandKind::Document),
    required("set", OperandKind::Text),
    required("values", OperandKind::Text),
    optional("severity", OperandKind::Text),
];
/// `rules_set_apply`: a set applied to a scope.
pub const RULES_SET_APPLY: &[Operand] = &[
    required("document", OperandKind::Document),
    required("set", OperandKind::Text),
    required("scope", OperandKind::Text),
];
/// `rules_preset_diff`, `rules_preset_apply`: a tier or rule profile.
pub const RULES_PRESET: &[Operand] = &[
    required("document", OperandKind::Document),
    required("preset", OperandKind::Text),
];
/// `set_board_panel` (#2410): one setting of the named board's panel, by its
/// name (`columns`, `spacing_x`, `separation`, ...) and its value: a whole
/// number, a length in mm, or `mouse_bites` / `v_score`.
pub const SET_BOARD_PANEL: &[Operand] = &[
    required("document", OperandKind::Document),
    required("field", OperandKind::Text),
    required("value", OperandKind::Text),
];

// The `project_*` commands (#1689-#1693): `document` is the project or any
// of its documents.

/// `project_move_sheet`: a sheet file of the project, `up` or `down`.
pub const PROJECT_MOVE_SHEET: &[Operand] = &[
    required("document", OperandKind::Document),
    required("sheet", OperandKind::Text),
    required("direction", OperandKind::Text),
];
/// `project_add_variant`, `project_remove_variant`: a variant name.
pub const PROJECT_VARIANT: &[Operand] = &[
    required("document", OperandKind::Document),
    required("name", OperandKind::Text),
];
/// `project_rename_variant`: the variant `name`, renamed to `value`.
pub const PROJECT_RENAME_VARIANT: &[Operand] = &[
    required("document", OperandKind::Document),
    required("name", OperandKind::Text),
    required("value", OperandKind::Text),
];
/// `project_set_active_variant`: a variant name; none, or `Base`, is Base.
pub const PROJECT_ACTIVE_VARIANT: &[Operand] = &[
    required("document", OperandKind::Document),
    optional("name", OperandKind::Text),
];
/// `project_move_library`: a library as the project stores its path, `up`
/// or `down`.
pub const PROJECT_MOVE_LIBRARY: &[Operand] = &[
    required("document", OperandKind::Document),
    required("library", OperandKind::Text),
    required("direction", OperandKind::Text),
];
/// `project_set_annotate_undo`: `project` or `per_sheet`; none follows
/// Preferences.
pub const PROJECT_ANNOTATE_UNDO: &[Operand] = &[
    required("document", OperandKind::Document),
    optional("value", OperandKind::Text),
];
/// `project_set_version_control`: commit every save, or not.
pub const PROJECT_VERSION_CONTROL: &[Operand] = &[
    required("document", OperandKind::Document),
    required("on", OperandKind::Bool),
];
/// `project_add_requirement` (#2339): a requirement `kind`, and its `id`
/// (the kind's name made unique when none is given).
pub const PROJECT_ADD_REQUIREMENT: &[Operand] = &[
    required("document", OperandKind::Document),
    required("kind", OperandKind::Text),
    optional("id", OperandKind::Text),
];
/// `project_set_requirement`: one `field` of the requirement with id
/// `requirement`, as typed in `value`; empty clears an optional field.
pub const PROJECT_SET_REQUIREMENT: &[Operand] = &[
    required("document", OperandKind::Document),
    required("requirement", OperandKind::Text),
    required("field", OperandKind::Text),
    required("value", OperandKind::Text),
];
/// `project_remove_requirement`: the requirement by id.
pub const PROJECT_REQUIREMENT: &[Operand] = &[
    required("document", OperandKind::Document),
    required("requirement", OperandKind::Text),
];
/// `project_enable_requirement`: the requirement by id, on or off.
pub const PROJECT_ENABLE_REQUIREMENT: &[Operand] = &[
    required("document", OperandKind::Document),
    required("requirement", OperandKind::Text),
    required("on", OperandKind::Bool),
];

/// `set_layer_stack` (#2401): the named board's whole stack, top to bottom,
/// as JSON rows (`{"layers": [...]}` or the list); a copper row names the
/// board layer it was, or none for a new layer.
pub const SET_LAYER_STACK: &[Operand] = &[
    required("document", OperandKind::Document),
    required("stack", OperandKind::Text),
];
/// `lock_board_items` and `unlock_board_items` (#2407), and the board Tools
/// edits (#2408, #2409: the tracks to gloss, the lines to close, the area or
/// lines to make a keepout): items of the named board by uuid, comma
/// separated.
pub const BOARD_ITEMS: &[Operand] = &[
    required("document", OperandKind::Document),
    required("ids", OperandKind::Targets),
];
/// The drawing order commands (#1583) and `delete_selection` (#2326): items
/// of the named sheet (for a delete, of the sheet or board) by uuid, comma
/// separated. The person's menu row, key and palette fill both in from the
/// document in view and its selection.
pub const DOCUMENT_ITEMS: &[Operand] = &[
    required("document", OperandKind::Document),
    required("ids", OperandKind::Targets),
];
/// `set_via_span` (#2407): vias by uuid and the two copper layers they join,
/// `TopCopper,Inner2Copper`.
pub const SET_VIA_SPAN: &[Operand] = &[
    required("document", OperandKind::Document),
    required("ids", OperandKind::Targets),
    required("layers", OperandKind::Text),
];
/// `set_layer_sizes` (#2401): vias or pads by uuid and their per-layer sizes
/// in mm: `simple`, `top=..;middle=..;bottom=..` or `<copper layer>=..;..`,
/// a via size one number, a pad size `<x>x<y>`.
pub const SET_LAYER_SIZES: &[Operand] = &[
    required("document", OperandKind::Document),
    required("ids", OperandKind::Targets),
    required("sizes", OperandKind::Text),
];
/// `set_pad_shape` (#2242): pads by uuid and their `shape` (`circle`, `rect`,
/// `oval`, `roundrect`, `trapezoid` or `custom`); a trapezoid's `delta`
/// `<x>,<y>` and a custom pad's `corners` `<x>,<y>;<x>,<y>;..`, in mm in the
/// pad's own frame.
pub const SET_PAD_SHAPE: &[Operand] = &[
    required("document", OperandKind::Document),
    required("ids", OperandKind::Targets),
    required("shape", OperandKind::Text),
    optional("delta", OperandKind::Text),
    optional("corners", OperandKind::Text),
];
