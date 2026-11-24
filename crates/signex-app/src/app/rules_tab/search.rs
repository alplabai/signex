//! The words of the constraint catalog (#2530) and the find-a-constraint
//! box: a typed name against every domain's column groups and columns, in
//! the locale's search fold. Typing jumps to the first match; Enter steps
//! to the next.

use signex_drc_dsl::table::catalog::{CatalogColumn, CatalogGroup, Domain, catalog};
use signex_i18n::{I18n, MessageId, ids, t};

/// The find box: what is typed and the columns it matches.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FindState {
    pub query: String,
    pub hits: Vec<FindHit>,
    /// The match shown, an index into `hits`.
    pub at: usize,
}

/// One column the find box matched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FindHit {
    pub domain: Domain,
    pub group: &'static CatalogGroup,
    pub column: &'static CatalogColumn,
}

/// Matches are listed from this many typed characters, and the grid jumps
/// to the first from [`JUMP_CHARS`].
const LIST_CHARS: usize = 2;
const JUMP_CHARS: usize = 3;

impl FindState {
    /// The match shown, once enough is typed to jump.
    pub fn current(&self) -> Option<FindHit> {
        (self.query.trim().chars().count() >= JUMP_CHARS)
            .then(|| self.hits.get(self.at).copied())
            .flatten()
    }

    /// Whether enough is typed to list matches.
    pub fn is_listing(&self) -> bool {
        self.query.trim().chars().count() >= LIST_CHARS
    }

    /// Type `query`: match it again and show the first match.
    pub fn typed(&mut self, query: String, i18n: &I18n) {
        self.hits = if query.trim().chars().count() >= LIST_CHARS {
            matches(&query, i18n)
        } else {
            Vec::new()
        };
        self.query = query;
        self.at = 0;
    }

    /// Enter: the next match, wrapping.
    pub fn step(&mut self) {
        if !self.hits.is_empty() {
            self.at = (self.at + 1) % self.hits.len();
        }
    }
}

/// Every column whose group, column or rule-type words hold `query`, in
/// catalog order.
pub(super) fn matches(query: &str, i18n: &I18n) -> Vec<FindHit> {
    let needle = i18n.fold_for_search(query.trim());
    if needle.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for group in catalog() {
        for column in group.columns {
            let hay = [
                t!(i18n, group_label(group.id)).into_string(),
                t!(i18n, type_label(group.id)).into_string(),
                t!(i18n, column_label(column.id)).into_string(),
            ]
            .join(" ");
            if i18n.fold_for_search(&hay).contains(&needle) {
                out.push(FindHit {
                    domain: group.domain,
                    group,
                    column,
                });
            }
        }
    }
    out
}

/// The tab label of a domain.
pub(crate) fn domain_label(domain: Domain) -> MessageId {
    match domain {
        Domain::Clearance => ids::RULES_DOMAIN_CLEARANCE,
        Domain::Physical => ids::RULES_DOMAIN_PHYSICAL,
        Domain::HighSpeed => ids::RULES_DOMAIN_HIGH_SPEED,
        Domain::Manufacturing => ids::RULES_DOMAIN_MANUFACTURING,
        Domain::Smt => ids::RULES_DOMAIN_SMT,
        Domain::Mask => ids::RULES_DOMAIN_MASK,
        Domain::Plane => ids::RULES_DOMAIN_PLANE,
        Domain::Testpoint => ids::RULES_DOMAIN_TESTPOINT,
        Domain::Placement => ids::RULES_DOMAIN_PLACEMENT,
        Domain::SignalIntegrity => ids::RULES_DOMAIN_SIGNAL_INTEGRITY,
    }
}

