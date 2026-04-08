import { create } from "zustand";
import type { LibSymbol, SchPin, Graphic, SchPoint } from "@/types";

export type LibEditMode = "select" | "addPin" | "addRect" | "addPolyline" | "addCircle" | "addArc" | "addText" | "addEllipse" | "addPolygon";

export type LibPanelView = "properties" | "pinTable";

export type LibSelectedItem =
  | { type: "pin"; index: number }
  | { type: "graphic"; index: number }
  | null;

interface LibraryEditorState {
  active: boolean;
  symbol: LibSymbol | null;
  selectedItem: LibSelectedItem;
  editMode: LibEditMode;
  displayMode: "normal" | "alternate"; // DeMorgan toggle
  dirty: boolean;
  undoStack: LibSymbol[];
  redoStack: LibSymbol[];
  // The source path + library key so we know where to save back
  sourcePath: string | null;
  sourceLibId: string | null;

  // Component metadata (persisted alongside symbol)
  designatorPrefix: string;
  comment: string;
  description: string;
  footprint: string;
  componentType: "standard" | "standard_no_bom" | "mechanical" | "graphical";
  mirrored: boolean;

  // Lifecycle
  openSymbol: (symbol: LibSymbol, sourcePath: string, libId: string) => void;
  closeEditor: () => void;
  setEditMode: (mode: LibEditMode) => void;
  setSelectedItem: (item: LibSelectedItem) => void;

  // Undo/Redo
  pushUndo: () => void;
  undo: () => void;
  redo: () => void;

  // Pin operations
  addPin: (pin: SchPin) => void;
  updatePin: (index: number, updates: Partial<SchPin>) => void;
  removePin: (index: number) => void;
  movePin: (index: number, position: SchPoint) => void;

  // Graphic operations
  addGraphic: (graphic: Graphic) => void;
  updateGraphic: (index: number, graphic: Graphic) => void;
  removeGraphic: (index: number) => void;

  // Symbol metadata
  updateSymbolMeta: (updates: Partial<Pick<LibSymbol, "show_pin_numbers" | "show_pin_names" | "pin_name_offset">>) => void;
  updateSymbolId: (id: string) => void;
  toggleDisplayMode: () => void;

  // Component metadata setters
  setDesignatorPrefix: (v: string) => void;
  setComment: (v: string) => void;
  setDescription: (v: string) => void;
  setFootprint: (v: string) => void;
  setComponentType: (v: LibraryEditorState["componentType"]) => void;
  setMirrored: (v: boolean) => void;

  // Panel view
  panelView: LibPanelView;
  setPanelView: (view: LibPanelView) => void;
}

function cloneSymbol(sym: LibSymbol): LibSymbol {
  return structuredClone(sym);
}

