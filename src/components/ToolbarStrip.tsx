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
} from "lucide-react";
import { cn } from "@/lib/utils";
import { useEditorStore } from "@/stores/editor";

interface ToolButton {
  icon: React.ReactNode;
  label: string;
  shortcut?: string;
  active?: boolean;
  disabled?: boolean;
  onClick?: () => void;
}

interface ToolGroup {
  label: string;
  tools: ToolButton[];
}

function ToolBtn({ icon, label, active, disabled, onClick }: ToolButton) {
  return (
    <button
      title={label}
      disabled={disabled}
      onClick={onClick}
      className={cn(
        "p-1.5 rounded hover:bg-bg-hover transition-colors",
        active && "bg-accent/20 text-accent",
        disabled && "opacity-30 cursor-default"
      )}
    >
      {icon}
    </button>
  );
}

function Separator() {
  return <div className="w-px h-5 bg-border mx-1" />;
}

export function ToolbarStrip() {
  const mode = useEditorStore((s) => s.mode);

  const editTools: ToolGroup = {
    label: "Edit",
    tools: [
      { icon: <Undo2 size={16} />, label: "Undo (Ctrl+Z)", disabled: true },
      { icon: <Redo2 size={16} />, label: "Redo (Ctrl+Y)", disabled: true },
    ],
  };

  const selectTools: ToolGroup = {
    label: "Select",
    tools: [
      { icon: <MousePointer2 size={16} />, label: "Select (Esc)", active: true },
      { icon: <Move size={16} />, label: "Move (M)", disabled: true },
      { icon: <RotateCw size={16} />, label: "Rotate (R)", disabled: true },
      { icon: <Copy size={16} />, label: "Copy (Ctrl+C)", disabled: true },
      { icon: <Trash2 size={16} />, label: "Delete (Del)", disabled: true },
    ],
  };

  const viewTools: ToolGroup = {
    label: "View",
    tools: [
      { icon: <ZoomIn size={16} />, label: "Zoom In (Ctrl++)" },
      { icon: <ZoomOut size={16} />, label: "Zoom Out (Ctrl+-)" },
      { icon: <Maximize size={16} />, label: "Fit View (Home)" },
      { icon: <Grid3x3 size={16} />, label: "Toggle Grid (G)" },
    ],
  };

  const schematicTools: ToolGroup = {
    label: "Place",
    tools: [
      { icon: <Minus size={16} />, label: "Wire (P,W)", disabled: true },
    ],
  };

  const groups =
    mode === "schematic"
      ? [editTools, selectTools, schematicTools, viewTools]
      : [editTools, selectTools, viewTools];

  return (
    <div className="flex items-center h-9 bg-bg-secondary border-b border-border px-2 gap-0.5">
      {groups.map((group, gIdx) => (
        <div key={group.label} className="flex items-center">
          {gIdx > 0 && <Separator />}
          {group.tools.map((tool, tIdx) => (
            <ToolBtn key={tIdx} {...tool} />
          ))}
        </div>
      ))}
      <div className="flex-1" />
      <span className="text-[11px] text-text-muted capitalize">{mode} Editor</span>
    </div>
  );
}
