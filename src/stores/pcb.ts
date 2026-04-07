import { create } from "zustand";
import type { PcbData, PcbPoint, PcbLayerId } from "@/types/pcb";

export type PcbEditMode =
  | "select"
  | "routeTrack"
  | "placeVia"
  | "placeFootprint"
  | "drawBoardOutline"
  | "placeZone"
  | "placeKeepout"
  | "drawLine"
  | "drawRect"
  | "placeText"
  | "placeDimension";

interface PcbState {
  data: PcbData | null;
  editMode: PcbEditMode;
  selectedIds: Set<string>;
  activeLayer: PcbLayerId;
  visibleLayers: Set<PcbLayerId>;
  dirty: boolean;
  undoStack: PcbData[];
  redoStack: PcbData[];

  // Routing state
  routingActive: boolean;
  routingPoints: PcbPoint[];
  routingLayer: PcbLayerId;
  routingWidth: number;
  routingNet: number;
  routeCornerMode: "45" | "90" | "arc45" | "arc90" | "any";

  // Actions
  loadPcb: (data: PcbData) => void;
  setEditMode: (mode: PcbEditMode) => void;
  pushUndo: () => void;
  undo: () => void;
  redo: () => void;

  // Selection
  select: (uuid: string) => void;
  selectMultiple: (uuids: string[]) => void;
  toggleSelect: (uuid: string) => void;
  deselectAll: () => void;
  selectAll: () => void;
  deleteSelected: () => void;

  // Layer
  setActiveLayer: (layer: PcbLayerId) => void;
  toggleLayerVisibility: (layer: PcbLayerId) => void;
  setAllLayersVisible: () => void;

  // Footprint placement
  moveFootprint: (uuid: string, pos: PcbPoint) => void;
  rotateFootprint: (uuid: string, delta: number) => void;
  flipFootprint: (uuid: string) => void;

  // Routing
  startRoute: (pos: PcbPoint, net: number) => void;
  addRoutePoint: (pos: PcbPoint) => void;
  finishRoute: () => void;
  cancelRoute: () => void;
  setRouteWidth: (width: number) => void;
  cycleCornerMode: () => void;

  // Via
  placeVia: (pos: PcbPoint, net: number) => void;

  // Zone
  addZone: (layer: PcbLayerId, net: number, outline: PcbPoint[]) => void;

  // Board outline
  setBoardOutline: (outline: PcbPoint[]) => void;
}

const MAX_UNDO = 50;

function cloneData(d: PcbData): PcbData {
  return structuredClone(d);
}

