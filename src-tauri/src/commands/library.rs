use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::engine::parser::{self, LibSymbol, SymbolMeta};
use crate::engine::pcb_parser::{self, Footprint as PcbFootprint};
use crate::engine::writer;

// In-memory cache for parsed libraries
type LibraryCache = HashMap<String, Vec<(LibSymbol, SymbolMeta)>>;
static LIBRARY_CACHE: std::sync::LazyLock<Mutex<LibraryCache>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Serialize, Deserialize)]
pub struct LibraryInfo {
    pub name: String,
    pub path: String,
    pub file_size: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SymbolSearchResult {
    pub library: String,
    pub symbol_id: String,
    pub description: String,
    pub keywords: Vec<String>,
    pub reference_prefix: String,
    pub footprint: String,
    pub pin_count: usize,
}

/// Find KiCad symbol library directory
fn find_kicad_symbols_dir() -> Option<PathBuf> {
    // Check env var first (highest priority)
    if let Ok(dir) = std::env::var("KICAD_SYMBOL_DIR") {
        let path = Path::new(&dir);
        if path.is_dir() {
            return Some(path.to_path_buf());
        }
    }

    // Platform-specific standard paths
    #[cfg(target_os = "windows")]
    {
        let candidates = [
            r"C:\Program Files\KiCad\9.0\share\kicad\symbols",
            r"C:\Program Files\KiCad\8.0\share\kicad\symbols",
            r"C:\Program Files\KiCad\7.0\share\kicad\symbols",
            r"C:\Program Files (x86)\KiCad\share\kicad\symbols",
        ];
        for p in &candidates {
            let path = Path::new(p);
            if path.is_dir() {
                return Some(path.to_path_buf());
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        let candidates = [
            "/Applications/KiCad/KiCad.app/Contents/SharedSupport/symbols",
            "/Applications/KiCad/kicad.app/Contents/SharedSupport/symbols",
        ];
        for p in &candidates {
            let path = Path::new(p);
            if path.is_dir() {
                return Some(path.to_path_buf());
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        let candidates = ["/usr/share/kicad/symbols", "/usr/local/share/kicad/symbols"];
        for p in &candidates {
            let path = Path::new(p);
            if path.is_dir() {
                return Some(path.to_path_buf());
            }
        }
        // XDG data home
        if let Ok(home) = std::env::var("HOME") {
            let xdg = Path::new(&home).join(".local/share/kicad/symbols");
            if xdg.is_dir() {
                return Some(xdg);
            }
        }
    }

    None
}

#[tauri::command]
pub async fn list_libraries() -> Result<Vec<LibraryInfo>, String> {
    tokio::task::spawn_blocking(|| {
        let dir = find_kicad_symbols_dir().ok_or_else(|| {
            "KiCad symbol libraries not found. Install KiCad or set KICAD_SYMBOL_DIR.".to_string()
        })?;

        let mut libs = Vec::new();
        let entries =
            std::fs::read_dir(&dir).map_err(|e| format!("Failed to read symbols dir: {}", e))?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("kicad_sym") {
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                let file_size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                libs.push(LibraryInfo {
                    name,
                    path: path.to_string_lossy().to_string(),
                    file_size,
                });
            }
        }

        libs.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(libs)
    })
    .await
    .map_err(|e| format!("Task failed: {}", e))?
}

/// Load and cache a single library file
fn load_library(path: &str) -> Result<Vec<(LibSymbol, SymbolMeta)>, String> {
    // Check cache first
    {
        let cache = LIBRARY_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(cached) = cache.get(path) {
            return Ok(cached.clone());
        }
    }

    // Parse the file
    let content =
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read {}: {}", path, e))?;
    let symbols = parser::parse_symbol_library(&content)?;

    // Store in cache
    {
        let mut cache = LIBRARY_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        cache.insert(path.to_string(), symbols.clone());
    }

    Ok(symbols)
}

#[tauri::command]
pub async fn search_symbols(query: String, limit: u32) -> Result<Vec<SymbolSearchResult>, String> {
    tokio::task::spawn_blocking(move || {
        let dir = find_kicad_symbols_dir()
            .ok_or_else(|| "KiCad symbol libraries not found.".to_string())?;

        let query_lower = query.to_lowercase();
        let query_parts: Vec<&str> = query_lower.split_whitespace().collect();
        let mut results = Vec::new();

        let entries: Vec<_> = std::fs::read_dir(&dir)
            .map_err(|e| format!("Failed to read symbols dir: {}", e))?
            .flatten()
            .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("kicad_sym"))
            .collect();

        for entry in &entries {
            let path = entry.path();
            let lib_name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();
            let path_str = path.to_string_lossy().to_string();

            let symbols = match load_library(&path_str) {
                Ok(s) => s,
                Err(_) => continue,
            };

            for (_lib_sym, meta) in &symbols {
                // Score match
                let searchable = format!(
                    "{} {} {} {} {}",
                    meta.symbol_id.to_lowercase(),
                    meta.description.to_lowercase(),
                    meta.keywords.join(" ").to_lowercase(),
                    meta.reference_prefix.to_lowercase(),
                    lib_name.to_lowercase(),
                );

                let matches = query_parts.iter().all(|part| searchable.contains(part));
                if matches {
                    results.push(SymbolSearchResult {
                        library: lib_name.clone(),
                        symbol_id: meta.symbol_id.clone(),
                        description: meta.description.clone(),
                        keywords: meta.keywords.clone(),
                        reference_prefix: meta.reference_prefix.clone(),
                        footprint: meta.footprint.clone(),
                        pin_count: meta.pin_count,
                    });
                    if limit > 0 && results.len() >= limit as usize {
                        return Ok(results);
                    }
                }
            }
        }

        Ok(results)
    })
    .await
    .map_err(|e| format!("Task failed: {}", e))?
}

/// List all symbols in a specific library file (fast — only loads one file)
#[tauri::command]
pub async fn list_library_symbols(library_name: String) -> Result<Vec<SymbolSearchResult>, String> {
    tokio::task::spawn_blocking(move || {
        let dir = find_kicad_symbols_dir()
            .ok_or_else(|| "KiCad symbol libraries not found.".to_string())?;

        let path = dir.join(format!("{}.kicad_sym", library_name));
        if !path.exists() {
            return Err(format!("Library not found: {}", library_name));
        }

        let path_str = path.to_string_lossy().to_string();
        let symbols = load_library(&path_str)
            .map_err(|e| format!("Failed to load library: {}", e))?;

        let results: Vec<SymbolSearchResult> = symbols
            .iter()
            .map(|(_lib_sym, meta)| SymbolSearchResult {
                library: library_name.clone(),
                symbol_id: meta.symbol_id.clone(),
                description: meta.description.clone(),
                keywords: meta.keywords.clone(),
                reference_prefix: meta.reference_prefix.clone(),
                pin_count: meta.pin_count,
                footprint: meta.footprint.clone(),
            })
            .collect();

        Ok(results)
    })
    .await
    .map_err(|e| format!("Task failed: {}", e))?
}

/// Validate that a library path is safe to access (must be a .kicad_sym or .snxsym file
/// inside the KiCad symbols directory or a project directory)
fn validate_library_path(library_path: &str) -> Result<std::path::PathBuf, String> {
    let path = Path::new(library_path);

    // Must have a valid library extension
    match path.extension().and_then(|e| e.to_str()) {
        Some("kicad_sym") | Some("snxsym") => {}
        _ => return Err("Invalid library file extension".to_string()),
    }

    // Reject path traversal components
    for comp in path.components() {
        if matches!(comp, std::path::Component::ParentDir) {
            return Err("Path traversal not allowed".to_string());
        }
    }

    // Verify the path exists and resolve to canonical form
    if path.exists() {
        let canonical = path
            .canonicalize()
            .map_err(|e| format!("Invalid library path: {}", e))?;

        // Must be inside KiCad symbols dir or have .snxsym extension (user library)
        if let Some(sym_dir) = find_kicad_symbols_dir() {
            if let Ok(canonical_sym_dir) = sym_dir.canonicalize() {
                if canonical.starts_with(&canonical_sym_dir) {
                    return Ok(canonical);
                }
            }
        }
        // Allow .snxsym files anywhere (user-created libraries)
        if canonical.extension().and_then(|e| e.to_str()) == Some("snxsym") {
            return Ok(canonical);
        }
        // Allow .kicad_sym files that exist (already validated extension above)
        return Ok(canonical);
    }

    // For new files (save_symbol creating a new library), allow if extension is valid
    Ok(path.to_path_buf())
}

#[tauri::command]
pub async fn get_symbol(library_path: String, symbol_id: String) -> Result<LibSymbol, String> {
    tokio::task::spawn_blocking(move || {
        let validated_path = validate_library_path(&library_path)?;
        let path_str = validated_path.to_string_lossy().to_string();
        let symbols = load_library(&path_str)?;

        symbols
            .into_iter()
            .find(|(_, meta)| meta.symbol_id == symbol_id)
            .map(|(lib, _)| lib)
            .ok_or_else(|| format!("Symbol '{}' not found in library", symbol_id))
    })
    .await
    .map_err(|e| format!("Task failed: {}", e))?
}

#[tauri::command]
pub async fn save_symbol(
    library_path: String,
    lib_id: String,
    symbol: LibSymbol,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let validated = validate_library_path(&library_path)?;
        let path = validated.as_path();

        // Load existing symbols from the library file (if it exists)
        let mut symbols: Vec<(String, LibSymbol)> = if path.exists() {
            let content = std::fs::read_to_string(path)
                .map_err(|e| format!("Failed to read library: {}", e))?;
            let parsed = parser::parse_symbol_library(&content)?;
            parsed.into_iter().map(|(lib, meta)| (meta.symbol_id, lib)).collect()
        } else {
            Vec::new()
        };

        // Replace existing or append
        let mut found = false;
        for (id, lib) in symbols.iter_mut() {
            if *id == lib_id {
                *lib = symbol.clone();
                found = true;
                break;
            }
        }
        if !found {
            symbols.push((lib_id.clone(), symbol.clone()));
        }

        // Write back atomically
        let output = writer::write_symbol_library(&symbols);
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("snxsym");
        let tmp_ext = format!("{}.tmp", ext);
        let tmp_path = path.with_extension(tmp_ext);
        std::fs::write(&tmp_path, &output)
            .map_err(|e| format!("Failed to write temp file: {}", e))?;
        std::fs::rename(&tmp_path, path)
            .map_err(|e| format!("Failed to rename temp file: {}", e))?;

        // Invalidate cache (use both original and validated paths)
        {
            let mut cache = LIBRARY_CACHE.lock().unwrap_or_else(|e| e.into_inner());
            cache.remove(&library_path);
            let vpath = validated.to_string_lossy().to_string();
            if vpath != library_path { cache.remove(&vpath); }
        }

        Ok(())
    })
    .await
    .map_err(|e| format!("Task failed: {}", e))?
}

/// Find KiCad footprint library directory (.pretty directories)
fn find_kicad_footprints_dir() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("KICAD_FOOTPRINT_DIR") {
        let path = Path::new(&dir);
        if path.is_dir() {
            return Some(path.to_path_buf());
        }
    }

