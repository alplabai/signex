//! Checked-in identity table for the stock footprints.
//!
//! Designs reference library primitives by `PrimitiveRef { library_id,
//! uuid }`, so these ids are a published contract: never regenerate or
//! edit them. They are the ids the first shipped `.snxfpt` files
//! carried; the generator reads them instead of minting fresh ones.
//! Adding a footprint means appending a row with freshly minted ids.

use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::PlaneId;
use uuid::Uuid;

/// Which builder authors a stock footprint. Keys each [`StockIds`]
/// row to its builder, so a new row cannot name a builder that does
/// not exist.
#[derive(Clone, Copy)]
pub enum StockKind {
    Soic8,
    Qfn16,
    R0805,
    MountingHole3p2,
    Fiducial1mm,
}

/// Pinned identity of one stock `.snxfpt` file.
pub struct StockIds {
    pub kind: StockKind,
    pub file_name: &'static str,
    file_uuid: &'static str,
    footprint_uuid: &'static str,
    plane: &'static str,
    /// One id per sketch entity, in the order the builder pushes them.
    entities: &'static [&'static str],
}

fn parse(id: &str) -> Uuid {
    Uuid::parse_str(id).expect("stock id table holds well-formed UUIDs")
}

impl StockIds {
    pub fn file_uuid(&self) -> Uuid {
        parse(self.file_uuid)
    }

    pub fn footprint_uuid(&self) -> Uuid {
        parse(self.footprint_uuid)
    }

    pub fn plane_id(&self) -> PlaneId {
        PlaneId(parse(self.plane))
    }

    pub fn entity_ids(&self) -> impl Iterator<Item = SketchEntityId> + '_ {
        self.entities.iter().map(|id| SketchEntityId(parse(id)))
    }

    /// Every id in the row, for the table-vs-committed-file check.
    #[allow(
        dead_code,
        reason = "used by the determinism test, not the generator binary"
    )]
    pub fn all_ids(&self) -> Vec<Uuid> {
        let mut ids = vec![self.file_uuid(), self.footprint_uuid(), parse(self.plane)];
        ids.extend(self.entities.iter().map(|id| parse(id)));
        ids
    }
}

pub const STOCK_IDS: &[StockIds] = &[
    StockIds {
        kind: StockKind::Soic8,
        file_name: "SOIC-8.snxfpt",
        file_uuid: "019df45d-af5f-7bb0-914a-26cc90477b10",
        footprint_uuid: "019df45d-af5f-7bb0-914a-26b538180e51",
        plane: "e09fba70-a152-4bdf-a5b9-63e910b7feb9",
        entities: &[
            "a8e5614e-fd34-4ed7-843f-4e14aa2ad385",
            "361ff8ec-6f30-4611-a6ae-23539330f3f6",
            "ab4568ce-13da-4245-9e41-f5bd0db95882",
            "53b621c3-5e5e-4776-935c-4b429da345ad",
            "ec21901f-1dec-4e86-9d8e-7f560512dde4",
            "f8cb1015-8afe-4520-b623-26a290925a83",
            "cd2740e0-0b32-4aa6-a641-87f428137e6e",
            "2af18275-f777-40c2-90ac-e382bb4da9d2",
        ],
    },
    StockIds {
        kind: StockKind::Qfn16,
        file_name: "QFN-16.snxfpt",
        file_uuid: "019df45d-af61-7612-8d18-c80e4c879451",
        footprint_uuid: "019df45d-af61-7612-8d18-c7f4c67e4b0e",
        plane: "820e5c14-38a5-480a-9098-08579b42ef2d",
        entities: &[
            "f83b5630-3d88-4bdd-b8f9-46f0a8565577",
            "1707c465-dcd3-4895-9926-70267973ba1c",
            "56b4c552-855b-4b61-8975-6f44107d97e5",
            "7a701422-da8f-4f1a-9915-de54ca7ea5db",
            "40918ad0-1223-4928-8d7d-7460fed0173a",
            "61e00863-ed6f-406d-9e62-f0b752e0203a",
            "53fb08b5-a61b-4bdb-a4d6-b567d5cf1909",
            "43661ebe-cad5-4976-aa03-80034b7cbe4c",
            "b842a0e5-c7b0-411a-bd83-d9ead411b85b",
            "d73e721d-e653-400c-baa5-4cb59d422bdc",
            "6b691a25-b7df-4e85-b9d0-202cba270eb0",
            "22ca65aa-011d-4db4-b215-19ec429ce816",
            "b2b8e853-45b5-4260-9acc-94f19e238c44",
            "97191339-fd95-4500-8021-72c38ac4c4f9",
            "434e5442-6831-4d41-9209-dac04c1069b1",
            "eab4a114-124e-44cb-b1c8-a3bfde628ca1",
        ],
    },
    StockIds {
        kind: StockKind::R0805,
        file_name: "R0805.snxfpt",
        file_uuid: "019df45d-af64-7bb2-b787-add74d246d40",
        footprint_uuid: "019df45d-af63-7823-b5c3-01594028ccef",
        plane: "cdbf6963-91f5-44b7-9f96-a2e01a046265",
        entities: &[
            "7c39d334-33d5-41a9-be67-8df6b3b0f382",
            "321de7ce-bd80-4bef-9fc4-a2a8fe6bc82c",
        ],
    },
    StockIds {
        kind: StockKind::MountingHole3p2,
        file_name: "MountingHole-3.2mm.snxfpt",
        file_uuid: "019df45d-af65-71a1-b568-29e703608649",
        footprint_uuid: "019df45d-af65-71a1-b568-29d0a5242184",
        plane: "6287d9cd-d2d8-4190-bd2e-46de49a389ac",
        entities: &["a2de0bf0-adc5-448c-adf8-3b190a702392"],
    },
    StockIds {
        kind: StockKind::Fiducial1mm,
        file_name: "Fiducial-1mm.snxfpt",
        file_uuid: "019df45d-af66-75c0-b9cf-2bbf173b0066",
        footprint_uuid: "019df45d-af66-75c0-b9cf-2ba7e253d69c",
        plane: "7c74fab9-10b4-4550-aad0-cec6d5b2c3e9",
        entities: &["4373d9fb-b5a8-49e6-adc7-5f6307803a36"],
    },
];
