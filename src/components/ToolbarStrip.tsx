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
  return <div className="w-px h-5 bg-border mx-1.5" />;
}

export function ToolbarStrip() {
  const appMode = useEditorStore((s) => s.mode);
  const editMode = useSchematicStore((s) => s.editMode);
  const setEditMode = useSchematicStore((s) => s.setEditMode);
  const undo = useSchematicStore((s) => s.undo);
  const redo = useSchematicStore((s) => s.redo);
  const deleteSelected = useSchematicStore((s) => s.deleteSelected);
  const rotateSelected = useSchematicStore((s) => s.rotateSelected);
  const selectedIds = useSchematicStore((s) => s.selectedIds);

  return (
    <div className="flex items-center h-10 bg-bg-secondary border-b border-border-subtle px-2 gap-0.5">
      {/* Undo / Redo */}
      <ToolBtn icon={<Undo2 size={17} />} label="Undo (Ctrl+Z)" onClick={undo} />
      <ToolBtn icon={<Redo2 size={17} />} label="Redo (Ctrl+Y)" onClick={redo} />

      <Separator />

      {/* Edit Mode */}
      <ToolBtn
        icon={<MousePointer2 size={17} />}
        label="Select (Esc)"
        active={editMode === "select"}
        onClick={() => setEditMode("select")}
      />
      <ToolBtn
        icon={<Minus size={17} />}
        label="Wire (W)"
        active={editMode === "drawWire"}
        onClick={() => setEditMode("drawWire")}
      />
      <ToolBtn
        icon={<CircleDot size={17} />}
        label="Component (P, C)"
        active={editMode === "placeSymbol"}
        onClick={() => setEditMode("placeSymbol")}
        disabled={appMode !== "schematic"}
      />

      <Separator />

      {/* Selection actions */}
      <ToolBtn
        icon={<RotateCw size={17} />}
        label="Rotate (R)"
        disabled={selectedIds.size === 0}
        onClick={rotateSelected}
      />
      <ToolBtn
        icon={<Trash2 size={17} />}
        label="Delete (Del)"
        disabled={selectedIds.size === 0}
        onClick={deleteSelected}
      />

      <Separator />

      {/* View */}
      <ToolBtn icon={<ZoomIn size={17} />} label="Zoom In" />
      <ToolBtn icon={<ZoomOut size={17} />} label="Zoom Out" />
      <ToolBtn icon={<Maximize size={17} />} label="Fit View (Home)" />
      <ToolBtn icon={<Grid3x3 size={17} />} label="Toggle Grid (G)" />

      <div className="flex-1" />

      {/* Mode indicator */}
      <div className="flex items-center gap-2 px-3 py-1 rounded bg-bg-surface text-[11px]">
        <div className={cn("w-2 h-2 rounded-full", editMode === "drawWire" ? "bg-warning" : "bg-accent")} />
        <span className="text-text-secondary capitalize">
          {editMode === "select" ? appMode : editMode === "drawWire" ? "Wire" : editMode}
        </span>
      </div>
    </div>
  );
}
