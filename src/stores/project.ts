import { create } from "zustand";
import type { DocumentTab, ProjectInfo } from "@/types";

interface ProjectState {
  project: ProjectInfo | null;
  openTabs: DocumentTab[];
  activeTabId: string | null;
  recentProjects: string[];
  activeTemplate: string; // Template name ("Blank", "ISO A4", "ANSI A")
  netScope: "global" | "flat" | "hierarchical";

  setProject: (project: ProjectInfo | null) => void;
  openTab: (tab: DocumentTab) => void;
  closeTab: (id: string) => void;
  setActiveTab: (id: string) => void;
  addRecentProject: (path: string) => void;
  setActiveTemplate: (name: string) => void;
  setNetScope: (scope: "global" | "flat" | "hierarchical") => void;
}

export const useProjectStore = create<ProjectState>()((set) => ({
  project: null,
  openTabs: [],
  activeTabId: null,
  recentProjects: [],
  activeTemplate: "Blank",
  netScope: "flat",

  setProject: (project) => set({ project }),

  openTab: (tab) =>
    set((s) => {
      const exists = s.openTabs.find((t) => t.id === tab.id);
      if (exists) return { activeTabId: tab.id };
      return {
        openTabs: [...s.openTabs, tab],
        activeTabId: tab.id,
      };
    }),

  closeTab: (id) =>
    set((s) => {
      const tabs = s.openTabs.filter((t) => t.id !== id);
      const activeTabId =
        s.activeTabId === id
          ? tabs[tabs.length - 1]?.id ?? null
          : s.activeTabId;
      return { openTabs: tabs, activeTabId };
    }),

  setActiveTab: (id) => set({ activeTabId: id }),

  addRecentProject: (path) =>
    set((s) => ({
      recentProjects: [path, ...s.recentProjects.filter((p) => p !== path)].slice(0, 10),
    })),

  setActiveTemplate: (name) => set({ activeTemplate: name }),
  setNetScope: (scope) => set({ netScope: scope }),
}));
