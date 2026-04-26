use signex_library::*;
use uuid::Uuid;

#[test]
fn full_component_round_trip_via_snxpart() {
    let mut rev = Revision {
        version: Version::new(1, 0),
        state: LifecycleState::Released,
        created: chrono::Utc::now(),
        author: "caner@alplab".into(),
        message: "initial release".into(),
        schematic: SchematicSide::default(),
        pcb: PcbSide::default(),
        shared: SharedSide {
            mpn: "RC0805FR-0710KL".into(),
            manufacturer: "Yageo".into(),
            description: "Resistor 10k 1% 0805".into(),
            ..Default::default()
        },
        content_hash: [0u8; 32],
    };
    rev.refresh_content_hash();

    let file = SnxPartFile {
        schema: 1,
        uuid: Uuid::now_v7(),
        internal_pn: InternalPn::new("R0805_10k"),
        revision: rev.clone(),
    };

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(snxpart_filename(file.uuid, rev.version));
    write_snxpart(&path, &file).unwrap();
    let back = read_snxpart(&path).unwrap();
    assert_eq!(file, back);
    assert_ne!(back.revision.content_hash, [0u8; 32]);
}

#[test]
fn shared_slice_used_for_embed_is_round_trippable() {
    let mut params = ParamMap::new();
    params.insert("value".into(), ParamValue::Text("10k".into()));
    let s = SharedSide {
        parameters: params,
        ..Default::default()
    };
    let slice = s.slice_for_embed();
    let json = serde_json::to_string(&slice).unwrap();
    let back: SharedSlice = serde_json::from_str(&json).unwrap();
    assert_eq!(slice, back);
}
