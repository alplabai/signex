import { create } from "zustand";
import type { EditorMode, StatusBarState } from "@/types";

interface EditorState {
  mode: EditorMode;
  statusBar: StatusBarState;

  setMode: (mode: EditorMode) => void;
  updateStatusBar: (partial: Partial<StatusBarState>) => void;
}

export const useEditorStore = create<EditorState>()((set) => ({
  mode: "schematic",
  statusBar: {
    cursorPosition: { x: 0, y: 0 },
    gridSize: 50,
    snapEnabled: true,
    units: "mil",
    activeLayer: "F.Cu",
    currentMode: "Select",
    zoom: 100,
  },

  setMode: (mode) => set({ mode }),
  updateStatusBar: (partial) =>
    set((s) => ({ statusBar: { ...s.statusBar, ...partial } })),
}));
