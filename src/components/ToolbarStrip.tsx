import {
  MousePointer2,
  RotateCw,
  Trash2,
  ZoomIn,
  ZoomOut,
  Maximize,
  Grid3x3,
  Minus,
  Undo2,
  Redo2,
  CircleDot,
  Tag,
  Zap,
  XCircle,
  Type,
  Copy,
  FlipHorizontal,
  FlipVertical,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { useEditorStore } from "@/stores/editor";
import { useSchematicStore } from "@/stores/schematic";

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
  return <div className="w-px h-5 bg-border mx-1" />;
}

export function ToolbarStrip() {
  const appMode = useEditorStore((s) => s.mode);
  const editMode = useSchematicStore((s) => s.editMode);
  const setEditMode = useSchematicStore((s) => s.setEditMode);
  const undo = useSchematicStore((s) => s.undo);
  const redo = useSchematicStore((s) => s.redo);
  const deleteSelected = useSchematicStore((s) => s.deleteSelected);
  const rotateSelected = useSchematicStore((s) => s.rotateSelected);
  const mirrorSelectedX = useSchematicStore((s) => s.mirrorSelectedX);
  const mirrorSelectedY = useSchematicStore((s) => s.mirrorSelectedY);
  const duplicateSelected = useSchematicStore((s) => s.duplicateSelected);
  const selectedIds = useSchematicStore((s) => s.selectedIds);
  const hasSel = selectedIds.size > 0;

  return (
    <div className="flex items-center h-10 bg-bg-secondary border-b border-border-subtle px-2 gap-0.5">
      {/* Undo / Redo */}
      <ToolBtn icon={<Undo2 size={16} />} label="Undo (Ctrl+Z)" onClick={undo} />
      <ToolBtn icon={<Redo2 size={16} />} label="Redo (Ctrl+Y)" onClick={redo} />

      <Separator />

      {/* Selection / Edit Mode */}
      <ToolBtn icon={<MousePointer2 size={16} />} label="Select (Esc)"
        active={editMode === "select"} onClick={() => setEditMode("select")} />

      <Separator />

      {/* Wiring toolbar group */}
      <ToolBtn icon={<Minus size={16} />} label="Wire (W)"
        active={editMode === "drawWire"} onClick={() => setEditMode("drawWire")} />
      <ToolBtn icon={
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5">
          <line x1="4" y1="4" x2="4" y2="20" /><line x1="8" y1="4" x2="8" y2="20" />
        </svg>
      } label="Bus (P, B)" disabled={appMode !== "schematic"} />
      <ToolBtn icon={<Tag size={16} />} label="Net Label (L)"
        active={editMode === "placeLabel"} onClick={() => setEditMode("placeLabel")} />
      <ToolBtn icon={<Zap size={16} />} label="Power Port (P, O)"
        active={editMode === "placePower"} onClick={() => setEditMode("placePower")} />
      <ToolBtn icon={<XCircle size={14} />} label="No Connect (P, X)"
        active={editMode === "placeNoConnect"} onClick={() => setEditMode("placeNoConnect")} />

      <Separator />

      {/* Placement */}
      <ToolBtn icon={<CircleDot size={16} />} label="Place Part (P, P)"
        active={editMode === "placeSymbol"}
        onClick={() => window.dispatchEvent(new KeyboardEvent("keydown", { key: "p" }))}
        disabled={appMode !== "schematic"} />
      <ToolBtn icon={<Type size={16} />} label="Text String (P, T)" disabled />

      <Separator />

      {/* Transform actions */}
      <ToolBtn icon={<RotateCw size={16} />} label="Rotate (Space)" disabled={!hasSel} onClick={rotateSelected} />
      <ToolBtn icon={<FlipHorizontal size={16} />} label="Mirror X" disabled={!hasSel} onClick={mirrorSelectedX} />
      <ToolBtn icon={<FlipVertical size={16} />} label="Mirror Y" disabled={!hasSel} onClick={mirrorSelectedY} />
      <ToolBtn icon={<Copy size={16} />} label="Duplicate (Ctrl+D)" disabled={!hasSel} onClick={duplicateSelected} />
      <ToolBtn icon={<Trash2 size={16} />} label="Delete (Del)" disabled={!hasSel} onClick={deleteSelected} />

      <Separator />

      {/* View */}
      <ToolBtn icon={<ZoomIn size={16} />} label="Zoom In (PgUp)" />
      <ToolBtn icon={<ZoomOut size={16} />} label="Zoom Out (PgDn)" />
      <ToolBtn icon={<Maximize size={16} />} label="Fit Document (Home)"
        onClick={() => window.dispatchEvent(new KeyboardEvent("keydown", { key: "Home" }))} />
      <ToolBtn icon={<Grid3x3 size={16} />} label="Toggle Grid (G)"
        onClick={() => useEditorStore.getState().toggleGrid()} />

      <div className="flex-1" />

      {/* Mode indicator */}
      <div className="flex items-center gap-2 px-3 py-1 rounded bg-bg-surface text-[11px]">
        <div className={cn("w-2 h-2 rounded-full",
          editMode === "drawWire" ? "bg-warning" :
          editMode.startsWith("place") ? "bg-success" : "bg-accent")} />
        <span className="text-text-secondary capitalize">
          {editMode === "select" ? appMode :
           editMode === "drawWire" ? "Wire" :
           editMode === "placeSymbol" ? "Place Part" :
           editMode === "placeLabel" ? "Net Label" :
           editMode === "placePower" ? "Power Port" :
           editMode === "placeNoConnect" ? "No Connect" : editMode}
        </span>
      </div>
    </div>
  );
}
