//! 3D tab editor state — storage-side metadata for the uploaded
//! STEP / WRL / GLB / glTF model.
//!
//! The actual storage IO (writing the bytes to
//! `shared/3d-models/<hash>.<ext>`) ships with WS-A's local-git
//! adapter — this UI module only mints the [`signex_library::ModelRef`]
//! pointer + a sidecar [`Model3dUploadInfo`] record so we can show the
//! filename and bytes-on-disk in the placeholder card.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Hash + filename + raw byte length captured at upload time. Lives
/// alongside `PcbSide.model_3d` on the working draft so the UI can
/// surface "model.step (123 KB)" without re-reading the file.
///
/// Persisting this sidecar to `.snxpart` is a Phase-3 concern —
/// today it's purely UI scratchpad and never round-trips through the
/// adapter.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct Model3dUploadInfo {
    /// Source filename as the user picked it (basename only, no path).
    pub filename: String,
    /// SHA-256 of the file bytes, lowercase hex (matches
    /// [`hash_bytes_hex`]).
    pub hash_hex: String,
    /// Raw file size in bytes.
    pub size_bytes: u64,
    /// Lowercased extension without the leading dot
    /// (`"step"` / `"stp"` / `"wrl"` / `"glb"` / `"gltf"`).
    pub extension: String,
}

impl Model3dUploadInfo {
    /// Build the relative storage path WS-A's adapter will write to:
    /// `shared/3d-models/<hash>.<extension>`.
    pub fn storage_path(&self) -> String {
        format!("shared/3d-models/{}.{}", self.hash_hex, self.extension)
    }

    /// Pretty-printed bytes-on-disk — `"123 B"`, `"4.5 KB"`, etc.
    /// Mirrors the rough-cut formatter used elsewhere in the app for
    /// import / export progress lines.
    pub fn human_size(&self) -> String {
        format_size(self.size_bytes)
    }
}

/// Compute the SHA-256 of `bytes`, returning lowercase hex. Single
/// canonical helper — every UI surface (datasheet, 3D, …) routes
/// through this to stay deterministic.
pub fn hash_bytes_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let result = hasher.finalize();
    // 64 hex chars + nul-cap → preallocate.
    let mut out = String::with_capacity(64);
    for byte in result.iter() {
        use std::fmt::Write as _;
        let _ = write!(&mut out, "{byte:02x}");
    }
    out
}

/// Recognised 3D model extensions. Filtering happens at the file-
/// dialog layer; this is the post-pick validation gate.
pub const SUPPORTED_EXTENSIONS: &[&str] = &["step", "stp", "wrl", "glb", "gltf"];

/// True when `ext` (case-insensitive, without leading dot) is one of
/// the [`SUPPORTED_EXTENSIONS`].
pub fn is_supported_extension(ext: &str) -> bool {
    let lower = ext.to_ascii_lowercase();
    SUPPORTED_EXTENSIONS.contains(&lower.as_str())
}

/// Format `bytes` as a human-readable string. Single-decimal
/// precision past KB.
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_bytes_hex_matches_known_sha256() {
        // SHA-256("abc") = ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
        let h = hash_bytes_hex(b"abc");
        assert_eq!(
            h,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn hash_bytes_hex_empty_input() {
        // SHA-256("") = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        let h = hash_bytes_hex(b"");
        assert_eq!(
            h,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn hash_bytes_hex_lowercase_only() {
        let h = hash_bytes_hex(&[0xff, 0xee, 0xdd]);
        assert!(h.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()));
    }

    #[test]
    fn supported_extensions_round_trip() {
        for ext in SUPPORTED_EXTENSIONS {
            assert!(is_supported_extension(ext));
            // Case-insensitive.
            assert!(is_supported_extension(&ext.to_ascii_uppercase()));
        }
        assert!(!is_supported_extension("png"));
        assert!(!is_supported_extension("pdf"));
    }

    #[test]
    fn storage_path_contains_hash_and_ext() {
        let info = Model3dUploadInfo {
            filename: "Component.STEP".into(),
            hash_hex: "deadbeef".repeat(8),
            size_bytes: 4096,
            extension: "step".into(),
        };
        let p = info.storage_path();
        assert!(p.starts_with("shared/3d-models/"));
        assert!(p.ends_with(".step"));
        assert!(p.contains(&info.hash_hex));
    }

    #[test]
    fn human_size_thresholds() {
        let case = |b: u64, want: &str| {
            let info = Model3dUploadInfo {
                filename: String::new(),
                hash_hex: String::new(),
                size_bytes: b,
                extension: "step".into(),
            };
            assert_eq!(info.human_size(), want);
        };
        case(0, "0 B");
        case(512, "512 B");
        case(2048, "2.0 KB");
        case(2 * 1024 * 1024, "2.0 MB");
    }

    #[test]
    fn upload_info_round_trips_via_json() {
        let original = Model3dUploadInfo {
            filename: "fpv-cam.step".into(),
            hash_hex: "abc123".into(),
            size_bytes: 12_345,
            extension: "step".into(),
        };
        let json = serde_json::to_string(&original).unwrap();
        let back: Model3dUploadInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(original, back);
    }
}
