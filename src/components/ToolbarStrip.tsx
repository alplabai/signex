import {
  MousePointer2,
  Move,
  RotateCw,
  Copy,
  Trash2,
  ZoomIn,
  ZoomOut,
  Maximize,
  Grid3x3,
  Minus,
  Undo2,
  Redo2,
  CircleDot,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { useEditorStore } from "@/stores/editor";

interface ToolButton {
  icon: React.ReactNode;
  label: string;
  active?: boolean;
  disabled?: boolean;
  onClick?: () => void;
}

function ToolBtn({ icon, label, active, disabled, onClick }: ToolButton) {
  return (
    <button
      title={label}
      disabled={disabled}
      onClick={onClick}
      className={cn(
        "p-1.5 rounded transition-colors",
        active
          ? "bg-accent/20 text-accent"
          : disabled
            ? "text-text-muted/30 cursor-default"
            : "text-text-secondary hover:bg-bg-hover hover:text-text-primary"
      )}
    >
      {icon}
    </button>
  );
}

function Separator() {
  return <div className="w-px h-5 bg-border mx-1.5" />;
}

export function ToolbarStrip() {
  const mode = useEditorStore((s) => s.mode);

  return (
    <div className="flex items-center h-10 bg-bg-secondary border-b border-border-subtle px-2 gap-0.5">
      {/* Undo / Redo */}
      <ToolBtn icon={<Undo2 size={17} />} label="Undo (Ctrl+Z)" disabled />
      <ToolBtn icon={<Redo2 size={17} />} label="Redo (Ctrl+Y)" disabled />

      <Separator />

      {/* Selection & Editing */}
      <ToolBtn icon={<MousePointer2 size={17} />} label="Select (Esc)" active />
      <ToolBtn icon={<Move size={17} />} label="Move (M)" disabled />
      <ToolBtn icon={<RotateCw size={17} />} label="Rotate (R)" disabled />
      <ToolBtn icon={<Copy size={17} />} label="Copy (Ctrl+C)" disabled />
      <ToolBtn icon={<Trash2 size={17} />} label="Delete (Del)" disabled />

      <Separator />

      {/* Schematic-specific */}
      {mode === "schematic" && (
        <>
          <ToolBtn icon={<Minus size={17} />} label="Wire (P, W)" disabled />
          <ToolBtn icon={<CircleDot size={17} />} label="Component (P, C)" disabled />
          <Separator />
        </>
      )}

      {/* View */}
      <ToolBtn icon={<ZoomIn size={17} />} label="Zoom In (Ctrl+=)" />
      <ToolBtn icon={<ZoomOut size={17} />} label="Zoom Out (Ctrl+-)" />
      <ToolBtn icon={<Maximize size={17} />} label="Fit View (Home)" />
      <ToolBtn icon={<Grid3x3 size={17} />} label="Toggle Grid (G)" />

      <div className="flex-1" />

      {/* Mode indicator */}
      <div className="flex items-center gap-2 px-3 py-1 rounded bg-bg-surface text-[11px]">
        <div className="w-2 h-2 rounded-full bg-accent" />
        <span className="text-text-secondary capitalize">{mode}</span>
      </div>
    </div>
  );
}
