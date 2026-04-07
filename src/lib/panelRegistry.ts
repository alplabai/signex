// Maps panel IDs to their React component and metadata

export type PanelId =
  | "projects" | "components" | "navigator"
  | "properties" | "filter" | "list"
  | "messages" | "output-jobs" | "signal"
  | "inspector" | "drc" | "layerStack" | "snippets" | "variants" | "boardCrossSection";

export type EditorContext = "schematic" | "pcb" | "both";

export interface PanelDef {
  id: PanelId;
  title: string;
  defaultDock: "left" | "right" | "bottom";
  context: EditorContext; // Which editor mode this panel is available in
}

export const PANEL_DEFS: PanelDef[] = [
  // Shared panels (available in both schematic and PCB)
  { id: "projects", title: "Projects", defaultDock: "left", context: "both" },
  { id: "components", title: "Components", defaultDock: "left", context: "both" },
  { id: "navigator", title: "Navigator", defaultDock: "left", context: "both" },
  { id: "properties", title: "Properties", defaultDock: "right", context: "both" },
  { id: "messages", title: "Messages", defaultDock: "bottom", context: "both" },
  { id: "signal", title: "Signal", defaultDock: "bottom", context: "both" },

  // Schematic-only panels
  { id: "filter", title: "SCH Filter", defaultDock: "right", context: "schematic" },
  { id: "list", title: "SCH List", defaultDock: "right", context: "schematic" },
  { id: "output-jobs", title: "Output Jobs", defaultDock: "bottom", context: "schematic" },
  { id: "inspector", title: "Inspector", defaultDock: "right", context: "schematic" },
  { id: "snippets", title: "Snippets", defaultDock: "right", context: "schematic" },
  { id: "variants", title: "Variants", defaultDock: "right", context: "schematic" },

  // PCB-only panels
  { id: "layerStack", title: "Layers", defaultDock: "right", context: "pcb" },
  { id: "drc", title: "DRC", defaultDock: "bottom", context: "pcb" },
  { id: "boardCrossSection", title: "Cross Section", defaultDock: "bottom", context: "pcb" },
];

/** Get panels available for the current editor mode */
export function getPanelsForContext(mode: "schematic" | "pcb"): PanelDef[] {
  return PANEL_DEFS.filter(p => p.context === "both" || p.context === mode);
}
