import {
  MousePointer2, Square, Minus, Circle, Spline, Type,
  Trash2, Save, Undo2, Redo2, X, CircleDot,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { useFootprintEditorStore } from "@/stores/footprintEditor";
import type { FootprintData } from "@/stores/footprintEditor";
import { LAYER_DISPLAY_NAMES, DEFAULT_LAYER_COLORS } from "@/types/pcb";
import type { PcbGraphic } from "@/types/pcb";
import { invoke } from "@tauri-apps/api/core";

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
  const canUndo = useFootprintEditorStore(s => s.undoStack.length > 0);
  const canRedo = useFootprintEditorStore(s => s.redoStack.length > 0);
  const footprint = useFootprintEditorStore(s => s.footprint);
  const activeLayer = useFootprintEditorStore(s => s.activeLayer);
  const setActiveLayer = useFootprintEditorStore(s => s.setActiveLayer);

  const handleDelete = () => {
    const s = useFootprintEditorStore.getState();
    if (s.selectedItem?.type === "pad") s.removePad(s.selectedItem.index);
    else if (s.selectedItem?.type === "graphic") s.removeGraphic(s.selectedItem.index);
  };

  const handleSave = async () => {
    const store = useFootprintEditorStore.getState();
    if (!store.footprint || !store.sourcePath) return;
    try {
      const rustFp = toRustFootprint(store.footprint);
      await invoke("save_footprint", { filePath: store.sourcePath, footprint: rustFp });
      useFootprintEditorStore.setState({ dirty: false });
    } catch (err) {
      alert(`Failed to save footprint: ${err}`);
    }
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
        disabled={!canUndo} onClick={() => useFootprintEditorStore.getState().undo()} />
      <ToolBtn icon={<Redo2 size={15} />} label="Redo (Ctrl+Y)"
        disabled={!canRedo} onClick={() => useFootprintEditorStore.getState().redo()} />

      <Sep />

      <ToolBtn icon={<Save size={15} />} label="Save Footprint"
        disabled={!dirty} onClick={handleSave} />

      <div className="flex-1" />

      <ToolBtn icon={<X size={15} />} label="Close Footprint Editor"
        onClick={() => useFootprintEditorStore.getState().closeEditor()} />
    </div>
  );
}

/** Convert frontend FootprintData to the shape Rust's PcbFootprint expects (snake_case keys) */
function toRustFootprint(fp: FootprintData) {
  return {
    uuid: crypto.randomUUID(),
    reference: "REF**",
    value: fp.id,
    footprint_id: fp.id,
    position: { x: 0, y: 0 },
    rotation: 0,
    layer: "F.Cu",
    locked: false,
    pads: fp.pads.map(p => ({
      uuid: p.uuid,
      number: p.number,
      pad_type: p.type,
      shape: p.shape,
      position: p.position,
      size: p.size,
      drill: p.drill ? { diameter: p.drill.diameter, shape: p.drill.shape ?? null } : null,
      layers: p.layers,
      net: null,
      roundrect_ratio: p.roundrectRatio ?? null,
    })),
    graphics: fp.graphics.map(toRustGraphic),
  };
}

function toRustGraphic(g: PcbGraphic) {
  return {
    graphic_type: g.type,
    layer: g.layer,
    width: "width" in g ? g.width : 0.12,
    start: "start" in g ? g.start : null,
    end: "end" in g ? g.end : null,
    center: "center" in g ? g.center : null,
    mid: "mid" in g ? g.mid : null,
    radius: "radius" in g ? g.radius : null,
    points: "points" in g ? g.points : [],
    text: g.type === "text" ? g.text : null,
    font_size: g.type === "text" ? g.fontSize : null,
    position: g.type === "text" ? g.position : null,
    rotation: g.type === "text" ? g.rotation : null,
    fill: "fill" in g ? g.fill : null,
  };
}
