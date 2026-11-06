//! The one-click placements as plain-value methods (#1930): power port,
//! no-connect, junction and no-ERC directive. Each takes an already-snapped
//! point and says whether it placed anything.

use signex_types::schematic::{Junction, NoConnect, Point};

use super::super::super::*;

impl Signex {
    /// Place the power port `net_name` drawn as library symbol `lib_id`.
    pub(crate) fn place_power_at(&mut self, at: Point, net_name: &str, lib_id: &str) -> bool {
        // The sheet keeps an older entry for this style (register inserts
        // only when absent): the text follows that one.
        let held_entry = self
            .document_state
            .active_engine()
            .and_then(|e| e.document().lib_symbols.get(lib_id).cloned())
            .unwrap_or_else(|| signex_types::schematic::power_port_lib_symbol(lib_id));
        let sym = signex_types::schematic::Symbol {
            unknown_columns: Default::default(),
            unknown: Default::default(),
            uuid: uuid::Uuid::now_v7(),
            lib_id: lib_id.to_string(),
            reference: "#PWR?".to_string(),
            value: net_name.to_string(),
            footprint: String::new(),
            datasheet: String::new(),
            position: at,
            rotation: 0.0,
            mirror_x: false,
            mirror_y: false,
            unit: 1,
            body_style: Default::default(),
            is_power: true,
            ref_text: None,
            val_text: Some(signex_types::schematic::power_port_value_field_for(
                &held_entry,
                at,
            )),
            fields_autoplaced: true,
            fields_user_placed: false,
            dnp: false,
            in_bom: false,
            on_board: true,
            exclude_from_sim: false,
            locked: false,
            fields: std::collections::HashMap::new(),
            custom_properties: Vec::new(),
            pin_uuids: std::collections::HashMap::new(),
            instances: Vec::new(),
            library_id: None,
            row_id: None,
            library_version: String::new(),
            library_stamp: None,
        };
        self.apply_engine_commands(signex_engine::place_power_port(sym), false, false)
    }

    pub(crate) fn place_no_connect_at(&mut self, at: Point) -> bool {
        let no_connect = NoConnect {
            uuid: uuid::Uuid::now_v7(),
            position: at,
        };
        self.apply_engine_command(
            signex_engine::Command::PlaceNoConnect { no_connect },
            false,
            false,
        )
    }

    /// A junction dot anywhere: the net builder accepts one off a wire too
    /// (it joins nothing). A second dot on the same point (the net builder's
    /// 1 um key) joins nothing new and would only stack in the file, so it
    /// is not placed.
    pub(crate) fn place_junction_at(&mut self, at: Point) -> bool {
        let already_there = self.document_state.active_engine().is_some_and(|engine| {
            engine
                .document()
                .junctions
                .iter()
                .any(|j| signex_net::pt_key(&j.position) == signex_net::pt_key(&at))
        });
        if already_there {
            return false;
        }
        let junction = Junction {
            uuid: uuid::Uuid::now_v7(),
            position: at,
            diameter: 0.0,
            minted: false,
            unknown: Default::default(),
            unknown_columns: Default::default(),
        };
        self.apply_engine_command(
            signex_engine::Command::PlaceJunction { junction },
            false,
            false,
        )
    }

    pub(crate) fn place_no_erc_directive_at(&mut self, at: Point) -> bool {
        let directive = NoConnect {
            uuid: uuid::Uuid::now_v7(),
            position: at,
        };
        self.apply_engine_command(
            signex_engine::Command::PlaceNoErcDirective { directive },
            false,
            false,
        )
    }
}
