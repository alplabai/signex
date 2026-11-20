//! The rule-type catalog of the Rules constraint grid (#2530): the domain
//! tabs, the column groups of each domain and their columns, in grid
//! order. Every limit the check reads ([`LimitField`]) is one column; a
//! column with no field is shown but not checked yet. Ids only: the app
//! owns the labels. See `docs/DRC_RULES.md`, "Rules manager".

use super::LimitField;
use super::matrix::OBJECT_PAIRS;

/// One domain tab of the grid, in tab order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Domain {
    Clearance,
    Physical,
    HighSpeed,
    Manufacturing,
    Smt,
    Mask,
    Plane,
    Testpoint,
    Placement,
    SignalIntegrity,
}

impl Domain {
    pub const ALL: [Domain; 10] = [
        Domain::Clearance,
        Domain::Physical,
        Domain::HighSpeed,
        Domain::Manufacturing,
        Domain::Smt,
        Domain::Mask,
        Domain::Plane,
        Domain::Testpoint,
        Domain::Placement,
        Domain::SignalIntegrity,
    ];

    /// Stable id.
    pub const fn as_str(self) -> &'static str {
        match self {
            Domain::Clearance => "clearance",
            Domain::Physical => "physical",
            Domain::HighSpeed => "high_speed",
            Domain::Manufacturing => "manufacturing",
            Domain::Smt => "smt",
            Domain::Mask => "mask",
            Domain::Plane => "plane",
            Domain::Testpoint => "testpoint",
            Domain::Placement => "placement",
            Domain::SignalIntegrity => "signal_integrity",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|d| d.as_str() == id)
    }

    /// Its column groups, in grid order.
    pub fn groups(self) -> impl Iterator<Item = &'static CatalogGroup> {
        catalog().iter().filter(move |g| g.domain == self)
    }
}

/// What a column's value is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColumnUnit {
    /// A length (`0.2mm`, `8mil`).
    Mm,
    Percent,
    Degrees,
    Count,
    /// One of a list of words.
    Choice,
}

/// One column of a group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CatalogColumn {
    /// Stable id, unique in the catalog.
    pub id: &'static str,
    /// The limit it shows; `None` for a column not checked yet.
    pub field: Option<LimitField>,
    pub unit: ColumnUnit,
}

impl CatalogColumn {
    /// Whether the check reads it.
    pub const fn is_checked(&self) -> bool {
        self.field.is_some()
    }
}

/// One column group (a rule type) of a domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CatalogGroup {
    /// Stable id, unique in the catalog.
    pub id: &'static str,
    pub domain: Domain,
    pub columns: &'static [CatalogColumn],
}

const fn col(id: &'static str, field: LimitField, unit: ColumnUnit) -> CatalogColumn {
    CatalogColumn {
        id,
        field: Some(field),
        unit,
    }
}

const fn unchecked(id: &'static str, unit: ColumnUnit) -> CatalogColumn {
    CatalogColumn {
        id,
        field: None,
        unit,
    }
}

/// A struct literal, so the column array is `'static` in the const.
macro_rules! group {
    ($id:expr, $domain:expr, $columns:expr $(,)?) => {
        CatalogGroup {
            id: $id,
            domain: $domain,
            columns: $columns,
        }
    };
}

