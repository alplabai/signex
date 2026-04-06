import { create } from "zustand";
import type { EditorMode, StatusBarState, SchPoint } from "@/types";

export interface ErcMarker {
  position: SchPoint;
  severity: "error" | "warning";
  message: string;
  uuids: string[];
}

interface EditorState {
  mode: EditorMode;
  gridVisible: boolean;
  netColorOverride: boolean;
  netColors: Map<string, string>;
  ercMarkers: ErcMarker[];
  showErcMarkers: boolean;
  autoFocusUuids: Set<string> | null; // When set, dim everything except these UUIDs
  statusBar: StatusBarState;

  setMode: (mode: EditorMode) => void;
  toggleGrid: () => void;
  toggleSnap: () => void;
  toggleNetColors: () => void;
  setGridSize: (size: number) => void;
  updateStatusBar: (partial: Partial<StatusBarState>) => void;
  setNetColors: (colors: Map<string, string>) => void;
  setErcMarkers: (markers: ErcMarker[]) => void;
  toggleErcMarkers: () => void;
  setAutoFocus: (uuids: string[] | null) => void;
}

export const useEditorStore = create<EditorState>()((set) => ({
  mode: "schematic",
  gridVisible: true,
  netColorOverride: false,
  netColors: new Map(),
  ercMarkers: [],
  showErcMarkers: true,
  autoFocusUuids: null,
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
  setAutoFocus: (uuids) => set({ autoFocusUuids: uuids ? new Set(uuids) : null }),
  setGridSize: (size) =>
    set((s) => ({ statusBar: { ...s.statusBar, gridSize: size } })),
  updateStatusBar: (partial) =>
    set((s) => ({ statusBar: { ...s.statusBar, ...partial } })),
}));
