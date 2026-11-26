//! What one output of an Export Job writes, and the group mockup row 114
//! lists it under. Schematic outputs are built from the project's sheets,
//! board outputs from its board.

use serde::Serialize;

/// The group an output is listed and run under (mockup row 114, "Add
/// Output"): the run flow lights them in this order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputGroup {
    Fabrication,
    Assembly,
    Documentation,
    Reports,
}

impl OutputGroup {
    pub const ALL: [Self; 4] = [
        Self::Fabrication,
        Self::Assembly,
        Self::Documentation,
        Self::Reports,
    ];

    /// The subfolder a profile with one folder per group writes into.
    pub fn folder(self) -> &'static str {
        match self {
            Self::Fabrication => "Fabrication",
            Self::Assembly => "Assembly",
            Self::Documentation => "Documentation",
            Self::Reports => "Reports",
        }
    }
}

/// What one output writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputKind {
    /// Every page of the project as one PDF.
    SchematicPdf,
    /// Every page of the project as a PNG file each.
    SchematicPng,
    /// The bill of materials; CSV, HTML or XLSX by the file's extension.
    Bom,
    /// The `.NET` netlist.
    Netlist,
    /// The board's Gerber X2 layers and their job file.
    Gerber,
    /// The board's drill files.
    NcDrill,
    /// The pick-and-place file.
    PickAndPlace,
    /// The IPC-D-356 test netlist.
    TestPoints,
    /// The IPC-2581 file.
    Ipc2581,
    /// The ODB++ job, one archive or its folder tree.
    Odb,
    /// The board and its parts as a STEP model.
    Step,
    /// The assembly drawings of both sides as one PDF.
    AssemblyPdf,
}

impl OutputKind {
    pub const ALL: [Self; 12] = [
        Self::SchematicPdf,
        Self::SchematicPng,
        Self::Bom,
        Self::Netlist,
        Self::Gerber,
        Self::NcDrill,
        Self::PickAndPlace,
        Self::TestPoints,
        Self::Ipc2581,
        Self::Odb,
        Self::Step,
        Self::AssemblyPdf,
    ];

    /// The id the file and the answers spell.
    pub fn id(self) -> &'static str {
        match self {
            Self::SchematicPdf => "schematic_pdf",
            Self::SchematicPng => "schematic_png",
            Self::Bom => "bom",
            Self::Netlist => "netlist",
            Self::Gerber => "gerber",
            Self::NcDrill => "nc_drill",
            Self::PickAndPlace => "pick_and_place",
            Self::TestPoints => "test_points",
            Self::Ipc2581 => "ipc2581",
            Self::Odb => "odb",
            Self::Step => "step",
            Self::AssemblyPdf => "assembly_pdf",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.id() == id.trim())
    }

    /// The group the output is listed and run under.
    pub fn group(self) -> OutputGroup {
        match self {
            Self::Gerber | Self::NcDrill | Self::TestPoints | Self::Ipc2581 | Self::Odb => {
                OutputGroup::Fabrication
            }
            Self::PickAndPlace | Self::AssemblyPdf => OutputGroup::Assembly,
            Self::SchematicPdf | Self::SchematicPng | Self::Step => OutputGroup::Documentation,
            Self::Bom | Self::Netlist => OutputGroup::Reports,
        }
    }

    /// Whether the output is made from the project's board, not its sheets.
    pub fn is_board(self) -> bool {
        !matches!(
            self,
            Self::SchematicPdf | Self::SchematicPng | Self::Bom | Self::Netlist
        )
    }

    /// Whether `file` names a stem every file of the output starts with
    /// (`<stem>-<layer>.gbr`), rather than one file.
    pub fn file_is_stem(self) -> bool {
        self.is_board()
    }

    /// The file an output of this kind writes when it names none: for a
    /// board output, the stem its files start with.
    pub fn default_file(self, stem: &str) -> String {
        match self {
            Self::SchematicPdf => format!("{stem}.pdf"),
            Self::SchematicPng => format!("{stem}.png"),
            Self::Bom => format!("{stem}-bom.csv"),
            Self::Netlist => format!("{stem}.NET"),
            _ => stem.to_string(),
        }
    }
}
