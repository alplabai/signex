use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::engine::parser::{self, LibSymbol, SymbolMeta};

// In-memory cache for parsed libraries
static LIBRARY_CACHE: std::sync::LazyLock<Mutex<HashMap<String, Vec<(LibSymbol, SymbolMeta)>>>> =
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
    pub pin_count: usize,
}

/// Find KiCad symbol library directory
fn find_kicad_symbols_dir() -> Option<PathBuf> {
    // Standard Windows paths
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

    // Check KICAD_SYMBOL_DIR env var
    if let Ok(dir) = std::env::var("KICAD_SYMBOL_DIR") {
        let path = Path::new(&dir);
        if path.is_dir() {
            return Some(path.to_path_buf());
        }
    }

    None
}

#[tauri::command]
pub async fn list_libraries() -> Result<Vec<LibraryInfo>, String> {
    tokio::task::spawn_blocking(|| {
        let dir = find_kicad_symbols_dir()
            .ok_or_else(|| "KiCad symbol libraries not found. Install KiCad or set KICAD_SYMBOL_DIR.".to_string())?;

        let mut libs = Vec::new();
        let entries = std::fs::read_dir(&dir)
            .map_err(|e| format!("Failed to read symbols dir: {}", e))?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("kicad_sym") {
                let name = path.file_stem()
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
        let cache = LIBRARY_CACHE.lock().map_err(|e| e.to_string())?;
        if let Some(cached) = cache.get(path) {
            return Ok(cached.clone());
        }
    }

    // Parse the file
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {}", path, e))?;
    let symbols = parser::parse_symbol_library(&content)?;

    // Store in cache
    {
        let mut cache = LIBRARY_CACHE.lock().map_err(|e| e.to_string())?;
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
            let lib_name = path.file_stem()
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
                        pin_count: meta.pin_count,
                    });
                    if results.len() >= limit as usize {
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

#[tauri::command]
pub async fn get_symbol(library_path: String, symbol_id: String) -> Result<LibSymbol, String> {
    tokio::task::spawn_blocking(move || {
        let symbols = load_library(&library_path)?;

        symbols
            .into_iter()
            .find(|(_, meta)| meta.symbol_id == symbol_id)
            .map(|(lib, _)| lib)
            .ok_or_else(|| format!("Symbol '{}' not found in library", symbol_id))
    })
    .await
    .map_err(|e| format!("Task failed: {}", e))?
}