/// The header of a column group.
pub(crate) fn group_label(group: &str) -> MessageId {
    match group {
        "clearance" => ids::RULES_GROUP_CLEARANCE,
        "creepage" => ids::RULES_GROUP_CREEPAGE,
        "short_circuit" => ids::RULES_GROUP_SHORT_CIRCUIT,
        "unrouted_net" => ids::RULES_GROUP_UNROUTED_NET,
        "unconnected_pin" => ids::RULES_GROUP_UNCONNECTED_PIN,
        "modified_pour" => ids::RULES_GROUP_MODIFIED_POUR,
        "width" => ids::RULES_GROUP_WIDTH,
        "via_style" => ids::RULES_GROUP_VIA_STYLE,
        "diff_pair" => ids::RULES_GROUP_DIFF_PAIR,
        "neck_down" => ids::RULES_GROUP_NECK_DOWN,
        "corners" => ids::RULES_GROUP_CORNERS,
        "routing" => ids::RULES_GROUP_ROUTING,
        "length" => ids::RULES_GROUP_LENGTH,
        "matched_length" => ids::RULES_GROUP_MATCHED_LENGTH,
        "stubs_and_vias" => ids::RULES_GROUP_STUBS_AND_VIAS,
        "return_path" => ids::RULES_GROUP_RETURN_PATH,
        "parallel_segment" => ids::RULES_GROUP_PARALLEL_SEGMENT,
        "hole_size" => ids::RULES_GROUP_HOLE_SIZE,
        "annular_ring" => ids::RULES_GROUP_ANNULAR_RING,
        "hole_to_hole" => ids::RULES_GROUP_HOLE_TO_HOLE,
        "layer_pairs" => ids::RULES_GROUP_LAYER_PAIRS,
        "acute_angle" => ids::RULES_GROUP_ACUTE_ANGLE,
        "mask_sliver" => ids::RULES_GROUP_MASK_SLIVER,
        "silk" => ids::RULES_GROUP_SILK,
        "net_antennae" => ids::RULES_GROUP_NET_ANTENNAE,
        "board_outline" => ids::RULES_GROUP_BOARD_OUTLINE,
        "smd_to_corner" => ids::RULES_GROUP_SMD_TO_CORNER,
        "smd_to_plane" => ids::RULES_GROUP_SMD_TO_PLANE,
        "smd_entry" => ids::RULES_GROUP_SMD_ENTRY,
        "solder_mask" => ids::RULES_GROUP_SOLDER_MASK,
        "paste_mask" => ids::RULES_GROUP_PASTE_MASK,
        "power_plane_connect" => ids::RULES_GROUP_POWER_PLANE_CONNECT,
        "power_plane_clearance" => ids::RULES_GROUP_POWER_PLANE_CLEARANCE,
        "pour_connect" => ids::RULES_GROUP_POUR_CONNECT,
        "fab_testpoint_style" => ids::RULES_GROUP_FAB_TESTPOINT_STYLE,
        "fab_testpoint_usage" => ids::RULES_GROUP_FAB_TESTPOINT_USAGE,
        "assembly_testpoint_style" => ids::RULES_GROUP_ASSEMBLY_TESTPOINT_STYLE,
        "assembly_testpoint_usage" => ids::RULES_GROUP_ASSEMBLY_TESTPOINT_USAGE,
        "component_clearance" => ids::RULES_GROUP_COMPONENT_CLEARANCE,
        "permitted_layers" => ids::RULES_GROUP_PERMITTED_LAYERS,
        "height" => ids::RULES_GROUP_HEIGHT,
        "orientation" => ids::RULES_GROUP_ORIENTATION,
        "room" => ids::RULES_GROUP_ROOM,
        "nets_to_ignore" => ids::RULES_GROUP_NETS_TO_IGNORE,
        _ => ids::RULES_GRID_UNNAMED,
    }
}

/// The full rule-type name of a column group (tooltips, find, captions).
pub(crate) fn type_label(group: &str) -> MessageId {
    match group {
        "creepage" => ids::RULES_TYPE_CREEPAGE,
        "diff_pair" => ids::RULES_TYPE_DIFF_PAIR,
        "neck_down" => ids::RULES_TYPE_NECK_DOWN,
        "corners" => ids::RULES_TYPE_CORNERS,
        "routing" => ids::RULES_TYPE_ROUTING,
        "matched_length" => ids::RULES_TYPE_MATCHED_LENGTH,
        "stubs_and_vias" => ids::RULES_TYPE_STUBS_AND_VIAS,
        "annular_ring" => ids::RULES_TYPE_ANNULAR_RING,
        "hole_to_hole" => ids::RULES_TYPE_HOLE_TO_HOLE,
        "mask_sliver" => ids::RULES_TYPE_MASK_SLIVER,
        "silk" => ids::RULES_TYPE_SILK,
        "board_outline" => ids::RULES_TYPE_BOARD_OUTLINE,
        "solder_mask" => ids::RULES_TYPE_SOLDER_MASK,
        "paste_mask" => ids::RULES_TYPE_PASTE_MASK,
        "power_plane_connect" => ids::RULES_TYPE_POWER_PLANE_CONNECT,
        "pour_connect" => ids::RULES_TYPE_POUR_CONNECT,
        "fab_testpoint_style" => ids::RULES_TYPE_FAB_TESTPOINT_STYLE,
        "fab_testpoint_usage" => ids::RULES_TYPE_FAB_TESTPOINT_USAGE,
        "assembly_testpoint_style" => ids::RULES_TYPE_ASSEMBLY_TESTPOINT_STYLE,
        "assembly_testpoint_usage" => ids::RULES_TYPE_ASSEMBLY_TESTPOINT_USAGE,
        "orientation" => ids::RULES_TYPE_ORIENTATION,
        "room" => ids::RULES_TYPE_ROOM,
        other => group_label(other),
    }
}

