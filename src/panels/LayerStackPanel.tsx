import { Eye, EyeOff } from "lucide-react";
import { usePcbStore } from "@/stores/pcb";
import { DEFAULT_LAYER_COLORS } from "@/types/pcb";
import type { PcbLayerId } from "@/types/pcb";
import { cn } from "@/lib/utils";

const COPPER_LAYERS: PcbLayerId[] = [
  "F.Cu", "In1.Cu", "In2.Cu", "In3.Cu", "In4.Cu", "In5.Cu", "In6.Cu",
  "In7.Cu", "In8.Cu", "In9.Cu", "In10.Cu", "In11.Cu", "In12.Cu",
  "In13.Cu", "In14.Cu", "In15.Cu", "In16.Cu", "In17.Cu", "In18.Cu",
  "In19.Cu", "In20.Cu", "In21.Cu", "In22.Cu", "In23.Cu", "In24.Cu",
  "In25.Cu", "In26.Cu", "In27.Cu", "In28.Cu", "In29.Cu", "In30.Cu", "B.Cu",
];

const TECH_LAYERS: { id: PcbLayerId; label: string }[] = [
  { id: "F.SilkS", label: "Front Silkscreen" },
  { id: "B.SilkS", label: "Back Silkscreen" },
  { id: "F.Mask", label: "Front Solder Mask" },
  { id: "B.Mask", label: "Back Solder Mask" },
  { id: "F.Paste", label: "Front Paste" },
  { id: "B.Paste", label: "Back Paste" },
  { id: "F.Fab", label: "Front Fabrication" },
  { id: "B.Fab", label: "Back Fabrication" },
  { id: "F.CrtYd", label: "Front Courtyard" },
  { id: "B.CrtYd", label: "Back Courtyard" },
  { id: "Edge.Cuts", label: "Board Outline" },
  { id: "Dwgs.User", label: "User Drawings" },
  { id: "Cmts.User", label: "User Comments" },
];

export function LayerStackPanel() {
  const data = usePcbStore((s) => s.data);
  const activeLayer = usePcbStore((s) => s.activeLayer);
  const visibleLayers = usePcbStore((s) => s.visibleLayers);
  const setActiveLayer = usePcbStore((s) => s.setActiveLayer);
  const toggleLayerVisibility = usePcbStore((s) => s.toggleLayerVisibility);

  // Determine how many copper layers are in use
  const copperCount = data?.board.layers.copperCount || 2;
  const activeCopperLayers = COPPER_LAYERS.filter((_, i) => {
    if (i === 0) return true; // F.Cu always
    if (i === COPPER_LAYERS.length - 1) return true; // B.Cu always
    return i < copperCount - 1; // Inner layers based on count
  });

  return (
    <div className="text-xs h-full flex flex-col">
      {/* Header */}
      <div className="flex items-center justify-between px-3 py-2 border-b border-border-subtle shrink-0">
        <span className="text-[11px] font-semibold text-text-secondary uppercase tracking-wider">Layers</span>
        <div className="flex gap-1">
          <button onClick={() => usePcbStore.getState().setAllLayersVisible()}
            className="text-[10px] text-accent hover:underline">All On</button>
        </div>
      </div>

      {/* Layer list */}
      <div className="flex-1 overflow-y-auto">
        {/* Copper Layers */}
        <div className="px-2 py-1 text-[9px] text-text-muted/50 uppercase tracking-wider">Copper</div>
        {activeCopperLayers.map((layer) => (
          <LayerRow
            key={layer}
            label={layer}
            color={DEFAULT_LAYER_COLORS[layer] || "#808080"}
            active={activeLayer === layer}
            visible={visibleLayers.has(layer)}
            onActivate={() => setActiveLayer(layer)}
            onToggleVisibility={() => toggleLayerVisibility(layer)}
          />
        ))}

        {/* Tech Layers */}
        <div className="px-2 py-1 mt-2 text-[9px] text-text-muted/50 uppercase tracking-wider">Technical</div>
        {TECH_LAYERS.map(({ id, label }) => (
          <LayerRow
            key={id}
            label={label}
            color={DEFAULT_LAYER_COLORS[id] || "#808080"}
            active={activeLayer === id}
            visible={visibleLayers.has(id)}
            onActivate={() => setActiveLayer(id)}
            onToggleVisibility={() => toggleLayerVisibility(id)}
          />
        ))}
      </div>

      {/* Footer: active layer info */}
      <div className="px-3 py-1.5 border-t border-border-subtle shrink-0">
        <div className="flex items-center gap-2">
          <div className="w-3 h-3 rounded-sm" style={{ backgroundColor: DEFAULT_LAYER_COLORS[activeLayer] || "#808080" }} />
          <span className="text-[11px] text-text-primary font-mono">{activeLayer}</span>
          <span className="text-[10px] text-text-muted/40 ml-auto">{copperCount}L stack</span>
        </div>
      </div>
    </div>
  );
}

function LayerRow({
  label, color, active, visible, onActivate, onToggleVisibility,
}: {
  label: string;
  color: string;
  active: boolean;
  visible: boolean;
  onActivate: () => void;
  onToggleVisibility: () => void;
}) {
  return (
    <div
      className={cn(
        "flex items-center gap-1.5 px-2 py-0.5 cursor-pointer transition-colors",
        active ? "bg-accent/15" : "hover:bg-bg-hover/50"
      )}
      onClick={onActivate}
    >
      <div className="w-2.5 h-2.5 rounded-sm shrink-0" style={{ backgroundColor: color, opacity: visible ? 1 : 0.2 }} />
      <span className={cn(
        "flex-1 text-[10px] truncate",
        active ? "text-accent font-semibold" : visible ? "text-text-secondary" : "text-text-muted/30"
      )}>
        {label}
      </span>
      <button
        onClick={(e) => { e.stopPropagation(); onToggleVisibility(); }}
        className="p-0.5 rounded text-text-muted/30 hover:text-text-secondary transition-colors"
      >
        {visible ? <Eye size={10} /> : <EyeOff size={10} />}
      </button>
    </div>
  );
}