const CATALOG: &[CatalogGroup] = {
    use ColumnUnit::{Choice, Count, Degrees, Mm, Percent};
    use Domain as D;
    use LimitField as F;
    &[
        // Clearance
        group!(
            "clearance",
            D::Clearance,
            &[col("clearance_min", F::MinClearance, Mm)],
        ),
        group!("creepage", D::Clearance, &[unchecked("creepage_min", Mm)]),
        group!(
            "short_circuit",
            D::Clearance,
            &[unchecked("short_circuit_allowed", Choice)],
        ),
        group!(
            "unrouted_net",
            D::Clearance,
            &[unchecked("unrouted_net_check", Choice)],
        ),
        group!(
            "unconnected_pin",
            D::Clearance,
            &[unchecked("unconnected_pin_check", Choice)],
        ),
        group!(
            "modified_pour",
            D::Clearance,
            &[unchecked("modified_pour_check", Choice)],
        ),
        // Physical
        group!(
            "width",
            D::Physical,
            &[
                col("width_min", F::MinWidth, Mm),
                unchecked("width_preferred", Mm),
                col("width_max", F::MaxWidth, Mm),
            ],
        ),
        group!(
            "via_style",
            D::Physical,
            &[
                col("via_diameter", F::MinDiameter, Mm),
                col("via_hole", F::MinDrill, Mm),
                col("via_pad_gap", F::MinPadGap, Mm),
                unchecked("via_finish", Choice),
            ],
        ),
        group!(
            "diff_pair",
            D::Physical,
            &[
                unchecked("diff_pair_width", Mm),
                unchecked("diff_pair_gap", Mm),
                unchecked("diff_pair_max_uncoupled", Mm),
            ],
        ),
        group!(
            "neck_down",
            D::Physical,
            &[unchecked("neck_down_min_of_pad", Percent)],
        ),
        group!(
            "corners",
            D::Physical,
            &[
                unchecked("corners_style", Choice),
                unchecked("corners_setback", Mm),
            ],
        ),
        group!(
            "routing",
            D::Physical,
            &[
                unchecked("routing_topology", Choice),
                unchecked("routing_priority", Count),
                unchecked("routing_layers", Choice),
                unchecked("routing_fanout", Choice),
            ],
        ),
        // High speed
        group!(
            "length",
            D::HighSpeed,
            &[
                unchecked("length_target", Mm),
                unchecked("length_tolerance", Mm),
                unchecked("length_topology", Choice),
            ],
        ),
        group!(
            "matched_length",
            D::HighSpeed,
            &[
                unchecked("matched_length_tolerance", Mm),
                unchecked("matched_length_max_skew", Mm),
            ],
        ),
        group!(
            "stubs_and_vias",
            D::HighSpeed,
            &[
                unchecked("daisy_stub_max", Mm),
                unchecked("via_count_max", Count),
                unchecked("back_drill_stub_max", Mm),
                unchecked("via_under_smd", Choice),
            ],
        ),
        group!(
            "return_path",
            D::HighSpeed,
            &[unchecked("return_path_max_gap", Mm)],
        ),
        group!(
            "parallel_segment",
            D::HighSpeed,
            &[
                unchecked("parallel_segment_gap", Mm),
                unchecked("parallel_segment_max_length", Mm),
            ],
        ),
        // Manufacturing
        group!(
            "hole_size",
            D::Manufacturing,
            &[
                col("hole_size_min", F::MinHole, Mm),
                col("hole_size_max", F::MaxHole, Mm),
            ],
        ),
        group!(
            "annular_ring",
            D::Manufacturing,
            &[col("annular_ring_min", F::MinAnnular, Mm)],
        ),
        group!(
            "hole_to_hole",
            D::Manufacturing,
            &[col("hole_to_hole_min", F::MinHoleSpacing, Mm)],
        ),
        group!(
            "layer_pairs",
            D::Manufacturing,
            &[unchecked("layer_pairs_enforce", Choice)],
        ),
        group!(
            "acute_angle",
            D::Manufacturing,
            &[unchecked("acute_angle_min", Degrees)],
        ),
        group!(
            "mask_sliver",
            D::Manufacturing,
            &[col("mask_sliver_min", F::MinSliver, Mm)],
        ),
        group!(
            "silk",
            D::Manufacturing,
            &[
                col("silk_to_pad", F::MinSilkPad, Mm),
                col("silk_to_silk", F::MinSilkSpacing, Mm),
            ],
        ),
        group!(
            "net_antennae",
            D::Manufacturing,
            &[unchecked("net_antennae_tolerance", Mm)],
        ),
        group!(
            "board_outline",
            D::Manufacturing,
            &[col("board_outline_min", F::MinEdgeClearance, Mm)],
        ),
        // SMT
        group!(
            "smd_to_corner",
            D::Smt,
            &[unchecked("smd_to_corner_min", Mm)]
        ),
        group!("smd_to_plane", D::Smt, &[unchecked("smd_to_plane_max", Mm)]),
        group!("smd_entry", D::Smt, &[unchecked("smd_entry", Choice)]),
        // Mask
        group!(
            "solder_mask",
            D::Mask,
            &[
                col("solder_mask_expansion_min", F::MinMaskExpansion, Mm),
                col("solder_mask_expansion_max", F::MaxMaskExpansion, Mm),
                unchecked("solder_mask_tented", Choice),
            ],
        ),
        group!(
            "paste_mask",
            D::Mask,
            &[
                col("paste_mask_expansion_min", F::MinPasteExpansion, Mm),
                col("paste_mask_expansion_max", F::MaxPasteExpansion, Mm),
            ],
        ),
        // Plane
        group!(
            "power_plane_connect",
            D::Plane,
            &[
                unchecked("power_plane_connect_style", Choice),
                unchecked("power_plane_connect_spokes", Count),
                unchecked("power_plane_connect_width", Mm),
                unchecked("power_plane_connect_air_gap", Mm),
            ],
        ),
        group!(
            "power_plane_clearance",
            D::Plane,
            &[unchecked("power_plane_clearance_min", Mm)],
        ),
        group!(
            "pour_connect",
            D::Plane,
            &[
                unchecked("pour_connect_spokes", Count),
                unchecked("pour_connect_width", Mm),
                unchecked("pour_connect_air_gap", Mm),
            ],
        ),
        // Testpoint
        group!(
            "fab_testpoint_style",
            D::Testpoint,
            &[
                unchecked("fab_testpoint_side", Choice),
                unchecked("fab_testpoint_pad", Mm),
                unchecked("fab_testpoint_grid", Mm),
                unchecked("fab_testpoint_spacing", Mm),
            ],
        ),
        group!(
            "fab_testpoint_usage",
            D::Testpoint,
            &[unchecked("fab_testpoint_required", Choice)],
        ),
        group!(
            "assembly_testpoint_style",
            D::Testpoint,
            &[
                unchecked("assembly_testpoint_side", Choice),
                unchecked("assembly_testpoint_pad", Mm),
                unchecked("assembly_testpoint_grid", Mm),
                unchecked("assembly_testpoint_spacing", Mm),
            ],
        ),
        group!(
            "assembly_testpoint_usage",
            D::Testpoint,
            &[unchecked("assembly_testpoint_required", Choice)],
        ),
        // Placement
        group!(
            "component_clearance",
            D::Placement,
            &[col("component_clearance_min", F::MinCourtyardSpacing, Mm)],
        ),
        group!(
            "permitted_layers",
            D::Placement,
            &[unchecked("permitted_sides", Choice)],
        ),
        group!(
            "height",
            D::Placement,
            &[
                unchecked("height_min", Mm),
                unchecked("height_preferred", Mm),
                unchecked("height_max", Mm),
            ],
        ),
        group!(
            "orientation",
            D::Placement,
            &[unchecked("orientation_allowed", Choice)],
        ),
        group!(
            "room",
            D::Placement,
            &[
                unchecked("room_keep", Choice),
                unchecked("room_side", Choice)
            ],
        ),
        group!(
            "nets_to_ignore",
            D::Placement,
            &[unchecked("nets_to_ignore", Choice)],
        ),
        // Signal integrity has no columns yet.
    ]
};

