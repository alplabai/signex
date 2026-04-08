import { create } from "zustand";
import type { PcbPad, PcbGraphic, PcbLayerId } from "@/types/pcb";

export type FpEditMode = "select" | "addPadSmd" | "addPadTh" | "addLine" | "addRect" | "addCircle" | "addArc" | "addPoly" | "addText";

export type FpSelectedItem =
  | { type: "pad"; index: number }
  | { type: "graphic"; index: number }
  | null;

export interface FootprintData {
  id: string;
  pads: PcbPad[];
  graphics: PcbGraphic[];
  courtyard: { x: number; y: number }[];
  model3d: string;
}

interface FootprintEditorState {
  active: boolean;
  footprint: FootprintData | null;
  selectedItem: FpSelectedItem;
  editMode: FpEditMode;
  activeLayer: PcbLayerId;
  dirty: boolean;
  undoStack: FootprintData[];
  redoStack: FootprintData[];
  sourcePath: string | null;
  sourceId: string | null;

  openFootprint: (fp: FootprintData, sourcePath: string, id: string) => void;
  closeEditor: () => void;
  setEditMode: (mode: FpEditMode) => void;
  setSelectedItem: (item: FpSelectedItem) => void;
  setActiveLayer: (layer: PcbLayerId) => void;

  pushUndo: () => void;
  undo: () => void;
  redo: () => void;

  addPad: (pad: PcbPad) => void;
  updatePad: (index: number, updates: Partial<PcbPad>) => void;
  removePad: (index: number) => void;

  addGraphic: (graphic: PcbGraphic) => void;
  updateGraphic: (index: number, graphic: PcbGraphic) => void;
  removeGraphic: (index: number) => void;

  updateFootprintId: (id: string) => void;
}

function clone(fp: FootprintData): FootprintData {
  return structuredClone(fp);
}

export const useFootprintEditorStore = create<FootprintEditorState>()((set, get) => ({
  active: false,
  footprint: null,
  selectedItem: null,
  editMode: "select",
  activeLayer: "F.Cu" as PcbLayerId,
  dirty: false,
  undoStack: [],
  redoStack: [],
  sourcePath: null,
  sourceId: null,

  openFootprint: (fp, sourcePath, id) => set({
    active: true,
    footprint: clone(fp),
    selectedItem: null,
    editMode: "select",
    dirty: false,
    undoStack: [],
    redoStack: [],
    sourcePath,
    sourceId: id,
  }),

  closeEditor: () => set({
    active: false,
    footprint: null,
    selectedItem: null,
    editMode: "select",
    dirty: false,
    undoStack: [],
    redoStack: [],
    sourcePath: null,
    sourceId: null,
  }),

  setEditMode: (mode) => set({ editMode: mode, selectedItem: null }),
  setSelectedItem: (item) => set({ selectedItem: item }),
  setActiveLayer: (layer) => set({ activeLayer: layer }),

  pushUndo: () => {
    const { footprint, undoStack } = get();
    if (!footprint) return;
    const stack = [...undoStack, clone(footprint)];
    if (stack.length > 50) stack.shift();
    set({ undoStack: stack, redoStack: [] });
  },

  undo: () => {
    const { footprint, undoStack, redoStack } = get();
    if (undoStack.length === 0 || !footprint) return;
    set({
      footprint: undoStack[undoStack.length - 1],
      undoStack: undoStack.slice(0, -1),
      redoStack: [...redoStack, clone(footprint)],
      selectedItem: null,
      dirty: true,
    });
  },

  redo: () => {
    const { footprint, undoStack, redoStack } = get();
    if (redoStack.length === 0 || !footprint) return;
    set({
      footprint: redoStack[redoStack.length - 1],
      redoStack: redoStack.slice(0, -1),
      undoStack: [...undoStack, clone(footprint!)],
      selectedItem: null,
      dirty: true,
    });
  },

  addPad: (pad) => {
    const { footprint } = get();
    if (!footprint) return;
    get().pushUndo();
    const fp = clone(footprint);
    fp.pads.push(pad);
    set({ footprint: fp, dirty: true });
  },

  updatePad: (index, updates) => {
    const { footprint } = get();
    if (!footprint || index < 0 || index >= footprint.pads.length) return;
    get().pushUndo();
    const fp = clone(footprint);
    fp.pads[index] = { ...fp.pads[index], ...updates };
    set({ footprint: fp, dirty: true });
  },

  removePad: (index) => {
    const { footprint } = get();
    if (!footprint || index < 0 || index >= footprint.pads.length) return;
    get().pushUndo();
    const fp = clone(footprint);
    fp.pads.splice(index, 1);
    set({ footprint: fp, dirty: true, selectedItem: null });
  },

  addGraphic: (graphic) => {
    const { footprint } = get();
    if (!footprint) return;
    get().pushUndo();
    const fp = clone(footprint);
    fp.graphics.push(graphic);
    set({ footprint: fp, dirty: true });
  },

  updateGraphic: (index, graphic) => {
    const { footprint } = get();
    if (!footprint || index < 0 || index >= footprint.graphics.length) return;
    get().pushUndo();
    const fp = clone(footprint);
    fp.graphics[index] = graphic;
    set({ footprint: fp, dirty: true });
  },

  removeGraphic: (index) => {
    const { footprint } = get();
    if (!footprint || index < 0 || index >= footprint.graphics.length) return;
    get().pushUndo();
    const fp = clone(footprint);
    fp.graphics.splice(index, 1);
    set({ footprint: fp, dirty: true, selectedItem: null });
  },

  updateFootprintId: (id) => {
    const { footprint } = get();
    if (!footprint) return;
    get().pushUndo();
    const fp = clone(footprint);
    fp.id = id;
    set({ footprint: fp, dirty: true, sourceId: id });
  },
}));
