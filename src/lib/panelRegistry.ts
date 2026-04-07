// Maps panel IDs to their React component and metadata

export type PanelId =
  | "projects" | "components" | "navigator"
  | "properties" | "filter" | "list"
  | "messages" | "output-jobs" | "signal"
  | "inspector" | "drc" | "layerStack" | "snippets" | "variants" | "boardCrossSection";

export interface PanelDef {
  id: PanelId;
  title: string;
  defaultDock: "left" | "right" | "bottom";
}

export const PANEL_DEFS: PanelDef[] = [
  { id: "projects", title: "Projects", defaultDock: "left" },
  { id: "components", title: "Components", defaultDock: "left" },
  { id: "navigator", title: "Navigator", defaultDock: "left" },
  { id: "properties", title: "Properties", defaultDock: "right" },
  { id: "filter", title: "Filter", defaultDock: "right" },
  { id: "list", title: "List", defaultDock: "right" },
  { id: "messages", title: "Messages", defaultDock: "bottom" },
  { id: "output-jobs", title: "Output Jobs", defaultDock: "bottom" },
  { id: "signal", title: "Signal", defaultDock: "bottom" },
];
