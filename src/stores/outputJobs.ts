import { create } from "zustand";
import { persist } from "zustand/middleware";

export type OutputJobType = "bom" | "netlist" | "pdf" | "png";

export interface OutputJobConfig {
  // BOM
  bomColumns?: string[];
  bomGroupBy?: string[];
  bomFormat?: "csv" | "tsv";
  // Netlist
  netlistFormat?: "kicad" | "xml";
  // PDF
  pdfDpi?: 150 | 300;
  pdfColorMode?: "color" | "monochrome";
  pdfShowGrid?: boolean;
}

export interface OutputJob {
  id: string;
  name: string;
  type: OutputJobType;
  enabled: boolean;
  config: OutputJobConfig;
}

interface OutputJobsState {
  jobs: OutputJob[];
  addJob: (type: OutputJobType) => void;
  removeJob: (id: string) => void;
  updateJob: (id: string, updates: Partial<Omit<OutputJob, "id">>) => void;
  toggleJob: (id: string) => void;
  reorderJob: (id: string, direction: "up" | "down") => void;
}

const defaultConfig: Record<OutputJobType, OutputJobConfig> = {
  bom: { bomColumns: ["Designator", "Value", "Footprint", "Library", "Quantity"], bomGroupBy: ["Value", "Footprint"], bomFormat: "csv" },
  netlist: { netlistFormat: "kicad" },
  pdf: { pdfDpi: 300, pdfColorMode: "color", pdfShowGrid: false },
  png: {},
};

const typeLabels: Record<OutputJobType, string> = {
  bom: "Bill of Materials",
  netlist: "Netlist",
  pdf: "PDF Export",
  png: "PNG Export",
};

export function getJobTypeLabel(type: OutputJobType): string {
  return typeLabels[type] || type;
}

export const useOutputJobsStore = create<OutputJobsState>()(
  persist(
    (set) => ({
      jobs: [],

      addJob: (type) => {
        const job: OutputJob = {
          id: crypto.randomUUID(),
          name: typeLabels[type],
          type,
          enabled: true,
          config: { ...defaultConfig[type] },
        };
        set((s) => ({ jobs: [...s.jobs, job] }));
      },

      removeJob: (id) => {
        set((s) => ({ jobs: s.jobs.filter((j) => j.id !== id) }));
      },

      updateJob: (id, updates) => {
        set((s) => ({
          jobs: s.jobs.map((j) => (j.id === id ? { ...j, ...updates } : j)),
        }));
      },

      toggleJob: (id) => {
        set((s) => ({
          jobs: s.jobs.map((j) => (j.id === id ? { ...j, enabled: !j.enabled } : j)),
        }));
      },

      reorderJob: (id, direction) => {
        set((s) => {
          const idx = s.jobs.findIndex((j) => j.id === id);
          if (idx < 0) return s;
          const newIdx = direction === "up" ? idx - 1 : idx + 1;
          if (newIdx < 0 || newIdx >= s.jobs.length) return s;
          const jobs = [...s.jobs];
          [jobs[idx], jobs[newIdx]] = [jobs[newIdx], jobs[idx]];
          return { jobs };
        });
      },
    }),
    { name: "signex-output-jobs" }
  )
);
