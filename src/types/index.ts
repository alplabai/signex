export type EditorMode = "schematic" | "pcb" | "library" | "output" | "3d" | "simulation";

export type PanelId =
  | "projects"
  | "components"
  | "navigator"
  | "properties"
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
