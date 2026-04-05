import { useEditorStore } from "@/stores/editor";

export function StatusBar() {
  const { statusBar } = useEditorStore();

  return (
    <div className="flex items-center h-6 bg-bg-tertiary border-t border-border px-3 text-[11px] text-text-secondary gap-4">
      <span>
        X: {statusBar.cursorPosition.x.toFixed(1)} Y:{" "}
        {statusBar.cursorPosition.y.toFixed(1)}
      </span>
      <span className="text-border">|</span>
      <span>Grid: {statusBar.gridSize} {statusBar.units}</span>
      <span className="text-border">|</span>
      <span>
        Snap: {statusBar.snapEnabled ? "ON" : "OFF"}
      </span>
      <span className="text-border">|</span>
      <span>Layer: {statusBar.activeLayer}</span>
      <span className="text-border">|</span>
      <span>{statusBar.currentMode}</span>
      <div className="flex-1" />
      <span>Zoom: {statusBar.zoom}%</span>
      <span className="text-border">|</span>
      <span className="uppercase">{statusBar.units}</span>
    </div>
  );
}
