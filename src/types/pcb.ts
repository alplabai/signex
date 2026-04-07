// ═══════════════════════════════════════════════════════════════
// PCB Data Model — KiCad .kicad_pcb compatible
// ═══════════════════════════════════════════════════════════════

export interface PcbPoint {
  x: number;
  y: number;
}

// --- Layer System ---

export type CopperLayerId =
  | "F.Cu" | "In1.Cu" | "In2.Cu" | "In3.Cu" | "In4.Cu" | "In5.Cu"
  | "In6.Cu" | "In7.Cu" | "In8.Cu" | "In9.Cu" | "In10.Cu" | "In11.Cu"
  | "In12.Cu" | "In13.Cu" | "In14.Cu" | "In15.Cu" | "In16.Cu" | "In17.Cu"
  | "In18.Cu" | "In19.Cu" | "In20.Cu" | "In21.Cu" | "In22.Cu" | "In23.Cu"
  | "In24.Cu" | "In25.Cu" | "In26.Cu" | "In27.Cu" | "In28.Cu" | "In29.Cu"
  | "In30.Cu" | "B.Cu";

export type TechLayerId =
  | "F.SilkS" | "B.SilkS"
  | "F.Mask" | "B.Mask"
  | "F.Paste" | "B.Paste"
  | "F.Fab" | "B.Fab"
  | "F.CrtYd" | "B.CrtYd"
  | "F.Adhes" | "B.Adhes"
  | "Edge.Cuts"
  | "Margin"
  | "Dwgs.User" | "Cmts.User" | "Eco1.User" | "Eco2.User";

export type PcbLayerId = CopperLayerId | TechLayerId | string;

export interface PcbLayer {
  id: PcbLayerId;
  name: string;
  type: "signal" | "power" | "mixed" | "jumper" | "user";
  visible: boolean;
  color: string;
  opacity: number;
}

export interface LayerStack {
  layers: PcbLayer[];
  copperCount: number; // 2, 4, 6, ... 32
}

// --- Board ---

export interface PcbBoard {
  uuid: string;
  version: string;
  generator: string;
  thickness: number; // Board thickness in mm
  outline: PcbPoint[]; // Board outline polygon
  layers: LayerStack;
  setup: PcbSetup;
}

export interface PcbSetup {
  gridSize: number;
  traceWidth: number; // Default trace width
  viaDiameter: number;
  viaDrill: number;
  clearance: number; // Default clearance
  trackMinWidth: number;
  viaMinDiameter: number;
  viaMinDrill: number;
  copperFinish: string;
}

// --- Footprints (Components on PCB) ---

export interface PcbFootprint {
  uuid: string;
  reference: string; // "U1", "R1", etc.
  value: string;
  footprintId: string; // Library footprint ID
  position: PcbPoint;
  rotation: number;
  layer: PcbLayerId; // "F.Cu" or "B.Cu"
  locked: boolean;
  pads: PcbPad[];
  graphics: PcbGraphic[];
  courtyard?: PcbPoint[]; // Courtyard polygon
  model3d?: string; // STEP model path
}

export interface PcbPad {
  uuid: string;
  number: string; // "1", "2", "A1", etc.
  type: "thru_hole" | "smd" | "np_thru_hole" | "connect";
  shape: "circle" | "rect" | "oval" | "roundrect" | "custom" | "trapezoid";
  position: PcbPoint; // Relative to footprint
  size: [number, number]; // [width, height]
  drill?: { diameter: number; shape?: "circle" | "oval"; offset?: PcbPoint };
  layers: PcbLayerId[];
  net?: { number: number; name: string };
  roundrectRatio?: number;
  thermalGap?: number;
  thermalWidth?: number;
  solderMaskExpansion?: number;
  pasteMaskExpansion?: number;
}

export type PcbGraphic =
  | { type: "line"; start: PcbPoint; end: PcbPoint; layer: PcbLayerId; width: number }
  | { type: "rect"; start: PcbPoint; end: PcbPoint; layer: PcbLayerId; width: number; fill?: boolean }
  | { type: "circle"; center: PcbPoint; radius: number; layer: PcbLayerId; width: number; fill?: boolean }
  | { type: "arc"; start: PcbPoint; mid: PcbPoint; end: PcbPoint; layer: PcbLayerId; width: number }
  | { type: "poly"; points: PcbPoint[]; layer: PcbLayerId; width: number; fill?: boolean }
  | { type: "text"; text: string; position: PcbPoint; layer: PcbLayerId; fontSize: number; rotation: number; mirror?: boolean };

