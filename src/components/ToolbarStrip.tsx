import { Undo2, Redo2 } from "lucide-react";
import { cn } from "@/lib/utils";
import { useEditorStore } from "@/stores/editor";
import { useSchematicStore } from "@/stores/schematic";

export function ToolbarStrip() {
  const appMode = useEditorStore((s) => s.mode);
  const editMode = useSchematicStore((s) => s.editMode);
  const undo = useSchematicStore((s) => s.undo);
  const redo = useSchematicStore((s) => s.redo);

  return (
    <div className="flex items-center h-7 bg-bg-secondary border-b border-border-subtle px-2 gap-0.5">
      <button title="Undo (Ctrl+Z)" onClick={undo}
        className="p-1 rounded transition-colors text-text-secondary hover:bg-bg-hover hover:text-text-primary">
        <Undo2 size={14} />
      </button>
      <button title="Redo (Ctrl+Y)" onClick={redo}
        className="p-1 rounded transition-colors text-text-secondary hover:bg-bg-hover hover:text-text-primary">
        <Redo2 size={14} />
      </button>

      <div className="flex-1" />

      {/* Mode indicator */}
      <div className="flex items-center gap-2 px-2 py-0.5 rounded bg-bg-surface text-[10px]">
        <div className={cn("w-1.5 h-1.5 rounded-full",
          editMode === "drawWire" ? "bg-warning" :
          editMode.startsWith("place") ? "bg-success" : "bg-accent")} />
        <span className="text-text-secondary capitalize">
          {editMode === "select" ? appMode :
           editMode === "drawWire" ? "Wire" :
           editMode === "drawBus" ? "Bus" :
           editMode === "placeSymbol" ? "Place Part" :
           editMode === "placeLabel" ? "Net Label" :
           editMode === "placePower" ? "Power Port" :
           editMode === "placeNoConnect" ? "No Connect" :
           editMode === "placeText" ? "Text" :
           editMode === "drawLine" ? "Line" :
           editMode === "drawRect" ? "Rectangle" :
           editMode === "drawCircle" ? "Circle" :
           editMode === "drawPolyline" ? "Polyline" : editMode}
        </span>
      </div>
    </div>
  );
}
