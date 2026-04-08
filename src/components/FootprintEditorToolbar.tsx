import {
  MousePointer2, Square, Minus, Circle, Spline, Type,
  Trash2, Save, Undo2, Redo2, X, CircleDot,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { useFootprintEditorStore } from "@/stores/footprintEditor";
import { LAYER_DISPLAY_NAMES, DEFAULT_LAYER_COLORS } from "@/types/pcb";

function ToolBtn({ icon, label, active, disabled, onClick }: {
  icon: React.ReactNode; label: string; active?: boolean; disabled?: boolean; onClick?: () => void;
}) {
  return (
    <button title={label} disabled={disabled} onClick={onClick}
      className={cn("p-1.5 rounded transition-colors",
        active ? "bg-accent/20 text-accent"
          : disabled ? "text-text-muted/30 cursor-default"
          : "text-text-secondary hover:bg-bg-hover hover:text-text-primary")}>
      {icon}
    </button>
  );
}

function Sep() { return <div className="w-px h-5 bg-border mx-1" />; }

const LAYER_IDS = ["F.Cu", "B.Cu", "F.SilkS", "B.SilkS", "F.Fab", "B.Fab", "F.CrtYd", "B.CrtYd", "F.Paste", "B.Paste", "F.Mask", "B.Mask"];
const LAYERS = LAYER_IDS.map(id => ({
  id,
  label: LAYER_DISPLAY_NAMES[id] || id,
  color: DEFAULT_LAYER_COLORS[id] || "#666",
}));

export function FootprintEditorToolbar() {
  const editMode = useFootprintEditorStore(s => s.editMode);
  const setEditMode = useFootprintEditorStore(s => s.setEditMode);
  const selectedItem = useFootprintEditorStore(s => s.selectedItem);
  const dirty = useFootprintEditorStore(s => s.dirty);
  const undoStack = useFootprintEditorStore(s => s.undoStack);
  const redoStack = useFootprintEditorStore(s => s.redoStack);
  const footprint = useFootprintEditorStore(s => s.footprint);
  const activeLayer = useFootprintEditorStore(s => s.activeLayer);
  const setActiveLayer = useFootprintEditorStore(s => s.setActiveLayer);

  const handleDelete = () => {
    const s = useFootprintEditorStore.getState();
    if (s.selectedItem?.type === "pad") s.removePad(s.selectedItem.index);
    else if (s.selectedItem?.type === "graphic") s.removeGraphic(s.selectedItem.index);
  };

  const handleSave = async () => {
    const s = useFootprintEditorStore.getState();
    if (!s.footprint || !s.sourcePath || !s.sourceId) return;
    // TODO: Tauri save_footprint command
    alert("Save footprint: " + s.sourceId + " → " + s.sourcePath + "\n(Backend save not yet implemented)");
  };

  return (
    <div className="flex items-center gap-0.5 px-3 h-9 bg-bg-secondary border-b border-border shrink-0 select-none">
      <div className="flex items-center gap-0.5 mr-2">
        <span className="text-[10px] font-semibold text-success uppercase tracking-wider mr-1">
          Footprint Editor
        </span>
        {footprint && (
          <span className="text-[10px] text-text-muted font-mono truncate max-w-[200px]">
            {footprint.id}
          </span>
        )}
        {dirty && <span className="text-[10px] text-warning ml-1">*</span>}
      </div>

      <Sep />

      <ToolBtn icon={<MousePointer2 size={15} />} label="Select (Esc)"
        active={editMode === "select"} onClick={() => setEditMode("select")} />

      <Sep />

      {/* Pads */}
      <ToolBtn icon={<Square size={15} />} label="SMD Pad"
        active={editMode === "addPadSmd"} onClick={() => setEditMode("addPadSmd")} />
      <ToolBtn icon={<CircleDot size={15} />} label="Through-Hole Pad"
        active={editMode === "addPadTh"} onClick={() => setEditMode("addPadTh")} />

      <Sep />

      {/* Graphics */}
      <ToolBtn icon={<Minus size={15} />} label="Line"
        active={editMode === "addLine"} onClick={() => setEditMode("addLine")} />
      <ToolBtn icon={<Square size={15} />} label="Rectangle"
        active={editMode === "addRect"} onClick={() => setEditMode("addRect")} />
      <ToolBtn icon={<Circle size={15} />} label="Circle"
        active={editMode === "addCircle"} onClick={() => setEditMode("addCircle")} />
      <ToolBtn icon={<Spline size={15} />} label="Arc"
        active={editMode === "addArc"} onClick={() => setEditMode("addArc")} />
      <ToolBtn icon={<Type size={15} />} label="Text"
        active={editMode === "addText"} onClick={() => setEditMode("addText")} />

      <Sep />

      {/* Active layer selector */}
      <select
        value={activeLayer}
        onChange={e => setActiveLayer(e.target.value as any)}
        className="bg-bg-secondary border border-border-subtle rounded px-1.5 py-0.5 text-[10px] text-text-secondary outline-none"
      >
        {LAYERS.map(l => (
          <option key={l.id} value={l.id}>{l.label}</option>
        ))}
      </select>

      <Sep />

      <ToolBtn icon={<Trash2 size={15} />} label="Delete (Del)"
        disabled={!selectedItem} onClick={handleDelete} />
      <ToolBtn icon={<Undo2 size={15} />} label="Undo (Ctrl+Z)"
        disabled={undoStack.length === 0} onClick={() => useFootprintEditorStore.getState().undo()} />
      <ToolBtn icon={<Redo2 size={15} />} label="Redo (Ctrl+Y)"
        disabled={redoStack.length === 0} onClick={() => useFootprintEditorStore.getState().redo()} />

      <Sep />

      <ToolBtn icon={<Save size={15} />} label="Save Footprint"
        disabled={!dirty} onClick={handleSave} />

      <div className="flex-1" />

      <ToolBtn icon={<X size={15} />} label="Close Footprint Editor"
        onClick={() => useFootprintEditorStore.getState().closeEditor()} />
    </div>
  );
}
