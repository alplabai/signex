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

/// Identity of the stock library itself. Designs reference stock
/// primitives by `PrimitiveRef { library_id, uuid }`, so like the rows
/// below this is a published contract: never regenerate or edit it.
pub const STOCK_LIBRARY_ID: &str = "019df45d-af5e-7a00-8c3b-dd3bf89b40ae";

/// Pinned identity of one stock `.snxfpt` file.
pub struct StockIds {
    pub kind: StockKind,
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
    /// On-disk name: `LocalGitAdapter` finds a footprint at
    /// `footprints/<footprint uuid>.snxfpt`, so the library lists and
    /// loads it only under that name.
    pub fn file_name(&self) -> String {
        format!("{}.snxfpt", self.footprint_uuid)
    }

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
        file_uuid: "019df45d-af65-71a1-b568-29e703608649",
        footprint_uuid: "019df45d-af65-71a1-b568-29d0a5242184",
        plane: "6287d9cd-d2d8-4190-bd2e-46de49a389ac",
        entities: &["a2de0bf0-adc5-448c-adf8-3b190a702392"],
    },
    StockIds {
        kind: StockKind::Fiducial1mm,
        file_uuid: "019df45d-af66-75c0-b9cf-2bbf173b0066",
        footprint_uuid: "019df45d-af66-75c0-b9cf-2ba7e253d69c",
        plane: "7c74fab9-10b4-4550-aad0-cec6d5b2c3e9",
        entities: &["4373d9fb-b5a8-49e6-adc7-5f6307803a36"],
    },
];

/// Pinned identity of one stock symbol: `(symbol name, file uuid,
/// symbol uuid)`. Each stock `.snxsym` holds a single symbol and is
/// written to `symbols/<name>.snxsym`. Like the footprint rows these
/// are a published contract: never regenerate or edit them, only
/// append rows with freshly minted ids.
pub const STOCK_SYMBOL_IDS: &[(&str, &str, &str)] = &[
    (
        "R",
        "57854c3b-c51b-403d-b488-5953c9567113",
        "bf88a609-74df-4453-a656-d68ef86e421d",
    ),
    (
        "C",
        "ca480ad0-f6a0-4798-a8e8-41d46082dbc0",
        "9b489446-5f4e-4f4f-a166-85c076b4ff18",
    ),
    (
        "C_Polarized",
        "253b6003-fa61-4cce-9cb0-6274b768ce2f",
        "2604013a-f69d-4802-827d-9e5add7d2356",
    ),
    (
        "L",
        "3a3a79ea-de2d-4995-9dd1-736030c60c4c",
        "53e23353-f544-4652-8123-b8666f5986b9",
    ),
    (
        "Ferrite",
        "e288a70a-095b-4259-944f-4f63a9f33a7d",
        "9b67ea2f-57cc-464f-b39b-80a1508c1ca9",
    ),
    (
        "Fuse",
        "6e84840e-a540-4cc1-a686-512202445150",
        "018d4323-4516-4c9d-8b60-fbc6662e7848",
    ),
    (
        "D",
        "cdca28d0-3cf7-4d61-b32b-d4f6a47db3f2",
        "1288921b-aa3c-46a2-8ef7-31d1f7ec5834",
    ),
    (
        "D_Schottky",
        "86ed58c6-db77-432b-85b1-09065d17e744",
        "3b71eb69-67e4-434a-9b74-ded23c704b68",
    ),
    (
        "D_Zener",
        "f1a4c0a6-bf5d-4636-8866-c8d88aeb9fad",
        "a23b688f-74d9-40b5-93c4-6d48114992bc",
    ),
    (
        "LED",
        "d50383cb-013d-4fc4-9874-6e6196548032",
        "c2368f96-f502-4125-94e5-029eaa698ede",
    ),
    (
        "Q_NPN",
        "8c48665d-b41e-433b-9edd-54b86ed4dfd4",
        "3aa4c65d-91dc-46f4-b1cd-bd010bce921d",
    ),
    (
        "Q_PNP",
        "5f2a25c6-f838-4446-8922-057c4109daa3",
        "0cbf07b0-c162-4567-9e9a-78c089e24d52",
    ),
    (
        "Q_NMOS",
        "246539ea-07cb-4232-a434-c64d22674dd9",
        "2a530743-7082-424f-bfad-ba7f0e1e858e",
    ),
    (
        "Q_PMOS",
        "11898e98-1cf7-4f6c-b0c7-c88853622bf7",
        "e7e9f067-7a54-43a3-bafb-87a4cfa17d79",
    ),
    (
        "Crystal",
        "d96f20fe-43e6-47e8-ba6f-f87041760138",
        "60f53907-9a68-4b5e-a5a2-24b258a0901a",
    ),
    (
        "GND",
        "1e63f24a-321a-4496-b4f9-3b32e1af6565",
        "95fe9d07-40cb-4756-903f-e722b9b3df63",
    ),
    (
        "VCC",
        "67ed41b2-79ca-401b-a8c4-d1ff9c6cffc2",
        "a320b61f-58ec-4ffb-b2f1-cf36c00ed9d6",
    ),
    (
        "+3V3",
        "e58c46cd-6870-4bfb-9278-eef1afa95d0b",
        "1908711d-a1cf-4cdd-9420-8bdf894680c1",
    ),
    (
        "+5V",
        "f1d0ec04-e9ea-40f4-ac6d-07848c8e7004",
        "e4ef583f-8d7b-4f10-a9c7-d879f9e11482",
    ),
    (
        "IC_Generic",
        "b606f296-15d0-4d79-8481-59c4edca9985",
        "15fc3bcf-6b70-413d-89a0-36e9d5ab5319",
    ),
];

/// Look up the pinned `(file uuid, symbol uuid)` of the symbol called
/// `name`. A builder without a row is a programming error.
pub fn symbol_ids(name: &str) -> (Uuid, Uuid) {
    let (_, file, symbol) = STOCK_SYMBOL_IDS
        .iter()
        .find(|(n, _, _)| *n == name)
        .unwrap_or_else(|| panic!("no pinned id row for stock symbol {name}"));
    (parse(file), parse(symbol))
}
