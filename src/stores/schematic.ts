import { create } from "zustand";
import { useEditorStore } from "@/stores/editor";
import type { SchematicData, SchPoint, SchSymbol, SchWire, SchLabel, SchJunction, SchNoConnect, TextNote, SchBus, SchBusEntry, SchDrawing, LibSymbol, SymbolSearchResult } from "@/types";

export type EditMode = "select" | "drawWire" | "drawBus" | "placeSymbol" | "placeLabel" | "placePower" | "placeNoConnect" | "placeNoErc" | "placePort" | "placeText" | "drawLine" | "drawRect";
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
  moveElements: (uuids: string[], dx: number, dy: number, noRubberBand?: boolean) => void;
  moveWireEndpoint: (uuid: string, endpoint: "start" | "end", pos: SchPoint) => void;
  breakWire: (uuid: string, atPoint: SchPoint) => void;
  addWire: (start: SchPoint, end: SchPoint) => void;
  addSymbol: (symbol: SchSymbol) => void;
  addLabel: (label: SchLabel) => void;
  addJunction: (pos: SchPoint) => void;
  deleteSelected: () => void;
  rotateSelected: () => void;
  mirrorSelectedX: () => void;
  mirrorSelectedY: () => void;
  duplicateSelected: () => void;

  // Property editing
  updateSymbolProp: (uuid: string, key: string, value: string) => void;
  updateLabelProp: (uuid: string, key: string, value: string) => void;
  updateTextNoteProp: (uuid: string, key: string, value: string) => void;

  // Clipboard
  copySelected: () => void;
  pasteClipboard: (offset: SchPoint) => void;
  clipboard: {
    symbols: SchSymbol[]; wires: SchWire[]; labels: SchLabel[];
    junctions: SchJunction[]; noConnects: SchNoConnect[]; textNotes: TextNote[];
    buses: SchBus[]; busEntries: SchBusEntry[];
  } | null;

  // Placement
  placeNoErcDirective: (pos: SchPoint) => void;
  toggleDesignatorLock: (uuid: string) => void;
  placeNetLabel: (pos: SchPoint, text: string) => void;
  placePowerPort: (pos: SchPoint, netName: string, style: string) => void;
  placeNoConnect: (pos: SchPoint) => void;
  placePort: (pos: SchPoint, text: string, shape: string) => void;
  addTextNote: (pos: SchPoint, text: string) => void;

  finishBus: () => void;

  // Drawing object placement
  addDrawing: (drawing: SchDrawing) => void;

  // Wire/Bus drawing
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

  // Z-ordering
  bringToFront: () => void;
  sendToBack: () => void;

  // Batch editing
  updateMultipleSymbolProp: (uuids: string[], key: string, value: string) => void;
  updateMultipleLabelProp: (uuids: string[], key: string, value: string) => void;

  // Find Similar & Annotation
  findSimilar: () => void;
  annotateAll: () => void;
  resetDesignators: () => void;
  resetDuplicateDesignators: () => void;
  annotateSelected: () => void;

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

  moveElements: (uuids, dx, dy, noRubberBand) => {
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
    for (const nc of newData.no_connects) {
      if (idSet.has(nc.uuid)) {
        nc.position.x += dx;
        nc.position.y += dy;
      }
    }
    for (const ne of newData.no_erc_directives) {
      if (idSet.has(ne.uuid)) {
        ne.position.x += dx;
        ne.position.y += dy;
      }
    }
    for (const note of newData.text_notes) {
      if (idSet.has(note.uuid)) {
        note.position.x += dx;
        note.position.y += dy;
      }
    }
    for (const b of newData.buses) {
      if (idSet.has(b.uuid)) {
        b.start.x += dx; b.start.y += dy;
        b.end.x += dx; b.end.y += dy;
      }
    }
    for (const be of newData.bus_entries) {
      if (idSet.has(be.uuid)) {
        be.position.x += dx;
        be.position.y += dy;
      }
    }
    for (const cs of newData.child_sheets) {
      if (idSet.has(cs.uuid)) {
        cs.position.x += dx;
        cs.position.y += dy;
        // Move sheet pins with the sheet
        for (const pin of cs.pins) {
          pin.position.x += dx;
          pin.position.y += dy;
        }
      }
    }

    for (const d of newData.drawings) {
      const uuid = "uuid" in d ? (d as { uuid: string }).uuid : "";
      if (!idSet.has(uuid)) continue;
      if (d.type === "Line") { d.start.x += dx; d.start.y += dy; d.end.x += dx; d.end.y += dy; }
      else if (d.type === "Rect") { d.start.x += dx; d.start.y += dy; d.end.x += dx; d.end.y += dy; }
      else if (d.type === "Circle") { d.center.x += dx; d.center.y += dy; }
      else if (d.type === "Arc") { d.start.x += dx; d.start.y += dy; d.mid.x += dx; d.mid.y += dy; d.end.x += dx; d.end.y += dy; }
      else if (d.type === "Polyline") { for (const p of d.points) { p.x += dx; p.y += dy; } }
    }

    // Rubber-banding: stretch non-selected wires connected to moving symbol pins
    if (noRubberBand) { set({ data: newData, dirty: true }); return; }
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

  moveWireEndpoint: (uuid, endpoint, pos) => {
    const { data } = get();
    if (!data) return;
    const newData = cloneData(data);
    const wire = newData.wires.find(w => w.uuid === uuid);
    if (!wire) return;
    const snapped = snapPoint(pos);
    if (endpoint === "start") { wire.start = snapped; }
    else { wire.end = snapped; }
    autoJunction(newData, [snapped]);
    set({ data: newData, dirty: true });
  },

  breakWire: (uuid, atPoint) => {
    const { data } = get();
    if (!data) return;
    get().pushUndo();
    const newData = cloneData(data);
    const idx = newData.wires.findIndex(w => w.uuid === uuid);
    if (idx === -1) return;
    const wire = newData.wires[idx];
    const snapped = snapPoint(atPoint);
    // Replace original wire with two halves
    newData.wires.splice(idx, 1,
      { uuid: wire.uuid, start: wire.start, end: snapped },
      { uuid: generateUuid(), start: snapped, end: wire.end }
    );
    // Add junction at split point
    newData.junctions.push({ uuid: generateUuid(), position: snapped });
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
    newData.no_connects = newData.no_connects.filter((nc) => !selectedIds.has(nc.uuid));
    newData.text_notes = newData.text_notes.filter((t) => !selectedIds.has(t.uuid));
    newData.buses = newData.buses.filter((b) => !selectedIds.has(b.uuid));
    newData.bus_entries = newData.bus_entries.filter((be) => !selectedIds.has(be.uuid));
    newData.child_sheets = newData.child_sheets.filter((cs) => !selectedIds.has(cs.uuid));
    newData.drawings = newData.drawings.filter((d) => !selectedIds.has(d.uuid));
    newData.no_erc_directives = newData.no_erc_directives.filter((d) => !selectedIds.has(d.uuid));
    set({ data: newData, dirty: true, selectedIds: new Set() });
  },

  rotateSelected: () => {
    const { data, selectedIds } = get();
    if (!data || selectedIds.size === 0) return;
    get().pushUndo();
    const newData = cloneData(data);
    for (const sym of newData.symbols) {
      if (selectedIds.has(sym.uuid)) {
        sym.rotation = (sym.rotation + 90) % 360;
      }
    }
    set({ data: newData, dirty: true });
  },

  mirrorSelectedX: () => {
    const { data, selectedIds } = get();
    if (!data || selectedIds.size === 0) return;
    get().pushUndo();
    const newData = cloneData(data);
    for (const sym of newData.symbols) {
      if (selectedIds.has(sym.uuid)) sym.mirror_x = !sym.mirror_x;
    }
    set({ data: newData, dirty: true });
  },

  mirrorSelectedY: () => {
    const { data, selectedIds } = get();
    if (!data || selectedIds.size === 0) return;
    get().pushUndo();
    const newData = cloneData(data);
    for (const sym of newData.symbols) {
      if (selectedIds.has(sym.uuid)) sym.mirror_y = !sym.mirror_y;
    }
    set({ data: newData, dirty: true });
  },

  duplicateSelected: () => {
    const { data, selectedIds } = get();
    if (!data || selectedIds.size === 0) return;
    get().pushUndo();
    const newData = cloneData(data);
    const newIds: string[] = [];
    const ox = 2.54, oy = 2.54;

    for (const sym of data.symbols) {
      if (!selectedIds.has(sym.uuid)) continue;
      const n = structuredClone(sym);
      n.uuid = generateUuid();
      n.position.x += ox; n.position.y += oy;
      n.ref_text.position.x += ox; n.ref_text.position.y += oy;
      n.val_text.position.x += ox; n.val_text.position.y += oy;
      // Reset reference to prefix? to avoid duplicate designators
      const prefix = n.reference.replace(/[0-9?]+$/, "");
      n.reference = `${prefix}?`;
      newData.symbols.push(n); newIds.push(n.uuid);
    }
    for (const w of data.wires) {
      if (!selectedIds.has(w.uuid)) continue;
      const n = structuredClone(w);
      n.uuid = generateUuid();
      n.start.x += ox; n.start.y += oy; n.end.x += ox; n.end.y += oy;
      newData.wires.push(n); newIds.push(n.uuid);
    }
    for (const l of data.labels) {
      if (!selectedIds.has(l.uuid)) continue;
      const n = structuredClone(l);
      n.uuid = generateUuid(); n.position.x += ox; n.position.y += oy;
      newData.labels.push(n); newIds.push(n.uuid);
    }
    for (const j of data.junctions) {
      if (!selectedIds.has(j.uuid)) continue;
      const n = structuredClone(j);
      n.uuid = generateUuid(); n.position.x += ox; n.position.y += oy;
      newData.junctions.push(n); newIds.push(n.uuid);
    }
    for (const d of data.drawings) {
      if (!selectedIds.has(d.uuid)) continue;
      const n = structuredClone(d);
      n.uuid = generateUuid();
      if (n.type === "Line" || n.type === "Rect") { n.start.x += ox; n.start.y += oy; n.end.x += ox; n.end.y += oy; }
      else if (n.type === "Circle") { n.center.x += ox; n.center.y += oy; }
      else if (n.type === "Arc") { n.start.x += ox; n.start.y += oy; n.mid.x += ox; n.mid.y += oy; n.end.x += ox; n.end.y += oy; }
      else if (n.type === "Polyline") { for (const p of n.points) { p.x += ox; p.y += oy; } }
      newData.drawings.push(n); newIds.push(n.uuid);
    }

    set({ data: newData, dirty: true, selectedIds: new Set(newIds) });
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
      case "x": sym.position.x = isNaN(parseFloat(value)) ? sym.position.x : parseFloat(value); break;
      case "y": sym.position.y = isNaN(parseFloat(value)) ? sym.position.y : parseFloat(value); break;
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
      case "x": label.position.x = isNaN(parseFloat(value)) ? label.position.x : parseFloat(value); break;
      case "y": label.position.y = isNaN(parseFloat(value)) ? label.position.y : parseFloat(value); break;
      case "rotation": label.rotation = (parseInt(value) || 0) % 360; break;
    }
    set({ data: newData, dirty: true });
  },

  updateTextNoteProp: (uuid, key, value) => {
    const { data } = get();
    if (!data) return;
    get().pushUndo();
    const newData = cloneData(data);
    const note = newData.text_notes.find((t) => t.uuid === uuid);
    if (!note) return;
    switch (key) {
      case "text": note.text = value; break;
      case "x": note.position.x = isNaN(parseFloat(value)) ? note.position.x : parseFloat(value); break;
      case "y": note.position.y = isNaN(parseFloat(value)) ? note.position.y : parseFloat(value); break;
      case "rotation": note.rotation = (parseInt(value) || 0) % 360; break;
    }
    set({ data: newData, dirty: true });
  },

  // Clipboard — copies all element types
  copySelected: () => {
    const { data, selectedIds } = get();
    if (!data || selectedIds.size === 0) return;
    set({
      clipboard: {
        symbols: structuredClone(data.symbols.filter((s) => selectedIds.has(s.uuid))),
        wires: structuredClone(data.wires.filter((w) => selectedIds.has(w.uuid))),
        labels: structuredClone(data.labels.filter((l) => selectedIds.has(l.uuid))),
        junctions: structuredClone(data.junctions.filter((j) => selectedIds.has(j.uuid))),
        noConnects: structuredClone(data.no_connects.filter((nc) => selectedIds.has(nc.uuid))),
        textNotes: structuredClone(data.text_notes.filter((t) => selectedIds.has(t.uuid))),
        buses: structuredClone(data.buses.filter((b) => selectedIds.has(b.uuid))),
        busEntries: structuredClone(data.bus_entries.filter((be) => selectedIds.has(be.uuid))),
      },
    });
  },

  pasteClipboard: (offset) => {
    const { data, clipboard } = get();
    if (!data || !clipboard) return;
    get().pushUndo();
    const newData = cloneData(data);
    const newIds: string[] = [];
    const ox = offset.x, oy = offset.y;

    for (const sym of clipboard.symbols) {
      const n = structuredClone(sym);
      n.uuid = generateUuid();
      n.position.x += ox; n.position.y += oy;
      n.ref_text.position.x += ox; n.ref_text.position.y += oy;
      n.val_text.position.x += ox; n.val_text.position.y += oy;
      // Reset reference to prefix? to avoid duplicate designators
      const prefix = n.reference.replace(/[0-9?]+$/, "");
      n.reference = `${prefix}?`;
      newData.symbols.push(n); newIds.push(n.uuid);
    }
    for (const w of clipboard.wires) {
      const n = structuredClone(w);
      n.uuid = generateUuid();
      n.start.x += ox; n.start.y += oy; n.end.x += ox; n.end.y += oy;
      newData.wires.push(n); newIds.push(n.uuid);
    }
    for (const l of clipboard.labels) {
      const n = structuredClone(l);
      n.uuid = generateUuid(); n.position.x += ox; n.position.y += oy;
      newData.labels.push(n); newIds.push(n.uuid);
    }
    for (const j of clipboard.junctions) {
      const n = structuredClone(j);
      n.uuid = generateUuid(); n.position.x += ox; n.position.y += oy;
      newData.junctions.push(n); newIds.push(n.uuid);
    }
    for (const nc of clipboard.noConnects) {
      const n = structuredClone(nc);
      n.uuid = generateUuid(); n.position.x += ox; n.position.y += oy;
      newData.no_connects.push(n); newIds.push(n.uuid);
    }
    for (const t of clipboard.textNotes) {
      const n = structuredClone(t);
      n.uuid = generateUuid(); n.position.x += ox; n.position.y += oy;
      newData.text_notes.push(n); newIds.push(n.uuid);
    }
    for (const b of clipboard.buses) {
      const n = structuredClone(b);
      n.uuid = generateUuid();
      n.start.x += ox; n.start.y += oy; n.end.x += ox; n.end.y += oy;
      newData.buses.push(n); newIds.push(n.uuid);
    }
    for (const be of clipboard.busEntries) {
      const n = structuredClone(be);
      n.uuid = generateUuid(); n.position.x += ox; n.position.y += oy;
      newData.bus_entries.push(n); newIds.push(n.uuid);
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
    newData.no_connects.push({ uuid: generateUuid(), position: snapped });
    set({ data: newData, dirty: true });
  },

  // No ERC directive placement
  placeNoErcDirective: (pos) => {
    const { data } = get();
    if (!data) return;
    get().pushUndo();
    const newData = cloneData(data);
    const snapped = snapPoint(pos);
    newData.no_erc_directives.push({ uuid: generateUuid(), position: snapped });
    set({ data: newData, dirty: true });
  },

  // Toggle designator lock on a symbol
  toggleDesignatorLock: (uuid) => {
    const { data } = get();
    if (!data) return;
    get().pushUndo();
    const newData = cloneData(data);
    const sym = newData.symbols.find(s => s.uuid === uuid);
    if (sym) sym.locked = !sym.locked;
    set({ data: newData, dirty: true });
  },

  // Port placement (hierarchical label)
  placePort: (pos, text, shape) => {
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
      label_type: "Hierarchical",
      shape: shape || "bidirectional",
      font_size: 1.27,
      justify: "left",
    });
    set({ data: newData, dirty: true });
  },

  // Text note placement
  addTextNote: (pos, text) => {
    const { data } = get();
    if (!data) return;
    get().pushUndo();
    const newData = cloneData(data);
    const snapped = snapPoint(pos);
    newData.text_notes.push({
      uuid: generateUuid(),
      text,
      position: snapped,
      rotation: 0,
      font_size: 1.27,
    });
    set({ data: newData, dirty: true });
  },

  addDrawing: (drawing) => {
    const { data } = get();
    if (!data) return;
    get().pushUndo();
    const newData = cloneData(data);
    newData.drawings.push(drawing);
    set({ data: newData, dirty: true });
  },

  // Wire drawing state machine
  startWire: (pos) => {
    // Caller handles snapping (electrical or grid) — use pos directly
    // Preserve current editMode (could be "drawWire" or "drawBus")
    const mode = get().wireDrawing.routingMode;
    const currentEditMode = get().editMode;
    set({
      editMode: currentEditMode === "drawBus" ? "drawBus" : "drawWire",
      wireDrawing: { points: [pos], active: true, routingMode: mode },
    });
  },

  addWirePoint: (pos) => {
    const { wireDrawing } = get();
    if (!wireDrawing.active || wireDrawing.points.length === 0) return;
    // Caller handles snapping — use pos directly
    const last = wireDrawing.points[wireDrawing.points.length - 1];
    // Skip if too close to last point (avoid zero-length segments)
    if (Math.abs(pos.x - last.x) < 0.01 && Math.abs(pos.y - last.y) < 0.01) return;
    const newPoints = [...wireDrawing.points];

    if (wireDrawing.routingMode === "manhattan") {
      // Manhattan: horizontal then vertical
      if (Math.abs(pos.x - last.x) > 0.01 && Math.abs(pos.y - last.y) > 0.01) {
        newPoints.push({ x: pos.x, y: last.y });
      }
    } else if (wireDrawing.routingMode === "diagonal") {
      // 45-degree: diagonal then orthogonal
      if (Math.abs(pos.x - last.x) > 0.01 && Math.abs(pos.y - last.y) > 0.01) {
        const dx = pos.x - last.x, dy = pos.y - last.y;
        const diag = Math.min(Math.abs(dx), Math.abs(dy));
        const mx = last.x + Math.sign(dx) * diag;
        const my = last.y + Math.sign(dy) * diag;
        newPoints.push({ x: mx, y: my });
      }
    }
    // "free" mode: direct line, no bend point

    newPoints.push(pos);
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

    // Create wire segments between consecutive points (spread to avoid storing by reference)
    const newWirePoints: SchPoint[] = [];
    for (let i = 0; i < wireDrawing.points.length - 1; i++) {
      newData.wires.push({
        uuid: generateUuid(),
        start: { ...wireDrawing.points[i] },
        end: { ...wireDrawing.points[i + 1] },
      });
      newWirePoints.push({ ...wireDrawing.points[i] });
    }
    newWirePoints.push({ ...wireDrawing.points[wireDrawing.points.length - 1] });

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

  finishBus: () => {
    const { wireDrawing, data } = get();
    const mode = wireDrawing.routingMode;
    if (!wireDrawing.active || wireDrawing.points.length < 2 || !data) {
      set({ wireDrawing: { points: [], active: false, routingMode: mode } });
      return;
    }
    get().pushUndo();
    const newData = cloneData(data);
    for (let i = 0; i < wireDrawing.points.length - 1; i++) {
      newData.buses.push({
        uuid: generateUuid(),
        start: wireDrawing.points[i],
        end: wireDrawing.points[i + 1],
      });
    }
    set({ data: newData, dirty: true, wireDrawing: { points: [], active: false, routingMode: mode } });
  },

  selectAll: () => {
    const { data } = get();
    if (!data) return;
    const ids = new Set<string>();
    for (const s of data.symbols) if (!s.is_power) ids.add(s.uuid);
    for (const w of data.wires) ids.add(w.uuid);
    for (const l of data.labels) ids.add(l.uuid);
    for (const j of data.junctions) ids.add(j.uuid);
    for (const nc of data.no_connects) ids.add(nc.uuid);
    for (const t of data.text_notes) ids.add(t.uuid);
    for (const b of data.buses) ids.add(b.uuid);
    for (const be of data.bus_entries) ids.add(be.uuid);
    for (const cs of data.child_sheets) ids.add(cs.uuid);
    for (const d of data.drawings) ids.add(d.uuid);
    for (const d of data.no_erc_directives) ids.add(d.uuid);
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
    const nextNum = (existing.length > 0 ? existing.reduce((max, v) => v > max ? v : max, 0) : 0) + 1;
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
      dnp: false,
      in_bom: true,
      on_board: true,
      exclude_from_sim: false,
      locked: false,
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

  // Find Similar: select all objects matching the selected object's type/lib_id
  findSimilar: () => {
    const { data, selectedIds } = get();
    if (!data || selectedIds.size !== 1) return;
    const selId = [...selectedIds][0];
    const ids = new Set<string>();

    // Check which type it is and find all similar
    const sym = data.symbols.find(s => s.uuid === selId);
    if (sym) {
      for (const s of data.symbols) {
        if (s.lib_id === sym.lib_id) ids.add(s.uuid);
      }
      set({ selectedIds: ids });
      return;
    }
    const wire = data.wires.find(w => w.uuid === selId);
    if (wire) {
      for (const w of data.wires) ids.add(w.uuid);
      set({ selectedIds: ids });
      return;
    }
    const label = data.labels.find(l => l.uuid === selId);
    if (label) {
      for (const l of data.labels) {
        if (l.label_type === label.label_type) ids.add(l.uuid);
      }
      set({ selectedIds: ids });
      return;
    }
    const junction = data.junctions.find(j => j.uuid === selId);
    if (junction) {
      for (const j of data.junctions) ids.add(j.uuid);
      set({ selectedIds: ids });
      return;
    }
    const nc = data.no_connects.find(n => n.uuid === selId);
    if (nc) {
      for (const n of data.no_connects) ids.add(n.uuid);
      set({ selectedIds: ids });
      return;
    }
  },

  // Z-ordering
  bringToFront: () => {
    const { data, selectedIds } = get();
    if (!data || selectedIds.size === 0) return;
    get().pushUndo();
    const nd = cloneData(data);
    // Move selected elements to end of their arrays (renders last = on top)
    for (const arr of [nd.symbols, nd.wires, nd.labels, nd.junctions, nd.no_connects, nd.text_notes, nd.buses, nd.bus_entries] as { uuid: string }[][]) {
      const sel = arr.filter(e => selectedIds.has(e.uuid));
      const rest = arr.filter(e => !selectedIds.has(e.uuid));
      arr.length = 0;
      arr.push(...rest, ...sel);
    }
    set({ data: nd, dirty: true });
  },

  sendToBack: () => {
    const { data, selectedIds } = get();
    if (!data || selectedIds.size === 0) return;
    get().pushUndo();
    const nd = cloneData(data);
    for (const arr of [nd.symbols, nd.wires, nd.labels, nd.junctions, nd.no_connects, nd.text_notes, nd.buses, nd.bus_entries] as { uuid: string }[][]) {
      const sel = arr.filter(e => selectedIds.has(e.uuid));
      const rest = arr.filter(e => !selectedIds.has(e.uuid));
      arr.length = 0;
      arr.push(...sel, ...rest);
    }
    set({ data: nd, dirty: true });
  },

  // Batch editing
  updateMultipleSymbolProp: (uuids, key, value) => {
    const { data } = get();
    if (!data) return;
    get().pushUndo();
    const nd = cloneData(data);
    for (const sym of nd.symbols) {
      if (!uuids.includes(sym.uuid)) continue;
      switch (key) {
        case "value": sym.value = value; break;
        case "footprint": sym.footprint = value; break;
      }
    }
    set({ data: nd, dirty: true });
  },

  updateMultipleLabelProp: (uuids, key, value) => {
    const { data } = get();
    if (!data) return;
    get().pushUndo();
    const nd = cloneData(data);
    for (const label of nd.labels) {
      if (!uuids.includes(label.uuid)) continue;
      if (key === "text") label.text = value;
    }
    set({ data: nd, dirty: true });
  },

  // Annotation: auto-assign designators (R1, R2, C1, C2, etc.)
  annotateAll: () => {
    const { data } = get();
    if (!data) return;
    get().pushUndo();
    const newData = cloneData(data);

    // Group non-power symbols by reference prefix
    const groups = new Map<string, typeof newData.symbols>();
    for (const sym of newData.symbols) {
      if (sym.is_power) continue;
      const prefix = sym.reference.replace(/[0-9?]+$/, "");
      if (!groups.has(prefix)) groups.set(prefix, []);
      groups.get(prefix)!.push(sym);
    }

    // For each prefix group, sort by position (top-to-bottom, left-to-right) and assign numbers
    for (const [prefix, syms] of groups) {
      // Collect already-assigned numbers
      const usedNumbers = new Set<number>();
      for (const sym of syms) {
        const num = parseInt(sym.reference.replace(/^[A-Z]+/, ""), 10);
        if (!isNaN(num) && !sym.reference.endsWith("?")) usedNumbers.add(num);
      }

      // Sort by Y first (top to bottom), then X (left to right)
      const toAnnotate = syms.filter(s => !s.locked && (s.reference.endsWith("?") || s.reference === prefix));
      toAnnotate.sort((a, b) => {
        const dy = a.position.y - b.position.y;
        if (Math.abs(dy) > 2) return dy;
        return a.position.x - b.position.x;
      });

      // Assign sequential numbers, skipping used ones
      let nextNum = 1;
      for (const sym of toAnnotate) {
        while (usedNumbers.has(nextNum)) nextNum++;
        sym.reference = `${prefix}${nextNum}`;
        // Update ref_text if it exists
        usedNumbers.add(nextNum);
        nextNum++;
      }
    }

    set({ data: newData, dirty: true });
  },

  // Annotate only selected symbols
  annotateSelected: () => {
    const { data, selectedIds } = get();
    if (!data || selectedIds.size === 0) return;
    get().pushUndo();
    const newData = cloneData(data);
    const selectedSyms = newData.symbols.filter(s => selectedIds.has(s.uuid) && !s.is_power && !s.locked);
    const groups = new Map<string, typeof selectedSyms>();
    for (const sym of selectedSyms) {
      const prefix = sym.reference.replace(/[0-9?]+$/, "");
      if (!groups.has(prefix)) groups.set(prefix, []);
      groups.get(prefix)!.push(sym);
    }
    for (const [prefix, syms] of groups) {
      const usedNumbers = new Set<number>();
      for (const s of newData.symbols) {
        if (s.is_power) continue;
        const num = parseInt(s.reference.replace(/^[A-Z]+/, ""), 10);
        if (!isNaN(num) && !s.reference.endsWith("?") && !selectedIds.has(s.uuid)) usedNumbers.add(num);
      }
      syms.sort((a, b) => {
        const dy = a.position.y - b.position.y;
        if (Math.abs(dy) > 2) return dy;
        return a.position.x - b.position.x;
      });
      let nextNum = 1;
      for (const sym of syms) {
        while (usedNumbers.has(nextNum)) nextNum++;
        sym.reference = `${prefix}${nextNum}`;
        usedNumbers.add(nextNum);
        nextNum++;
      }
    }
    set({ data: newData, dirty: true });
  },

  // Reset ALL designators to prefix + "?"
  resetDesignators: () => {
    const { data } = get();
    if (!data) return;
    get().pushUndo();
    const nd = cloneData(data);
    for (const sym of nd.symbols) {
      if (sym.is_power || sym.locked) continue;
      const prefix = sym.reference.replace(/[0-9?]+$/, "");
      sym.reference = `${prefix}?`;
    }
    set({ data: nd, dirty: true });
  },

  // Reset only DUPLICATE designators to "?"
  resetDuplicateDesignators: () => {
    const { data } = get();
    if (!data) return;
    get().pushUndo();
    const nd = cloneData(data);
    const seen = new Map<string, string>(); // ref → first uuid
    for (const sym of nd.symbols) {
      if (sym.is_power) continue;
      if (sym.reference.endsWith("?")) continue;
      if (seen.has(sym.reference)) {
        // Duplicate — reset to "?"
        const prefix = sym.reference.replace(/[0-9]+$/, "");
        sym.reference = `${prefix}?`;
      } else {
        seen.set(sym.reference, sym.uuid);
      }
    }
    set({ data: nd, dirty: true });
  },
}));
