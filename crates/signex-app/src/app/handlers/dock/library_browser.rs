use anyhow::{Context, Result};

use super::super::super::*;

impl Signex {
    pub(super) fn handle_dock_library_browser_message(
        &mut self,
        panel_msg: &crate::panels::PanelMsg,
    ) -> bool {
        match panel_msg {
            crate::panels::PanelMsg::SelectLibrary(name) => {
                if let Err(error) = self.load_library_browser_state(name.clone()) {
                    crate::diagnostics::log_error("Failed to load selected library", &error);
                }
                true
            }
            crate::panels::PanelMsg::SelectComponent(lib_id) => {
                self.select_library_component(lib_id.clone());
                true
            }
            _ => false,
        }
    }

    fn load_library_browser_state(&mut self, selected_library: String) -> Result<()> {
        let Some(library_root) = self.document_state.kicad_lib_dir.clone() else {
            crate::diagnostics::log_warning(
                "Library selection ignored because no KiCad library directory is configured",
            );
            return Ok(());
        };

        let library_names: Vec<String> = if selected_library == helpers::ALL_LIBRARIES {
            self.document_state
                .panel_ctx
                .kicad_libraries
                .iter()
                .filter(|entry| entry.as_str() != helpers::ALL_LIBRARIES)
                .cloned()
                .collect()
        } else {
            vec![selected_library.clone()]
        };

        let mut loaded_symbols = std::collections::HashMap::new();
        let mut library_entries = Vec::new();

        for library_name in library_names {
            let library_path = library_root.join(format!("{library_name}.kicad_sym"));
            if let Err(error) = Self::append_library_symbols(
                &library_path,
                &library_name,
                &mut library_entries,
                &mut loaded_symbols,
            ) {
                crate::diagnostics::log_error("Failed to load KiCad symbol library", &error);
            }
        }

        library_entries.sort_by(|left, right| {
            left.symbol_name
                .cmp(&right.symbol_name)
                .then_with(|| left.library_name.cmp(&right.library_name))
        });

        self.document_state.panel_ctx.library_symbols = library_entries;
        self.document_state.panel_ctx.active_library = Some(selected_library);
        self.document_state.panel_ctx.selected_component = None;
        self.document_state.panel_ctx.selected_pins.clear();
        self.document_state.panel_ctx.selected_lib_symbol = None;
        self.document_state.loaded_lib = loaded_symbols;
        Ok(())
    }

    fn append_library_symbols(
        library_path: &std::path::Path,
        library_name: &str,
        library_entries: &mut Vec<crate::panels::LibrarySymbolEntry>,
        loaded_symbols: &mut std::collections::HashMap<String, signex_types::schematic::LibSymbol>,
    ) -> Result<()> {
        let content = std::fs::read_to_string(library_path)
            .with_context(|| format!("read {}", library_path.display()))?;
        let parsed_library = kicad_parser::parse_symbol_lib(&content)
            .with_context(|| format!("parse symbol library {}", library_name))?;

        for (lib_id, lib_symbol) in parsed_library {
            library_entries.push(crate::panels::LibrarySymbolEntry {
                symbol_name: lib_id.rsplit(':').next().unwrap_or(&lib_id).to_string(),
                library_name: library_name.to_string(),
                pin_count: lib_symbol.pins.len(),
                lib_id: lib_id.clone(),
            });
            loaded_symbols.insert(lib_id, lib_symbol);
        }

        Ok(())
    }

    fn select_library_component(&mut self, lib_id: String) {
        if let Some(symbol) = self.document_state.loaded_lib.get(&lib_id) {
            let library_name = lib_id
                .split(':')
                .next()
                .unwrap_or(helpers::ALL_LIBRARIES)
                .to_string();
            self.document_state.panel_ctx.selected_component = Some(lib_id);
            self.document_state.panel_ctx.selected_pins = symbol
                .pins
                .iter()
                .map(|pin| {
                    (
                        pin.pin.number.clone(),
                        pin.pin.name.clone(),
                        format!("{:?}", pin.pin.pin_type),
                    )
                })
                .collect();
            self.document_state.panel_ctx.selected_lib_symbol = Some(symbol.clone());
            if self.document_state.panel_ctx.active_library.is_none() {
                self.document_state.panel_ctx.active_library = Some(library_name);
            }
        }
    }
}
