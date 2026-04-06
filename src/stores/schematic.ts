import { create } from "zustand";
import { useEditorStore } from "@/stores/editor";
import type { SchematicData, SchPoint, SchSymbol, SchWire, SchLabel, LibSymbol, SymbolSearchResult } from "@/types";

export type EditMode = "select" | "drawWire" | "placeSymbol" | "placeLabel" | "placePower" | "placeNoConnect";
export type WireRoutingMode = "manhattan" | "diagonal" | "free";

interface WireDrawState {
  points: SchPoint[];
  active: boolean;
  routingMode: WireRoutingMode;
}

interface PlacingSymbol {
  lib: LibSymbol;
  meta: SymbolSearchResult;
  rotation: number;
  mirrorX: boolean;
  mirrorY: boolean;
}

interface SchematicState {
  // Document
  data: SchematicData | null;
  dirty: boolean;

  // Edit mode
  editMode: EditMode;
  wireDrawing: WireDrawState;
  placingSymbol: PlacingSymbol | null;

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

  // Property editing
  updateSymbolProp: (uuid: string, key: string, value: string) => void;
  updateLabelProp: (uuid: string, key: string, value: string) => void;

  // Clipboard
  copySelected: () => void;
  pasteClipboard: (offset: SchPoint) => void;
  clipboard: { symbols: SchSymbol[]; wires: SchWire[]; labels: SchLabel[] } | null;

  // Net label / power port placement
  placeNetLabel: (pos: SchPoint, text: string) => void;
  placePowerPort: (pos: SchPoint, netName: string, style: string) => void;
  placeNoConnect: (pos: SchPoint) => void;

  // Wire drawing
  startWire: (pos: SchPoint) => void;
  addWirePoint: (pos: SchPoint) => void;
  removeLastWirePoint: () => void;
  cycleWireRouting: () => void;
  finishWire: () => void;
  cancelWire: () => void;

  // Selection helpers
  selectAll: () => void;

  // Alignment
  alignSelected: (direction: "left" | "right" | "top" | "bottom" | "centerH" | "centerV") => void;
  distributeSelected: (axis: "horizontal" | "vertical") => void;

  // Component placement
  startPlacement: (lib: LibSymbol, meta: SymbolSearchResult) => void;
  rotatePlacement: () => void;
  mirrorPlacementX: () => void;
  mirrorPlacementY: () => void;
  placeSymbolAt: (pos: SchPoint) => void;
  cancelPlacement: () => void;
}

function generateUuid(): string {
  return crypto.randomUUID();
}

function snapToGrid(v: number, grid: number): number {
  return Math.round(v / grid) * grid;
}

export function snapPoint(p: SchPoint, grid?: number): SchPoint {
  const editor = useEditorStore.getState();
  if (!editor.statusBar.snapEnabled) return p;
  const g = grid ?? editor.statusBar.gridSize;
  return { x: snapToGrid(p.x, g), y: snapToGrid(p.y, g) };
}

function cloneData(data: SchematicData): SchematicData {
  return structuredClone(data);
}

/** Check if point lies on a wire segment (not at endpoints) */
function pointOnWireSegment(p: SchPoint, start: SchPoint, end: SchPoint, tol = 0.05): boolean {
  // Skip if point is at either endpoint
  if (Math.abs(p.x - start.x) < tol && Math.abs(p.y - start.y) < tol) return false;
  if (Math.abs(p.x - end.x) < tol && Math.abs(p.y - end.y) < tol) return false;

  // Check collinearity and within bounds
  const dx = end.x - start.x, dy = end.y - start.y;
  const len = Math.hypot(dx, dy);
  if (len < tol) return false;

  // Distance from point to line
  const dist = Math.abs(dx * (start.y - p.y) - dy * (start.x - p.x)) / len;
  if (dist > tol) return false;

  // Check within segment bounds
  const t = ((p.x - start.x) * dx + (p.y - start.y) * dy) / (len * len);
  return t > 0.01 && t < 0.99;
}

