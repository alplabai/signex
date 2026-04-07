export type EditorMode = "schematic" | "pcb" | "library" | "output" | "3d" | "simulation";

export type PanelId =
  | "projects"
  | "components"
  | "navigator"
  | "properties"
  | "filter"
  | "list"
  | "inspector"
  | "rules"
  | "messages"
  | "drc"
  | "ai-chat";

export type PanelPosition = "left" | "right" | "bottom";

export interface PanelConfig {
  id: PanelId;
  title: string;
  position: PanelPosition;
  visible: boolean;
  width?: number;
  height?: number;
}

export interface DocumentTab {
  id: string;
  name: string;
  type: DocumentType;
  path?: string;
  dirty: boolean;
}

export type DocumentType = "schematic" | "pcb" | "library" | "output-job" | "3d-view";

export interface ProjectInfo {
  name: string;
  path: string;
  dir: string;
  format: string;
  schematic_root: string | null;
  pcb_file: string | null;
  sheets: SheetInfo[];
}

export interface SheetInfo {
  name: string;
  filename: string;
  symbols_count: number;
  wires_count: number;
  labels_count: number;
}

export interface AppInfo {
  name: string;
  version: string;
}

export interface StatusBarState {
  cursorPosition: { x: number; y: number };
  gridSize: number;
  snapEnabled: boolean;
  units: "mil" | "mm" | "inch";
  activeLayer: string;
  currentMode: string;
  zoom: number;
}

// --- Schematic data from Rust parser ---

export interface SchematicData {
  uuid: string;
  version: string;
  generator: string;
  generator_version: string;
  paper_size: string;
  symbols: SchSymbol[];
  wires: SchWire[];
  junctions: SchJunction[];
  labels: SchLabel[];
  child_sheets: SchChildSheet[];
  no_connects: SchNoConnect[];
  text_notes: TextNote[];
  rectangles: SchRectangle[];
  buses: SchBus[];
  bus_entries: SchBusEntry[];
  drawings: SchDrawing[];
  no_erc_directives: SchNoErcDirective[];
  title_block: Record<string, string>;
  net_classes: NetClass[];
  variants: DesignVariant[];
  document_parameters: DocumentParameter[];
  groups: SchGroup[];
  differential_pairs: DifferentialPair[];
  signal_harnesses: SignalHarness[];
  constraints: DesignConstraint[];
  lib_symbols: Record<string, LibSymbol>;
}

export interface NetClass {
  name: string;
  nets: string[];
  color?: string;
}

export interface DesignVariant {
  name: string;
  description: string;
  // Per-component overrides: uuid → { fitted: boolean, altValue?: string, altFootprint?: string }
  components: Record<string, { fitted: boolean; altValue?: string; altFootprint?: string }>;
}

export interface DocumentParameter {
  key: string;
  value: string;
  scope: "document" | "project";
}

export interface DifferentialPair {
  name: string; // Common name (e.g., "USB_D")
  positiveNet: string; // Net name with _P suffix
  negativeNet: string; // Net name with _N suffix
  impedance?: number; // Target impedance in ohms
  netClass?: string; // Associated net class
}

export interface SignalHarness {
  uuid: string;
  name: string;
  type: string; // Harness type identifier
  members: HarnessMember[];
}

export interface HarnessMember {
  name: string;
  kind: "net" | "bus" | "harness"; // Can contain nets, buses, or nested harnesses
  ref?: string; // Reference to net name, bus name, or child harness uuid
}

export interface DesignConstraint {
  uuid: string;
  name: string;
  type: "clearance" | "trace_width" | "via_size" | "diff_pair_gap" | "length_match" | "custom";
  scope: ConstraintScope;
  value: number;
  unit: "mm" | "mil";
  enabled: boolean;
  priority: number; // Higher = overrides lower
}

export interface ConstraintScope {
  kind: "all" | "net_class" | "net" | "diff_pair" | "component" | "between";
  target?: string; // Net class name, net name, diff pair name, or component ref
  target2?: string; // Second target for "between" kind
}

export interface ProjectParameter {
  key: string;
  value: string;
}

export interface SchGroup {
  uuid: string;
  name: string;
  memberUuids: string[];
}

export interface SchNoConnect {
  uuid: string;
  position: SchPoint;
}

export interface TextNote {
  uuid: string;
  text: string;
  position: SchPoint;
  rotation: number;
  font_size: number;
}

export interface SchRectangle {
  uuid: string;
  start: SchPoint;
  end: SchPoint;
  stroke_type: string;
}

export type LineStyle = "solid" | "dash" | "dot" | "dash_dot";
export type ArrowStyle = "none" | "open" | "closed" | "diamond";

