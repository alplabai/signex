import { create } from "zustand";
import { persist } from "zustand/middleware";
import type { PanelConfig, PanelId } from "@/types";

interface LayoutState {
  leftPanelWidth: number;
  rightPanelWidth: number;
  bottomPanelHeight: number;
  leftCollapsed: boolean;
  rightCollapsed: boolean;
  bottomCollapsed: boolean;
  panels: PanelConfig[];

  setLeftWidth: (w: number) => void;
  setRightWidth: (w: number) => void;
  setBottomHeight: (h: number) => void;
  toggleLeft: () => void;
  toggleRight: () => void;
  toggleBottom: () => void;
  togglePanel: (id: PanelId) => void;
}

const defaultPanels: PanelConfig[] = [
  { id: "projects", title: "Projects", position: "left", visible: true },
  { id: "components", title: "Components", position: "left", visible: true },
  { id: "navigator", title: "Navigator", position: "left", visible: false },
  { id: "properties", title: "Properties", position: "right", visible: true },
  { id: "inspector", title: "Inspector", position: "right", visible: false },
  { id: "rules", title: "Design Rules", position: "right", visible: false },
  { id: "messages", title: "Messages", position: "bottom", visible: true },
  { id: "drc", title: "DRC Violations", position: "bottom", visible: false },
  { id: "ai-chat", title: "AI Copilot", position: "bottom", visible: false },
];

export const useLayoutStore = create<LayoutState>()(
  persist(
    (set) => ({
      leftPanelWidth: 260,
      rightPanelWidth: 300,
      bottomPanelHeight: 200,
      leftCollapsed: false,
      rightCollapsed: false,
      bottomCollapsed: true,
      panels: defaultPanels,

      setLeftWidth: (w) => set({ leftPanelWidth: w }),
      setRightWidth: (w) => set({ rightPanelWidth: w }),
      setBottomHeight: (h) => set({ bottomPanelHeight: h }),
      toggleLeft: () => set((s) => ({ leftCollapsed: !s.leftCollapsed })),
      toggleRight: () => set((s) => ({ rightCollapsed: !s.rightCollapsed })),
      toggleBottom: () => set((s) => ({ bottomCollapsed: !s.bottomCollapsed })),
      togglePanel: (id) =>
        set((s) => ({
          panels: s.panels.map((p) =>
            p.id === id ? { ...p, visible: !p.visible } : p
          ),
        })),
    }),
    { name: "alp-eda-layout" }
  )
);
