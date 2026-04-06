import { create } from "zustand";
import type { EditorMode, StatusBarState } from "@/types";

interface EditorState {
  mode: EditorMode;
  gridVisible: boolean;
  statusBar: StatusBarState;

  setMode: (mode: EditorMode) => void;
  toggleGrid: () => void;
  toggleSnap: () => void;
  setGridSize: (size: number) => void;
  updateStatusBar: (partial: Partial<StatusBarState>) => void;
}

export const useEditorStore = create<EditorState>()((set) => ({
  mode: "schematic",
  gridVisible: true,
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
  setGridSize: (size) =>
    set((s) => ({ statusBar: { ...s.statusBar, gridSize: size } })),
  updateStatusBar: (partial) =>
    set((s) => ({ statusBar: { ...s.statusBar, ...partial } })),
}));
