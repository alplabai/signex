import {
  MousePointer2, Minus, Circle, Square, Type, Ruler,
  Layers, FlipVertical, Palette, RotateCw,
  Undo2, Redo2, Trash2,
  AlignStartVertical, AlignEndVertical, AlignCenterVertical,
  AlignStartHorizontal, AlignEndHorizontal, AlignCenterHorizontal,
  ArrowLeftRight, ArrowUpDown, Droplets,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { usePcbStore } from "@/stores/pcb";
import { DEFAULT_LAYER_COLORS, LAYER_DISPLAY_NAMES } from "@/types/pcb";
import type { PcbEditMode } from "@/stores/pcb";
import { alignFootprints, distributeFootprints } from "@/lib/pcbPlacement";
import { generateTeardrops } from "@/lib/pcbRouter";

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

function Sep() {
  return <div className="w-px h-5 bg-border mx-1" />;
}

export function PcbToolbar() {
  const editMode = usePcbStore((s) => s.editMode);
  const activeLayer = usePcbStore((s) => s.activeLayer);
  const singleLayerMode = usePcbStore((s) => s.singleLayerMode);
  const boardFlipped = usePcbStore((s) => s.boardFlipped);
  const netColorEnabled = usePcbStore((s) => s.netColorEnabled);
  const store = usePcbStore;

  const setMode = (mode: PcbEditMode) => store.getState().setEditMode(mode);

  return (
    <div className="flex items-center gap-0.5 px-3 h-9 bg-bg-secondary border-b border-border shrink-0 select-none overflow-x-auto">
      {/* Mode label */}
      <span className="text-[10px] font-semibold text-accent uppercase tracking-wider mr-2 shrink-0">PCB</span>

      {/* Selection */}
      <ToolBtn icon={<MousePointer2 size={15} />} label="Select (Esc)" active={editMode === "select"} onClick={() => setMode("select")} />
      <Sep />

      {/* Routing */}
      <ToolBtn icon={<Minus size={15} />} label="Route Track (X)" active={editMode === "routeTrack"} onClick={() => setMode("routeTrack")} />
      <ToolBtn icon={<Circle size={15} />} label="Place Via" active={editMode === "placeVia"} onClick={() => setMode("placeVia")} />
      <select value={store.getState().routeMode}
        onChange={(e) => store.setState({ routeMode: e.target.value as "ignore" | "walkaround" | "push" | "hug_push" })}
        title="Route Mode"
        className="bg-transparent border border-border-subtle rounded px-1 py-0.5 text-[9px] text-text-secondary outline-none focus:border-accent max-w-[85px]">
        <option value="ignore">Ignore</option>
        <option value="walkaround">Walk</option>
        <option value="push">Push</option>
        <option value="hug_push">Hug</option>
      </select>
      <select value={store.getState().routeCornerMode}
        onChange={(e) => store.setState({ routeCornerMode: e.target.value as "45" | "90" | "arc45" | "arc90" | "any" })}
        title="Corner Style"
        className="bg-transparent border border-border-subtle rounded px-1 py-0.5 text-[9px] text-text-secondary outline-none focus:border-accent max-w-[65px]">
        <option value="45">45°</option>
        <option value="90">90°</option>
        <option value="arc45">Arc 45</option>
        <option value="arc90">Arc 90</option>
        <option value="any">Any</option>
      </select>
      <Sep />

      {/* Drawing */}
      <ToolBtn icon={<Square size={15} />} label="Board Outline" active={editMode === "drawBoardOutline"} onClick={() => setMode("drawBoardOutline")} />
      <ToolBtn icon={<Layers size={15} />} label="Place Zone" active={editMode === "placeZone"} onClick={() => setMode("placeZone")} />
      <ToolBtn icon={<Type size={15} />} label="Place Text" active={editMode === "placeText"} onClick={() => setMode("placeText")} />
      <ToolBtn icon={<Ruler size={15} />} label="Dimension" active={editMode === "placeDimension"} onClick={() => setMode("placeDimension")} />
      <Sep />

      {/* Display */}
      <ToolBtn icon={<Layers size={15} />} label={`Single Layer: ${singleLayerMode} (Shift+S)`}
        active={singleLayerMode !== "off"} onClick={() => store.getState().cycleSingleLayerMode()} />
      <ToolBtn icon={<FlipVertical size={15} />} label="Board Flip (Ctrl+F)" active={boardFlipped}
        onClick={() => store.getState().toggleBoardFlip()} />
      <ToolBtn icon={<Palette size={15} />} label="Net Colors (F5)" active={netColorEnabled}
        onClick={() => store.getState().toggleNetColors()} />
      <Sep />

      {/* Alignment */}
      <ToolBtn icon={<AlignStartVertical size={15} />} label="Align Left" onClick={() => alignFootprints("left")} />
      <ToolBtn icon={<AlignEndVertical size={15} />} label="Align Right" onClick={() => alignFootprints("right")} />
      <ToolBtn icon={<AlignCenterVertical size={15} />} label="Align Center H" onClick={() => alignFootprints("centerH")} />
      <ToolBtn icon={<AlignStartHorizontal size={15} />} label="Align Top" onClick={() => alignFootprints("top")} />
      <ToolBtn icon={<AlignEndHorizontal size={15} />} label="Align Bottom" onClick={() => alignFootprints("bottom")} />
      <ToolBtn icon={<AlignCenterHorizontal size={15} />} label="Align Center V" onClick={() => alignFootprints("centerV")} />
      <ToolBtn icon={<ArrowLeftRight size={15} />} label="Distribute Horizontally" onClick={() => distributeFootprints("horizontal")} />
      <ToolBtn icon={<ArrowUpDown size={15} />} label="Distribute Vertically" onClick={() => distributeFootprints("vertical")} />
      <Sep />

      {/* Teardrops */}
      <ToolBtn icon={<Droplets size={15} />} label="Generate Teardrops"
        onClick={() => {
          const s = store.getState();
          if (!s.data) return;
          s.pushUndo();
          const nd = structuredClone(s.data);
          const newSegs = generateTeardrops(nd, 0.5, 0.5);
          nd.segments = [...nd.segments, ...newSegs];
          usePcbStore.setState({ data: nd, dirty: true });
        }} />
      <Sep />

      {/* Actions */}
      <ToolBtn icon={<RotateCw size={15} />} label="Rotate (Space)"
        onClick={() => { for (const id of store.getState().selectedIds) store.getState().rotateFootprint(id, 90); }} />
      <ToolBtn icon={<Trash2 size={15} />} label="Delete (Del)" onClick={() => store.getState().deleteSelected()} />
      <ToolBtn icon={<Undo2 size={15} />} label="Undo (Ctrl+Z)" onClick={() => store.getState().undo()} />
      <ToolBtn icon={<Redo2 size={15} />} label="Redo (Ctrl+Y)" onClick={() => store.getState().redo()} />
      <Sep />

      {/* Active layer selector */}
      <div className="flex items-center gap-1.5 shrink-0">
        <div className="w-3 h-3 rounded-sm" style={{ backgroundColor: DEFAULT_LAYER_COLORS[activeLayer] || "#808080" }} />
        <select value={activeLayer}
          onChange={(e) => store.getState().setActiveLayer(e.target.value)}
          className="bg-transparent border border-border-subtle rounded px-1.5 py-0.5 text-[10px] text-text-primary outline-none focus:border-accent max-w-[120px]">
          {["F.Cu", "B.Cu", "In1.Cu", "In2.Cu", "In3.Cu", "In4.Cu"].map((l) => (
            <option key={l} value={l}>{LAYER_DISPLAY_NAMES[l] || l}</option>
          ))}
        </select>
      </div>
    </div>
  );
}
