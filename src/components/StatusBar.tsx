import { useEditorStore } from "@/stores/editor";
import { Crosshair, Grid3x3, Magnet, Layers, MousePointer2 } from "lucide-react";

function StatusItem({
  icon,
  children,
}: {
  icon?: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <div className="flex items-center gap-1.5 text-text-muted">
      {icon && <span className="text-text-muted/60">{icon}</span>}
      <span>{children}</span>
    </div>
  );
}

function Divider() {
  return <div className="w-px h-3 bg-border-subtle" />;
}

export function StatusBar() {
  const { statusBar } = useEditorStore();

  return (
    <div className="flex items-center h-[22px] bg-bg-tertiary border-t border-border-subtle px-3 text-[11px] gap-3">
      <StatusItem icon={<Crosshair size={11} />}>
        {statusBar.cursorPosition.x.toFixed(1)}, {statusBar.cursorPosition.y.toFixed(1)}
      </StatusItem>
      <Divider />
      <StatusItem icon={<Grid3x3 size={11} />}>
        {statusBar.gridSize} {statusBar.units}
      </StatusItem>
      <Divider />
      <StatusItem icon={<Magnet size={11} />}>
        {statusBar.snapEnabled ? "Snap" : "Free"}
      </StatusItem>
      <Divider />
      <StatusItem icon={<Layers size={11} />}>
        {statusBar.activeLayer}
      </StatusItem>
      <Divider />
      <StatusItem icon={<MousePointer2 size={11} />}>
        {statusBar.currentMode}
      </StatusItem>

      <div className="flex-1" />

      <span className="text-text-muted/60">
        {statusBar.zoom}%
      </span>
      <Divider />
      <span className="text-text-muted/60 uppercase font-mono text-[10px]">
        {statusBar.units}
      </span>
    </div>
  );
}
