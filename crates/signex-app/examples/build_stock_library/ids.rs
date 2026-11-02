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
    /// Generic symmetric 0805 two-pad chip land.
    Chip0805,
    /// The same land with a cathode mark; pad 1 is the cathode.
    Chip0805Polarised,
    /// The other IPC-7351B chip sizes, plain then polarised; see `chip`.
    Chip0201,
    Chip0402,
    Chip0603,
    Chip1206,
    Chip1210,
    Chip2512,
    Chip0201Polarised,
    Chip0402Polarised,
    Chip0603Polarised,
    Chip1206Polarised,
    Chip1210Polarised,
    Chip2512Polarised,
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
        kind: StockKind::Chip0805,
        file_uuid: "019df45d-af64-7bb2-b787-add74d246d40",
        footprint_uuid: "019df45d-af63-7823-b5c3-01594028ccef",
        plane: "cdbf6963-91f5-44b7-9f96-a2e01a046265",
        entities: &[
            "7c39d334-33d5-41a9-be67-8df6b3b0f382",
            "321de7ce-bd80-4bef-9fc4-a2a8fe6bc82c",
        ],
    },
    StockIds {
        kind: StockKind::Chip0805Polarised,
        file_uuid: "019df45d-af67-7c20-9d3e-5a1b6e0c4f11",
        footprint_uuid: "019df45d-af67-7c20-9d3e-8b2f1d7a3c05",
        plane: "8a342c96-d25f-4908-b59a-7f6af7468ca1",
        entities: &[
            "b708e751-f11f-4bbc-8100-c75df9fdec73",
            "d454d0c0-8993-44dc-bbc6-ddee08f74868",
            "c28fd9f3-638f-4440-9ed8-190c58a6f17e",
            "58ea49eb-0a16-407d-b677-1a20322ad110",
            "7fff648f-930d-4e7e-a1e8-ddcc9023e196",
        ],
    },
    StockIds {
        kind: StockKind::Chip0201,
        file_uuid: "01a19e33-2796-74bf-abe5-fe785f7fc80a",
        footprint_uuid: "01a19e33-2796-7da8-baed-c4c3724be106",
        plane: "a833dda6-499a-4cbf-939a-bd72a859c5e0",
        entities: &[
            "e30941c4-5e1f-4649-8a8b-91f86c019117",
            "84b977da-682f-41a0-a0a1-06f10b7f3138",
        ],
    },
    StockIds {
        kind: StockKind::Chip0402,
        file_uuid: "01a19e33-2797-75c9-b10b-f10dd41c31a5",
        footprint_uuid: "01a19e33-2797-745c-9675-54d6083ca3c0",
        plane: "bc0d6e23-2bbc-4dfb-97d9-2900379cfb8c",
        entities: &[
            "be677e0d-f7d9-4ad0-bbd1-7b7ba69b98ee",
            "5ddaf2b8-e805-4b47-ab32-0123cd159ed9",
        ],
    },
    StockIds {
        kind: StockKind::Chip0603,
        file_uuid: "01a19e33-2798-7dab-9895-3fa02e2c366d",
        footprint_uuid: "01a19e33-2798-728e-8be0-44de7d2aebed",
        plane: "03404470-7339-4778-b0ab-46299ae7154b",
        entities: &[
            "97241bd9-24ef-412f-afdd-fd4a5942f055",
            "3b8c5c18-c887-4e5a-99e1-405c47d913e8",
        ],
    },
    StockIds {
        kind: StockKind::Chip1206,
        file_uuid: "01a19e33-2799-7161-9341-e36bb2bf77d6",
        footprint_uuid: "01a19e33-2799-7943-b61c-4a0297f0155d",
        plane: "2a60d2e8-c51b-4fa9-b61a-1cc5eabfaa4c",
        entities: &[
            "8edf6d6e-a01c-45fc-bbce-6ccf8bb45973",
            "b788abc2-fa1b-42ea-b876-767ffa4e5dd9",
        ],
    },
    StockIds {
        kind: StockKind::Chip1210,
        file_uuid: "01a19e33-279a-7e0e-bea2-2b118af8d0e0",
        footprint_uuid: "01a19e33-279a-7520-8444-679b090e7b0e",
        plane: "dfda13b6-64dc-4c86-8291-8dc23af6f1a7",
        entities: &[
            "c2a44ca5-5fac-48e5-8a05-2377fd50da8f",
            "6d2a59ee-b0d8-45b0-bdf1-1e11bf490e97",
        ],
    },
    StockIds {
        kind: StockKind::Chip2512,
        file_uuid: "01a19e33-279b-7e29-afd6-d47a9e279e66",
        footprint_uuid: "01a19e33-279b-774f-a1d0-e5ad64238119",
        plane: "6b20128f-05eb-44b8-bd27-e86683473972",
        entities: &[
            "0e71df6f-d4cd-44c0-835e-6881de15cab4",
            "cda853a4-3665-4ffd-9f56-f1d4c28076e0",
        ],
    },
    StockIds {
        kind: StockKind::Chip0201Polarised,
        file_uuid: "01a19e33-279c-7c97-8510-0c7ce339402b",
        footprint_uuid: "01a19e33-279c-7d8b-82d2-948cae6fd72c",
        plane: "54da35ee-d9c3-4b5c-9e44-3f0986d67f35",
        entities: &[
            "8a3f6e82-1107-4720-852c-73d95061aa9f",
            "ad470e63-e8b2-4f2d-8e9c-a594ffd6a026",
            "ed758f45-b9ac-4b73-abb5-4c2281e51076",
            "99d787fc-f213-4354-8f5d-021abc4f5f9b",
            "c5819f67-5479-4152-8c1b-c6372053f7f8",
        ],
    },
    StockIds {
        kind: StockKind::Chip0402Polarised,
        file_uuid: "01a19e33-279d-732e-80f7-070a8edf5dc9",
        footprint_uuid: "01a19e33-279d-7096-920f-4ddc00d6d8da",
        plane: "aef479f2-3b07-446a-af52-d9c9847ea0e8",
        entities: &[
            "e37d8410-d595-4e88-a0cd-149027ed62f7",
            "143e650f-329e-4f96-9251-c4adc3d401d8",
            "63e72586-fd60-4ba0-9893-bbc19d016826",
            "a4f822f2-00bb-4b5d-945b-2ec82cd9b517",
            "52b6e0e0-1b9a-4670-8490-2202f6976209",
        ],
    },
    StockIds {
        kind: StockKind::Chip0603Polarised,
        file_uuid: "01a19e33-279e-714b-b048-d3c5e94a3dfa",
        footprint_uuid: "01a19e33-279e-7342-98bb-a399d29a6f05",
        plane: "02543940-60e4-4691-adbf-80c3513d28dd",
        entities: &[
            "e73a0f12-8e63-4d6c-9e90-3c4e5cdc7a32",
            "06b7f3bb-1d76-4988-98e3-ff2b6e99df56",
            "35a084cb-a867-4e53-8215-1e03e99e140c",
            "f0ead269-723a-4ae1-97b8-24ab103f8c13",
            "bcc8b02b-bd8c-485c-b93d-12c130e997ad",
        ],
    },
    StockIds {
        kind: StockKind::Chip1206Polarised,
        file_uuid: "01a19e33-279f-7607-8aeb-6a00c22de9ca",
        footprint_uuid: "01a19e33-279f-7357-a84f-b0975a165008",
        plane: "be3169f8-3221-42e9-892d-e6b73c5a8a1d",
        entities: &[
            "b33182ff-c9bd-43b1-a495-d5c67e766ff8",
            "e66108fc-40ef-488a-bb2b-2dcad0d2906b",
            "d010410d-c168-45bb-981a-ba70eb7fe873",
            "1e9af685-2c1f-4a66-bfbb-b6d706afe5b6",
            "c38324b6-9477-4bfd-afc3-a22c45a1f00f",
        ],
    },
    StockIds {
        kind: StockKind::Chip1210Polarised,
        file_uuid: "01a19e33-27a0-7708-a9c8-f2f94b56205f",
        footprint_uuid: "01a19e33-27a0-7771-b34f-1d31e6970134",
        plane: "aacb68d6-d9d7-4398-8c49-4f81b60ba474",
        entities: &[
            "1bcd4d36-06a5-429d-a3dc-972608961b01",
            "a6503928-5ac0-4a11-9e48-9b97a5a9e7b9",
            "bc7cd778-fd98-4e34-9e48-829e7efd10bb",
            "ee10a4e8-17be-487f-b1d9-e750498a8421",
            "f082ed9a-dac5-4592-a395-9289e66edeef",
        ],
    },
    StockIds {
        kind: StockKind::Chip2512Polarised,
        file_uuid: "01a19e33-27a1-700b-b310-48232a08c82f",
        footprint_uuid: "01a19e33-27a1-74e9-87bd-333cfbb61d03",
        plane: "d4f7b99d-557a-4d74-8cc0-75445341a856",
        entities: &[
            "90aa3c83-521b-428d-b5ae-e3ea1e73bf64",
            "e33300bd-35e1-445f-a0c7-3103e47aa2fc",
            "03bbd0cc-5b97-4cb8-94f7-fe0af079ed99",
            "5932cf4b-af7a-48c4-b5b1-764e14d662f4",
            "aa6684f6-ecd8-4d75-b623-6eb72b305eeb",
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
    (
        "Conn_1x02",
        "c42a7c67-f9a0-4467-be0b-a67fa56bc2c6",
        "01a13ac9-cda8-487d-a53f-945732e945e0",
    ),
    (
        "Conn_1x03",
        "abca7e23-d806-4d39-95f4-d9a2f17ee48a",
        "d4f30085-41dc-458b-b7cc-0a3be9242226",
    ),
    (
        "Conn_1x04",
        "62131cb1-c0e0-43e0-8f12-4e5ac5e4a3b1",
        "a6bcc7d8-26ad-48ca-9347-7ddd4ff2326a",
    ),
    (
        "Conn_1x05",
        "a28dd740-b994-421c-9e77-c3b20da8df5e",
        "1048ab67-cd77-4cad-a969-6380151c78de",
    ),
    (
        "Conn_1x06",
        "15411de1-42d3-4ae8-b105-8ead8714b324",
        "a8dc8059-32fa-4317-922c-2664893d3451",
    ),
    (
        "Conn_1x08",
        "db1cb22c-05c9-492b-96de-14a362d971fe",
        "36e24ede-f621-49cb-8c6c-3016d54640b9",
    ),
    (
        "Conn_1x10",
        "c29d38e1-580f-4b5b-800d-90050306e7c6",
        "8d2e9e62-fc08-4777-bc43-dd5e0b68cf19",
    ),
    (
        "Conn_2x05",
        "c8916cf4-7898-403c-9555-c102d965ea86",
        "243191a8-f6cd-472f-8356-61f3df22e5eb",
    ),
    (
        "Conn_2x10",
        "01d68439-df1d-4464-857c-cb82439bd559",
        "d64fe024-5686-4d71-96c9-dcf1ce295ebd",
    ),
    (
        "Screw_1x02",
        "a3c3f38f-10dc-4990-acc1-e2d3e6b86176",
        "f3cab735-da24-44e6-bb3e-4615efc900f1",
    ),
    (
        "Screw_1x03",
        "fe5d8b88-5429-4c0a-945b-8b597d837c2f",
        "b5a279c3-0dcc-44e8-b2e4-6659eeb85bf0",
    ),
    (
        "74HC00",
        "e8769a2c-0221-4c74-b9e2-fab7fcc927d1",
        "89216e25-1574-48a2-b782-bf247ea267ac",
    ),
    (
        "74HC02",
        "91da6eb5-dfe4-446c-86df-2c9d834f1112",
        "e35c6a45-ff32-4c9d-8569-41ad556d3343",
    ),
    (
        "74HC04",
        "c4061779-8926-469c-9ccb-50e0bb59ebe9",
        "d22fdf64-507b-4437-b995-58404735a4aa",
    ),
    (
        "74HC08",
        "41cb3b0d-568b-4e77-bcaf-9b2c6b07d9b8",
        "85b91fa5-96c5-4d28-b3c5-6f672b72fc15",
    ),
    (
        "74HC32",
        "c68aead3-7ed8-41da-831e-2fd80ef1fb04",
        "8e2bf1d2-8728-4329-ac45-9db91ccf043f",
    ),
    (
        "74HC86",
        "01f2137c-4d2e-4b80-8756-358a0aba2f5d",
        "f5650d62-9751-4109-a389-5bcbdaeb099c",
    ),
    (
        "74HC14",
        "ada8ef93-8c05-4507-a5c7-475a6bfe4d8f",
        "208165e1-a261-489c-829f-d9d2c551f940",
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