export type SchDrawing =
  | { type: "Line"; uuid: string; start: SchPoint; end: SchPoint; width: number; color?: string; lineStyle?: LineStyle; arrowStart?: ArrowStyle; arrowEnd?: ArrowStyle }
  | { type: "Rect"; uuid: string; start: SchPoint; end: SchPoint; width: number; fill: boolean; fillColor?: string; color?: string; lineStyle?: LineStyle }
  | { type: "Circle"; uuid: string; center: SchPoint; radius: number; width: number; fill: boolean; fillColor?: string; color?: string; lineStyle?: LineStyle }
  | { type: "Arc"; uuid: string; start: SchPoint; mid: SchPoint; end: SchPoint; width: number; color?: string; lineStyle?: LineStyle }
  | { type: "Polyline"; uuid: string; points: SchPoint[]; width: number; fill: boolean; fillColor?: string; color?: string; lineStyle?: LineStyle; arrowStart?: ArrowStyle; arrowEnd?: ArrowStyle }
  | { type: "TextFrame"; uuid: string; start: SchPoint; end: SchPoint; text: string; fontSize: number; width: number; fill: boolean; fillColor?: string; color?: string; lineStyle?: LineStyle }
  | { type: "Ellipse"; uuid: string; center: SchPoint; radiusX: number; radiusY: number; width: number; fill: boolean; fillColor?: string; color?: string; lineStyle?: LineStyle }
  | { type: "RoundRect"; uuid: string; start: SchPoint; end: SchPoint; cornerRadius: number; width: number; fill: boolean; fillColor?: string; color?: string; lineStyle?: LineStyle }
  | { type: "Polygon"; uuid: string; points: SchPoint[]; width: number; fill: boolean; fillColor?: string; color?: string; lineStyle?: LineStyle }
  | { type: "Image"; uuid: string; start: SchPoint; end: SchPoint; dataUrl: string; aspectLocked?: boolean };

export interface SchNoErcDirective {
  uuid: string;
  position: SchPoint;
}

export interface SchBus {
  uuid: string;
  start: SchPoint;
  end: SchPoint;
}

export interface SchBusEntry {
  uuid: string;
  position: SchPoint;
  size: [number, number];
}

export interface LibSymbol {
  id: string;
  graphics: Graphic[];
  pins: SchPin[];
  show_pin_numbers: boolean;
  show_pin_names: boolean;
  pin_name_offset: number;
  unit_count?: number; // Number of parts (1 = single, 2+ = multi-part like quad gates)
  units?: LibSymbolUnit[]; // Per-unit graphics/pins for multi-part symbols
  has_alternate?: boolean; // DeMorgan alternate display mode available
  alternate_graphics?: Graphic[]; // Alternate (DeMorgan) body graphics
  alternate_pins?: SchPin[]; // Alternate pin configuration (same numbers, different graphics)
}

export type Graphic =
  | { type: "Polyline"; points: SchPoint[]; width: number; fill: boolean }
  | { type: "Rectangle"; start: SchPoint; end: SchPoint; width: number; fill: boolean }
  | { type: "Circle"; center: SchPoint; radius: number; width: number; fill: boolean }
  | { type: "Arc"; start: SchPoint; mid: SchPoint; end: SchPoint; width: number };

export interface SchPin {
  pin_type: string;
  shape: string;
  position: SchPoint;
  rotation: number;
  length: number;
  name: string;
  number: string;
  name_visible: boolean;
  number_visible: boolean;
  hidden?: boolean; // Hidden power pin (still connects electrically)
}

export interface LibSymbolUnit {
  id: number; // Unit number (0 = common, 1+ = specific part)
  graphics: Graphic[];
  pins: SchPin[];
}

export interface SchSymbol {
  uuid: string;
  lib_id: string;
  reference: string;
  value: string;
  footprint: string;
  position: SchPoint;
  rotation: number;
  mirror_x: boolean;
  mirror_y: boolean;
  unit: number;
  is_power: boolean;
  ref_text: TextPropData;
  val_text: TextPropData;
  fields_autoplaced: boolean;
  // KiCad 10 fields
  dnp: boolean;
  in_bom: boolean;
  on_board: boolean;
  exclude_from_sim: boolean;
  locked: boolean;
  fields: Record<string, string>;
}

export interface TextPropData {
  position: SchPoint;
  rotation: number;
  font_size: number;
  justify_h: string;
  justify_v: string;
  hidden: boolean;
}

export interface SchWire {
  uuid: string;
  start: SchPoint;
  end: SchPoint;
}

export interface SchJunction {
  uuid: string;
  position: SchPoint;
}

export interface SchLabel {
  uuid: string;
  text: string;
  position: SchPoint;
  rotation: number;
  label_type: "Net" | "Global" | "Hierarchical" | "Power";
  shape: string;
  font_size: number;
  justify: string;
}

export interface SchSheetPin {
  uuid: string;
  name: string;
  direction: string;
  position: SchPoint;
  rotation: number;
}

export interface SchChildSheet {
  uuid: string;
  name: string;
  filename: string;
  position: SchPoint;
  size: [number, number];
  pins: SchSheetPin[];
  // Multi-channel: Repeat(ChannelId, StartIdx, EndIdx) — e.g., "Repeat(CH, 1, 4)"
  repeat?: string;
  channelCount?: number;
}

export interface SchPoint {
  x: number;
  y: number;
}

// --- Library types ---

export interface LibraryInfo {
  name: string;
  path: string;
  file_size: number;
}

export interface SymbolSearchResult {
  library: string;
  symbol_id: string;
  description: string;
  keywords: string[];
  reference_prefix: string;
  pin_count: number;
}