// --- Traces & Vias ---

export interface PcbTrace {
  uuid: string;
  net: number; // Net number
  segments: PcbSegment[];
}

export interface PcbSegment {
  uuid: string;
  start: PcbPoint;
  end: PcbPoint;
  width: number;
  layer: PcbLayerId;
  net: number;
}

export interface PcbVia {
  uuid: string;
  position: PcbPoint;
  diameter: number;
  drill: number;
  layers: [PcbLayerId, PcbLayerId]; // [start_layer, end_layer]
  net: number;
  type: "through" | "blind" | "buried" | "micro";
}

// --- Copper Zones (Pours) ---

export interface PcbZone {
  uuid: string;
  net: number;
  netName: string;
  layer: PcbLayerId;
  outline: PcbPoint[];
  priority: number;
  fillType: "solid" | "hatch" | "none";
  thermalRelief: boolean;
  thermalGap: number;
  thermalWidth: number;
  clearance: number;
  minThickness: number;
  keepout?: {
    tracks: boolean;
    vias: boolean;
    pads: boolean;
    copperpour: boolean;
    footprints: boolean;
  };
  filled?: PcbPoint[][]; // Filled polygons (computed)
}

// --- Nets ---

export interface PcbNet {
  number: number;
  name: string;
}

// --- Ratsnest ---

export interface RatsnestLine {
  padA: { footprintUuid: string; padNumber: string };
  padB: { footprintUuid: string; padNumber: string };
  net: number;
}

// --- Design Rules ---

export interface PcbDesignRule {
  uuid: string;
  name: string;
  type: "clearance" | "trace_width" | "via_size" | "diff_pair" | "annular_ring" | "hole_size";
  scope: {
    kind: "all" | "net_class" | "net" | "layer";
    target?: string;
  };
  min?: number;
  preferred?: number;
  max?: number;
  enabled: boolean;
}

// --- DRC ---

export interface DrcViolation {
  uuid: string;
  type: "clearance" | "unrouted" | "short_circuit" | "annular_ring" | "minimum_width" | "drill_size" | "board_outline";
  severity: "error" | "warning";
  message: string;
  position: PcbPoint;
  layer?: PcbLayerId;
  objects: string[]; // UUIDs of involved objects
}

// --- Full PCB Document ---

export interface PcbData {
  board: PcbBoard;
  footprints: PcbFootprint[];
  segments: PcbSegment[]; // All trace segments (flat, not grouped by trace)
  vias: PcbVia[];
  zones: PcbZone[];
  nets: PcbNet[];
  graphics: PcbGraphic[]; // Board-level graphics (dimensions, drawings)
  texts: { uuid: string; text: string; position: PcbPoint; layer: PcbLayerId; fontSize: number; rotation: number }[];
  designRules: PcbDesignRule[];
}

// --- Layer Colors (Altium-inspired) ---

export const DEFAULT_LAYER_COLORS: Record<string, string> = {
  "F.Cu": "#cc0000",     // Red
  "B.Cu": "#0000cc",     // Blue
  "In1.Cu": "#c8c800",   // Yellow
  "In2.Cu": "#00c8c8",   // Cyan
  "In3.Cu": "#c800c8",   // Magenta
  "In4.Cu": "#00c800",   // Green
  "F.SilkS": "#c8c8c8",  // Light gray
  "B.SilkS": "#808080",  // Dark gray
  "F.Mask": "#800080",   // Purple (transparent)
  "B.Mask": "#008080",   // Teal (transparent)
  "F.Paste": "#808000",  // Olive
  "B.Paste": "#008000",  // Dark green
  "F.Fab": "#aaaa00",    // Yellow-green
  "B.Fab": "#00aaaa",    // Cyan-green
  "F.CrtYd": "#c8c800",  // Yellow outline
  "B.CrtYd": "#00c8c8",  // Cyan outline
  "Edge.Cuts": "#c8c800", // Yellow
  "Dwgs.User": "#808080",
  "Cmts.User": "#606060",
};
