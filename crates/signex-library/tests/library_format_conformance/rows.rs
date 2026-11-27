//! The part rows, the `.snxlib` that holds them and the content-hash input.

use super::*;

// ── Part rows ────────────────────────────────────────────────────────────

pub(crate) fn measurement(value: f64, unit: &str) -> ParamValue {
    ParamValue::Measurement {
        value,
        unit: unit.into(),
    }
}

pub(crate) fn row(row_id: &str, pn: &str, class: &str, symbol: &str) -> ComponentRow {
    ComponentRow {
        row_id: id(row_id),
        internal_pn: InternalPn::new(pn),
        class: ComponentClass::new(class),
        datasheet: DatasheetRef::default(),
        datasheet_label: String::new(),
        description: String::new(),
        more_datasheets: Vec::new(),
        state: LifecycleState::Released,
        symbol_ref: lib_ref(symbol),
        alternate_symbols: Vec::new(),
        footprint_ref: None,
        alternate_footprints: Vec::new(),
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft("", ""),
        alternates: Vec::new(),
        supply: Vec::new(),
        parameters: ParamMap::new(),
        plm: PlmReserved::default(),
        version: "1.0.0".into(),
        released: false,
        symbol_version: "1.0.0".into(),
        footprint_version: String::new(),
        sim_version: String::new(),
        created: created(),
        updated: updated(),
        content_hash: [0u8; 32],
        server_revision: None,
    }
}

/// Object datasheet, footprint array with a pinned alternate, an alternate
/// symbol, measurement and text parameters, alternate MPN, a listing.
pub(crate) fn row_r10k() -> ComponentRow {
    let mut r = row(ROW_R10K, "R-10K-0603", "resistor", RESISTOR_SYMBOL);
    r.datasheet = DatasheetRef::url("https://example.com/datasheets/r0603.pdf");
    r.description = "10 kΩ 1% 0.1 W 0603".into();
    r.released = true;
    r.footprint_ref = Some(lib_ref(R0603_FOOTPRINT));
    r.footprint_version = "1.0.0".into();
    r.alternate_footprints = vec![FootprintBinding {
        version: "1.0.0".into(),
        ..FootprintBinding::new(lib_ref(R0805_FOOTPRINT))
    }];
    r.alternate_symbols = vec![AlternateSymbol::new(
        lib_ref(NAND_SYMBOL),
        "1.0.0",
        "Alternate drawing",
    )];
    r.primary_mpn = ManufacturerPart::draft("Example Corp", "EX-R0603-10K");
    r.alternates = vec![ManufacturerPart {
        status: AlternateStatus::Approved,
        notes: Some("drop-in".into()),
        ..ManufacturerPart::draft("Sample Inc", "SI-10K-0603")
    }];
    r.supply = vec![DistributorListing {
        url: Some("https://example.com/p/ED-123".into()),
        moq: Some(100),
        ..DistributorListing::new("ExampleDistributor", "ED-123")
    }];
    r.parameters
        .insert("resistance".into(), measurement(10_000.0, "ohm"));
    r.parameters.insert("power".into(), measurement(0.1, "W"));
    r.parameters
        .insert("tolerance".into(), ParamValue::Text("1%".into()));
    r
}

/// Labelled documents (array form), a pin map, number and bool parameters,
/// the single-object footprint form, no alternate symbols (empty cell).
pub(crate) fn row_r1k() -> ComponentRow {
    let mut r = row(ROW_R1K, "R-1K-0805", "resistor", RESISTOR_SYMBOL);
    r.state = LifecycleState::Draft;
    r.version = "0.0.1".into();
    r.set_documents(vec![
        DatasheetDoc::new(
            "Datasheet",
            DatasheetRef::url("https://example.com/datasheets/r0805.pdf"),
        ),
        DatasheetDoc::new(
            "Errata",
            DatasheetRef::hash_pinned(hex(&Sha256::digest(b"errata")), "errata.pdf"),
        ),
    ]);
    r.footprint_ref = Some(lib_ref(R0805_FOOTPRINT));
    r.footprint_version = "1.0.0".into();
    r.pin_map_overrides = vec![PinPadOverride::new("1", "2"), PinPadOverride::new("2", "1")];
    r.primary_mpn = ManufacturerPart::draft("Example Corp", "EX-R0805-1K");
    r.parameters
        .insert("aec_q200".into(), ParamValue::Bool(true));
    r.parameters
        .insert("pad_count".into(), ParamValue::Number(2.0));
    r
}

/// A symbol-only part: empty footprint cell, no listing.
pub(crate) fn row_nand() -> ComponentRow {
    let mut r = row(ROW_NAND, "U-NAND-DUAL", "ic", NAND_SYMBOL);
    r.state = LifecycleState::InReview;
    r.description = "Dual NAND, symbol only".into();
    r.primary_mpn = ManufacturerPart::draft("Example Corp", "EX-74-00");
    r.parameters
        .insert("supply_voltage".into(), measurement(5.0, "V"));
    r.parameters
        .insert("designator_prefix".into(), ParamValue::Text("U".into()));
    r
}

pub(crate) fn rows() -> Vec<ComponentRow> {
    [row_r10k(), row_r1k(), row_nand()]
        .into_iter()
        .map(|mut r| {
            r.refresh_content_hash().expect("fixture rows hash");
            r
        })
        .collect()
}