/// Every column group, in grid order (domain by domain).
pub fn catalog() -> &'static [CatalogGroup] {
    CATALOG
}

/// The domain of the first of `limits` (the check's limit kinds) that has a
/// column: where a finding of those limits, or a rule checking them, is
/// shown. `None` when none has one (a short, an unconnected link).
pub fn domain_of_limits(limits: &[signex_drc::LimitKind]) -> Option<Domain> {
    limits.iter().find_map(|kind| {
        let field = LimitField::ALL
            .into_iter()
            .find(|f| f.limit_kind() == *kind)?;
        group_of(field).map(|g| g.domain)
    })
}

/// The group whose column shows `field`.
pub fn group_of(field: LimitField) -> Option<&'static CatalogGroup> {
    catalog()
        .iter()
        .find(|g| g.columns.iter().any(|c| c.field == Some(field)))
}

/// The column that shows `field`.
pub fn column_of(field: LimitField) -> Option<&'static CatalogColumn> {
    catalog()
        .iter()
        .flat_map(|g| g.columns.iter())
        .find(|c| c.field == Some(field))
}

/// The copper objects a clearance rule's `between` names and the check
/// measures, in the class matrix's object-pair order (#90): the kinds of
/// [`OBJECT_PAIRS`], each once.
pub fn object_kinds() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for kind in OBJECT_PAIRS.iter().flatten() {
        if !out.contains(kind) {
            out.push(*kind);
        }
    }
    out
}

/// The object pairs of the matrix's object-pair card, in order.
pub fn object_pairs() -> &'static [[&'static str; 2]] {
    &OBJECT_PAIRS
}
