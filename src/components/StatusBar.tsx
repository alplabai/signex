import { useEditorStore } from "@/stores/editor";
import { Crosshair, Grid3x3, Magnet, Layers, MousePointer2 } from "lucide-react";
import { cn } from "@/lib/utils";

function StatusItem({
  icon,
  children,
  onClick,
  active,
}: {
  icon?: React.ReactNode;
  children: React.ReactNode;
  onClick?: () => void;
  active?: boolean;
}) {
  return (
    <div
      onClick={onClick}
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

  return (
    <div className="flex items-center h-[22px] bg-bg-tertiary border-t border-border-subtle px-3 text-[11px] gap-3 select-none">
      <StatusItem icon={<Crosshair size={11} />}>
        {cursorX.toFixed(2)}, {cursorY.toFixed(2)}
      </StatusItem>
      <Divider />
      <StatusItem icon={<Grid3x3 size={11} />} onClick={toggleGrid} active={gridVisible}>
        {gridSize} {units}
      </StatusItem>
      <Divider />
      <StatusItem icon={<Magnet size={11} />} onClick={toggleSnap} active={snapEnabled}>
        {snapEnabled ? "Snap" : "Free"}
      </StatusItem>
      <Divider />
      <StatusItem icon={<Layers size={11} />}>
        {activeLayer}
      </StatusItem>
      <Divider />
      <StatusItem icon={<MousePointer2 size={11} />}>
        {currentMode}
      </StatusItem>

      <div className="flex-1" />

      <span className="text-text-muted/60">
        {zoom}%
      </span>
      <Divider />
      <span className="text-text-muted/60 uppercase font-mono text-[10px]">
        {units}
      </span>
    </div>
  );
}
