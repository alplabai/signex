//! Function audit, wave C: mockup rows 152 to 158 (signal harnesses, net
//! directives, Compiled Design, Change Component and Smart Paste, schematic
//! tables, Bezier and dimension, named unions). Each row's controls are
//! driven through `Signex::update` and checked by what they change.

#[allow(dead_code)]
#[path = "../regression/library_place_symbol.rs"]
mod library_place_symbol;

#[path = "c_nets/m152_harness.rs"]
mod m152_harness;
#[path = "c_nets/m153_classes.rs"]
mod m153_classes;
#[path = "c_nets/m153_directives.rs"]
mod m153_directives;
#[path = "c_nets/m154_compiled.rs"]
mod m154_compiled;
#[path = "c_nets/m155_change.rs"]
mod m155_change;
#[path = "c_nets/m155_paste.rs"]
mod m155_paste;
#[path = "c_nets/m156_tables.rs"]
mod m156_tables;
#[path = "c_nets/m157_annotation.rs"]
mod m157_annotation;
#[path = "c_nets/m158_unions.rs"]
mod m158_unions;
#[path = "c_nets/support.rs"]
mod support;
