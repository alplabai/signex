import { create } from "zustand";
import type { EditorMode, StatusBarState, SchPoint } from "@/types";

export interface ErcMarker {
  position: SchPoint;
  severity: "error" | "warning";
  message: string;
  uuids: string[];
}

export interface FilterState {
  visible: boolean;
  selectable: boolean;
}

export type SelectionFilter = Record<string, FilterState>;

const DEFAULT_FILTER: SelectionFilter = {
  components: { visible: true, selectable: true },
  wires: { visible: true, selectable: true },
  buses: { visible: true, selectable: true },
  labels: { visible: true, selectable: true },
  powerPorts: { visible: true, selectable: true },
  junctions: { visible: true, selectable: true },
  noConnects: { visible: true, selectable: true },
  textNotes: { visible: true, selectable: true },
  drawings: { visible: true, selectable: true },
  sheetSymbols: { visible: true, selectable: true },
};

interface EditorState {
  mode: EditorMode;
  gridVisible: boolean;
  netColorOverride: boolean;
  netColors: Record<string, string>;
  ercMarkers: ErcMarker[];
  showErcMarkers: boolean;
  autoFocusUuids: string[] | null; // When set, dim everything except these UUIDs
  selectionFilter: SelectionFilter;
  autoJunction: boolean;
  electricalSnapRange: number;
  ercSeverity: Record<string, "error" | "warning" | "none">;
  statusBar: StatusBarState;

  setMode: (mode: EditorMode) => void;
  toggleGrid: () => void;
  toggleSnap: () => void;
  toggleNetColors: () => void;
  setGridSize: (size: number) => void;
  updateStatusBar: (partial: Partial<StatusBarState>) => void;
  setNetColors: (colors: Record<string, string>) => void;
  setErcMarkers: (markers: ErcMarker[]) => void;
  toggleErcMarkers: () => void;
  setAutoFocus: (uuids: string[] | null) => void;
  setFilterItem: (key: string, field: "visible" | "selectable", value: boolean) => void;
  resetFilter: () => void;
  setAutoJunction: (v: boolean) => void;
  setElectricalSnapRange: (v: number) => void;
  setErcSeverity: (type: string, severity: "error" | "warning" | "none") => void;
}

export const useEditorStore = create<EditorState>()((set) => ({
  mode: "schematic",
  gridVisible: true,
  netColorOverride: false,
  netColors: {},
  ercMarkers: [],
  showErcMarkers: true,
  autoFocusUuids: null,
  selectionFilter: { ...DEFAULT_FILTER },
  autoJunction: true,
  electricalSnapRange: 2.0,
  ercSeverity: {
    duplicate_designator: "error",
    unconnected_pin: "warning",
    output_conflict: "error",
    single_pin_net: "warning",
    no_driver: "warning",
  },
  statusBar: {
    cursorPosition: { x: 0, y: 0 },
    gridSize: 1.27,
    snapEnabled: true,
    units: "mm",
    activeLayer: "F.Cu",
    currentMode: "Select",
    zoom: 100,
  },

  setMode: (mode) => set({ mode }),
  toggleGrid: () => set((s) => ({ gridVisible: !s.gridVisible })),
  toggleSnap: () =>
    set((s) => ({ statusBar: { ...s.statusBar, snapEnabled: !s.statusBar.snapEnabled } })),
  toggleNetColors: () => set((s) => ({ netColorOverride: !s.netColorOverride })),
  setNetColors: (colors) => set({ netColors: colors }),
  setErcMarkers: (markers) => set({ ercMarkers: markers }),
  toggleErcMarkers: () => set((s) => ({ showErcMarkers: !s.showErcMarkers })),
  setAutoFocus: (uuids) => set({ autoFocusUuids: uuids ?? null }),
  setFilterItem: (key, field, value) =>
    set((s) => ({
      selectionFilter: {
        ...s.selectionFilter,
        [key]: { ...s.selectionFilter[key], [field]: value },
      },
    })),
  resetFilter: () => set({ selectionFilter: { ...DEFAULT_FILTER } }),
  setAutoJunction: (v) => set({ autoJunction: v }),
  setElectricalSnapRange: (v) => set({ electricalSnapRange: v }),
  setErcSeverity: (type, severity) =>
    set((s) => ({ ercSeverity: { ...s.ercSeverity, [type]: severity } })),
  setGridSize: (size) =>
    set((s) => ({ statusBar: { ...s.statusBar, gridSize: size } })),
  updateStatusBar: (partial) =>
    set((s) => ({ statusBar: { ...s.statusBar, ...partial } })),
}));
