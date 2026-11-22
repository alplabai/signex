//! The uuid of a board item a pattern instance baked to. It is derived, so
//! an unchanged sketch re-bakes equal, and it names its source entity, so a
//! re-bake finds the instances of an earlier bake on the board even when the
//! pattern has since shrunk or gone. See `docs/PCB_SKETCH.md`, "Patterned
//! board items".

use uuid::{Builder, Uuid};

use crate::sketch::array::ArrayId;

const FNV_OFFSET: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
const FNV_PRIME: u128 = (1 << 88) + 0x13b;

/// The bits of an instance uuid that name its source: the high half without
/// the version nibble.
const OWNER_BITS: u128 = 0xffff_ffff_ffff_0fff_0000_0000_0000_0000;

/// The version [`Builder::from_custom_bytes`] stamps on an instance uuid.
const INSTANCE_VERSION: usize = 8;

fn fnv(bytes: impl Iterator<Item = u8>) -> u128 {
    bytes.fold(FNV_OFFSET, |hash, byte| {
        (hash ^ u128::from(byte)).wrapping_mul(FNV_PRIME)
    })
}

/// The key every instance of `source` carries.
pub fn instance_owner_key(source: Uuid) -> u128 {
    fnv(source.as_bytes().iter().copied()) & OWNER_BITS
}

/// The owner key `item` carries; `None` when it is not an instance uuid.
pub fn owner_key_of(item: Uuid) -> Option<u128> {
    (item.get_version_num() == INSTANCE_VERSION).then(|| item.as_u128() & OWNER_BITS)
}

/// The uuid of instance `(i, j)` of `pattern` repeating `source`.
pub fn pattern_instance_uuid(source: Uuid, pattern: ArrayId, i: u32, j: u32) -> Uuid {
    let instance = fnv(source
        .as_bytes()
        .iter()
        .chain(pattern.0.as_bytes())
        .chain(&i.to_le_bytes())
        .chain(&j.to_le_bytes())
        .copied());
    let bits = instance_owner_key(source) | (instance & u128::from(u64::MAX));
    Builder::from_custom_bytes(bits.to_be_bytes()).into_uuid()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: Uuid = Uuid::from_u128(0x0a01);
    const OTHER: Uuid = Uuid::from_u128(0x0a02);
    const PATTERN: ArrayId = ArrayId(Uuid::from_u128(0x0b01));

    #[test]
    fn an_instance_names_its_source_and_no_other() {
        let item = pattern_instance_uuid(SOURCE, PATTERN, 3, 1);
        assert_eq!(owner_key_of(item), Some(instance_owner_key(SOURCE)));
        assert_ne!(owner_key_of(item), Some(instance_owner_key(OTHER)));
    }

    #[test]
    fn instances_differ_and_rebuild_equal() {
        let first = pattern_instance_uuid(SOURCE, PATTERN, 1, 0);
        assert_ne!(first, pattern_instance_uuid(SOURCE, PATTERN, 2, 0));
        assert_ne!(first, pattern_instance_uuid(SOURCE, PATTERN, 0, 1));
        assert_eq!(first, pattern_instance_uuid(SOURCE, PATTERN, 1, 0));
    }

    #[test]
    fn a_uuid_of_another_version_is_no_instance() {
        assert_eq!(owner_key_of(SOURCE), None);
        assert_eq!(owner_key_of(Uuid::now_v7()), None);
    }
}
