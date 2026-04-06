import { useEditorStore } from "@/stores/editor";
import { Crosshair, Grid3x3, Magnet, Layers, MousePointer2, Zap } from "lucide-react";
import { cn } from "@/lib/utils";

// Unit conversion for display
function mmToDisplay(mm: number, unit: "mm" | "mil" | "inch"): string {
  if (unit === "mil") return (mm / 0.0254).toFixed(0);
  if (unit === "inch") return (mm / 25.4).toFixed(4);
  return mm.toFixed(2);
}

function StatusItem({
  icon,
  children,
  onClick,
  active,
  title,
}: {
  icon?: React.ReactNode;
  children: React.ReactNode;
  onClick?: () => void;
  active?: boolean;
  title?: string;
}) {
  return (
    <div
      onClick={onClick}
      title={title}
      className={cn(
        "flex items-center gap-1.5",
        onClick ? "cursor-pointer hover:text-text-primary transition-colors" : "",
        active ? "text-accent" : "text-text-muted",
      )}
    >
      {icon && <span className={active ? "text-accent" : "text-text-muted/60"}>{icon}</span>}
      <span>{children}</span>
    </div>
  );
}

function Divider() {
  return <div className="w-px h-3 bg-border-subtle" />;
}

export function StatusBar() {
  const cursorX = useEditorStore((s) => s.statusBar.cursorPosition.x);
  const cursorY = useEditorStore((s) => s.statusBar.cursorPosition.y);
  const gridSize = useEditorStore((s) => s.statusBar.gridSize);
  const gridVisible = useEditorStore((s) => s.gridVisible);
  const snapEnabled = useEditorStore((s) => s.statusBar.snapEnabled);
  const units = useEditorStore((s) => s.statusBar.units);
  const activeLayer = useEditorStore((s) => s.statusBar.activeLayer);
  const currentMode = useEditorStore((s) => s.statusBar.currentMode);
  const zoom = useEditorStore((s) => s.statusBar.zoom);
  const toggleGrid = useEditorStore((s) => s.toggleGrid);
  const toggleSnap = useEditorStore((s) => s.toggleSnap);
  const cycleUnits = () => {
    const u = units;
    useEditorStore.getState().updateStatusBar({ units: u === "mm" ? "mil" : u === "mil" ? "inch" : "mm" });
  };

  return (
    <div className="flex items-center h-[22px] bg-bg-tertiary border-t border-border-subtle px-3 text-[11px] gap-3 select-none">
      {/* Cursor position with unit conversion */}
      <StatusItem icon={<Crosshair size={11} />} title="Cursor position">
        <span className="font-mono tabular-nums">
          X:{mmToDisplay(cursorX, units)} Y:{mmToDisplay(cursorY, units)}
        </span>
      </StatusItem>
      <Divider />

      {/* Grid with unit conversion */}
      <StatusItem icon={<Grid3x3 size={11} />} onClick={toggleGrid} active={gridVisible}
        title="Grid size (G to cycle, Shift+Ctrl+G to toggle)">
        <span className="font-mono">{mmToDisplay(gridSize, units)}{units}</span>
      </StatusItem>
      <Divider />

      {/* Snap */}
      <StatusItem icon={<Magnet size={11} />} onClick={toggleSnap} active={snapEnabled}
        title="Snap to grid">
        {snapEnabled ? "Snap" : "Free"}
      </StatusItem>
      <Divider />

      {/* Electrical snap */}
      <StatusItem icon={<Zap size={11} />} active={true}
        title="Electrical grid snap (Shift+E)">
        E-Snap
      </StatusItem>
      <Divider />

      {/* Layer */}
      <StatusItem icon={<Layers size={11} />}>
        {activeLayer}
      </StatusItem>
      <Divider />

      {/* Mode */}
      <StatusItem icon={<MousePointer2 size={11} />}>
        {currentMode}
      </StatusItem>

      <div className="flex-1" />

      {/* Zoom */}
      <span className="text-text-muted/60 font-mono tabular-nums">
        {zoom}%
      </span>
      <Divider />

      {/* Units (clickable to cycle) */}
      <button onClick={cycleUnits} title="Toggle units (Ctrl+Q)"
        className="text-text-muted/60 uppercase font-mono text-[10px] hover:text-accent transition-colors px-1">
        {units}
      </button>
    </div>
  );
}
