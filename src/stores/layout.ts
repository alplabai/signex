import { create } from "zustand";
import { persist } from "zustand/middleware";
import type { PanelConfig, PanelId } from "@/types";
import type { PanelId as DockPanelId } from "@/lib/panelRegistry";

type DockId = "left" | "right" | "bottom";

interface LayoutState {
  leftPanelWidth: number;
  rightPanelWidth: number;
  bottomPanelHeight: number;
  leftCollapsed: boolean;
  rightCollapsed: boolean;
  bottomCollapsed: boolean;
  panels: PanelConfig[];
  docks: {
    left: DockPanelId[];
    right: DockPanelId[];
    bottom: DockPanelId[];
  };
  activeTab: Record<string, DockPanelId>;

  setLeftWidth: (w: number) => void;
  setRightWidth: (w: number) => void;
  setBottomHeight: (h: number) => void;
  toggleLeft: () => void;
  toggleRight: () => void;
  toggleBottom: () => void;
  togglePanel: (id: PanelId) => void;
  movePanel: (panelId: string, targetDock: DockId, index?: number) => void;
  removePanel: (panelId: string) => void;
  setDockActiveTab: (dock: string, panelId: string) => void;
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
      docks: {
        left: ["projects", "components", "navigator"],
        right: ["properties", "list"],
        bottom: ["messages", "output-jobs", "signal"],
      },
      activeTab: { left: "projects", right: "properties", bottom: "messages" },

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
      movePanel: (panelId, targetDock, index) =>
        set((s) => {
          const newDocks = {
            left: [...s.docks.left],
            right: [...s.docks.right],
            bottom: [...s.docks.bottom],
          };
          const newActiveTab = { ...s.activeTab };

          // Remove from current dock
          for (const dock of ["left", "right", "bottom"] as DockId[]) {
            const idx = newDocks[dock].indexOf(panelId as DockPanelId);
            if (idx !== -1) {
              newDocks[dock].splice(idx, 1);
              // If active tab was removed, pick first remaining
              if (newActiveTab[dock] === panelId && newDocks[dock].length > 0) {
                newActiveTab[dock] = newDocks[dock][0];
              }
              break;
            }
          }

          // Insert into target dock
          const insertIdx = index !== undefined ? index : newDocks[targetDock].length;
          newDocks[targetDock].splice(insertIdx, 0, panelId as DockPanelId);
          // Make the moved panel the active tab in the target dock
          newActiveTab[targetDock] = panelId as DockPanelId;

          return { docks: newDocks, activeTab: newActiveTab };
        }),
      removePanel: (panelId) =>
        set((s) => {
          const newDocks = {
            left: [...s.docks.left],
            right: [...s.docks.right],
            bottom: [...s.docks.bottom],
          };
          const newActiveTab = { ...s.activeTab };

          for (const dock of ["left", "right", "bottom"] as DockId[]) {
            const idx = newDocks[dock].indexOf(panelId as DockPanelId);
            if (idx !== -1) {
              newDocks[dock].splice(idx, 1);
              if (newActiveTab[dock] === panelId && newDocks[dock].length > 0) {
                newActiveTab[dock] = newDocks[dock][0];
              }
              break;
            }
          }

          return { docks: newDocks, activeTab: newActiveTab };
        }),
      setDockActiveTab: (dock, panelId) =>
        set((s) => ({
          activeTab: { ...s.activeTab, [dock]: panelId as DockPanelId },
        })),
    }),
    {
      name: "signex-layout",
      version: 3,
      migrate: (persisted: unknown, version: number) => {
        const state = persisted as Record<string, unknown>;
        if (version < 3) {
          // v0/v1 → v2: add docks and activeTab
          return {
            ...state,
            docks: {
              left: ["projects", "components", "navigator"],
              right: ["properties", "list"],
              bottom: ["messages", "output-jobs", "signal"],
            },
            activeTab: { left: "projects", right: "properties", bottom: "messages" },
          } as unknown as LayoutState;
        }
        return persisted as LayoutState;
      },
    }
  )
);
