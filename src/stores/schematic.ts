import { create } from "zustand";
import type { SchematicData, SchPoint, SchSymbol, SchLabel } from "@/types";

export type EditMode = "select" | "drawWire" | "placeSymbol" | "placeLabel";

interface WireDrawState {
  points: SchPoint[];
  active: boolean;
  cursor: SchPoint; // Live cursor position for preview
}

interface SchematicState {
  // Document
  data: SchematicData | null;
  dirty: boolean;

  // Edit mode
  editMode: EditMode;
  wireDrawing: WireDrawState;
  updateWireCursor: (pos: SchPoint) => void;

  // Selection
  selectedIds: Set<string>;

  // Undo / Redo
  undoStack: SchematicData[];
  redoStack: SchematicData[];

  // Document actions
  loadSchematic: (data: SchematicData) => void;
  setEditMode: (mode: EditMode) => void;

  // Undo / Redo
  pushUndo: () => void;
  undo: () => void;
  redo: () => void;

  // Selection
  select: (uuid: string) => void;
  selectMultiple: (uuids: string[]) => void;
  toggleSelect: (uuid: string) => void;
  deselectAll: () => void;

  // Editing
  moveElements: (uuids: string[], dx: number, dy: number) => void;
  addWire: (start: SchPoint, end: SchPoint) => void;
  addSymbol: (symbol: SchSymbol) => void;
  addLabel: (label: SchLabel) => void;
  addJunction: (pos: SchPoint) => void;
  deleteSelected: () => void;
  rotateSelected: () => void;

  // Wire drawing
  startWire: (pos: SchPoint) => void;
  addWirePoint: (pos: SchPoint) => void;
  finishWire: () => void;
  cancelWire: () => void;
}

function generateUuid(): string {
  return crypto.randomUUID();
}

function snapToGrid(v: number, grid: number = 1.27): number {
  return Math.round(v / grid) * grid;
}

export function snapPoint(p: SchPoint, grid: number = 1.27): SchPoint {
  return { x: snapToGrid(p.x, grid), y: snapToGrid(p.y, grid) };
}

function cloneData(data: SchematicData): SchematicData {
  return JSON.parse(JSON.stringify(data));
}

const MAX_UNDO = 50;