/** Auto-place junctions where wire endpoints land on existing wire midpoints */
function autoJunction(data: SchematicData, newPoints: SchPoint[]): void {
  const existingJunctions = new Set(
    data.junctions.map((j) => `${j.position.x.toFixed(3)},${j.position.y.toFixed(3)}`)
  );

  for (const pt of newPoints) {
    const key = `${pt.x.toFixed(3)},${pt.y.toFixed(3)}`;
    if (existingJunctions.has(key)) continue;

    // Check if this point is on any existing wire (T-junction)
    for (const wire of data.wires) {
      if (pointOnWireSegment(pt, wire.start, wire.end)) {
        data.junctions.push({ uuid: generateUuid(), position: { x: pt.x, y: pt.y } });
        existingJunctions.add(key);
        break;
      }
    }
  }

  // Also check existing wire endpoints against the new wire segments
  const newWires = data.wires.slice(-newPoints.length + 1); // Last N-1 wires are the new ones
  for (const wire of data.wires) {
    if (newWires.includes(wire)) continue;
    for (const endPt of [wire.start, wire.end]) {
      const key = `${endPt.x.toFixed(3)},${endPt.y.toFixed(3)}`;
      if (existingJunctions.has(key)) continue;
      for (const nw of newWires) {
        if (pointOnWireSegment(endPt, nw.start, nw.end)) {
          data.junctions.push({ uuid: generateUuid(), position: { x: endPt.x, y: endPt.y } });
          existingJunctions.add(key);
          break;
        }
      }
    }
  }
}

const MAX_UNDO = 50;

