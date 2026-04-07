import {
  MousePointer2,
  Pin,
  Square,
  Minus,
  Circle,
  Spline,
  Trash2,
  Save,
  Undo2,
  Redo2,
  X,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { useLibraryEditorStore } from "@/stores/libraryEditor";
import type { LibEditMode } from "@/stores/libraryEditor";

interface ToolButton {
  icon: React.ReactNode;
  label: string;
  mode?: LibEditMode;
  active?: boolean;
  disabled?: boolean;
  onClick?: () => void;
}

function ToolBtn({ icon, label, active, disabled, onClick }: ToolButton) {
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

function Separator() {
  return <div className="w-px h-5 bg-border mx-1" />;
}

export function LibraryEditorToolbar() {
  const editMode = useLibraryEditorStore((s) => s.editMode);
  const setEditMode = useLibraryEditorStore((s) => s.setEditMode);
  const selectedItem = useLibraryEditorStore((s) => s.selectedItem);
  const dirty = useLibraryEditorStore((s) => s.dirty);
  const undoStack = useLibraryEditorStore((s) => s.undoStack);
  const redoStack = useLibraryEditorStore((s) => s.redoStack);
  const symbol = useLibraryEditorStore((s) => s.symbol);

  const handleDelete = () => {
    const store = useLibraryEditorStore.getState();
    if (store.selectedItem?.type === "pin") store.removePin(store.selectedItem.index);
    else if (store.selectedItem?.type === "graphic") store.removeGraphic(store.selectedItem.index);
  };

  const handleSave = async () => {
    const store = useLibraryEditorStore.getState();
    if (!store.symbol || !store.sourcePath || !store.sourceLibId) return;
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("save_symbol", {
        libraryPath: store.sourcePath,
        libId: store.sourceLibId,
        symbol: store.symbol,
      });
      useLibraryEditorStore.setState({ dirty: false });
    } catch (e) {
      console.error("Save symbol failed:", e);
      alert("Save failed: " + (e instanceof Error ? e.message : String(e)));
    }
  };

  return (
    <div className="flex items-center gap-0.5 px-3 h-9 bg-bg-secondary border-b border-border shrink-0 select-none">
      <div className="flex items-center gap-0.5 mr-2">
        <span className="text-[10px] font-semibold text-accent uppercase tracking-wider mr-1">
          Library Editor
        </span>
        {symbol && (
          <span className="text-[10px] text-text-muted font-mono truncate max-w-[200px]">
            {symbol.id}
          </span>
        )}
        {dirty && <span className="text-[10px] text-warning ml-1">*</span>}
      </div>

      <Separator />

      <ToolBtn icon={<MousePointer2 size={15} />} label="Select (Esc)"
        active={editMode === "select"} onClick={() => setEditMode("select")} />
      <ToolBtn icon={<Pin size={15} />} label="Add Pin"
        active={editMode === "addPin"} onClick={() => setEditMode("addPin")} />

      <Separator />

      <ToolBtn icon={<Square size={15} />} label="Add Rectangle"
        active={editMode === "addRect"} onClick={() => setEditMode("addRect")} />
      <ToolBtn icon={<Minus size={15} />} label="Add Polyline"
        active={editMode === "addPolyline"} onClick={() => setEditMode("addPolyline")} />
      <ToolBtn icon={<Circle size={15} />} label="Add Circle"
        active={editMode === "addCircle"} onClick={() => setEditMode("addCircle")} />
      <ToolBtn icon={<Spline size={15} />} label="Add Arc"
        active={editMode === "addArc"} onClick={() => setEditMode("addArc")} />

      <Separator />

      <ToolBtn icon={<Trash2 size={15} />} label="Delete Selected (Del)"
        disabled={!selectedItem} onClick={handleDelete} />
      <ToolBtn icon={<Undo2 size={15} />} label="Undo (Ctrl+Z)"
        disabled={undoStack.length === 0} onClick={() => useLibraryEditorStore.getState().undo()} />
      <ToolBtn icon={<Redo2 size={15} />} label="Redo (Ctrl+Y)"
        disabled={redoStack.length === 0} onClick={() => useLibraryEditorStore.getState().redo()} />

      <Separator />

      <ToolBtn icon={<Save size={15} />} label="Save Symbol"
        disabled={!dirty} onClick={handleSave} />

      <div className="flex-1" />

      <ToolBtn icon={<X size={15} />} label="Close Library Editor"
        onClick={() => useLibraryEditorStore.getState().closeEditor()} />
    </div>
  );
}