/// The header of one column under its group.
pub(crate) fn column_label(column: &str) -> MessageId {
    match column {
        "clearance_min"
        | "creepage_min"
        | "hole_size_min"
        | "annular_ring_min"
        | "hole_to_hole_min"
        | "acute_angle_min"
        | "mask_sliver_min"
        | "board_outline_min"
        | "smd_to_corner_min"
        | "power_plane_clearance_min"
        | "component_clearance_min"
        | "width_min"
        | "height_min" => ids::RULES_COLUMN_MIN,
        "width_max" | "hole_size_max" | "smd_to_plane_max" | "height_max" => ids::RULES_COLUMN_MAX,
        "width_preferred" | "height_preferred" => ids::RULES_COLUMN_PREFERRED,
        "via_diameter" => ids::RULES_COLUMN_DIAMETER,
        "via_hole" => ids::RULES_COLUMN_HOLE,
        "via_pad_gap" => ids::RULES_COLUMN_PAD_GAP,
        "via_finish" => ids::RULES_COLUMN_FINISH,
        "diff_pair_width" | "power_plane_connect_width" | "pour_connect_width" => {
            ids::RULES_COLUMN_WIDTH
        }
        "diff_pair_gap" | "parallel_segment_gap" => ids::RULES_COLUMN_GAP,
        "diff_pair_max_uncoupled" => ids::RULES_COLUMN_MAX_UNCOUPLED,
        "neck_down_min_of_pad" => ids::RULES_COLUMN_MIN_OF_PAD,
        "corners_style" | "power_plane_connect_style" => ids::RULES_COLUMN_STYLE,
        "corners_setback" => ids::RULES_COLUMN_SETBACK,
        "routing_topology" | "length_topology" => ids::RULES_COLUMN_TOPOLOGY,
        "routing_priority" => ids::RULES_COLUMN_PRIORITY,
        "routing_layers" => ids::RULES_COLUMN_LAYERS,
        "routing_fanout" => ids::RULES_COLUMN_FANOUT,
        "length_target" => ids::RULES_COLUMN_TARGET,
        "length_tolerance" => ids::RULES_COLUMN_PLUS_MINUS,
        "matched_length_tolerance" | "net_antennae_tolerance" => ids::RULES_COLUMN_TOLERANCE,
        "matched_length_max_skew" => ids::RULES_COLUMN_MAX_SKEW,
        "daisy_stub_max" => ids::RULES_COLUMN_DAISY_STUB,
        "via_count_max" => ids::RULES_COLUMN_MAX_VIAS,
        "back_drill_stub_max" => ids::RULES_COLUMN_BACK_DRILL_STUB,
        "via_under_smd" => ids::RULES_COLUMN_UNDER_SMD,
        "return_path_max_gap" => ids::RULES_COLUMN_MAX_GAP,
        "parallel_segment_max_length" => ids::RULES_COLUMN_MAX_LENGTH,
        "layer_pairs_enforce" => ids::RULES_COLUMN_ENFORCE,
        "silk_to_pad" => ids::RULES_COLUMN_TO_PAD,
        "silk_to_silk" => ids::RULES_COLUMN_TO_SILK,
        "solder_mask_expansion_min" | "paste_mask_expansion_min" => ids::RULES_COLUMN_MIN_EXPANSION,
        "solder_mask_expansion_max" | "paste_mask_expansion_max" => ids::RULES_COLUMN_MAX_EXPANSION,
        "solder_mask_tented" => ids::RULES_COLUMN_TENTED,
        "power_plane_connect_spokes" | "pour_connect_spokes" => ids::RULES_COLUMN_SPOKES,
        "power_plane_connect_air_gap" | "pour_connect_air_gap" => ids::RULES_COLUMN_AIR_GAP,
        "fab_testpoint_side" | "assembly_testpoint_side" | "room_side" => ids::RULES_COLUMN_SIDE,
        "fab_testpoint_pad" | "assembly_testpoint_pad" => ids::RULES_COLUMN_PAD_DIAMETER,
        "fab_testpoint_grid" | "assembly_testpoint_grid" => ids::RULES_COLUMN_GRID,
        "fab_testpoint_spacing" | "assembly_testpoint_spacing" => ids::RULES_COLUMN_SPACING,
        "fab_testpoint_required" | "assembly_testpoint_required" => ids::RULES_COLUMN_REQUIRED,
        "permitted_sides" => ids::RULES_COLUMN_SIDES,
        "orientation_allowed" | "short_circuit_allowed" => ids::RULES_COLUMN_ALLOWED,
        "room_keep" => ids::RULES_COLUMN_KEEP,
        "nets_to_ignore" => ids::RULES_COLUMN_IGNORE,
        "unrouted_net_check" | "unconnected_pin_check" | "modified_pour_check" => {
            ids::RULES_COLUMN_CHECK
        }
        "smd_entry" => ids::RULES_COLUMN_ENTRY,
        _ => ids::RULES_GRID_UNNAMED,
    }
}