export const useLibraryEditorStore = create<LibraryEditorState>()((set, get) => ({
  active: false,
  symbol: null,
  selectedItem: null,
  editMode: "select",
  displayMode: "normal",
  dirty: false,
  undoStack: [],
  redoStack: [],
  sourcePath: null,
  sourceLibId: null,
  designatorPrefix: "U",
  comment: "*",
  description: "",
  footprint: "",
  componentType: "standard_no_bom" as LibraryEditorState["componentType"],
  mirrored: false,
  panelView: "properties" as LibPanelView,

  openSymbol: (symbol, sourcePath, libId) => {
    set({
      active: true,
      symbol: cloneSymbol(symbol),
      selectedItem: null,
      editMode: "select",
      dirty: false,
      undoStack: [],
      redoStack: [],
      sourcePath,
      sourceLibId: libId,
    });
  },

  closeEditor: () => {
    set({
      active: false,
      symbol: null,
      selectedItem: null,
      editMode: "select",
      dirty: false,
      undoStack: [],
      redoStack: [],
      sourcePath: null,
      sourceLibId: null,
    });
  },

  setEditMode: (mode) => set({ editMode: mode, selectedItem: null }),
  setSelectedItem: (item) => set({ selectedItem: item }),

  pushUndo: () => {
    const { symbol, undoStack } = get();
    if (!symbol) return;
    const stack = [...undoStack, cloneSymbol(symbol)];
    if (stack.length > 50) stack.shift();
    set({ undoStack: stack, redoStack: [] });
  },

  undo: () => {
    const { symbol, undoStack, redoStack } = get();
    if (undoStack.length === 0 || !symbol) return;
    const prev = undoStack[undoStack.length - 1];
    set({
      symbol: prev,
      undoStack: undoStack.slice(0, -1),
      redoStack: [...redoStack, cloneSymbol(symbol)],
      selectedItem: null,
      dirty: true,
    });
  },

  redo: () => {
    const { symbol, undoStack, redoStack } = get();
    if (redoStack.length === 0 || !symbol) return;
    const next = redoStack[redoStack.length - 1];
    set({
      symbol: next,
      redoStack: redoStack.slice(0, -1),
      undoStack: [...undoStack, cloneSymbol(symbol!)],
      selectedItem: null,
      dirty: true,
    });
  },

  addPin: (pin) => {
    const { symbol } = get();
    if (!symbol) return;
    get().pushUndo();
    const newSym = cloneSymbol(symbol);
    newSym.pins.push(pin);
    set({ symbol: newSym, dirty: true });
  },

  updatePin: (index, updates) => {
    const { symbol } = get();
    if (!symbol || index < 0 || index >= symbol.pins.length) return;
    get().pushUndo();
    const newSym = cloneSymbol(symbol);
    newSym.pins[index] = { ...newSym.pins[index], ...updates };
    set({ symbol: newSym, dirty: true });
  },

  removePin: (index) => {
    const { symbol } = get();
    if (!symbol || index < 0 || index >= symbol.pins.length) return;
    get().pushUndo();
    const newSym = cloneSymbol(symbol);
    newSym.pins.splice(index, 1);
    set({ symbol: newSym, dirty: true, selectedItem: null });
  },

  movePin: (index, position) => {
    const { symbol } = get();
    if (!symbol || index < 0 || index >= symbol.pins.length) return;
    get().pushUndo();
    const newSym = cloneSymbol(symbol);
    newSym.pins[index].position = position;
    set({ symbol: newSym, dirty: true });
  },

  addGraphic: (graphic) => {
    const { symbol } = get();
    if (!symbol) return;
    get().pushUndo();
    const newSym = cloneSymbol(symbol);
    newSym.graphics.push(graphic);
    set({ symbol: newSym, dirty: true });
  },

  updateGraphic: (index, graphic) => {
    const { symbol } = get();
    if (!symbol || index < 0 || index >= symbol.graphics.length) return;
    get().pushUndo();
    const newSym = cloneSymbol(symbol);
    newSym.graphics[index] = graphic;
    set({ symbol: newSym, dirty: true });
  },

  removeGraphic: (index) => {
    const { symbol } = get();
    if (!symbol || index < 0 || index >= symbol.graphics.length) return;
    get().pushUndo();
    const newSym = cloneSymbol(symbol);
    newSym.graphics.splice(index, 1);
    set({ symbol: newSym, dirty: true, selectedItem: null });
  },

  updateSymbolMeta: (updates) => {
    const { symbol } = get();
    if (!symbol) return;
    get().pushUndo();
    const newSym = cloneSymbol(symbol);
    Object.assign(newSym, updates);
    set({ symbol: newSym, dirty: true });
  },

  updateSymbolId: (id) => {
    const { symbol } = get();
    if (!symbol) return;
    get().pushUndo();
    const newSym = cloneSymbol(symbol);
    newSym.id = id;
    set({ symbol: newSym, dirty: true, sourceLibId: id });
  },

  toggleDisplayMode: () => {
    set((s) => ({ displayMode: s.displayMode === "normal" ? "alternate" : "normal" }));
  },

  setDesignatorPrefix: (v) => set({ designatorPrefix: v, dirty: true }),
  setComment: (v) => set({ comment: v, dirty: true }),
  setDescription: (v) => set({ description: v, dirty: true }),
  setFootprint: (v) => set({ footprint: v, dirty: true }),
  setComponentType: (v) => set({ componentType: v, dirty: true }),
  setMirrored: (v) => set({ mirrored: v, dirty: true }),

  setPanelView: (view) => set({ panelView: view }),
}));
