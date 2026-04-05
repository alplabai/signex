import { useProjectStore } from "@/stores/project";
import { Zap, FolderOpen } from "lucide-react";

export function EditorCanvas() {
  const project = useProjectStore((s) => s.project);
  const activeTabId = useProjectStore((s) => s.activeTabId);

  if (!project || !activeTabId) {
    return (
      <div className="flex flex-col items-center justify-center h-full bg-bg-primary gap-6">
        <div className="flex items-center gap-3">
          <Zap size={48} className="text-accent opacity-60" />
          <div>
            <h1 className="text-2xl font-bold text-text-primary">Alp EDA</h1>
            <p className="text-sm text-text-secondary">
              AI-First Electronic Design Automation
            </p>
          </div>
        </div>
        <div className="flex flex-col items-center gap-3 text-sm text-text-muted">
          <div className="flex items-center gap-2">
            <FolderOpen size={16} />
            <span>Open Project (Ctrl+O)</span>
          </div>
          <span className="text-text-muted/50">
            or use Command Palette (Ctrl+K)
          </span>
        </div>
        <div className="mt-8 grid grid-cols-3 gap-4 text-xs text-text-muted/60">
          <div className="flex flex-col items-center gap-1 p-3 rounded-lg bg-bg-surface/50">
            <span className="text-accent">Phase 0</span>
            <span>Viewer</span>
          </div>
          <div className="flex flex-col items-center gap-1 p-3 rounded-lg bg-bg-surface/50">
            <span className="text-text-muted/40">Phase 1</span>
            <span>Schematic</span>
          </div>
          <div className="flex flex-col items-center gap-1 p-3 rounded-lg bg-bg-surface/50">
            <span className="text-text-muted/40">Phase 2</span>
            <span>PCB Layout</span>
          </div>
        </div>
      </div>
    );
  }

  // Canvas placeholder — wgpu rendering will replace this in Week 3
  return (
    <div className="relative h-full bg-bg-primary">
      <div
        className="absolute inset-0"
        style={{
          backgroundImage:
            "radial-gradient(circle, var(--color-border) 1px, transparent 1px)",
          backgroundSize: "20px 20px",
          opacity: 0.3,
        }}
      />
      <div className="absolute inset-0 flex items-center justify-center text-text-muted text-sm">
        wgpu canvas — rendering engine coming in Phase 0, Week 3
      </div>
    </div>
  );
}
