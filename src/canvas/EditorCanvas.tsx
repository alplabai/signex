import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useProjectStore } from "@/stores/project";
import { useEditorStore } from "@/stores/editor";
import { SchematicRenderer } from "./SchematicRenderer";
import { Zap, FolderOpen, Cpu, Layers, Loader2 } from "lucide-react";
import type { SchematicData } from "@/types";

interface EditorCanvasProps {
  onOpenProject?: () => void;
}

export function EditorCanvas({ onOpenProject }: EditorCanvasProps) {
  const project = useProjectStore((s) => s.project);
  const activeTabId = useProjectStore((s) => s.activeTabId);
  const activeTab = useProjectStore((s) =>
    s.openTabs.find((t) => t.id === s.activeTabId)
  );
  const [schematic, setSchematic] = useState<SchematicData | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const setMode = useEditorStore((s) => s.setMode);

  // Load schematic when active tab changes
  useEffect(() => {
    if (!project || !activeTab) {
      setSchematic(null);
      return;
    }

    if (activeTab.type !== "schematic") return;

    // Find the sheet filename from the tab
    const sheet = project.sheets.find(
      (s) => `sch-${project.path}:${s.filename}` === activeTabId
    );
    // Fallback: if tab was opened with project path, use root schematic
    const filename = sheet?.filename || project.schematic_root;
    if (!filename) return;

    setLoading(true);
    setError(null);
    setMode("schematic");

    invoke<SchematicData>("get_schematic", {
      projectDir: project.dir,
      filename,
    })
      .then((data) => {
        setSchematic(data);
      })
      .catch((err) => {
        setError(String(err));
        setSchematic(null);
      })
      .finally(() => setLoading(false));
  }, [project, activeTab, activeTabId, setMode]);

  // No project — welcome screen
  if (!project || !activeTabId) {
    return (
      <div className="flex flex-col items-center justify-center h-full bg-bg-primary relative overflow-hidden">
        <div
          className="absolute inset-0 opacity-[0.04]"
          style={{
            backgroundImage:
              "linear-gradient(var(--color-text-muted) 1px, transparent 1px), linear-gradient(90deg, var(--color-text-muted) 1px, transparent 1px)",
            backgroundSize: "40px 40px",
          }}
        />
        <div className="absolute inset-0 bg-radial-[circle_at_center] from-accent/5 via-transparent to-transparent" />
        <div className="relative z-10 flex flex-col items-center gap-8">
          <div className="flex items-center gap-4">
            <div className="p-3 rounded-2xl bg-accent/10 border border-accent/20">
              <Zap size={36} className="text-accent" />
            </div>
            <div>
              <h1 className="text-3xl font-bold text-text-primary tracking-tight">
                Alp EDA
              </h1>
              <p className="text-sm text-text-secondary mt-0.5">
                AI-First Electronic Design Automation
              </p>
            </div>
          </div>
          <div className="flex flex-col items-center gap-2 mt-2">
            <button
              onClick={onOpenProject}
              className="flex items-center gap-2.5 px-5 py-2.5 rounded-lg bg-accent/10 border border-accent/20 text-accent hover:bg-accent/20 transition-colors text-sm font-medium cursor-pointer"
            >
              <FolderOpen size={16} />
              Open Project
              <span className="text-accent/50 text-xs ml-1">Ctrl+O</span>
            </button>
            <span className="text-text-muted/40 text-xs mt-1">
              or press Ctrl+K for Command Palette
            </span>
          </div>
          <div className="flex gap-3 mt-6">
            {[
              { phase: "0", label: "Viewer", icon: <Zap size={16} />, active: true },
              { phase: "1", label: "Schematic", icon: <Layers size={16} />, active: false },
              { phase: "2", label: "PCB Layout", icon: <Cpu size={16} />, active: false },
            ].map((p) => (
              <div
                key={p.phase}
                className={`flex flex-col items-center gap-1.5 px-5 py-3 rounded-xl border transition-colors ${
                  p.active
                    ? "bg-accent/10 border-accent/30 text-accent"
                    : "bg-bg-surface/30 border-border-subtle text-text-muted/40"
                }`}
              >
                {p.icon}
                <span className="text-[10px] font-bold uppercase tracking-wider">
                  Phase {p.phase}
                </span>
                <span className="text-[11px]">{p.label}</span>
              </div>
            ))}
          </div>
        </div>
      </div>
    );
  }

  if (loading) {
    return (
      <div className="flex items-center justify-center h-full bg-bg-primary">
        <Loader2 size={24} className="text-accent animate-spin" />
        <span className="ml-3 text-text-secondary text-sm">Loading schematic...</span>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex items-center justify-center h-full bg-bg-primary">
        <span className="text-error text-sm">{error}</span>
      </div>
    );
  }

  if (schematic) {
    return <SchematicRenderer data={schematic} />;
  }

  return (
    <div className="flex items-center justify-center h-full bg-bg-primary text-text-muted text-sm">
      Select a sheet to view
    </div>
  );
}
