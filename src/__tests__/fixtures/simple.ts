import type { SchematicData, SchSymbol, LibSymbol } from "@/types";

/** A minimal resistor library symbol */
export const resistorLib: LibSymbol = {
  id: "Device:R",
  graphics: [
    { type: "Rectangle", start: { x: -1.016, y: -2.54 }, end: { x: 1.016, y: 2.54 }, width: 0.254, fill: false },
  ],
  pins: [
    { pin_type: "passive", shape: "line", position: { x: 0, y: 3.81 }, rotation: 270, length: 1.27, name: "~", number: "1", name_visible: false, number_visible: false },
    { pin_type: "passive", shape: "line", position: { x: 0, y: -3.81 }, rotation: 90, length: 1.27, name: "~", number: "2", name_visible: false, number_visible: false },
  ],
  show_pin_numbers: false,
  show_pin_names: false,
  pin_name_offset: 0,
};

/** A minimal capacitor library symbol */
export const capacitorLib: LibSymbol = {
  id: "Device:C",
  graphics: [
    { type: "Polyline", points: [{ x: -2.032, y: -0.762 }, { x: 2.032, y: -0.762 }], width: 0.508, fill: false },
    { type: "Polyline", points: [{ x: -2.032, y: 0.762 }, { x: 2.032, y: 0.762 }], width: 0.508, fill: false },
  ],
  pins: [
    { pin_type: "passive", shape: "line", position: { x: 0, y: 2.54 }, rotation: 270, length: 1.778, name: "~", number: "1", name_visible: false, number_visible: false },
    { pin_type: "passive", shape: "line", position: { x: 0, y: -2.54 }, rotation: 90, length: 1.778, name: "~", number: "2", name_visible: false, number_visible: false },
  ],
  show_pin_numbers: false,
  show_pin_names: false,
  pin_name_offset: 0,
};

const defaultTextProp = { position: { x: 0, y: 0 }, rotation: 0, font_size: 1.27, justify_h: "center", justify_v: "center", hidden: false };

function makeSymbol(_id: string, ref: string, value: string, x: number, y: number, libId: string): SchSymbol {
  return {
    uuid: `sym-${ref}`, lib_id: libId, reference: ref, value, footprint: "",
    position: { x, y }, rotation: 0, mirror_x: false, mirror_y: false, unit: 1, is_power: false,
    ref_text: { ...defaultTextProp, position: { x, y: y - 2 } },
    val_text: { ...defaultTextProp, position: { x, y: y + 2 } },
    fields_autoplaced: true,
    dnp: false, in_bom: true, on_board: true, exclude_from_sim: false, locked: false,
    fields: {},
  };
}

/**
 * Simple schematic: 2 resistors connected by wires with a net label
 *
 *   R1 (10,20) ---- wire ---- R2 (30,20)
 *                    |
 *                  "VCC" label at (20,20)
 */
export function createSimpleSchematic(): SchematicData {
  return {
    uuid: "test-simple",
    version: "20231120",
    generator: "test",
    generator_version: "0.1",
    paper_size: "A4",
    symbols: [
      makeSymbol("1", "R1", "10k", 10, 20, "Device:R"),
      makeSymbol("2", "R2", "10k", 30, 20, "Device:R"),
    ],
    wires: [
      { uuid: "w1", start: { x: 10, y: 17.46 }, end: { x: 20, y: 17.46 } },  // R1 pin1 tip → mid
      { uuid: "w2", start: { x: 20, y: 17.46 }, end: { x: 30, y: 17.46 } },  // mid → R2 pin1 tip
    ],
    junctions: [],
    labels: [
      { uuid: "l1", text: "VCC", position: { x: 20, y: 17.46 }, rotation: 0, label_type: "Net", shape: "", font_size: 1.27, justify: "left" },
    ],
    child_sheets: [],
    no_connects: [
      { uuid: "nc1", position: { x: 10, y: 22.54 } },  // R1 pin2 tip
    ],
    text_notes: [
      { uuid: "tn1", text: "Test note", position: { x: 50, y: 50 }, rotation: 0, font_size: 1.27 },
    ],
    rectangles: [],
    buses: [],
    bus_entries: [],
    drawings: [],
    no_erc_directives: [],
    title_block: {},
    net_classes: [],
    lib_symbols: {
      "Device:R": resistorLib,
      "Device:C": capacitorLib,
    },
  };
}

/**
 * Schematic with duplicate designators and ERC issues
 */
export function createErcTestSchematic(): SchematicData {
  const base = createSimpleSchematic();
  return {
    ...base,
    uuid: "test-erc",
    symbols: [
      ...base.symbols,
      makeSymbol("3", "R1", "4.7k", 50, 20, "Device:R"),  // Duplicate R1!
      makeSymbol("4", "C?", "100nF", 50, 40, "Device:C"),  // Unannotated
    ],
    wires: [
      ...base.wires,
      { uuid: "w3", start: { x: 50, y: 50 }, end: { x: 70, y: 50 } },  // Floating wire (no pins)
    ],
    no_connects: [],  // Remove no-connects so R1/R2 pin2 is unconnected
  };
}
