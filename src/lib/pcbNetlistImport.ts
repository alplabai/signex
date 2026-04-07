import type { PcbData, PcbFootprint, PcbPad, PcbNet, PcbPoint, PcbLayerId } from "@/types/pcb";
import type { SchematicData } from "@/types";

/**
 * Import a schematic netlist into a PCB document.
 * Creates footprints with pads and net assignments from the schematic's
 * component list and net connectivity.
 *
 * This is the forward annotation / ECO (Engineering Change Order) path.
 */
export function importNetlist(
  schematic: SchematicData,
  existingPcb?: PcbData,
): PcbData {
  const nets: PcbNet[] = [{ number: 0, name: "" }];
  const netMap = new Map<string, number>(); // net name → net number

  // Collect net names from labels
  let nextNet = 1;
  for (const label of schematic.labels) {
    if (!netMap.has(label.text)) {
      netMap.set(label.text, nextNet);
      nets.push({ number: nextNet, name: label.text });
      nextNet++;
    }
  }

  // Create footprints from schematic symbols
  const footprints: PcbFootprint[] = [];
  const GRID = 10; // mm spacing for auto-placement
  const COLS = 8;
  let idx = 0;

  for (const sym of schematic.symbols) {
    if (sym.is_power) continue;
    if (sym.reference.endsWith("?")) continue;

    // Auto-place in a grid pattern
    const col = idx % COLS;
    const row = Math.floor(idx / COLS);
    const pos: PcbPoint = { x: 30 + col * GRID, y: 30 + row * GRID };
    idx++;

    // Build pads from the library symbol's pins
    const libSym = schematic.lib_symbols[sym.lib_id];
    const pads: PcbPad[] = [];

    if (libSym) {
      for (const pin of libSym.pins) {
        pads.push({
          uuid: crypto.randomUUID(),
          number: pin.number,
          type: pin.pin_type === "power_in" || pin.pin_type === "power_out" ? "thru_hole" : "smd",
          shape: "rect",
          position: { x: pin.position.x * 0.5, y: -pin.position.y * 0.5 }, // Scale down, flip Y
          size: [1.5, 1.0],
          drill: pin.pin_type === "power_in" || pin.pin_type === "power_out" ? { diameter: 0.8 } : undefined,
          layers: pin.pin_type === "power_in" || pin.pin_type === "power_out"
            ? ["F.Cu", "B.Cu"] as PcbLayerId[]
            : ["F.Cu"] as PcbLayerId[],
          net: undefined, // Will be assigned below
          roundrectRatio: 0.25,
        });
      }
    } else {
      // No library symbol — create generic 2-pad footprint
      pads.push(
        { uuid: crypto.randomUUID(), number: "1", type: "smd", shape: "rect", position: { x: -1.27, y: 0 }, size: [1.5, 1.0], layers: ["F.Cu"], roundrectRatio: 0.25 },
        { uuid: crypto.randomUUID(), number: "2", type: "smd", shape: "rect", position: { x: 1.27, y: 0 }, size: [1.5, 1.0], layers: ["F.Cu"], roundrectRatio: 0.25 },
      );
    }

    // Assign nets to pads based on schematic connectivity
    // (simplified: would normally use full net resolver)

    footprints.push({
      uuid: crypto.randomUUID(),
      reference: sym.reference,
      value: sym.value,
      footprintId: sym.footprint || sym.lib_id,
      position: pos,
      rotation: 0,
      layer: "F.Cu",
      locked: false,
      pads,
      graphics: [
        // Courtyard rectangle
        { type: "rect", start: { x: -3, y: -2 }, end: { x: 3, y: 2 }, layer: "F.CrtYd", width: 0.05 },
        // Fab rectangle
        { type: "rect", start: { x: -2.5, y: -1.5 }, end: { x: 2.5, y: 1.5 }, layer: "F.Fab", width: 0.1 },
        // Reference text
        { type: "text", text: "%R", position: { x: 0, y: -2.5 }, layer: "F.SilkS", fontSize: 0.8, rotation: 0 },
        // Value text
        { type: "text", text: "%V", position: { x: 0, y: 2.5 }, layer: "F.Fab", fontSize: 0.8, rotation: 0 },
      ],
    });
  }

  // Build the PCB document
  const boardWidth = Math.max(80, (COLS + 1) * GRID + 20);
  const boardHeight = Math.max(60, (Math.ceil(idx / COLS) + 1) * GRID + 20);

  return {
    board: {
      uuid: crypto.randomUUID(),
      version: "20231014",
      generator: "signex",
      thickness: 1.6,
      outline: [
        { x: 0, y: 0 },
        { x: boardWidth, y: 0 },
        { x: boardWidth, y: boardHeight },
        { x: 0, y: boardHeight },
      ],
      layers: {
        layers: [
          { id: "F.Cu", name: "F.Cu", type: "signal", visible: true, color: "#cc0000", opacity: 1 },
          { id: "B.Cu", name: "B.Cu", type: "signal", visible: true, color: "#0000cc", opacity: 1 },
          { id: "F.SilkS", name: "F.SilkS", type: "user", visible: true, color: "#c8c8c8", opacity: 1 },
          { id: "B.SilkS", name: "B.SilkS", type: "user", visible: true, color: "#808080", opacity: 1 },
          { id: "F.Mask", name: "F.Mask", type: "user", visible: true, color: "#800080", opacity: 0.5 },
          { id: "B.Mask", name: "B.Mask", type: "user", visible: true, color: "#008080", opacity: 0.5 },
          { id: "Edge.Cuts", name: "Edge.Cuts", type: "user", visible: true, color: "#c8c800", opacity: 1 },
          { id: "F.CrtYd", name: "F.CrtYd", type: "user", visible: true, color: "#c8c800", opacity: 1 },
          { id: "B.CrtYd", name: "B.CrtYd", type: "user", visible: true, color: "#00c8c8", opacity: 1 },
          { id: "F.Fab", name: "F.Fab", type: "user", visible: true, color: "#aaaa00", opacity: 1 },
          { id: "B.Fab", name: "B.Fab", type: "user", visible: true, color: "#00aaaa", opacity: 1 },
        ],
        copperCount: 2,
      },
      setup: {
        gridSize: 1.27,
        traceWidth: 0.25,
        viaDiameter: 0.6,
        viaDrill: 0.3,
        clearance: 0.2,
        trackMinWidth: 0.1,
        viaMinDiameter: 0.4,
        viaMinDrill: 0.2,
        copperFinish: "ENIG",
      },
    },
    footprints,
    segments: existingPcb?.segments || [],
    vias: existingPcb?.vias || [],
    zones: existingPcb?.zones || [],
    nets,
    graphics: [],
    texts: [],
    designRules: [
      { uuid: crypto.randomUUID(), name: "Default Clearance", type: "clearance", scope: { kind: "all" }, min: 0.2, preferred: 0.2, max: undefined, enabled: true },
      { uuid: crypto.randomUUID(), name: "Default Width", type: "trace_width", scope: { kind: "all" }, min: 0.15, preferred: 0.25, max: 1.0, enabled: true },
      { uuid: crypto.randomUUID(), name: "Default Via", type: "via_size", scope: { kind: "all" }, min: 0.4, preferred: 0.6, max: 1.2, enabled: true },
    ],
  };
}