export const usePcbStore = create<PcbState>()((set, get) => ({
  data: null,
  editMode: "select",
  selectedIds: new Set<string>(),
  activeLayer: "F.Cu" as PcbLayerId,
  visibleLayers: new Set<PcbLayerId>([
    "F.Cu", "B.Cu", "In1.Cu", "In2.Cu",
    "F.SilkS", "B.SilkS", "F.Mask", "B.Mask",
    "F.Paste", "B.Paste", "F.Fab", "B.Fab",
    "F.CrtYd", "B.CrtYd", "Edge.Cuts",
  ]),
  dirty: false,
  undoStack: [],
  redoStack: [],

  routingActive: false,
  routingPoints: [],
  routingLayer: "F.Cu" as PcbLayerId,
  routingWidth: 0.25,
  routingNet: 0,
  routeCornerMode: "45",

  loadPcb: (data) => set({
    data,
    dirty: false,
    selectedIds: new Set(),
    undoStack: [],
    redoStack: [],
    editMode: "select",
  }),

  setEditMode: (mode) => {
    const { routingActive } = get();
    if (routingActive && mode !== "routeTrack") get().cancelRoute();
    set({ editMode: mode, selectedIds: new Set() });
  },

  pushUndo: () => {
    const { data, undoStack } = get();
    if (!data) return;
    const stack = [...undoStack, cloneData(data)];
    if (stack.length > MAX_UNDO) stack.shift();
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
      selectedIds: new Set(),
      dirty: true,
    });
  },

  redo: () => {
    const { data, undoStack, redoStack } = get();
    if (redoStack.length === 0 || !data) return;
    const next = redoStack[redoStack.length - 1];
    set({
      data: next,
      redoStack: redoStack.slice(0, -1),
      undoStack: [...undoStack, cloneData(data!)],
      selectedIds: new Set(),
      dirty: true,
    });
  },

  // --- Selection ---
  select: (uuid) => set({ selectedIds: new Set([uuid]) }),
  selectMultiple: (uuids) => set({ selectedIds: new Set(uuids) }),
  toggleSelect: (uuid) => set((s) => {
    const ids = new Set(s.selectedIds);
    if (ids.has(uuid)) ids.delete(uuid); else ids.add(uuid);
    return { selectedIds: ids };
  }),
  deselectAll: () => set({ selectedIds: new Set() }),
  selectAll: () => {
    const { data } = get();
    if (!data) return;
    const ids = new Set<string>();
    for (const f of data.footprints) ids.add(f.uuid);
    for (const s of data.segments) ids.add(s.uuid);
    for (const v of data.vias) ids.add(v.uuid);
    for (const z of data.zones) ids.add(z.uuid);
    set({ selectedIds: ids });
  },

  deleteSelected: () => {
    const { data, selectedIds } = get();
    if (!data || selectedIds.size === 0) return;
    get().pushUndo();
    const nd = cloneData(data);
    nd.footprints = nd.footprints.filter((f) => !selectedIds.has(f.uuid));
    nd.segments = nd.segments.filter((s) => !selectedIds.has(s.uuid));
    nd.vias = nd.vias.filter((v) => !selectedIds.has(v.uuid));
    nd.zones = nd.zones.filter((z) => !selectedIds.has(z.uuid));
    set({ data: nd, dirty: true, selectedIds: new Set() });
  },

  // --- Layers ---
  setActiveLayer: (layer) => set({ activeLayer: layer }),
  toggleLayerVisibility: (layer) => set((s) => {
    const vis = new Set(s.visibleLayers);
    if (vis.has(layer)) vis.delete(layer); else vis.add(layer);
    return { visibleLayers: vis };
  }),
  setAllLayersVisible: () => set((s) => {
    if (!s.data) return {};
    const vis = new Set<PcbLayerId>(s.data.board.layers.layers.map((l) => l.id));
    return { visibleLayers: vis };
  }),

  // --- Footprint operations ---
  moveFootprint: (uuid, pos) => {
    const { data } = get();
    if (!data) return;
    const nd = cloneData(data);
    const fp = nd.footprints.find((f) => f.uuid === uuid);
    if (fp) { fp.position = pos; }
    set({ data: nd, dirty: true });
  },

  rotateFootprint: (uuid, delta) => {
    const { data } = get();
    if (!data) return;
    get().pushUndo();
    const nd = cloneData(data);
    const fp = nd.footprints.find((f) => f.uuid === uuid);
    if (fp) { fp.rotation = (fp.rotation + delta) % 360; }
    set({ data: nd, dirty: true });
  },

  flipFootprint: (uuid) => {
    const { data } = get();
    if (!data) return;
    get().pushUndo();
    const nd = cloneData(data);
    const fp = nd.footprints.find((f) => f.uuid === uuid);
    if (fp) {
      fp.layer = fp.layer === "F.Cu" ? "B.Cu" : "F.Cu";
      // Flip all pad layers
      for (const pad of fp.pads) {
        pad.layers = pad.layers.map((l) => {
          if (l === "F.Cu") return "B.Cu";
          if (l === "B.Cu") return "F.Cu";
          if (l === "F.SilkS") return "B.SilkS";
          if (l === "B.SilkS") return "F.SilkS";
          if (l === "F.Mask") return "B.Mask";
          if (l === "B.Mask") return "F.Mask";
          return l;
        }) as PcbLayerId[];
      }
    }
    set({ data: nd, dirty: true });
  },

  // --- Routing ---
  startRoute: (pos, net) => {
    const { activeLayer, routingWidth } = get();
    set({
      routingActive: true,
      routingPoints: [pos],
      routingLayer: activeLayer,
      routingNet: net,
      routingWidth,
      editMode: "routeTrack",
    });
  },

  addRoutePoint: (pos) => {
    set((s) => ({ routingPoints: [...s.routingPoints, pos] }));
  },

  finishRoute: () => {
    const { data, routingPoints, routingLayer, routingWidth, routingNet } = get();
    if (!data || routingPoints.length < 2) {
      set({ routingActive: false, routingPoints: [] });
      return;
    }
    get().pushUndo();
    const nd = cloneData(data);
    for (let i = 0; i < routingPoints.length - 1; i++) {
      nd.segments.push({
        uuid: crypto.randomUUID(),
        start: { ...routingPoints[i] },
        end: { ...routingPoints[i + 1] },
        width: routingWidth,
        layer: routingLayer,
        net: routingNet,
      });
    }
    set({ data: nd, dirty: true, routingActive: false, routingPoints: [] });
  },

  cancelRoute: () => set({ routingActive: false, routingPoints: [] }),

  setRouteWidth: (width) => set({ routingWidth: width }),

  cycleCornerMode: () => set((s) => {
    const modes: PcbState["routeCornerMode"][] = ["45", "90", "arc45", "arc90", "any"];
    const idx = modes.indexOf(s.routeCornerMode);
    return { routeCornerMode: modes[(idx + 1) % modes.length] };
  }),

  // --- Via ---
  placeVia: (pos, net) => {
    const { data } = get();
    if (!data) return;
    get().pushUndo();
    const nd = cloneData(data);
    const setup = nd.board.setup;
    nd.vias.push({
      uuid: crypto.randomUUID(),
      position: pos,
      diameter: setup.viaDiameter,
      drill: setup.viaDrill,
      layers: ["F.Cu", "B.Cu"],
      net,
      type: "through",
    });
    set({ data: nd, dirty: true });
  },

  // --- Zone ---
  addZone: (layer, net, outline) => {
    const { data } = get();
    if (!data) return;
    get().pushUndo();
    const nd = cloneData(data);
    const netInfo = nd.nets.find((n) => n.number === net);
    nd.zones.push({
      uuid: crypto.randomUUID(),
      net,
      netName: netInfo?.name || "",
      layer,
      outline,
      priority: 0,
      fillType: "solid",
      thermalRelief: true,
      thermalGap: 0.508,
      thermalWidth: 0.254,
      clearance: nd.board.setup.clearance,
      minThickness: 0.254,
    });
    set({ data: nd, dirty: true });
  },

  // --- Board outline ---
  setBoardOutline: (outline) => {
    const { data } = get();
    if (!data) return;
    get().pushUndo();
    const nd = cloneData(data);
    nd.board.outline = outline;
    set({ data: nd, dirty: true });
  },
}));