pub(crate) fn table(columns: Vec<String>, rows: &[&ComponentRow]) -> LibraryTable {
    LibraryTable {
        columns,
        rows: rows
            .iter()
            .map(|r| {
                component_to_library_row(r)
                    .map(Arc::new)
                    .expect("fixture row")
            })
            .collect(),
        column_types: BTreeMap::new(),
        pricing: BTreeMap::new(),
    }
}

pub(crate) fn header(with_alternate_symbols: bool) -> Vec<String> {
    let extra = with_alternate_symbols.then_some(ALTERNATE_SYMBOLS_COLUMN);
    TABLE_HEADER
        .iter()
        .copied()
        .chain(extra)
        .map(str::to_string)
        .collect()
}

pub(crate) fn library_file() -> LibraryFile {
    let rows = rows();
    // Inserted out of id order: the writer sorts by `row_id`.
    let mut resistors = table(header(true), &[&rows[1], &rows[0]]);
    resistors
        .column_types
        .insert("released".into(), ColumnType::Bool);
    let price = CachedPricing {
        sku: "ED-123".into(),
        currency: "EUR".into(),
        stock: Some(5000),
        breaks: [(1, 0.1), (100, 0.02), (1000, 0.0085)]
            .into_iter()
            .map(|(qty, unit_price)| PriceBreak { qty, unit_price })
            .collect(),
        fetched_at: Utc.with_ymd_and_hms(2026, 1, 3, 10, 0, 0).unwrap(),
    };
    resistors.pricing.insert(
        ROW_R10K.into(),
        BTreeMap::from([(
            "ExampleDistributor".to_string(),
            BTreeMap::from([("ED-123".to_string(), price)]),
        )]),
    );
    let logic = table(header(false), &[&rows[2]]);
    let mut users = UsersConfig {
        default_role: "Designer".into(),
        ..UsersConfig::default()
    };
    users.by_email.insert(
        "ada@example.com".into(),
        UserEntry {
            display_name: "Ada".into(),
            roles: vec!["Librarian".into()],
        },
    );
    LibraryFile {
        layout: Default::default(),
        manifest: SnxlibManifest {
            format: FORMAT_TOKEN.into(),
            library_id: id(LIBRARY_ID),
            library: LibrarySection {
                name: "Conformance".into(),
                description: Some(
                    "Signex library format conformance fixture (docs/LIBRARY_FORMAT.md).".into(),
                ),
                ..Default::default()
            },
            mode: LibraryMode::LocalGit,
            workflow: WorkflowConfig::default(),
            users,
            classes: vec![
                ClassEntry {
                    key: "resistor".into(),
                    label: "Resistor".into(),
                },
                ClassEntry {
                    key: "ic".into(),
                    label: "Integrated Circuit".into(),
                },
            ],
        },
        tables: BTreeMap::from([
            ("logic".to_string(), logic),
            ("resistors".to_string(), resistors),
        ]),
    }
}

// ── Content hash, transcribed from the spec ──────────────────────────────

/// `docs/LIBRARY_FORMAT.md` §6 written out independently of `hash.rs`, so a
/// change to either shows up here.
#[derive(Serialize)]
pub(crate) struct CanonicalRow<'a> {
    row_id: &'a Uuid,
    internal_pn: &'a InternalPn,
    class: &'a ComponentClass,
    state: &'a LifecycleState,
    datasheet: &'a DatasheetRef,
    #[serde(skip_serializing_if = "str::is_empty")]
    datasheet_label: &'a str,
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    more_datasheets: &'a [DatasheetDoc],
    #[serde(skip_serializing_if = "str::is_empty")]
    description: &'a str,
    symbol_ref: &'a PrimitiveRef,
    footprint_ref: Option<&'a PrimitiveRef>,
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    alternate_footprints: &'a [FootprintBinding],
    sim_ref: Option<&'a PrimitiveRef>,
    pin_map_overrides: &'a [PinPadOverride],
    primary_mpn: &'a ManufacturerPart,
    alternates: &'a [ManufacturerPart],
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    alternate_symbols: &'a [AlternateSymbol],
    supply: &'a [DistributorListing],
    parameters: &'a ParamMap,
    plm: &'a PlmReserved,
}

pub(crate) fn canonical_json(r: &ComponentRow) -> Vec<u8> {
    serde_json::to_vec(&CanonicalRow {
        row_id: &r.row_id,
        internal_pn: &r.internal_pn,
        class: &r.class,
        state: &r.state,
        datasheet: &r.datasheet,
        datasheet_label: &r.datasheet_label,
        more_datasheets: &r.more_datasheets,
        description: &r.description,
        symbol_ref: &r.symbol_ref,
        footprint_ref: r.footprint_ref.as_ref(),
        alternate_footprints: &r.alternate_footprints,
        sim_ref: r.sim_ref.as_ref(),
        pin_map_overrides: &r.pin_map_overrides,
        primary_mpn: &r.primary_mpn,
        alternates: &r.alternates,
        alternate_symbols: &r.alternate_symbols,
        supply: &r.supply,
        parameters: &r.parameters,
        plm: &r.plm,
    })
    .expect("canonical row json")
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
