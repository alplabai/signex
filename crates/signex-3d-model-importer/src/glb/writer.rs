use crate::error::ModelImportError;

/// Serializes a GLB 2.0 binary container from a JSON chunk and an optional
/// binary buffer chunk. Follows the glTF 2.0 specification (Khronos, Apache 2.0).
///
/// GLB layout:
///   12-byte header | JSON chunk (padded to 4 bytes) | BIN chunk (optional, padded)
pub fn write_glb(json_bytes: &[u8], bin_bytes: &[u8]) -> Result<Vec<u8>, ModelImportError> {
    const GLB_MAGIC: u32 = 0x46546C67;
    const GLB_VERSION: u32 = 2;
    const CHUNK_JSON: u32 = 0x4E4F534A;
    const CHUNK_BIN: u32 = 0x004E4942;

    let json_padded = pad4(json_bytes, b' ');
    let bin_padded = pad4(bin_bytes, 0u8);

    let json_chunk_len = 8 + json_padded.len();
    let bin_chunk_len = if bin_bytes.is_empty() { 0 } else { 8 + bin_padded.len() };
    let total_len = 12 + json_chunk_len + bin_chunk_len;

    let mut out = Vec::with_capacity(total_len);

    // Header
    out.extend_from_slice(&GLB_MAGIC.to_le_bytes());
    out.extend_from_slice(&GLB_VERSION.to_le_bytes());
    out.extend_from_slice(&(total_len as u32).to_le_bytes());

    // JSON chunk
    out.extend_from_slice(&(json_padded.len() as u32).to_le_bytes());
    out.extend_from_slice(&CHUNK_JSON.to_le_bytes());
    out.extend_from_slice(&json_padded);

    // BIN chunk (only if non-empty)
    if !bin_bytes.is_empty() {
        out.extend_from_slice(&(bin_padded.len() as u32).to_le_bytes());
        out.extend_from_slice(&CHUNK_BIN.to_le_bytes());
        out.extend_from_slice(&bin_padded);
    }

    Ok(out)
}

/// Pad a byte slice to the next 4-byte boundary using `fill`.
fn pad4<T: Copy + Into<u8>>(data: &[u8], fill: T) -> Vec<u8> {
    let pad = (4 - (data.len() % 4)) % 4;
    let mut out = data.to_vec();
    for _ in 0..pad {
        out.push(fill.into());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glb_header_magic_and_version() {
        let json = br#"{"asset":{"version":"2.0"}}"#;
        let glb = write_glb(json, &[]).expect("write failed");
        assert_eq!(&glb[0..4], b"glTF");
        assert_eq!(u32::from_le_bytes(glb[4..8].try_into().unwrap()), 2);
    }

    #[test]
    fn glb_total_length_matches_byte_count() {
        let json = br#"{"asset":{"version":"2.0"}}"#;
        let glb = write_glb(json, &[]).expect("write failed");
        let reported = u32::from_le_bytes(glb[8..12].try_into().unwrap()) as usize;
        assert_eq!(reported, glb.len());
    }

    #[test]
    fn glb_json_chunk_type_is_correct() {
        let json = br#"{"asset":{"version":"2.0"}}"#;
        let glb = write_glb(json, &[]).expect("write failed");
        let chunk_type = u32::from_le_bytes(glb[16..20].try_into().unwrap());
        assert_eq!(chunk_type, 0x4E4F534A); // JSON
    }

    #[test]
    fn glb_with_bin_chunk_has_correct_structure() {
        let json = br#"{"asset":{"version":"2.0"}}"#;
        let bin = vec![0u8; 12]; // 12-byte buffer
        let glb = write_glb(json, &bin).expect("write failed");
        let reported = u32::from_le_bytes(glb[8..12].try_into().unwrap()) as usize;
        assert_eq!(reported, glb.len());
        // Find BIN chunk type after JSON chunk
        let json_chunk_len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
        let bin_type_offset = 12 + 8 + json_chunk_len;
        let bin_chunk_type = u32::from_le_bytes(
            glb[bin_type_offset + 4..bin_type_offset + 8].try_into().unwrap()
        );
        assert_eq!(bin_chunk_type, 0x004E4942); // BIN
    }

    #[test]
    fn glb_json_padded_to_4_byte_boundary() {
        // JSON that is NOT already 4-byte aligned
        let json = b"{}"; // 2 bytes → padded to 4
        let glb = write_glb(json, &[]).expect("write failed");
        let json_chunk_data_len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
        assert_eq!(json_chunk_data_len % 4, 0);
    }
}