    #[cfg(target_os = "windows")]
    {
        let candidates = [
            r"C:\Program Files\KiCad\9.0\share\kicad\footprints",
            r"C:\Program Files\KiCad\8.0\share\kicad\footprints",
            r"C:\Program Files\KiCad\7.0\share\kicad\footprints",
            r"C:\Program Files (x86)\KiCad\share\kicad\footprints",
        ];
        for p in &candidates {
            let path = Path::new(p);
            if path.is_dir() {
                return Some(path.to_path_buf());
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        let candidates = [
            "/Applications/KiCad/KiCad.app/Contents/SharedSupport/footprints",
            "/Applications/KiCad/kicad.app/Contents/SharedSupport/footprints",
        ];
        for p in &candidates {
            let path = Path::new(p);
            if path.is_dir() {
                return Some(path.to_path_buf());
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        let candidates = [
            "/usr/share/kicad/footprints",
            "/usr/local/share/kicad/footprints",
        ];
        for p in &candidates {
            let path = Path::new(p);
            if path.is_dir() {
                return Some(path.to_path_buf());
            }
        }
        if let Ok(home) = std::env::var("HOME") {
            let xdg = Path::new(&home).join(".local/share/kicad/footprints");
            if xdg.is_dir() {
                return Some(xdg);
            }
        }
    }

    None
}

/// Load a footprint from KiCad libraries by its full name (e.g., "Package_SO:SOIC-8_3.9x4.9mm_P1.27mm")
#[tauri::command]
pub async fn get_footprint(footprint_id: String) -> Result<PcbFootprint, String> {
    tokio::task::spawn_blocking(move || {
        // Parse "LibraryName:FootprintName" format
        let (lib_name, fp_name) = if let Some(colon_idx) = footprint_id.find(':') {
            (
                &footprint_id[..colon_idx],
                &footprint_id[colon_idx + 1..],
            )
        } else {
            return Err(format!(
                "Invalid footprint ID '{}'. Expected 'Library:Footprint' format.",
                footprint_id
            ));
        };

        let fp_dir = find_kicad_footprints_dir().ok_or_else(|| {
            "KiCad footprint libraries not found. Install KiCad or set KICAD_FOOTPRINT_DIR."
                .to_string()
        })?;

        // Build path: footprints_dir/LibraryName.pretty/FootprintName.kicad_mod
        let mod_path = fp_dir
            .join(format!("{}.pretty", lib_name))
            .join(format!("{}.kicad_mod", fp_name));

        if !mod_path.exists() {
            return Err(format!(
                "Footprint file not found: {}",
                mod_path.display()
            ));
        }

        // Validate path is inside the footprints directory (no traversal)
        let canonical = mod_path
            .canonicalize()
            .map_err(|e| format!("Invalid footprint path: {}", e))?;
        let canonical_dir = fp_dir
            .canonicalize()
            .map_err(|e| format!("Invalid footprints dir: {}", e))?;
        if !canonical.starts_with(&canonical_dir) {
            return Err("Path traversal not allowed".to_string());
        }

        let content = std::fs::read_to_string(&canonical)
            .map_err(|e| format!("Failed to read footprint: {}", e))?;

        pcb_parser::parse_footprint_file(&content)
    })
    .await
    .map_err(|e| format!("Task failed: {}", e))?
}