export const useSchematicStore = create<SchematicState>()((set, get) => ({
  data: null,
  dirty: false,
  editMode: "select",
    wireDrawing: { points: [], active: false, cursor: { x: 0, y: 0 } },
  selectedIds: new Set<string>(),
  undoStack: [],
  redoStack: [],

  loadSchematic: (data) =>
    set({
      data,
      dirty: false,
      selectedIds: new Set(),
      undoStack: [],
      redoStack: [],
      editMode: "select",
        wireDrawing: { points: [], active: false, cursor: { x: 0, y: 0 } },
    }),

  setEditMode: (mode) => {
    const state = get();
    // Cancel any active wire drawing when switching modes
    if (state.wireDrawing.active && mode !== "drawWire") {
      set({ editMode: mode, wireDrawing: { points: [], active: false, cursor: { x: 0, y: 0 } } });
    } else {
      set({ editMode: mode });
    }
  },

  pushUndo: () => {
    const { data, undoStack } = get();
    if (!data) return;
    const stack = [...undoStack, cloneData(data)].slice(-MAX_UNDO);
    set({ undoStack: stack, redoStack: [] });
  },

  undo: () => {
    const { data, undoStack, redoStack } = get();
    if (undoStack.length === 0 || !data) return;
    const prev = undoStack[undoStack.length - 1];
    set({
      data: prev,
      undoStack: undoStack.slice(0, -1),
      redoStack: [...redoStack, cloneData(data)],
      dirty: true,
      selectedIds: new Set(),
    });
  },

  redo: () => {
    const { data, undoStack, redoStack } = get();
    if (redoStack.length === 0 || !data) return;
    const next = redoStack[redoStack.length - 1];
    set({
      data: next,
      redoStack: redoStack.slice(0, -1),
      undoStack: [...undoStack, cloneData(data)],
      dirty: true,
      selectedIds: new Set(),
    });
  },

  select: (uuid) => set({ selectedIds: new Set([uuid]) }),
  selectMultiple: (uuids) => set({ selectedIds: new Set(uuids) }),
  toggleSelect: (uuid) => {
    const ids = new Set(get().selectedIds);
    if (ids.has(uuid)) ids.delete(uuid);
    else ids.add(uuid);
    set({ selectedIds: ids });
  },
  deselectAll: () => set({ selectedIds: new Set() }),

  moveElements: (uuids, dx, dy) => {
    const { data } = get();
    if (!data) return;

    const idSet = new Set(uuids);
    const newData = cloneData(data);

    for (const sym of newData.symbols) {
      if (idSet.has(sym.uuid)) {
        sym.position.x += dx;
        sym.position.y += dy;
        sym.ref_text.position.x += dx;
        sym.ref_text.position.y += dy;
        sym.val_text.position.x += dx;
        sym.val_text.position.y += dy;
      }
    }
    for (const wire of newData.wires) {
      if (idSet.has(wire.uuid)) {
        wire.start.x += dx;
        wire.start.y += dy;
        wire.end.x += dx;
        wire.end.y += dy;
      }
    }
    for (const label of newData.labels) {
      if (idSet.has(label.uuid)) {
        label.position.x += dx;
        label.position.y += dy;
      }
    }
    for (const j of newData.junctions) {
      if (idSet.has(j.uuid)) {
        j.position.x += dx;
        j.position.y += dy;
      }
    }

    set({ data: newData, dirty: true });
  },

  addWire: (start, end) => {
    const { data } = get();
    if (!data) return;
    get().pushUndo();
    const newData = cloneData(data);
    newData.wires.push({
      uuid: generateUuid(),
      start: snapPoint(start),
      end: snapPoint(end),
    });
    set({ data: newData, dirty: true });
  },

  addSymbol: (symbol) => {
    const { data } = get();
    if (!data) return;
    get().pushUndo();
    const newData = cloneData(data);
    newData.symbols.push({ ...symbol, uuid: generateUuid() });
    set({ data: newData, dirty: true });
  },

  addLabel: (label) => {
    const { data } = get();
    if (!data) return;
    get().pushUndo();
    const newData = cloneData(data);
    newData.labels.push({ ...label, uuid: generateUuid() });
    set({ data: newData, dirty: true });
  },

  addJunction: (pos) => {
    const { data } = get();
    if (!data) return;
    get().pushUndo();
    const newData = cloneData(data);
    newData.junctions.push({ uuid: generateUuid(), position: snapPoint(pos) });
    set({ data: newData, dirty: true });
  },

  deleteSelected: () => {
    const { data, selectedIds } = get();
    if (!data || selectedIds.size === 0) return;
    get().pushUndo();
    const newData = cloneData(data);
    newData.symbols = newData.symbols.filter((s) => !selectedIds.has(s.uuid));
    newData.wires = newData.wires.filter((w) => !selectedIds.has(w.uuid));
    newData.labels = newData.labels.filter((l) => !selectedIds.has(l.uuid));
    newData.junctions = newData.junctions.filter((j) => !selectedIds.has(j.uuid));
    set({ data: newData, dirty: true, selectedIds: new Set() });
  },

  rotateSelected: () => {
    const { data, selectedIds } = get();
    if (!data || selectedIds.size === 0) return;
    get().pushUndo();
    const newData = cloneData(data);
    for (const sym of newData.symbols) {
      if (selectedIds.has(sym.uuid)) {
        sym.rotation = ((sym.rotation + 90) % 360) as 0 | 90 | 180 | 270;
      }
    }
    set({ data: newData, dirty: true });
  },

  updateWireCursor: (pos) => {
    const { wireDrawing } = get();
    if (wireDrawing.active) {
      set({ wireDrawing: { ...wireDrawing, cursor: snapPoint(pos) } });
    }
  },

  // Wire drawing state machine
  startWire: (pos) => {
    const snapped = snapPoint(pos);
    set({
      editMode: "drawWire",
      wireDrawing: { points: [snapped], active: true, cursor: snapped },
    });
  },

  addWirePoint: (pos) => {
    const { wireDrawing } = get();
    if (!wireDrawing.active) return;
    const snapped = snapPoint(pos);
    // Add Manhattan-routed segments (horizontal then vertical)
    const last = wireDrawing.points[wireDrawing.points.length - 1];
    const newPoints = [...wireDrawing.points];
    if (Math.abs(snapped.x - last.x) > 0.01 && Math.abs(snapped.y - last.y) > 0.01) {
      // Add bend point for Manhattan routing
      newPoints.push({ x: snapped.x, y: last.y });
    }
    newPoints.push(snapped);
    set({ wireDrawing: { points: newPoints, active: true, cursor: snapped } });
  },

  finishWire: () => {
    const { wireDrawing, data } = get();
    if (!wireDrawing.active || wireDrawing.points.length < 2 || !data) {
      set({ wireDrawing: { points: [], active: false, cursor: { x: 0, y: 0 } } });
      return;
    }

    get().pushUndo();
    const newData = cloneData(data);

    // Create wire segments between consecutive points
    for (let i = 0; i < wireDrawing.points.length - 1; i++) {
      newData.wires.push({
        uuid: generateUuid(),
        start: wireDrawing.points[i],
        end: wireDrawing.points[i + 1],
      });
    }

    set({
      data: newData,
      dirty: true,
        wireDrawing: { points: [], active: false, cursor: { x: 0, y: 0 } },
    });
  },

  cancelWire: () => {
    set({   wireDrawing: { points: [], active: false, cursor: { x: 0, y: 0 } }, editMode: "select" });
  },
}));