export const useSchematicStore = create<SchematicState>()((set, get) => ({
  data: null,
  dirty: false,
  editMode: "select",
  wireDrawing: { points: [], active: false, routingMode: "manhattan" as WireRoutingMode },
  placingSymbol: null,
  clipboard: null,
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
        wireDrawing: { points: [], active: false, routingMode: "manhattan" as WireRoutingMode },
    }),

  setEditMode: (mode) => {
    const state = get();
    // Cancel any active wire drawing when switching modes
    if (state.wireDrawing.active && mode !== "drawWire") {
      set({ editMode: mode, wireDrawing: { points: [], active: false, routingMode: "manhattan" as WireRoutingMode } });
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
    const tol = 0.05;

    // Collect pin positions of moving symbols BEFORE move (for rubber-banding)
    const movingPinPositions: SchPoint[] = [];
    for (const sym of newData.symbols) {
      if (!idSet.has(sym.uuid)) continue;
      const lib = newData.lib_symbols[sym.lib_id];
      if (!lib) continue;
      for (const pin of lib.pins) {
        // Transform pin to schematic coords (Y-flip + rotation + mirror)
        const ly = -pin.position.y;
        const rad = -(sym.rotation * Math.PI) / 180;
        const cos = Math.cos(rad), sin = Math.sin(rad);
        let rx = pin.position.x * cos - ly * sin;
        let ry = pin.position.x * sin + ly * cos;
        if (sym.mirror_x) ry = -ry;
        if (sym.mirror_y) rx = -rx;
        movingPinPositions.push({ x: sym.position.x + rx, y: sym.position.y + ry });
      }
    }

    // Move selected elements
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

    // Rubber-banding: stretch non-selected wires connected to moving symbol pins
    for (const wire of newData.wires) {
      if (idSet.has(wire.uuid)) continue; // Already moved
      for (const pinPos of movingPinPositions) {
        if (Math.abs(wire.start.x - pinPos.x) < tol && Math.abs(wire.start.y - pinPos.y) < tol) {
          wire.start.x += dx;
          wire.start.y += dy;
        }
        if (Math.abs(wire.end.x - pinPos.x) < tol && Math.abs(wire.end.y - pinPos.y) < tol) {
          wire.end.x += dx;
          wire.end.y += dy;
        }
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

  // Property editing
  updateSymbolProp: (uuid, key, value) => {
    const { data } = get();
    if (!data) return;
    get().pushUndo();
    const newData = cloneData(data);
    const sym = newData.symbols.find((s) => s.uuid === uuid);
    if (!sym) return;
    switch (key) {
      case "reference": sym.reference = value; break;
      case "value": sym.value = value; break;
      case "footprint": sym.footprint = value; break;
      case "x": sym.position.x = parseFloat(value) || sym.position.x; break;
      case "y": sym.position.y = parseFloat(value) || sym.position.y; break;
      case "rotation": sym.rotation = (parseInt(value) || 0) % 360; break;
    }
    set({ data: newData, dirty: true });
  },

  updateLabelProp: (uuid, key, value) => {
    const { data } = get();
    if (!data) return;
    get().pushUndo();
    const newData = cloneData(data);
    const label = newData.labels.find((l) => l.uuid === uuid);
    if (!label) return;
    switch (key) {
      case "text": label.text = value; break;
      case "x": label.position.x = parseFloat(value) || label.position.x; break;
      case "y": label.position.y = parseFloat(value) || label.position.y; break;
    }
    set({ data: newData, dirty: true });
  },

  // Clipboard
  copySelected: () => {
    const { data, selectedIds } = get();
    if (!data || selectedIds.size === 0) return;
    const symbols = data.symbols.filter((s) => selectedIds.has(s.uuid));
    const wires = data.wires.filter((w) => selectedIds.has(w.uuid));
    const labels = data.labels.filter((l) => selectedIds.has(l.uuid));
    set({
      clipboard: {
        symbols: structuredClone(symbols),
        wires: structuredClone(wires),
        labels: structuredClone(labels),
      },
    });
  },

  pasteClipboard: (offset) => {
    const { data, clipboard } = get();
    if (!data || !clipboard) return;
    get().pushUndo();
    const newData = cloneData(data);
    const newIds: string[] = [];

    for (const sym of clipboard.symbols) {
      const newSym = structuredClone(sym);
      newSym.uuid = generateUuid();
      newSym.position.x += offset.x;
      newSym.position.y += offset.y;
      newSym.ref_text.position.x += offset.x;
      newSym.ref_text.position.y += offset.y;
      newSym.val_text.position.x += offset.x;
      newSym.val_text.position.y += offset.y;
      newData.symbols.push(newSym);
      newIds.push(newSym.uuid);
    }
    for (const wire of clipboard.wires) {
      const newWire = structuredClone(wire);
      newWire.uuid = generateUuid();
      newWire.start.x += offset.x; newWire.start.y += offset.y;
      newWire.end.x += offset.x; newWire.end.y += offset.y;
      newData.wires.push(newWire);
      newIds.push(newWire.uuid);
    }
    for (const label of clipboard.labels) {
      const newLabel = structuredClone(label);
      newLabel.uuid = generateUuid();
      newLabel.position.x += offset.x;
      newLabel.position.y += offset.y;
      newData.labels.push(newLabel);
      newIds.push(newLabel.uuid);
    }

    set({ data: newData, dirty: true, selectedIds: new Set(newIds) });
  },

  // Net label placement
  placeNetLabel: (pos, text) => {
    const { data } = get();
    if (!data) return;
    get().pushUndo();
    const newData = cloneData(data);
    const snapped = snapPoint(pos);
    newData.labels.push({
      uuid: generateUuid(),
      text,
      position: snapped,
      rotation: 0,
      label_type: "Net",
      shape: "",
      font_size: 1.27,
      justify: "left",
    });
    set({ data: newData, dirty: true });
  },

  // Power port placement (creates a power symbol)
  placePowerPort: (pos, netName, _style) => {
    const { data } = get();
    if (!data) return;
    get().pushUndo();
    const newData = cloneData(data);
    const snapped = snapPoint(pos);
    newData.labels.push({
      uuid: generateUuid(),
      text: netName,
      position: snapped,
      rotation: 0,
      label_type: "Power",
      shape: "input",
      font_size: 1.27,
      justify: "left",
    });
    set({ data: newData, dirty: true });
  },

  // No-connect marker
  placeNoConnect: (pos) => {
    const { data } = get();
    if (!data) return;
    get().pushUndo();
    const newData = cloneData(data);
    const snapped = snapPoint(pos);
    newData.no_connects.push(snapped);
    set({ data: newData, dirty: true });
  },

  // Wire drawing state machine
  startWire: (pos) => {
    const snapped = snapPoint(pos);
    const mode = get().wireDrawing.routingMode;
    set({
      editMode: "drawWire",
      wireDrawing: { points: [snapped], active: true, routingMode: mode },
    });
  },

  addWirePoint: (pos) => {
    const { wireDrawing } = get();
    if (!wireDrawing.active || wireDrawing.points.length === 0) return;
    const snapped = snapPoint(pos);
    const last = wireDrawing.points[wireDrawing.points.length - 1];
    const newPoints = [...wireDrawing.points];

    if (wireDrawing.routingMode === "manhattan") {
      // Manhattan: horizontal then vertical
      if (Math.abs(snapped.x - last.x) > 0.01 && Math.abs(snapped.y - last.y) > 0.01) {
        newPoints.push({ x: snapped.x, y: last.y });
      }
    } else if (wireDrawing.routingMode === "diagonal") {
      // 45-degree: diagonal then orthogonal
      if (Math.abs(snapped.x - last.x) > 0.01 && Math.abs(snapped.y - last.y) > 0.01) {
        const dx = snapped.x - last.x, dy = snapped.y - last.y;
        const diag = Math.min(Math.abs(dx), Math.abs(dy));
        const mx = last.x + Math.sign(dx) * diag;
        const my = last.y + Math.sign(dy) * diag;
        newPoints.push({ x: mx, y: my });
      }
    }
    // "free" mode: direct line, no bend point

    newPoints.push(snapped);
    set({ wireDrawing: { ...wireDrawing, points: newPoints } });
  },

  removeLastWirePoint: () => {
    const { wireDrawing } = get();
    if (!wireDrawing.active || wireDrawing.points.length <= 1) return;
    const newPoints = wireDrawing.points.slice(0, -1);
    set({ wireDrawing: { ...wireDrawing, points: newPoints } });
  },

  cycleWireRouting: () => {
    const { wireDrawing } = get();
    const modes: WireRoutingMode[] = ["manhattan", "diagonal", "free"];
    const idx = modes.indexOf(wireDrawing.routingMode);
    const next = modes[(idx + 1) % modes.length];
    set({ wireDrawing: { ...wireDrawing, routingMode: next } });
  },

  finishWire: () => {
    const { wireDrawing, data } = get();
    const mode = wireDrawing.routingMode;
    if (!wireDrawing.active || wireDrawing.points.length < 2 || !data) {
      set({ wireDrawing: { points: [], active: false, routingMode: mode } });
      return;
    }

    get().pushUndo();
    const newData = cloneData(data);

    // Create wire segments between consecutive points
    const newWirePoints: SchPoint[] = [];
    for (let i = 0; i < wireDrawing.points.length - 1; i++) {
      newData.wires.push({
        uuid: generateUuid(),
        start: wireDrawing.points[i],
        end: wireDrawing.points[i + 1],
      });
      newWirePoints.push(wireDrawing.points[i]);
    }
    newWirePoints.push(wireDrawing.points[wireDrawing.points.length - 1]);

    // Auto-junction: check if any new wire endpoint touches an existing wire's midpoint
    autoJunction(newData, newWirePoints);

    set({
      data: newData,
      dirty: true,
      wireDrawing: { points: [], active: false, routingMode: mode },
    });
  },

  cancelWire: () => {
    const mode = get().wireDrawing.routingMode;
    set({ wireDrawing: { points: [], active: false, routingMode: mode }, editMode: "select" });
  },

  selectAll: () => {
    const { data } = get();
    if (!data) return;
    const ids = new Set<string>();
    for (const s of data.symbols) if (!s.is_power) ids.add(s.uuid);
    for (const w of data.wires) ids.add(w.uuid);
    for (const l of data.labels) ids.add(l.uuid);
    for (const j of data.junctions) ids.add(j.uuid);
    set({ selectedIds: ids });
  },

  alignSelected: (direction) => {
    const { data, selectedIds } = get();
    if (!data || selectedIds.size < 2) return;

    // Collect positions of selected symbols
    const positions: { uuid: string; x: number; y: number }[] = [];
    for (const sym of data.symbols) {
      if (selectedIds.has(sym.uuid)) positions.push({ uuid: sym.uuid, x: sym.position.x, y: sym.position.y });
    }
    for (const label of data.labels) {
      if (selectedIds.has(label.uuid)) positions.push({ uuid: label.uuid, x: label.position.x, y: label.position.y });
    }
    if (positions.length < 2) return;

    get().pushUndo();
    const newData = cloneData(data);

    let target: number;
    switch (direction) {
      case "left": target = Math.min(...positions.map(p => p.x)); break;
      case "right": target = Math.max(...positions.map(p => p.x)); break;
      case "top": target = Math.min(...positions.map(p => p.y)); break;
      case "bottom": target = Math.max(...positions.map(p => p.y)); break;
      case "centerH": target = positions.reduce((s, p) => s + p.x, 0) / positions.length; break;
      case "centerV": target = positions.reduce((s, p) => s + p.y, 0) / positions.length; break;
    }

    for (const pos of positions) {
      const isH = direction === "left" || direction === "right" || direction === "centerH";
      const delta = isH ? target - pos.x : target - pos.y;

      const sym = newData.symbols.find(s => s.uuid === pos.uuid);
      if (sym) {
        if (isH) { sym.position.x += delta; sym.ref_text.position.x += delta; sym.val_text.position.x += delta; }
        else { sym.position.y += delta; sym.ref_text.position.y += delta; sym.val_text.position.y += delta; }
      }
      const label = newData.labels.find(l => l.uuid === pos.uuid);
      if (label) {
        if (isH) label.position.x += delta;
        else label.position.y += delta;
      }
    }

    set({ data: newData, dirty: true });
  },

  distributeSelected: (axis) => {
    const { data, selectedIds } = get();
    if (!data || selectedIds.size < 3) return;

    const positions: { uuid: string; x: number; y: number }[] = [];
    for (const sym of data.symbols) {
      if (selectedIds.has(sym.uuid)) positions.push({ uuid: sym.uuid, x: sym.position.x, y: sym.position.y });
    }
    for (const label of data.labels) {
      if (selectedIds.has(label.uuid)) positions.push({ uuid: label.uuid, x: label.position.x, y: label.position.y });
    }
    if (positions.length < 3) return;

    get().pushUndo();
    const newData = cloneData(data);

    const isH = axis === "horizontal";
    positions.sort((a, b) => isH ? a.x - b.x : a.y - b.y);
    const first = isH ? positions[0].x : positions[0].y;
    const last = isH ? positions[positions.length - 1].x : positions[positions.length - 1].y;
    const step = (last - first) / (positions.length - 1);

    for (let i = 1; i < positions.length - 1; i++) {
      const target = first + step * i;
      const pos = positions[i];
      const delta = target - (isH ? pos.x : pos.y);

      const sym = newData.symbols.find(s => s.uuid === pos.uuid);
      if (sym) {
        if (isH) { sym.position.x += delta; sym.ref_text.position.x += delta; sym.val_text.position.x += delta; }
        else { sym.position.y += delta; sym.ref_text.position.y += delta; sym.val_text.position.y += delta; }
      }
      const label = newData.labels.find(l => l.uuid === pos.uuid);
      if (label) {
        if (isH) label.position.x += delta;
        else label.position.y += delta;
      }
    }

    set({ data: newData, dirty: true });
  },

  // Component placement
  startPlacement: (lib, meta) => {
    set({
      editMode: "placeSymbol",
      placingSymbol: { lib, meta, rotation: 0, mirrorX: false, mirrorY: false },
      wireDrawing: { points: [], active: false, routingMode: "manhattan" as WireRoutingMode },
    });
  },

  rotatePlacement: () => {
    const { placingSymbol } = get();
    if (!placingSymbol) return;
    set({ placingSymbol: { ...placingSymbol, rotation: (placingSymbol.rotation + 90) % 360 } });
  },

  mirrorPlacementX: () => {
    const { placingSymbol } = get();
    if (!placingSymbol) return;
    set({ placingSymbol: { ...placingSymbol, mirrorX: !placingSymbol.mirrorX } });
  },

  mirrorPlacementY: () => {
    const { placingSymbol } = get();
    if (!placingSymbol) return;
    set({ placingSymbol: { ...placingSymbol, mirrorY: !placingSymbol.mirrorY } });
  },

  placeSymbolAt: (pos) => {
    const { data, placingSymbol } = get();
    if (!data || !placingSymbol) return;

    get().pushUndo();
    const newData = cloneData(data);
    const snapped = snapPoint(pos);

    // Auto-generate reference designator
    const prefix = placingSymbol.meta.reference_prefix || "U";
    const existing = newData.symbols
      .filter((s) => s.reference.startsWith(prefix))
      .map((s) => parseInt(s.reference.slice(prefix.length)) || 0);
    const nextNum = existing.length > 0 ? Math.max(...existing) + 1 : 1;
    const reference = `${prefix}${nextNum}`;

    const fs = 1.27;
    const newSymbol: SchSymbol = {
      uuid: generateUuid(),
      lib_id: `${placingSymbol.meta.library}:${placingSymbol.meta.symbol_id}`,
      reference,
      value: placingSymbol.meta.symbol_id,
      footprint: "",
      position: snapped,
      rotation: placingSymbol.rotation,
      mirror_x: placingSymbol.mirrorX,
      mirror_y: placingSymbol.mirrorY,
      unit: 1,
      is_power: false,
      ref_text: {
        position: { x: snapped.x, y: snapped.y - 2 },
        rotation: 0, font_size: fs,
        justify_h: "center", justify_v: "bottom",
        hidden: false,
      },
      val_text: {
        position: { x: snapped.x, y: snapped.y + 2 },
        rotation: 0, font_size: fs,
        justify_h: "center", justify_v: "top",
        hidden: false,
      },
      fields_autoplaced: true,
    };

    // Also add the lib symbol to the document so rendering works
    const libId = newSymbol.lib_id;
    if (!newData.lib_symbols[libId]) {
      newData.lib_symbols[libId] = placingSymbol.lib;
    }

    newData.symbols.push(newSymbol);
    set({ data: newData, dirty: true });
    // Stay in placement mode for placing more of the same
  },

  cancelPlacement: () => {
    set({ placingSymbol: null, editMode: "select" });
  },
}));
