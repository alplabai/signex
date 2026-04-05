import { useProjectStore } from "@/stores/project";
import { Zap, FolderOpen, Cpu, Layers } from "lucide-react";

interface EditorCanvasProps {
  onOpenProject?: () => void;
}

export function EditorCanvas({ onOpenProject }: EditorCanvasProps) {
  const project = useProjectStore((s) => s.project);
  const activeTabId = useProjectStore((s) => s.activeTabId);

  if (!project || !activeTabId) {
    return (
      <div className="flex flex-col items-center justify-center h-full bg-bg-primary relative overflow-hidden">
        {/* Background grid pattern */}
        <div
          className="absolute inset-0 opacity-[0.04]"
          style={{
            backgroundImage:
              "linear-gradient(var(--color-text-muted) 1px, transparent 1px), linear-gradient(90deg, var(--color-text-muted) 1px, transparent 1px)",
            backgroundSize: "40px 40px",
          }}
        />

        {/* Radial glow */}
        <div className="absolute inset-0 bg-radial-[circle_at_center] from-accent/5 via-transparent to-transparent" />

        {/* Content */}
        <div className="relative z-10 flex flex-col items-center gap-8">
          {/* Logo */}
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

          {/* Quick actions */}
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

          {/* Phase roadmap */}
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

  // Canvas placeholder — wgpu rendering replaces this in Week 3
  return (
    <div className="relative h-full bg-bg-primary">
      <div
        className="absolute inset-0"
        style={{
          backgroundImage:
            "radial-gradient(circle, var(--color-border) 1px, transparent 1px)",
          backgroundSize: "20px 20px",
          opacity: 0.2,
        }}
      />
      <div className="absolute inset-0 flex items-center justify-center text-text-muted text-sm">
        wgpu canvas — rendering engine coming in Phase 0, Week 3
      </div>
    </div>
  );
}
